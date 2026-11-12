//! Conexões salvas (requisitos 1 e 24).
//!
//! Ficam em `%APPDATA%\Cururu\conexoes.json`. A senha nunca é gravada em
//! texto puro: quando "Salvar Senha" está marcado, ela vai cifrada pelo DPAPI
//! do Windows. A listagem devolve apenas `temSenhaSalva`; a senha guardada
//! é usada ao conectar ou quando o usuário pede para mostrar ou copiar.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::dpapi;
use crate::erro::{Erro, Resultado};

/// Banco de dados a que a conexão se destina (requisito 1).
///
/// Oracle é o padrão porque é o que existia antes: os `conexoes.json` gravados
/// até aqui não têm o campo, e o `serde(default)` precisa lê-los como Oracle
/// sem migração nenhuma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TipoBanco {
    #[default]
    Oracle,
    SqlServer,
    MongoDb,
}

/// Modo de autenticação do SQL Server, como no SSMS.
///
/// `Windows` sem usuário informado é a autenticação integrada: vale a
/// identidade de quem está logado no Windows. Com usuário e senha, é a mesma
/// autenticação do Windows feita com outra credencial — `DOMINIO\usuario`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AutenticacaoSql {
    #[default]
    SqlServer,
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TipoAutenticacao {
    #[default]
    Padrao,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Atribuicao {
    #[default]
    Padrao,
    Sysdba,
    Sysoper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TipoConexao {
    #[default]
    Basico,
    Tns,
    Avancado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IdentificacaoBanco {
    Sid,
    #[default]
    Servico,
}

/// Perfil como a tela conhece. Também é o que trafega nos comandos.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Perfil {
    #[serde(default)]
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub cor: Option<String>,
    /// Qual banco esta conexão abre. É o que decide quais campos da tela valem:
    /// os do Oracle ou os do SQL Server.
    #[serde(default)]
    pub tipo_banco: TipoBanco,
    #[serde(default)]
    pub tipo_autenticacao: TipoAutenticacao,
    pub usuario: String,
    /// Só existe no sentido tela → backend. Nunca é gravada nem devolvida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub senha: Option<String>,
    #[serde(default)]
    pub salvar_senha: bool,
    /// Calculado na leitura: existe senha guardada para este perfil?
    #[serde(default)]
    pub tem_senha_salva: bool,
    #[serde(default)]
    pub atribuicao: Atribuicao,
    #[serde(default)]
    pub tipo_conexao: TipoConexao,
    #[serde(default)]
    pub host: String,
    #[serde(default = "porta_padrao")]
    pub porta: u16,
    #[serde(default)]
    pub identificacao: IdentificacaoBanco,
    #[serde(default)]
    pub sid: String,
    #[serde(default)]
    pub servico: String,
    #[serde(default)]
    pub alias_tns: String,
    #[serde(default)]
    pub url_personalizada: String,
    #[serde(default)]
    pub usar_proxy: bool,
    #[serde(default)]
    pub usuario_proxy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub senha_proxy: Option<String>,

    // --- Só do SQL Server ---------------------------------------------------
    /// "Nome do Servidor" do SSMS: `HOST`, `HOST\INSTANCIA` ou `HOST,porta`.
    ///
    /// Fica num campo só, e não em host + porta como no Oracle, porque é assim
    /// que o SSMS pede e é a única forma em que uma instância nomeada cabe —
    /// dividir em dois campos perderia o `\INSTANCIA`.
    #[serde(default)]
    pub servidor: String,
    /// Autenticação do SQL Server ou do Windows.
    ///
    /// `usuario`, `senha` e `salvar_senha` são os mesmos campos do Oracle: o
    /// conceito é o mesmo e só o rótulo da tela muda ("Logon" ou "Nome de
    /// usuário"). Guardar um segundo par de credenciais duplicaria o segredo no
    /// DPAPI sem nenhum ganho.
    #[serde(default)]
    pub autenticacao_sql: AutenticacaoSql,

    /// Auto-commit é comportamento do cliente no Oracle. Desligado por padrão,
    /// como no SQL Developer (requisito 7).
    #[serde(default)]
    pub auto_commit: bool,
}

fn porta_padrao() -> u16 {
    1521
}

/// Como o perfil é guardado em disco: os mesmos campos mais as senhas cifradas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerfilEmDisco {
    #[serde(flatten)]
    dados: Perfil,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    senha_protegida: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    senha_proxy_protegida: Option<String>,
}

/// Repositório das conexões salvas.
///
/// O `Mutex` serializa as gravações para dois comandos simultâneos não
/// sobrescreverem um ao outro.
#[derive(Default)]
pub struct Repositorio {
    trava: Mutex<()>,
}

impl Repositorio {
    pub fn listar(&self) -> Resultado<Vec<Perfil>> {
        Ok(self
            .ler_arquivo()?
            .into_iter()
            .map(|guardado| {
                let tem_senha = guardado.senha_protegida.is_some();
                Perfil {
                    senha: None,
                    tem_senha_salva: tem_senha,
                    senha_proxy: None,
                    ..guardado.dados
                }
            })
            .collect())
    }

    /// Insere ou atualiza. Devolve o perfil como a tela deve passar a conhecê-lo.
    pub fn salvar(&self, mut perfil: Perfil) -> Resultado<Perfil> {
        validar_cor(perfil.cor.as_deref())?;
        if perfil.tipo_banco == TipoBanco::MongoDb {
            crate::mongo::validar_uri(&perfil.servidor)?;
        }
        let _trava = self.trava.lock().unwrap_or_else(|erro| erro.into_inner());
        let mut guardados = self.ler_arquivo()?;

        if perfil.id.is_empty() {
            perfil.id = identificador_novo();
        }
        let anterior = guardados.iter().position(|g| g.dados.id == perfil.id);

        // Senha: usa a nova quando informada; senão preserva a que já existia.
        let senha_anterior = anterior.and_then(|i| guardados[i].senha_protegida.clone());
        let senha_protegida = match (perfil.salvar_senha, perfil.senha.as_deref()) {
            (false, _) => None,
            (true, Some(nova)) if !nova.is_empty() => Some(dpapi::proteger(nova)?),
            (true, _) => senha_anterior,
        };

        let proxy_anterior = anterior.and_then(|i| guardados[i].senha_proxy_protegida.clone());
        let senha_proxy_protegida = match (perfil.salvar_senha, perfil.senha_proxy.as_deref()) {
            (false, _) => None,
            (true, Some(nova)) if !nova.is_empty() => Some(dpapi::proteger(nova)?),
            (true, _) => proxy_anterior,
        };

        let mut dados = perfil.clone();
        dados.senha = None;
        dados.senha_proxy = None;
        dados.tem_senha_salva = senha_protegida.is_some();

        let registro = PerfilEmDisco {
            dados: dados.clone(),
            senha_protegida,
            senha_proxy_protegida,
        };
        match anterior {
            Some(indice) => guardados[indice] = registro,
            None => guardados.push(registro),
        }
        self.gravar_arquivo(&guardados)?;

        Ok(dados)
    }

    pub fn excluir(&self, id: &str) -> Resultado<()> {
        let _trava = self.trava.lock().unwrap_or_else(|erro| erro.into_inner());
        let mut guardados = self.ler_arquivo()?;
        guardados.retain(|g| g.dados.id != id);
        self.gravar_arquivo(&guardados)
    }

    /// Representação completa usada pelo backup, inclusive senhas já protegidas
    /// pelo DPAPI. O backup recebe apenas o conteúdo cifrado.
    pub fn dados_para_backup(&self) -> Resultado<serde_json::Value> {
        let _trava = self.trava.lock().unwrap_or_else(|erro| erro.into_inner());
        Ok(serde_json::to_value(self.ler_arquivo()?)?)
    }

    /// Substitui as conexões pelo conteúdo previamente validado de um backup.
    pub fn restaurar_backup(&self, valor: serde_json::Value) -> Resultado<()> {
        let _trava = self.trava.lock().unwrap_or_else(|erro| erro.into_inner());
        let perfis: Vec<PerfilEmDisco> = serde_json::from_value(valor)
            .map_err(|erro| Erro::novo(format!("conexões inválidas no backup: {erro}")))?;
        for perfil in &perfis {
            if perfil.dados.tipo_banco == TipoBanco::MongoDb {
                crate::mongo::validar_uri(&perfil.dados.servidor)?;
            }
        }
        self.gravar_arquivo(&perfis)
    }

    /// Abre somente a senha pedida explicitamente no editor da conexão.
    pub fn senha_salva(&self, id: &str) -> Resultado<Option<String>> {
        let protegida = self
            .ler_arquivo()?
            .into_iter()
            .find(|guardado| !id.is_empty() && guardado.dados.id == id)
            .and_then(|guardado| guardado.senha_protegida);
        protegida.as_deref().map(dpapi::desproteger).transpose()
    }

    /// Completa o perfil vindo da tela com as senhas guardadas, quando a tela
    /// não informou uma nova.
    pub fn com_senhas(&self, perfil: &Perfil) -> Resultado<Perfil> {
        let mut completo = perfil.clone();
        if senha_em_branco(&completo.senha) || senha_em_branco(&completo.senha_proxy) {
            if let Some(guardado) = self
                .ler_arquivo()?
                .into_iter()
                .find(|g| !perfil.id.is_empty() && g.dados.id == perfil.id)
            {
                if senha_em_branco(&completo.senha) {
                    completo.senha = match guardado.senha_protegida {
                        Some(cifrada) => Some(dpapi::desproteger(&cifrada)?),
                        None => completo.senha,
                    };
                }
                if senha_em_branco(&completo.senha_proxy) {
                    completo.senha_proxy = match guardado.senha_proxy_protegida {
                        Some(cifrada) => Some(dpapi::desproteger(&cifrada)?),
                        None => completo.senha_proxy,
                    };
                }
            }
        }
        Ok(completo)
    }

    // --- Arquivo ------------------------------------------------------------

    fn ler_arquivo(&self) -> Resultado<Vec<PerfilEmDisco>> {
        let caminho = caminho_do_arquivo()?;
        match std::fs::read_to_string(&caminho) {
            Ok(conteudo) if conteudo.trim().is_empty() => Ok(Vec::new()),
            Ok(conteudo) => Ok(serde_json::from_str(&conteudo)?),
            Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(erro) => Err(Erro::novo(format!(
                "não foi possível ler {}: {erro}",
                caminho.display()
            ))),
        }
    }

    fn gravar_arquivo(&self, perfis: &[PerfilEmDisco]) -> Resultado<()> {
        let caminho = caminho_do_arquivo()?;
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta)?;
        }
        let conteudo = serde_json::to_string_pretty(perfis)
            .map_err(|erro| Erro::novo(format!("falha ao montar o arquivo: {erro}")))?;

        // Grava em arquivo temporário e troca, para não corromper o original se
        // faltar energia no meio da escrita.
        let temporario = caminho.with_extension("json.tmp");
        std::fs::write(&temporario, conteudo)?;
        std::fs::rename(&temporario, &caminho)?;
        Ok(())
    }
}

