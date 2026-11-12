//! Backup das conexões, dos botões rápidos e dos atalhos de teclado.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::perfis::Repositorio;

const VERSAO_ATUAL: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotaoRapido {
    pub rotulo: String,
    #[serde(default)]
    pub atalho: String,
    #[serde(default)]
    pub conteudo: String,
    #[serde(default)]
    pub executar_automaticamente: bool,
    #[serde(default)]
    pub nao_inserir_na_janela: bool,
}

/// Uma ação aceita duas teclas. Backups gravados antes desta versão guardam
/// uma `String` solta; o `untagged` lê os dois formatos sem precisar de uma
/// versão de arquivo nova.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AtalhoDoArquivo {
    Duplo(String, String),
    Unico(String),
}

impl AtalhoDoArquivo {
    fn normalizar(&self) -> (String, String) {
        match self {
            AtalhoDoArquivo::Duplo(principal, secundario) => {
                (principal.clone(), secundario.clone())
            }
            AtalhoDoArquivo::Unico(principal) => (principal.clone(), String::new()),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArquivoDeBackup {
    versao: u32,
    conexoes: serde_json::Value,
    botoes: Vec<BotaoRapido>,
    #[serde(default)]
    atalhos: std::collections::HashMap<String, AtalhoDoArquivo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConteudoRestaurado {
    botoes: Vec<BotaoRapido>,
    atalhos: std::collections::HashMap<String, (String, String)>,
}

pub fn exportar(
    caminho: &Path,
    botoes: Vec<BotaoRapido>,
    atalhos: std::collections::HashMap<String, (String, String)>,
    perfis: &Repositorio,
) -> Resultado<()> {
    validar_botoes(&botoes)?;
    validar_atalhos(&atalhos)?;
    let arquivo = ArquivoDeBackup {
        versao: VERSAO_ATUAL,
        conexoes: perfis.dados_para_backup()?,
        botoes,
        atalhos: atalhos
            .into_iter()
            .map(|(id, (principal, secundario))| {
                (id, AtalhoDoArquivo::Duplo(principal, secundario))
            })
            .collect(),
    };
    let conteudo = serde_json::to_string_pretty(&arquivo)
        .map_err(|erro| Erro::novo(format!("não foi possível montar o backup: {erro}")))?;
    std::fs::write(caminho, conteudo.as_bytes())?;
    Ok(())
}

pub fn importar(caminho: &Path, perfis: &Repositorio) -> Resultado<ConteudoRestaurado> {
    let conteudo = std::fs::read_to_string(caminho)
        .map_err(|erro| Erro::novo(format!("não foi possível ler o backup: {erro}")))?;
    let arquivo: ArquivoDeBackup = serde_json::from_str(&conteudo)
        .map_err(|erro| Erro::novo(format!("arquivo de backup inválido: {erro}")))?;
    if arquivo.versao != VERSAO_ATUAL {
        return Err(Erro::novo(format!(
            "versão de backup não suportada: {}",
            arquivo.versao
        )));
    }
    let atalhos: std::collections::HashMap<String, (String, String)> = arquivo
        .atalhos
        .iter()
        .map(|(id, valor)| (id.clone(), valor.normalizar()))
        .collect();
    validar_botoes(&arquivo.botoes)?;
    validar_atalhos(&atalhos)?;
    perfis.restaurar_backup(arquivo.conexoes)?;
    Ok(ConteudoRestaurado {
        botoes: arquivo.botoes,
        atalhos,
    })
}

fn validar_atalhos(atalhos: &std::collections::HashMap<String, (String, String)>) -> Resultado<()> {
    let mut usados = std::collections::HashSet::new();
    for (id, (principal, secundario)) in atalhos {
        if id.trim().is_empty() || principal.trim().is_empty() {
            return Err(Erro::novo("o backup contém um atalho inválido"));
        }
        if !usados.insert(principal.as_str()) {
            return Err(Erro::novo(format!(
                "o atalho {principal} aparece mais de uma vez no backup"
            )));
        }
        if !secundario.trim().is_empty() && !usados.insert(secundario.as_str()) {
            return Err(Erro::novo(format!(
                "o atalho {secundario} aparece mais de uma vez no backup"
            )));
        }
    }
    Ok(())
}

fn validar_botoes(botoes: &[BotaoRapido]) -> Resultado<()> {
    let mut atalhos = std::collections::HashSet::new();
    for botao in botoes {
        let tamanho = botao.rotulo.chars().count();
        if botao.rotulo.trim().is_empty() || tamanho > 8 {
            return Err(Erro::novo("o backup contém um botão com label inválido"));
        }
        if !botao.atalho.is_empty() && !atalhos.insert(&botao.atalho) {
            return Err(Erro::novo(format!(
                "o atalho {} aparece em mais de um botão do backup",
                botao.atalho
            )));
        }
    }
    Ok(())
}
