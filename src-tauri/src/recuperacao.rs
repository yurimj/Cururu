//! Cópia de recuperação das consultas abertas (requisito 12).
//!
//! Não é salvamento automático: o arquivo do usuário nunca é tocado. O que
//! existe aqui é uma cópia de socorro, para o texto não se perder num
//! travamento ou num fechamento sem querer, e que só volta se ele pedir.
//!
//! Cada aba tem dois arquivos em `%APPDATA%\Cururu\recuperacao\`: o `.sql` com
//! o texto, que muda a cada pausa da digitação, e o `.json` com o que a tela de
//! recuperação precisa mostrar, que quase nunca muda. Dois arquivos pequenos em
//! vez de um índice único — o índice é justamente o arquivo que corrompe quando
//! o aplicativo morre no meio da escrita.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::registro;

/// Por quanto tempo uma cópia é guardada, contado da última gravação.
const VALIDADE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Teto de entradas, para quem abre muitas abas não acumular sem limite.
const MAXIMO_DE_ENTRADAS: usize = 200;

/// Quanto do começo do texto vai para a lista, para a consulta ser reconhecida.
const TAMANHO_DA_AMOSTRA: usize = 160;

/// O que a tela de recuperação mostra de cada cópia.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entrada {
    pub id: String,
    /// Data e hora da última gravação, já formatada para exibição.
    pub quando: String,
    pub titulo: String,
    pub conexao: String,
    /// Arquivo de onde a consulta veio, quando veio de um.
    pub origem: Option<String>,
    /// Primeiras linhas do texto: é o que faz reconhecer qual consulta era.
    pub amostra: String,
}

/// Dados que acompanham o texto, gravados ao lado dele.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descricao {
    pub titulo: String,
    pub conexao: String,
    pub origem: Option<String>,
}

/// Grava — ou regrava — a cópia de uma aba.
pub fn gravar(id: &str, texto: &str, descricao: &Descricao) -> Resultado<()> {
    let pasta = pasta()?;
    std::fs::create_dir_all(&pasta)?;

    let identificador = validar_identificador(id)?;
    std::fs::write(pasta.join(format!("{identificador}.sql")), texto.as_bytes())?;

    // A descrição muda muito menos que o texto; regravar sempre custa pouco e
    // evita ter de descobrir quando ela mudou.
    let json = serde_json::to_vec_pretty(descricao)
        .map_err(|erro| Erro::novo(format!("descrição inválida: {erro}")))?;
    std::fs::write(pasta.join(format!("{identificador}.json")), json)?;
    Ok(())
}

