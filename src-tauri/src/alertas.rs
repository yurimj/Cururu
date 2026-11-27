//! Regras locais por perfil. Cada verificação abre uma sessão exclusiva de
//! leitura com as credenciais da conexão que o usuário já abriu na árvore.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::erro::{Erro, Resultado};
use crate::perfis::{self, TipoBanco};
use crate::sessao::{self, Sessoes};
use crate::{dpapi, registro, sql_texto, sqlserver, teams};

const ESPERA_ORACLE: Duration = Duration::from_secs(15);
const LIMITE_DE_LOGS: usize = 200;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Regra {
    id: String,
    nome: String,
    banco: Option<String>,
    minutos: u16,
    #[serde(default)]
    repetir_apos: Option<u16>,
    consulta: String,
    ativo: bool,
    #[serde(default = "contagem_legada")]
    contagem_legada: bool,
}

fn contagem_legada() -> bool {
    true
}

#[derive(Serialize)]
pub struct ResultadoDaVerificacao {
    mensagem: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Registro {
    #[serde(flatten)]
    regra: Regra,
    webhook_protegido: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alerta {
    #[serde(flatten)]
    regra: Regra,
    tem_teams: bool,
}

impl From<&Registro> for Alerta {
    fn from(registro: &Registro) -> Self {
        Self {
            regra: registro.regra.clone(),
            tem_teams: registro.webhook_protegido.is_some(),
        }
    }
}

type Arquivo = HashMap<String, Vec<Registro>>;

#[derive(Default)]
pub struct Repositorio(Mutex<()>);

impl Repositorio {
    fn alterar<T>(&self, acao: impl FnOnce(&mut Arquivo) -> Resultado<T>) -> Resultado<T> {
        let _guarda = self.0.lock().unwrap();
        let mut arquivo = ler_arquivo()?;
        let resultado = acao(&mut arquivo)?;
        let pasta = perfis::pasta_de_dados()?;
        std::fs::create_dir_all(&pasta)?;
        let temporario = pasta.join("alertas.json.tmp");
        std::fs::write(&temporario, serde_json::to_vec_pretty(&arquivo)?)?;
        std::fs::rename(temporario, pasta.join("alertas.json"))?;
        Ok(resultado)
    }

    fn listar(&self, perfil: &str) -> Resultado<Vec<Registro>> {
        let _guarda = self.0.lock().unwrap();
        Ok(ler_arquivo()?.remove(perfil).unwrap_or_default())
    }

    pub fn esquecer(&self, perfil: &str) -> Resultado<()> {
        self.alterar(|arquivo| {
            arquivo.remove(perfil);
            Ok(())
        })
    }

    fn obter(&self, perfil: &str, id: &str) -> Resultado<Option<Registro>> {
        Ok(self
            .listar(perfil)?
            .into_iter()
            .find(|item| item.regra.id == id))
    }
}

fn ler_arquivo() -> Resultado<Arquivo> {
    match std::fs::read(perfis::pasta_de_dados()?.join("alertas.json")) {
        Ok(conteudo) => Ok(serde_json::from_slice(&conteudo)?),
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => Ok(Arquivo::new()),
        Err(erro) => Err(erro.into()),
    }
}

#[tauri::command(async)]
pub fn listar_alertas(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<Vec<Alerta>> {
    Ok(alertas
        .listar(&sessoes.obter(&sessao_id)?.perfil.id)?
        .iter()
        .map(Alerta::from)
        .collect())
}

#[tauri::command(async)]
pub fn ler_logs_alerta(
    sessao_id: String,
    id: String,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<Vec<String>> {
    let sessao = sessoes.obter(&sessao_id)?;
    if alertas.obter(&sessao.perfil.id, &id)?.is_none() {
        return Err(Erro::novo("alerta inexistente"));
    }

    let pasta = perfis::pasta_de_dados()?.join("logs");
    let arquivos = match std::fs::read_dir(pasta) {
        Ok(arquivos) => arquivos,
        Err(erro) if erro.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(erro) => return Err(erro.into()),
    };
    let mut recentes = BinaryHeap::new();
    for entrada in arquivos {
        let entrada = entrada?;
        let nome = entrada.file_name();
        let nome = nome.to_string_lossy();
        if !nome.starts_with("cururu-")
            || !nome.ends_with(".log")
            || !entrada.file_type()?.is_file()
        {
            continue;
        }
        let arquivo = match File::open(entrada.path()) {
            Ok(arquivo) => arquivo,
            Err(erro) if erro.kind() == ErrorKind::NotFound => continue,
            Err(erro) => return Err(erro.into()),
        };
        for linha in BufReader::new(arquivo).lines() {
            let linha = linha?;
            if e_log_do_alerta(&linha, &id) {
                recentes.push(Reverse(linha));
                if recentes.len() > LIMITE_DE_LOGS {
                    recentes.pop();
                }
            }
        }
    }
    // O carimbo ISO no começo da linha permite ordenar inclusive registros
    // escritos por instâncias do Cururu abertas em dias diferentes.
    Ok(recentes
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse(linha)| linha)
        .collect())
}

fn e_log_do_alerta(linha: &str, id: &str) -> bool {
    let Some((cabecalho, mensagem)) = linha.split_once(" [alertas] ") else {
        return false;
    };
    let Some((_, nivel)) = cabecalho.split_once(' ') else {
        return false;
    };
    let Some((_, nivel)) = nivel.split_once(' ') else {
        return false;
    };
    if !matches!(nivel.trim(), "INFO" | "AVISO" | "ERRO") {
        return false;
    }
    let Some((inicio, resto)) = mensagem.split_once("regra ") else {
        return false;
    };
    matches!(
        inicio,
        "" | "verificando " | "enviando " | "Teams aceitou " | "falha no Teams para "
    ) && resto.split_once(' ').is_some_and(|(regra, _)| regra == id)
}

#[tauri::command(async)]
pub fn ler_webhook_alerta(
    sessao_id: String,
    id: String,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<Option<String>> {
    let sessao = sessoes.obter(&sessao_id)?;
    let alerta = alertas
        .obter(&sessao.perfil.id, &id)?
        .ok_or_else(|| Erro::novo("alerta inexistente"))?;
    alerta
        .webhook_protegido
        .as_deref()
        .map(dpapi::desproteger)
        .transpose()
}

#[tauri::command(async)]
pub fn salvar_alerta(
    sessao_id: String,
    mut regra: Regra,
    teams: bool,
    webhook: String,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<Alerta> {
    let sessao = sessoes.obter(&sessao_id)?;
    regra.nome = regra.nome.trim().to_string();
    if regra.nome.is_empty() || !(1..=1440).contains(&regra.minutos) {
        return Err(Erro::novo("alerta campos invalidos"));
    }
    if regra
        .repetir_apos
        .is_some_and(|minutos| !(1..=1440).contains(&minutos))
    {
        return Err(Erro::novo("alerta repeticao invalida"));
    }
    regra.consulta = consulta_unica(&regra.consulta, sessao.perfil.tipo_banco)?;
    match sessao.perfil.tipo_banco {
        TipoBanco::Oracle => regra.banco = None,
        TipoBanco::SqlServer if regra.banco.as_ref().is_some_and(|banco| !banco.is_empty()) => {}
        _ => return Err(Erro::novo("alerta banco obrigatorio")),
    }
    let novo_webhook = webhook.trim();
    let protegido = if teams && !novo_webhook.is_empty() {
        crate::teams::separar_webhook(novo_webhook)?;
        Some(dpapi::proteger(novo_webhook)?)
    } else {
        None
    };
    alertas.alterar(|arquivo| {
        let regras = arquivo.entry(sessao.perfil.id.clone()).or_default();
        let indice = regras.iter().position(|item| item.regra.id == regra.id);
        regra.contagem_legada = indice.is_some_and(|i| {
            let anterior = &regras[i].regra;
            anterior.contagem_legada && anterior.consulta == regra.consulta
        });
        if regra.id.is_empty() {
            regra.id = perfis::identificador_novo();
        } else if indice.is_none() {
            return Err(Erro::novo("alerta inexistente"));
        }
        let webhook_protegido = if teams {
            protegido
                .or_else(|| indice.and_then(|i| regras[i].webhook_protegido.clone()))
                .ok_or_else(|| Erro::novo("alerta webhook invalido"))
                .map(Some)?
        } else {
            None
        };
        let registro = Registro {
            regra,
            webhook_protegido,
        };
        let resposta = Alerta::from(&registro);
        if let Some(indice) = indice {
            regras[indice] = registro;
        } else {
            regras.push(registro);
        }
        Ok(resposta)
    })
}

#[tauri::command(async)]
pub fn excluir_alerta(
    sessao_id: String,
    id: String,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<()> {
    let sessao = sessoes.obter(&sessao_id)?;
    alertas.alterar(|arquivo| {
        if let Some(regras) = arquivo.get_mut(&sessao.perfil.id) {
            regras.retain(|item| item.regra.id != id);
        }
        Ok(())
    })
}

fn consulta_unica(texto: &str, tipo: TipoBanco) -> Resultado<String> {
    let texto = texto.trim();
    let sql = texto.strip_suffix(';').unwrap_or(texto).trim_end();
    if sql_texto::palavras_iniciais(sql, 1)
        .first()
        .is_some_and(|(_, palavra)| palavra.eq_ignore_ascii_case("SELECT"))
        && sql_texto::pode_embutir_consulta(sql, tipo == TipoBanco::SqlServer)
    {
        return Ok(sql.to_string());
    }
    Err(Erro::novo("alerta consulta invalida"))
}

#[tauri::command(async)]
pub fn verificar_alerta(
    sessao_id: String,
    id: String,
    manual: Option<bool>,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<Option<ResultadoDaVerificacao>> {
    let sessao = sessoes.obter(&sessao_id)?;
    let Some(item) = alertas
        .obter(&sessao.perfil.id, &id)?
        .filter(|item| item.regra.ativo || manual.unwrap_or(false))
    else {
        return Ok(None);
    };
    let regra = item.regra;
    let consulta = consulta_unica(&regra.consulta, sessao.perfil.tipo_banco)?;
    let inicio = Instant::now();
    registro::informacao(
        "alertas",
        format!("verificando regra {id} da sessão {sessao_id}"),
    );
    let resultado = match sessao.perfil.tipo_banco {
        TipoBanco::Oracle => consultar_oracle(&sessao.perfil, &consulta, regra.contagem_legada),
        TipoBanco::SqlServer => sqlserver::consultar_alerta(
            &sessao.perfil,
            regra.banco.as_deref().unwrap_or_default(),
            &consulta,
            regra.contagem_legada,
        ),
        TipoBanco::MongoDb => return Ok(None),
    };
    let duracao = inicio.elapsed().as_millis();
    match resultado {
        Ok(resultado) => {
            let mensagem = mensagem_da_consulta(resultado, regra.contagem_legada);
            let estado = if mensagem.is_some() {
                "com mensagem"
            } else {
                "sem mensagem"
            };
            registro::informacao(
                "alertas",
                format!("regra {id} verificada em {duracao} ms — {estado}"),
            );
            Ok(sessoes
                .obter(&sessao_id)
                .ok()
                .map(|_| ResultadoDaVerificacao { mensagem }))
        }
        Err(erro) => {
            registro::aviso(
                "alertas",
                format!("regra {id} sem resultado após {duracao} ms: {erro}"),
            );
            Ok(None)
        }
    }
}

fn mensagem_da_consulta(resultado: Option<String>, legada: bool) -> Option<String> {
    resultado.filter(|mensagem| {
        if legada {
            mensagem.parse::<i64>().is_ok_and(|valor| valor > 1)
        } else {
            !mensagem.trim().is_empty()
        }
    })
}

fn consultar_oracle(
    perfil: &perfis::Perfil,
    consulta: &str,
    legada: bool,
) -> Resultado<Option<String>> {
    let abrir = || sessao::abrir(perfil);
    let conexao = abrir().or_else(|_| abrir())?;
    let ler = |conexao: &oracle::Connection| {
        conexao.set_call_timeout(Some(ESPERA_ORACLE))?;
        conexao.execute("SET TRANSACTION READ ONLY", &[])?;
        let limite = if legada { 2 } else { 1 };
        let sql = format!("SELECT * FROM (\n{consulta}\n) WHERE ROWNUM <= {limite}");
        let mut linhas = conexao.query(&sql, &[])?;
        if linhas.column_info().len() != 1 {
            return Err(Erro::novo(
                "a consulta do alerta precisa retornar uma coluna",
            ));
        }
        let Some(linha) = linhas.next().transpose()? else {
            return Ok(None);
        };
        if !legada {
            return Ok(linha.get::<_, Option<String>>(0)?);
        }
        let valor = linha.get::<_, i64>(0)?;
        if linhas.next().transpose()?.is_some() {
            return Err(Erro::novo(
                "a consulta do alerta retornou mais de uma linha",
            ));
        }
        Ok(Some(valor.to_string()))
    };
    match ler(&conexao) {
        Err(erro) if sessao::e_falha_de_conexao(&erro, &conexao) => ler(&abrir()?),
        resultado => resultado,
    }
}

#[tauri::command(async)]
pub fn notificar_alerta_teams(
    sessao_id: String,
    id: String,
    manual: Option<bool>,
    mensagem: String,
    sessoes: State<'_, Sessoes>,
    alertas: State<'_, Repositorio>,
) -> Resultado<bool> {
    let sessao = sessoes.obter(&sessao_id)?;
    let Some(item) = alertas
        .obter(&sessao.perfil.id, &id)?
        .filter(|item| item.regra.ativo || manual.unwrap_or(false))
    else {
        return Ok(false);
    };
    let Some(protegido) = item.webhook_protegido else {
        return Ok(false);
    };
    let Some(mensagem) = mensagem_da_consulta(Some(mensagem), item.regra.contagem_legada) else {
        return Ok(false);
    };
    let mensagem = if item.regra.contagem_legada {
        let conexao = &sessao.perfil.nome;
        let destino = match item.regra.banco.as_deref() {
            Some(banco) => format!("{conexao} · {banco}"),
            None => conexao.clone(),
        };
        format!("Cururu · {}\n{destino}\n{mensagem} > 1", item.regra.nome)
    } else {
        mensagem
    };
    let inicio = Instant::now();
    registro::informacao("alertas", format!("enviando regra {id} ao Teams"));
    let resultado =
        dpapi::desproteger(&protegido).and_then(|url| teams::notificar(&url, &mensagem));
    let duracao = inicio.elapsed().as_millis();
    match resultado {
        Ok(()) => registro::informacao(
            "alertas",
            format!("Teams aceitou regra {id} em {duracao} ms"),
        ),
        Err(ref erro) => registro::aviso(
            "alertas",
            format!("falha no Teams para regra {id} após {duracao} ms: {erro}"),
        ),
    }
    Ok(resultado.is_ok())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn seleciona_so_linhas_da_regra_exata() {
        let linha = "2026-11-10 16:20:42.047 INFO  [alertas] Teams aceitou regra abc1 em 962 ms";
        assert!(e_log_do_alerta(linha, "abc1"));
        assert!(!e_log_do_alerta(linha, "abc"));
        assert!(!e_log_do_alerta(linha, "abc12"));
        assert!(!e_log_do_alerta(
            &linha.replace("[alertas]", "[sql]"),
            "abc1"
        ));
        assert!(e_log_do_alerta(
            "2026-11-10 16:21:42.047 AVISO [alertas] regra abc1 sem resultado após 10 ms: timeout",
            "abc1",
        ));
        assert!(!e_log_do_alerta(
            "2026-11-10 16:21:42.047 AVISO [alertas] regra xyz sem resultado após 10 ms: regra abc1",
            "abc1",
        ));
    }

    #[test]
    fn le_alerta_salvo_antes_das_mensagens_e_da_repeticao() {
        let regra: Regra = serde_json::from_value(serde_json::json!({
            "id": "1",
            "nome": "Regra antiga",
            "banco": null,
            "minutos": 5,
            "consulta": "SELECT COUNT(*) FROM pedidos",
            "ativo": true
        }))
        .unwrap();
        assert!(regra.contagem_legada);
        assert_eq!(regra.repetir_apos, None);
    }

    #[test]
    fn preserva_a_mensagem_e_distingue_as_contagens_antigas() {
        let texto = "  Pedido 123\nTravado há uma hora  ".to_owned();
        assert_eq!(
            mensagem_da_consulta(Some(texto.clone()), false),
            Some(texto)
        );
        assert_eq!(
            mensagem_da_consulta(Some("1".into()), false).as_deref(),
            Some("1")
        );
        assert_eq!(mensagem_da_consulta(Some("1".into()), true), None);
        assert_eq!(
            mensagem_da_consulta(Some("2".into()), true).as_deref(),
            Some("2")
        );
        assert_eq!(mensagem_da_consulta(Some(" \n".into()), false), None);
        assert_eq!(mensagem_da_consulta(None, false), None);
    }

    #[test]
    fn aceita_contagem_e_recusa_sql_que_sai_da_consulta() {
        let sql = "SELECT COUNT(*) FROM pedidos WHERE estado = 'Pendente';";
        for tipo in [TipoBanco::Oracle, TipoBanco::SqlServer] {
            assert_eq!(
                consulta_unica(sql, tipo).unwrap(),
                sql.trim_end_matches(';')
            );
            assert!(consulta_unica("SELECT 1; DELETE FROM pedidos", tipo).is_err());
            assert!(consulta_unica("SELECT 1) AS alerta DELETE FROM pedidos", tipo).is_err());
        }
        assert!(consulta_unica(
            "SELECT COUNT(*) FROM [pedidos;)]](novos] /* ( /* ) */ ; */ -- )",
            TipoBanco::SqlServer,
        )
        .is_ok());
        assert!(consulta_unica(
            "SELECT COUNT(*) FROM [pedidos]](novos]) AS alerta DELETE FROM pedidos",
            TipoBanco::SqlServer,
        )
        .is_err());
        assert!(consulta_unica(
            "SELECT COUNT(*) FROM pedidos -- comentário\r) AS alerta DELETE FROM pedidos",
            TipoBanco::SqlServer,
        )
        .is_err());
    }
}
