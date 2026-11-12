//! Persistência dos filtros da árvore (requisito 4).
//!
//! O filtro é configuração da conexão, como a cor: sobrevive a fechar o
//! aplicativo. Guardado em `%APPDATA%\Cururu\filtros.json`, separado das
//! conexões para que mexer num não arrisque o outro.
//!
//! Não confundir com o requisito 12: lá o que **não** se guarda é o conteúdo
//! das abas de consulta SQL.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::erro::{Erro, Resultado};
use crate::filtro::Filtro;
use crate::objetos::Categoria;

/// Filtros de uma conexão, por categoria.
pub type FiltrosDaConexao = HashMap<Categoria, Filtro>;

/// Tudo que está guardado, indexado pelo id do perfil de conexão.
type Arquivo = HashMap<String, FiltrosDaConexao>;

/// Repositório dos filtros salvos.
///
/// O `Mutex` serializa as gravações para dois comandos simultâneos não
/// sobrescreverem um ao outro.
#[derive(Default)]
pub struct RepositorioDeFiltros {
    trava: Mutex<()>,
}

impl RepositorioDeFiltros {
    /// Filtros guardados para uma conexão. Sem nada guardado, devolve vazio.
    pub fn ler(&self, perfil_id: &str) -> Resultado<FiltrosDaConexao> {
        Ok(self.ler_arquivo()?.remove(perfil_id).unwrap_or_default())
    }

    /// Grava o filtro de uma categoria. `None` apaga o filtro daquela categoria.
    pub fn gravar(
        &self,
        perfil_id: &str,
        categoria: Categoria,
        filtro: Option<Filtro>,
    ) -> Resultado<()> {
        let _trava = self.trava.lock().unwrap_or_else(|erro| erro.into_inner());
        let mut arquivo = self.ler_arquivo()?;
        let da_conexao = arquivo.entry(perfil_id.to_string()).or_default();

        match filtro {
            Some(filtro) => da_conexao.insert(categoria, filtro),
            None => da_conexao.remove(&categoria),
        };
        // Conexão sem filtro nenhum não precisa ocupar espaço no arquivo.
        if da_conexao.is_empty() {
            arquivo.remove(perfil_id);
        }
        self.gravar_arquivo(&arquivo)
    }

    /// Esquece os filtros de uma conexão que deixou de existir.
    pub fn esquecer(&self, perfil_id: &str) -> Resultado<()> {
        let _trava = self.trava.lock().unwrap_or_else(|erro| erro.into_inner());
        let mut arquivo = self.ler_arquivo()?;
        if arquivo.remove(perfil_id).is_some() {
            self.gravar_arquivo(&arquivo)?;
        }
        Ok(())
    }

    fn ler_arquivo(&self) -> Resultado<Arquivo> {
        let caminho = caminho_do_arquivo()?;
        match std::fs::read_to_string(&caminho) {
            Ok(conteudo) if conteudo.trim().is_empty() => Ok(Arquivo::new()),
            Ok(conteudo) => Ok(serde_json::from_str(&conteudo)?),
            Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => Ok(Arquivo::new()),
            Err(erro) => Err(Erro::novo(format!(
                "não foi possível ler {}: {erro}",
                caminho.display()
            ))),
        }
    }

    fn gravar_arquivo(&self, arquivo: &Arquivo) -> Resultado<()> {
        let caminho = caminho_do_arquivo()?;
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta)?;
        }
        let conteudo = serde_json::to_string_pretty(arquivo)
            .map_err(|erro| Erro::novo(format!("falha ao montar o arquivo: {erro}")))?;

        // Grava em temporário e troca, para não corromper o original se faltar
        // energia no meio da escrita.
        let temporario = caminho.with_extension("json.tmp");
        std::fs::write(&temporario, conteudo)?;
        std::fs::rename(&temporario, &caminho)?;
        Ok(())
    }
}

fn caminho_do_arquivo() -> Resultado<PathBuf> {
    Ok(crate::perfis::pasta_de_dados()?.join("filtros.json"))
}
