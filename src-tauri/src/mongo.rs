//! Navegação e consultas somente leitura do MongoDB.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use mongodb::bson::{doc, Bson, Document};
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection};
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::erro::{Erro, Resultado};
use crate::perfis::Perfil;
use crate::registro;

const PRAZO: Duration = Duration::from_secs(12);
const TAMANHO_DA_AMOSTRA: i64 = 12;
const LIMITE_DE_CAMPOS: usize = 500;
const LIMITE_DE_RESULTADO: usize = 8 * 1024 * 1024;

type Amostra = Arc<OnceCell<Vec<String>>>;

pub struct Sessao {
    cliente: Client,
    amostras: Mutex<HashMap<(String, String), Amostra>>,
}

/// A URI é gravada no perfil. A credencial passa pelos campos protegidos pelo
/// DPAPI e só entra na URI temporária entregue ao driver.
pub fn validar_uri(uri: &str) -> Resultado<()> {
    let (_, destino) = partes_da_uri(uri)?;
    let autoridade = destino.split(['/', '?']).next().unwrap_or_default();
    if autoridade.is_empty() || autoridade.contains('@') {
        return Err(Erro::novo(
            "informe o endereço MongoDB e coloque usuário e senha nos campos próprios",
        ));
    }
    if let Some((_, opcoes)) = destino.split_once('?') {
        if opcoes.split('&').any(|opcao| {
            opcao.split_once('=').is_some_and(|(chave, _)| {
                let chave = percent_decode_str(chave).decode_utf8_lossy();
                chave.eq_ignore_ascii_case("authMechanismProperties")
                    || chave.to_ascii_lowercase().contains("password")
            })
        }) {
            return Err(Erro::novo(
                "retire as opções de credenciais da URI do MongoDB",
            ));
        }
    }
    Ok(())
}

fn partes_da_uri(uri: &str) -> Resultado<(&str, &str)> {
    uri.trim()
        .split_once("://")
        .filter(|(esquema, _)| matches!(*esquema, "mongodb" | "mongodb+srv"))
        .ok_or_else(|| Erro::novo("use uma URI iniciada por mongodb:// ou mongodb+srv://"))
}

fn uri_de_conexao(perfil: &Perfil) -> Resultado<String> {
    validar_uri(&perfil.servidor)?;
    if perfil.usuario.is_empty() {
        if perfil
            .senha
            .as_deref()
            .is_some_and(|senha| !senha.is_empty())
        {
            return Err(Erro::novo("informe o usuário da senha do MongoDB"));
        }
        return Ok(perfil.servidor.trim().to_string());
    }
    let (esquema, destino) = partes_da_uri(&perfil.servidor)?;
    let usuario = utf8_percent_encode(&perfil.usuario, NON_ALPHANUMERIC);
    let senha = utf8_percent_encode(perfil.senha.as_deref().unwrap_or(""), NON_ALPHANUMERIC);
    Ok(format!("{esquema}://{usuario}:{senha}@{destino}"))
}

impl Sessao {
    pub async fn abrir(perfil: &Perfil) -> Resultado<Self> {
        let uri = uri_de_conexao(perfil)?;
        let mut opcoes = ClientOptions::parse(uri)
            .await
            .map_err(|_| Erro::novo("não foi possível interpretar ou resolver a URI do MongoDB"))?;
        opcoes.server_selection_timeout = Some(
            opcoes
                .server_selection_timeout
                .map_or(PRAZO, |tempo| tempo.min(PRAZO)),
        );
        opcoes.app_name = Some("Cururu".to_string());
        let cliente = Client::with_options(opcoes)
            .map_err(|_| Erro::novo("as opções de conexão do MongoDB são inválidas"))?;
        cliente
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(erro_do_driver)?;
        Ok(Self {
            cliente,
            amostras: Mutex::new(HashMap::new()),
        })
    }

    pub async fn bancos(&self) -> Resultado<Vec<String>> {
        let mut bancos = self
            .cliente
            .list_database_names()
            .authorized_databases(true)
            .await
            .map_err(erro_do_driver)?;
        if let Some(banco) = self.cliente.default_database() {
            if !bancos.iter().any(|nome| nome == banco.name()) {
                bancos.push(banco.name().to_string());
            }
        }
        bancos.sort_unstable();
        Ok(bancos)
    }

