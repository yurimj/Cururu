//! Importação de conexões já existentes na máquina (requisito 25).
//!
//! Duas origens: as conexões do Oracle SQL Developer e as entradas do
//! `tnsnames.ora`. Senha nunca é importada — o SQL Developer guarda a dele
//! cifrada com chave própria, e o `tnsnames.ora` não tem senha.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::erro::Resultado;
use crate::perfis::{IdentificacaoBanco, Perfil};

/// Uma conexão encontrada, pronta para virar perfil.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConexaoImportavel {
    pub nome: String,
    /// De onde veio, para a tela mostrar ao usuário.
    pub origem: String,
    pub perfil: Perfil,
}

/// Procura conexões em todos os lugares conhecidos.
pub fn procurar() -> Resultado<Vec<ConexaoImportavel>> {
    let mut achadas = Vec::new();
    achadas.extend(do_sql_developer());
    achadas.extend(dos_tnsnames());

    // Mesma conexão em duas origens: fica a primeira.
    achadas.sort_by(|a, b| a.nome.to_uppercase().cmp(&b.nome.to_uppercase()));
    achadas.dedup_by(|a, b| a.nome.eq_ignore_ascii_case(&b.nome));
    Ok(achadas)
}

// --- SQL Developer ----------------------------------------------------------

/// Lê os `connections.json` do SQL Developer.
fn do_sql_developer() -> Vec<ConexaoImportavel> {
    let mut achadas = Vec::new();
    for arquivo in arquivos_de_conexao_do_sql_developer() {
        let Ok(conteudo) = std::fs::read_to_string(&arquivo) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&conteudo) else {
            continue;
        };
        let Some(lista) = json.get("connections").and_then(|c| c.as_array()) else {
            continue;
        };
        for entrada in lista {
            if let Some(conexao) = converter_do_sql_developer(entrada) {
                achadas.push(conexao);
            }
        }
    }
    achadas
}

fn converter_do_sql_developer(entrada: &serde_json::Value) -> Option<ConexaoImportavel> {
    let nome = texto(entrada.get("name"))?;
    let info = entrada.get("info")?;

    let host = texto(info.get("hostname")).unwrap_or_default();
    let servico = texto(info.get("serviceName")).unwrap_or_default();
    let sid = texto(info.get("sid")).unwrap_or_default();
    // Sem host não há como montar a conexão no modo Básico.
    if host.is_empty() {
        return None;
    }

    let porta = texto(info.get("port"))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1521);

    Some(ConexaoImportavel {
        nome: nome.clone(),
        origem: "SQL Developer".to_string(),
        perfil: Perfil {
            nome,
            usuario: texto(info.get("user")).unwrap_or_default(),
            host,
            porta,
            identificacao: if servico.is_empty() {
                IdentificacaoBanco::Sid
            } else {
                IdentificacaoBanco::Servico
            },
            sid,
            servico,
            ..Default::default()
        },
    })
}

fn texto(valor: Option<&serde_json::Value>) -> Option<String> {
    let texto = valor?.as_str()?.trim().to_string();
    (!texto.is_empty()).then_some(texto)
}

/// Vasculha `%APPDATA%\SQL Developer` atrás dos arquivos de conexão.
fn arquivos_de_conexao_do_sql_developer() -> Vec<PathBuf> {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return Vec::new();
    };
    let raiz = PathBuf::from(appdata).join("SQL Developer");
    let mut achados = Vec::new();
    procurar_arquivo(&raiz, "connections.json", 4, &mut achados);
    achados
}

/// Busca em profundidade limitada, para não varrer a máquina inteira.
fn procurar_arquivo(pasta: &Path, nome: &str, profundidade: u8, achados: &mut Vec<PathBuf>) {
    if profundidade == 0 {
        return;
    }
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            procurar_arquivo(&caminho, nome, profundidade - 1, achados);
        } else if caminho.file_name().is_some_and(|n| n == nome) {
            achados.push(caminho);
        }
    }
}