/// Lista as cópias existentes, da mais recente para a mais antiga.
pub fn listar() -> Resultado<Vec<Entrada>> {
    let pasta = pasta()?;
    let Ok(conteudo) = std::fs::read_dir(&pasta) else {
        // Pasta ainda não criada é o caso normal de quem nunca digitou nada.
        return Ok(Vec::new());
    };

    let mut entradas: Vec<(SystemTime, Entrada)> = Vec::new();
    for arquivo in conteudo.flatten() {
        let caminho = arquivo.path();
        if caminho.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let Some(id) = caminho.file_stem().and_then(|nome| nome.to_str()) else {
            continue;
        };
        let quando = arquivo
            .metadata()
            .and_then(|dados| dados.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let texto = std::fs::read_to_string(&caminho).unwrap_or_default();
        let descricao = ler_descricao(&pasta, id);

        entradas.push((
            quando,
            Entrada {
                id: id.to_string(),
                quando: registro::formatar(quando),
                titulo: descricao.titulo,
                conexao: descricao.conexao,
                origem: descricao.origem,
                amostra: amostrar(&texto),
            },
        ));
    }

    entradas.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(entradas.into_iter().map(|(_, entrada)| entrada).collect())
}

/// Texto guardado de uma cópia, para reabrir numa aba.
pub fn texto(id: &str) -> Resultado<String> {
    let identificador = validar_identificador(id)?;
    let caminho = pasta()?.join(format!("{identificador}.sql"));
    Ok(std::fs::read_to_string(caminho)?)
}

/// Apaga uma cópia, a pedido do usuário.
pub fn descartar(id: &str) -> Resultado<()> {
    let identificador = validar_identificador(id)?;
    apagar(&pasta()?, identificador);
    Ok(())
}

/// Remove as cópias vencidas e o excesso. Roda uma vez, na subida.
pub fn limpar() {
    let Ok(pasta) = pasta() else { return };
    let Ok(conteudo) = std::fs::read_dir(&pasta) else { return };

    let agora = SystemTime::now();
    let mut vivas: Vec<(SystemTime, String)> = Vec::new();
    let mut vencidas = 0;

    for arquivo in conteudo.flatten() {
        let caminho = arquivo.path();
        if caminho.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let Some(id) = caminho.file_stem().and_then(|nome| nome.to_str()) else {
            continue;
        };
        let quando = arquivo
            .metadata()
            .and_then(|dados| dados.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let vencida = agora
            .duration_since(quando)
            .map(|idade| idade > VALIDADE)
            .unwrap_or(false);
        if vencida {
            apagar(&pasta, id);
            vencidas += 1;
        } else {
            vivas.push((quando, id.to_string()));
        }
    }

    // Passando do teto, saem as mais antigas.
    vivas.sort_by(|a, b| b.0.cmp(&a.0));
    let excedentes = vivas.len().saturating_sub(MAXIMO_DE_ENTRADAS);
    for (_, id) in vivas.iter().skip(MAXIMO_DE_ENTRADAS) {
        apagar(&pasta, id);
    }

    registro::informacao(
        "recuperacao",
        format!(
            "{} cópia(s) guardada(s), {vencidas} vencida(s) e {excedentes} excedente(s) removida(s)",
            vivas.len().min(MAXIMO_DE_ENTRADAS)
        ),
    );
}

fn apagar(pasta: &std::path::Path, id: &str) {
    let _ = std::fs::remove_file(pasta.join(format!("{id}.sql")));
    let _ = std::fs::remove_file(pasta.join(format!("{id}.json")));
}

/// Sem o `.json` ao lado — cópia de uma versão anterior, ou gravação
/// interrompida no meio —, a entrada ainda serve: o texto é o que importa.
fn ler_descricao(pasta: &std::path::Path, id: &str) -> Descricao {
    std::fs::read_to_string(pasta.join(format!("{id}.json")))
        .ok()
        .and_then(|texto| serde_json::from_str::<Descricao>(&texto).ok())
        .unwrap_or_else(|| Descricao {
            titulo: "Consulta".to_string(),
            conexao: String::new(),
            origem: None,
        })
}

/// Primeiras linhas com conteúdo, para a lista dar para reconhecer a consulta.
fn amostrar(texto: &str) -> String {
    let util: String = texto
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if util.chars().count() <= TAMANHO_DA_AMOSTRA {
        return util;
    }
    // Corta por caractere, e não por byte: cortar no meio de um acento
    // produziria texto inválido.
    let cortado: String = util.chars().take(TAMANHO_DA_AMOSTRA).collect();
    format!("{cortado}…")
}

/// O identificador vira nome de arquivo, então não pode trazer caminho junto.
fn validar_identificador(id: &str) -> Resultado<&str> {
    let aceitavel = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if aceitavel {
        Ok(id)
    } else {
        Err(Erro::novo("identificador de recuperação inválido"))
    }
}

fn pasta() -> Resultado<PathBuf> {
    Ok(crate::perfis::pasta_de_dados()?.join("recuperacao"))
}

#[cfg(test)]
mod testes {
    use super::{amostrar, validar_identificador, TAMANHO_DA_AMOSTRA};

    #[test]
    fn amostra_junta_linhas_uteis_e_corta_no_limite() {
        assert_eq!(amostrar("\n  SELECT 1\n\n  FROM DUAL\n"), "SELECT 1 FROM DUAL");
        let longo = "ç".repeat(TAMANHO_DA_AMOSTRA + 40);
        let amostra = amostrar(&longo);
        assert_eq!(amostra.chars().count(), TAMANHO_DA_AMOSTRA + 1);
        assert!(amostra.ends_with('…'));
    }

    #[test]
    fn identificador_nao_aceita_caminho() {
        assert!(validar_identificador("aba-3_x").is_ok());
        assert!(validar_identificador("..\\..\\conexoes").is_err());
        assert!(validar_identificador("").is_err());
    }
}