    pub async fn colecoes(&self, banco: &str) -> Resultado<Vec<String>> {
        validar_nome(banco)?;
        let mut colecoes = self
            .cliente
            .database(banco)
            .list_collection_names()
            .authorized_collections(true)
            .await
            .map_err(erro_do_driver)?;
        colecoes.sort_unstable();
        Ok(colecoes)
    }

    pub async fn consultar(
        &self,
        banco: &str,
        colecao: &str,
        filtro: &str,
        limite: u32,
    ) -> Resultado<Vec<Value>> {
        let filtro = ler_filtro(filtro)?;
        let mut cursor = self
            .colecao(banco, colecao)?
            .find(filtro)
            .limit(i64::from(limite.clamp(1, 100)))
            .batch_size(16)
            .max_time(PRAZO)
            .await
            .map_err(erro_do_driver)?;
        let mut documentos = Vec::new();
        let mut bytes = 0usize;
        while let Some(documento) = cursor.try_next().await.map_err(erro_do_driver)? {
            let documento = Bson::Document(documento).into_canonical_extjson();
            bytes += documento.to_string().len();
            if !documentos.is_empty() && bytes > LIMITE_DE_RESULTADO {
                break;
            }
            documentos.push(documento);
        }
        Ok(documentos)
    }

    pub async fn campos(&self, banco: &str, colecao: &str) -> Resultado<Vec<String>> {
        let alvo = self.colecao(banco, colecao)?;
        let amostra = self
            .amostras
            .lock()
            .unwrap_or_else(|erro| erro.into_inner())
            .entry((banco.to_string(), colecao.to_string()))
            .or_default()
            .clone();
        let campos = amostra
            .get_or_try_init(|| async {
                let mut cursor = alvo
                    .find(doc! {})
                    .limit(TAMANHO_DA_AMOSTRA)
                    .batch_size(4)
                    .max_time(PRAZO)
                    .await
                    .map_err(erro_do_driver)?;
                let mut campos = BTreeSet::new();
                while let Some(documento) = cursor.try_next().await.map_err(erro_do_driver)? {
                    coletar_campos(&documento, "", 0, &mut campos);
                    if campos.len() >= LIMITE_DE_CAMPOS {
                        break;
                    }
                }
                Ok::<_, Erro>(campos.into_iter().collect())
            })
            .await?;
        Ok(campos.clone())
    }

    fn colecao(&self, banco: &str, colecao: &str) -> Resultado<Collection<Document>> {
        validar_nome(banco)?;
        validar_nome(colecao)?;
        Ok(self.cliente.database(banco).collection(colecao))
    }
}

fn validar_nome(nome: &str) -> Resultado<()> {
    if nome.is_empty() || nome.contains('\0') {
        return Err(Erro::novo(
            "informe um banco e uma coleção válidos do MongoDB",
        ));
    }
    Ok(())
}

fn ler_filtro(filtro: &str) -> Resultado<Document> {
    if filtro.trim().is_empty() {
        return Ok(doc! {});
    }
    // Desserializar BSON direto mantém a ordem de chaves em subdocumentos,
    // relevante para comparações de igualdade de documentos no MongoDB.
    serde_json::from_str(filtro).map_err(|erro| {
        Erro::novo(format!(
            "use um objeto Extended JSON como filtro (linha {}, coluna {})",
            erro.line(),
            erro.column(),
        ))
    })
}

fn coletar_campos(
    documento: &Document,
    prefixo: &str,
    nivel: usize,
    campos: &mut BTreeSet<String>,
) {
    if nivel >= 8 {
        return;
    }
    for (nome, valor) in documento {
        if campos.len() >= LIMITE_DE_CAMPOS {
            break;
        }
        let caminho = format!("{prefixo}{nome}");
        campos.insert(caminho.clone());
        let prefixo_filho = format!("{caminho}.");
        match valor {
            Bson::Document(filho) => coletar_campos(filho, &prefixo_filho, nivel + 1, campos),
            Bson::Array(itens) => {
                for item in itens.iter().take(12) {
                    if let Bson::Document(filho) = item {
                        coletar_campos(filho, &prefixo_filho, nivel + 1, campos);
                    }
                }
            }
            _ => {}
        }
    }
}