// --- tnsnames.ora -----------------------------------------------------------

fn dos_tnsnames() -> Vec<ConexaoImportavel> {
    let mut achadas = Vec::new();
    for arquivo in arquivos_tnsnames() {
        let Ok(conteudo) = std::fs::read_to_string(&arquivo) else {
            continue;
        };
        let origem = arquivo.display().to_string();
        for (alias, bloco) in entradas_do_tnsnames(&conteudo) {
            let host = valor_do_bloco(&bloco, "HOST").unwrap_or_default();
            if host.is_empty() {
                continue;
            }
            let servico = valor_do_bloco(&bloco, "SERVICE_NAME").unwrap_or_default();
            let sid = valor_do_bloco(&bloco, "SID").unwrap_or_default();

            achadas.push(ConexaoImportavel {
                nome: alias.clone(),
                origem: origem.clone(),
                perfil: Perfil {
                    nome: alias,
                    host,
                    porta: valor_do_bloco(&bloco, "PORT")
                        .and_then(|p| p.parse::<u16>().ok())
                        .unwrap_or(1521),
                    identificacao: if servico.is_empty() {
                        IdentificacaoBanco::Sid
                    } else {
                        IdentificacaoBanco::Servico
                    },
                    sid,
                    servico,
                    ..Default::default()
                },
            });
        }
    }
    achadas
}