fn senha_em_branco(senha: &Option<String>) -> bool {
    senha.as_deref().is_none_or(str::is_empty)
}

/// A cor é obrigatória e precisa continuar legível como identificação na UI.
/// Os limites eliminam preto/branco, cinzas e tons excessivamente claros ou escuros.
fn validar_cor(cor: Option<&str>) -> Resultado<()> {
    let Some(cor) = cor else {
        return Err(Erro::novo("escolha uma cor para identificar a conexão"));
    };
    let bytes = cor.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' {
        return Err(Erro::novo("a cor da conexão deve estar no formato #RRGGBB"));
    }

    let canal = |inicio: usize| {
        std::str::from_utf8(&bytes[inicio..inicio + 2])
            .ok()
            .and_then(|trecho| u8::from_str_radix(trecho, 16).ok())
    };
    let Some((vermelho, verde, azul)) = canal(1)
        .zip(canal(3))
        .zip(canal(5))
        .map(|((vermelho, verde), azul)| (vermelho, verde, azul))
    else {
        return Err(Erro::novo("a cor da conexão deve estar no formato #RRGGBB"));
    };

    let maior = vermelho.max(verde).max(azul) as u16;
    let menor = vermelho.min(verde).min(azul) as u16;
    if maior + menor < 143 || maior + menor > 367 || maior - menor < 50 {
        return Err(Erro::novo(
            "escolha uma cor média e bem definida; preto, branco, cinza e tons muito claros ou escuros não são aceitos",
        ));
    }
    Ok(())
}