fn erro_do_driver(erro: mongodb::error::Error) -> Erro {
    // O texto bruto do driver pode trazer detalhes do filtro ou da credencial.
    match erro.kind.as_ref() {
        mongodb::error::ErrorKind::Command(erro) => Erro::novo(format!(
            "MongoDB recusou a operação ({}, código {})",
            erro.code_name, erro.code
        )),
        mongodb::error::ErrorKind::Authentication { .. } => {
            Erro::novo("autenticação do MongoDB recusada; confira usuário, senha e authSource")
        }
        mongodb::error::ErrorKind::ServerSelection { .. } => {
            Erro::novo("não foi possível alcançar o servidor MongoDB")
        }
        _ => Erro::novo("falha ao executar a operação no MongoDB"),
    }
}

pub async fn registrar<T>(
    origem: &str,
    operacao: impl Future<Output = Resultado<T>>,
) -> Resultado<T> {
    let inicio = Instant::now();
    let resultado = tokio::time::timeout(PRAZO, operacao)
        .await
        .unwrap_or_else(|_| Err(Erro::novo("tempo esgotado na operação do MongoDB")));
    let ms = inicio.elapsed().as_millis();
    match &resultado {
        Ok(_) => registro::informacao(origem, format!("ok em {ms} ms")),
        Err(erro) => registro::aviso(origem, format!("falhou em {ms} ms: {erro}")),
    }
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn extended_json_preserva_tipos_e_ordem_dos_subdocumentos() {
        let filtro = ler_filtro(r#"{"_id":{"$oid":"5e349915cebae490877d561d"},"ordem":{"z":1,"a":{"$numberLong":"9007199254740993"}}}"#).unwrap();
        assert_eq!(
            filtro.get_object_id("_id").unwrap().to_hex(),
            "5e349915cebae490877d561d"
        );
        let ordem = filtro.get_document("ordem").unwrap();
        assert_eq!(
            ordem.keys().map(String::as_str).collect::<Vec<_>>(),
            ["z", "a"]
        );
        assert_eq!(ordem.get_i64("a").unwrap(), 9_007_199_254_740_993);
        assert!(ler_filtro("[]").is_err());
        assert!(ler_filtro(r#"{"_id":{"$oid":"inválido"}}"#).is_err());
    }

    #[test]
    fn uri_gravavel_nao_aceita_segredos_e_credenciais_sao_codificadas() {
        assert!(validar_uri("mongodb+srv://usuario:segredo@cluster.example").is_err());
        assert!(validar_uri("mongodb://localhost/?authMechanismProperties=TOKEN:segredo").is_err());
        assert!(
            validar_uri("mongodb://localhost/?tlsCertificateKeyFile%50assword=segredo").is_err()
        );
        let perfil = Perfil {
            servidor: "mongodb://localhost/teste?authSource=admin".to_string(),
            usuario: "u@".to_string(),
            senha: Some("p:/#".to_string()),
            ..Perfil::default()
        };
        assert_eq!(
            uri_de_conexao(&perfil).unwrap(),
            "mongodb://u%40:p%3A%2F%23@localhost/teste?authSource=admin"
        );
    }

    #[test]
    fn amostra_inclui_campos_aninhados_e_arrays_de_documentos() {
        let mut campos = BTreeSet::new();
        coletar_campos(
            &doc! { "pedido": { "codigo": 3 }, "itens": [{ "produto": "A" }] },
            "",
            0,
            &mut campos,
        );
        coletar_campos(&doc! { "itens": [{ "preco": 2 }] }, "", 0, &mut campos);
        assert_eq!(
            campos.into_iter().collect::<Vec<_>>(),
            [
                "itens",
                "itens.preco",
                "itens.produto",
                "pedido",
                "pedido.codigo"
            ]
        );
    }
}