fn arquivos_tnsnames() -> Vec<PathBuf> {
    let mut candidatos = Vec::new();
    for variavel in ["TNS_ADMIN", "ORACLE_HOME"] {
        if let Some(valor) = std::env::var_os(variavel) {
            let base = PathBuf::from(valor);
            candidatos.push(base.join("tnsnames.ora"));
            candidatos.push(base.join("network").join("admin").join("tnsnames.ora"));
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidatos.push(
            PathBuf::from(appdata)
                .join("SQL Developer")
                .join("tnsnames.ora"),
        );
    }
    candidatos.retain(|caminho| caminho.is_file());
    candidatos.dedup();
    candidatos
}

/// Separa o arquivo em `(alias, bloco)`.
///
/// Um alias começa na coluna zero; tudo que vem indentado ou entre parênteses
/// pertence ao bloco dele.
fn entradas_do_tnsnames(conteudo: &str) -> Vec<(String, String)> {
    let mut entradas = Vec::new();
    let mut alias = String::new();
    let mut bloco = String::new();

    for linha in conteudo.lines() {
        let sem_comentario = linha.split('#').next().unwrap_or("");
        if sem_comentario.trim().is_empty() {
            continue;
        }

        let comeca_na_margem = !sem_comentario.starts_with([' ', '\t', '(', ')']);
        if comeca_na_margem && sem_comentario.contains('=') {
            if !alias.is_empty() {
                entradas.push((alias.clone(), std::mem::take(&mut bloco)));
            }
            let (nome, resto) = sem_comentario.split_once('=').unwrap();
            alias = nome.trim().to_string();
            bloco = resto.to_string();
        } else {
            bloco.push_str(sem_comentario);
        }
    }
    if !alias.is_empty() {
        entradas.push((alias, bloco));
    }
    // Aliases podem vir em lista separada por vírgula: fica o primeiro.
    entradas
        .into_iter()
        .filter_map(|(alias, bloco)| {
            let primeiro = alias.split(',').next()?.trim().to_string();
            (!primeiro.is_empty()).then_some((primeiro, bloco))
        })
        .collect()
}

/// Extrai `(CHAVE = valor)` de dentro do bloco, sem diferenciar caixa.
fn valor_do_bloco(bloco: &str, chave: &str) -> Option<String> {
    let maiusculo = bloco.to_uppercase();
    let mut procura_de = 0;

    while let Some(posicao) = maiusculo[procura_de..].find(chave) {
        let inicio = procura_de + posicao;
        let depois = &bloco[inicio + chave.len()..];
        let depois_limpo = depois.trim_start();

        // Precisa ser mesmo a chave, seguida de "=" — e não parte de outra
        // palavra, como SERVICE_NAME quando se procura SID.
        let antes_ok = inicio == 0
            || !bloco[..inicio]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        if antes_ok {
            if let Some(resto) = depois_limpo.strip_prefix('=') {
                let valor: String = resto
                    .trim_start()
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != ')' && *c != '(')
                    .collect();
                if !valor.is_empty() {
                    return Some(valor);
                }
            }
        }
        procura_de = inicio + chave.len();
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    const EXEMPLO: &str = "\
# comentário
EXEMPLO_PRD =
  (DESCRIPTION =
    (ADDRESS = (PROTOCOL = TCP)(HOST = servidor1.empresa.local)(PORT = 1521))
    (CONNECT_DATA = (SERVER = DEDICATED)(SERVICE_NAME = exemplo))
  )

XE, XE_LOCAL =
  (DESCRIPTION =
    (ADDRESS = (PROTOCOL = TCP)(HOST = localhost)(PORT = 1522))
    (CONNECT_DATA = (SID = xe))
  )
";

    #[test]
    fn separa_as_entradas_do_tnsnames() {
        let entradas = entradas_do_tnsnames(EXEMPLO);
        assert_eq!(entradas.len(), 2);
        assert_eq!(entradas[0].0, "EXEMPLO_PRD");
        assert_eq!(entradas[1].0, "XE");
    }

    #[test]
    fn extrai_host_porta_e_servico() {
        let entradas = entradas_do_tnsnames(EXEMPLO);
        let bloco = &entradas[0].1;
        assert_eq!(valor_do_bloco(bloco, "HOST").as_deref(), Some("servidor1.empresa.local"));
        assert_eq!(valor_do_bloco(bloco, "PORT").as_deref(), Some("1521"));
        assert_eq!(valor_do_bloco(bloco, "SERVICE_NAME").as_deref(), Some("exemplo"));
    }

    #[test]
    fn nao_confunde_sid_com_service_name() {
        let entradas = entradas_do_tnsnames(EXEMPLO);
        // O primeiro bloco só tem SERVICE_NAME; procurar SID não pode achar
        // o pedaço final de "SERVICE_NAME".
        assert_eq!(valor_do_bloco(&entradas[0].1, "SID"), None);
        assert_eq!(valor_do_bloco(&entradas[1].1, "SID").as_deref(), Some("xe"));
    }

    #[test]
    fn comentario_e_ignorado() {
        let entradas = entradas_do_tnsnames("# NAO_DEVE = (aparecer)\nREAL = (HOST = h)\n");
        assert_eq!(entradas.len(), 1);
        assert_eq!(entradas[0].0, "REAL");
    }

    #[test]
    fn converte_conexao_do_sql_developer() {
        let entrada = serde_json::json!({
            "name": "EXEMPLO PRD",
            "info": {
                "hostname": "servidor1",
                "port": "1521",
                "serviceName": "exemplo",
                "user": "exemplo"
            }
        });
        let conexao = converter_do_sql_developer(&entrada).expect("deveria converter");
        assert_eq!(conexao.nome, "EXEMPLO PRD");
        assert_eq!(conexao.perfil.host, "servidor1");
        assert_eq!(conexao.perfil.porta, 1521);
        assert_eq!(conexao.perfil.servico, "exemplo");
        assert_eq!(conexao.perfil.usuario, "exemplo");
        // Senha nunca é importada.
        assert!(conexao.perfil.senha.is_none());
    }

    #[test]
    fn conexao_sem_host_e_descartada() {
        let entrada = serde_json::json!({ "name": "sem host", "info": { "port": "1521" } });
        assert!(converter_do_sql_developer(&entrada).is_none());
    }
}