pub fn pasta_de_dados() -> Resultado<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| Erro::novo("variável de ambiente APPDATA não definida"))?;
    Ok(PathBuf::from(appdata).join("Cururu"))
}

fn caminho_do_arquivo() -> Resultado<PathBuf> {
    Ok(pasta_de_dados()?.join("conexoes.json"))
}

/// Identificador estável, derivado do relógio. Não precisa ser aleatório: só
/// precisa não repetir dentro do mesmo arquivo.
pub(crate) fn identificador_novo() -> String {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}{:x}", agora.as_secs(), agora.subsec_nanos())
}

#[cfg(test)]
mod tests {
    use super::validar_cor;

    #[test]
    fn aceita_toda_a_paleta_de_conexoes() {
        for cor in [
            "#3569D4", "#E97814", "#2F9B58", "#9B4BB3", "#1596A6", "#D64F87", "#B28704", "#D94B3D",
            "#5961C7", "#936748", "#758D24", "#3E9DDB",
        ] {
            assert!(validar_cor(Some(cor)).is_ok(), "cor rejeitada: {cor}");
        }
    }

    #[test]
    fn rejeita_ausencia_formato_e_cores_problematicas() {
        for cor in [
            None,
            Some(""),
            Some("azul"),
            Some("#000000"),
            Some("#FFFFFF"),
            Some("#777777"),
        ] {
            assert!(
                validar_cor(cor).is_err(),
                "cor aceita indevidamente: {cor:?}"
            );
        }
    }
}
