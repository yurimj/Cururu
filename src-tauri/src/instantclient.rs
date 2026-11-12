//! Localização e carga do Oracle Instant Client embutido.
//!
//! Conforme `premissas.md`, o Instant Client é distribuído junto do Cururu e
//! carregado da **própria pasta do aplicativo** — nunca de uma instalação
//! Oracle da máquina. Isso dispensa `ORACLE_HOME`/`PATH` e evita conflito com
//! outros clientes Oracle já presentes.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Biblioteca que precisa existir na pasta para considerá-la um Instant Client.
const BIBLIOTECA_PRINCIPAL: &str = "oci.dll";

/// Nome que aparece no banco como driver da sessão (`v$session_connect_info`).
const NOME_DO_DRIVER: &str = "Cururu";

/// Conteúdo do `sqlnet.ora` que o Cururu usa.
///
/// `ALLOWED_LOGON_VERSION` em 8 permite autenticar em bases que ainda guardam
/// verificadores de senha antigos, que de outro modo recusariam um client 19c
/// com `ORA-28040`. Ver `premissas.md`, "Versões do Oracle suportadas".
const SQLNET_ORA: &str = "\
# Gerado pelo Cururu na primeira execução. Pode ser editado.
# Aceita bases com verificadores de senha antigos (evita ORA-28040).
SQLNET.ALLOWED_LOGON_VERSION_CLIENT = 8
SQLNET.ALLOWED_LOGON_VERSION_SERVER = 8
";

/// Situação do Instant Client, exposta à interface para diagnóstico.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Situacao {
    /// Pasta do Instant Client, quando encontrada e carregada.
    pub diretorio: Option<String>,
    /// Mensagem pronta para exibição quando algo impede o uso do banco.
    pub problema: Option<String>,
    /// Pastas consultadas, para diagnosticar uma instalação torta.
    pub procurado_em: Vec<String>,
}

impl Situacao {
    pub fn utilizavel(&self) -> bool {
        self.diretorio.is_some() && self.problema.is_none()
    }
}

/// Localiza o Instant Client e o registra no driver.
///
/// Precisa rodar **antes** de qualquer conexão: uma vez inicializado, o driver
/// não aceita trocar a pasta da biblioteca.
pub fn preparar() -> Situacao {
    let candidatos = diretorios_candidatos();
    let procurado_em = candidatos.iter().map(|c| c.display().to_string()).collect();

    let Some(diretorio) = candidatos
        .into_iter()
        .find(|c| c.join(BIBLIOTECA_PRINCIPAL).is_file())
    else {
        return Situacao {
            diretorio: None,
            problema: Some(format!(
                "Não foi encontrado o Oracle Instant Client. Esperava achar \
                 {BIBLIOTECA_PRINCIPAL} numa pasta chamada \"instantclient\" ao \
                 lado do executável do Cururu."
            )),
            procurado_em,
        };
    };

    let problema = carregar(&diretorio).err();
    Situacao {
        diretorio: Some(diretorio.display().to_string()),
        problema,
        procurado_em,
    }
}

/// Aponta o driver para a pasta do Instant Client e para a nossa configuração
/// de rede.
fn carregar(diretorio: &Path) -> Result<(), String> {
    let pasta_rede = preparar_configuracao_de_rede()
        .map_err(|erro| format!("não foi possível preparar o sqlnet.ora: {erro}"))?;

    let mut parametros = oracle::InitParams::new();
    let montagem = (|| -> oracle::Result<()> {
        parametros.oracle_client_lib_dir(diretorio.as_os_str())?;
        parametros.oracle_client_config_dir(pasta_rede.as_os_str())?;
        parametros.default_driver_name(NOME_DO_DRIVER)?;
        Ok(())
    })();
    if let Err(erro) = montagem {
        // Acontece, por exemplo, quando o caminho tem caracteres fora da página
        // de código do Windows — limitação da própria biblioteca da Oracle.
        return Err(format!(
            "o caminho do Instant Client não pôde ser usado ({}): {erro}",
            diretorio.display()
        ));
    }

    parametros
        .init()
        .map(|_| ())
        .map_err(|erro| format!("falha ao carregar o Instant Client: {erro}"))
}

/// Pastas onde o Instant Client pode estar, em ordem de preferência.
fn diretorios_candidatos() -> Vec<PathBuf> {
    let mut candidatos = Vec::new();

    // 1. Caminho forçado por variável de ambiente, útil para depuração.
    if let Ok(caminho) = std::env::var("CURURU_INSTANTCLIENT") {
        candidatos.push(PathBuf::from(caminho));
    }

    // 2. Ao lado do executável — é como o aplicativo é distribuído.
    if let Ok(executavel) = std::env::current_exe() {
        if let Some(pasta) = executavel.parent() {
            candidatos.push(pasta.join("instantclient"));
        }
    }

    // 3. Em desenvolvimento, a cópia baixada fica em runtime/ na raiz do
    //    repositório, com o número da versão no nome da pasta.
    if cfg!(debug_assertions) {
        if let Some(raiz) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
            candidatos.extend(subpastas_de_instantclient(&raiz.join("runtime")));
        }
    }

    candidatos
}

/// Lista subpastas cujo nome começa com `instantclient`, para não depender do
/// número da versão no caminho.
fn subpastas_de_instantclient(runtime: &Path) -> Vec<PathBuf> {
    let Ok(entradas) = std::fs::read_dir(runtime) else {
        return Vec::new();
    };
    let mut pastas: Vec<PathBuf> = entradas
        .flatten()
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            caminho.is_dir()
                && caminho
                    .file_name()
                    .and_then(|nome| nome.to_str())
                    .is_some_and(|nome| nome.starts_with("instantclient"))
        })
        .collect();
    // Mais recente primeiro, caso convivam duas versões.
    pastas.sort();
    pastas.reverse();
    pastas
}

#[cfg(test)]
mod testes {
    /// Garante que o Instant Client embutido é achado e carregado de fato.
    /// Não depende de banco algum: só exercita a carga da biblioteca.
    #[test]
    fn encontra_e_carrega_o_instant_client() {
        let situacao = super::preparar();
        assert!(
            situacao.utilizavel(),
            "Instant Client não utilizável: {:?}",
            situacao
        );
    }
}

/// Cria, na primeira execução, a pasta de configuração de rede do Cururu com o
/// `sqlnet.ora` próprio, e devolve o caminho dela.
///
/// Fica no perfil do usuário, e não na pasta do aplicativo, para funcionar
/// mesmo com o Cururu num diretório sem permissão de escrita.
fn preparar_configuracao_de_rede() -> std::io::Result<PathBuf> {
    let pasta = crate::perfis::pasta_de_dados()
        .map_err(|erro| std::io::Error::other(erro.mensagem))?
        .join("rede");
    std::fs::create_dir_all(&pasta)?;

    let sqlnet = pasta.join("sqlnet.ora");
    if !sqlnet.exists() {
        std::fs::write(&sqlnet, SQLNET_ORA)?;
    }
    Ok(pasta)
}
