//! Metadados do SQL Server para o autocomplete (requisitos 3 e 23).
//!
//! O que a [`crate::metadados`] pergunta ao dicionário do Oracle, este módulo
//! pergunta ao catálogo do SQL Server: os **nomes** que começam por um prefixo e
//! as **colunas** de um objeto. O tipo que sobe é o mesmo, e o cache é o mesmo —
//! só as consultas mudam.
//!
//! ## Por que o banco vai no nome, e não num `use`
//!
//! Quem responde ao autocomplete é a sessão **da conexão** (a que a árvore
//! abriu), e não a da aba. É preciso ser assim: no TDS a conexão atende um
//! pedido de cada vez, e a sessão da aba pode estar com uma consulta correndo ou
//! com um grid pela metade — pedir sugestão ali faria a tecla digitada esperar a
//! consulta terminar, ou pior, derrubar o cursor do grid.
//!
//! Só que a aba pode estar em outro banco (requisito 3), e o autocomplete tem de
//! falar do banco **da aba**. Daí o nome de três partes: `[Vendas].sys.objects`.
//! Um `use` na sessão da conexão resolveria a leitura e estragaria a árvore, que
//! está justamente olhando o banco de onde ela veio.

use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tiberius::{Row, ToSql};

use crate::erro::Resultado;
use crate::metadados::Sugestao;
use crate::objetos::Coluna;
use crate::perfis::{self, Perfil};
use crate::sqlserver::{entre_colchetes, Cliente};

/// Mesmo teto da busca no Oracle: o que passa disso não cabe numa lista.
const LIMITE_DE_SUGESTOES: usize = 300;

/// Tipos de `sys.objects` que interessam numa consulta.
///
/// `U` tabela, `V` view, `SN` sinônimo, `P` procedure, `FN`/`IF`/`TF` função
/// escalar, em linha e de tabela, `AF` função de agregação do CLR.
const TIPOS: &str = "'U', 'V', 'SN', 'P', 'FN', 'IF', 'TF', 'AF'";

/// Uma leitura do catálogo alimenta tanto o FROM quanto o WHERE. Se o banco
/// passar do teto, a consulta pontual continua sendo a fonte de sugestões.
const LIMITE_DO_CATALOGO: usize = 50_000;

#[derive(Serialize, Deserialize)]
pub struct Catalogo {
    objetos: Vec<(String, Sugestao)>,
    colunas: HashMap<String, HashMap<String, Vec<Coluna>>>,
    esquema_padrao: String,
}

impl Catalogo {
    pub fn sugerir(&self, prefixo: &str, esquema: Option<&str>) -> Option<Vec<Sugestao>> {
        // Quando a grafia do esquema não é a do catálogo, deixe o servidor
        // resolver o nome de acordo com a collation do banco.
        if esquema.is_some_and(|esquema| !self.objetos.iter().any(|(dono, _)| dono == esquema)) {
            return None;
        }
        let prefixo = prefixo.trim().to_uppercase();
        Some(
            self.objetos
                .iter()
                .filter(|(dono, objeto)| {
                    esquema.map_or(true, |esquema| dono == esquema)
                        && objeto.nome.to_uppercase().starts_with(&prefixo)
                })
                .take(LIMITE_DE_SUGESTOES)
                .map(|(_, objeto)| objeto.clone())
                .collect(),
        )
    }

    pub fn colunas(&self, objeto: &str) -> Option<Vec<Coluna>> {
        let objeto = objeto.trim();
        let (esquema, nome) = objeto
            .split_once('.')
            .unwrap_or((&self.esquema_padrao, objeto));
        self.colunas
            .get(esquema)?
            .get(nome)
            .filter(|colunas| !colunas.is_empty())
            .cloned()
    }
}

/// Só nomes e tipos de metadados ficam em disco. Mudanças no servidor ou no
/// login do perfil mudam a chave, mesmo que o identificador seja o mesmo.
pub fn caminho_do_catalogo(perfil: &Perfil, banco: &str) -> Resultado<PathBuf> {
    let mut hash = DefaultHasher::new();
    (
        &perfil.id,
        &perfil.servidor,
        &perfil.usuario,
        serde_json::to_string(&perfil.autenticacao_sql)?,
        banco,
    )
        .hash(&mut hash);
    Ok(perfis::pasta_de_dados()?
        .join("indice")
        .join(format!("sqlserver-v1-{:016x}.json", hash.finish())))
}

pub fn ler_do_disco(caminho: &Path) -> Resultado<Option<Catalogo>> {
    let arquivo = match std::fs::File::open(caminho) {
        Ok(arquivo) => arquivo,
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(erro) => return Err(erro.into()),
    };
    Ok(Some(serde_json::from_reader(std::io::BufReader::new(
        arquivo,
    ))?))
}

pub fn guardar_no_disco(caminho: &Path, catalogo: &Catalogo) -> Resultado<()> {
    static PROXIMO: AtomicU64 = AtomicU64::new(0);
    let temporario = caminho.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::Relaxed)
    ));
    if let Some(pasta) = caminho.parent() {
        std::fs::create_dir_all(pasta)?;
    }
    std::fs::write(&temporario, serde_json::to_vec(catalogo)?)?;
    let resultado = std::fs::rename(&temporario, caminho);
    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporario);
    }
    Ok(resultado?)
}

/// A conexão de leitura já está no banco alvo. SCHEMA_NAME e TYPE_NAME usam
/// esse contexto, inclusive quando o esquema padrão ou o tipo é personalizado.
pub async fn catalogo(cliente: &mut Cliente) -> Resultado<Option<Catalogo>> {
    let sql = format!(
        "select top ({LIMITE_DO_CATALOGO}) o.object_id, o.name as objeto, o.type as especie, \
                e.name as esquema, schema_name() as esquema_padrao, c.name as nome, \
                type_name(c.user_type_id) as tipo, c.max_length as tamanho, \
                c.precision as precisao, c.scale as escala, c.is_nullable as aceita_nulo \
           from sys.objects o \
           join sys.schemas e on e.schema_id = o.schema_id \
           left join sys.columns c on c.object_id = o.object_id \
          where o.is_ms_shipped = 0 and o.type in ({TIPOS}) \
          order by case when e.name = 'dbo' then 0 else 1 end, o.name, o.object_id, c.column_id"
    );
    let linhas = cliente.simple_query(sql).await?.into_first_result().await?;
    if linhas.len() == LIMITE_DO_CATALOGO {
        return Ok(None);
    }
    let mut catalogo = Catalogo {
        objetos: Vec::new(),
        colunas: HashMap::new(),
        esquema_padrao: linhas
            .first()
            .and_then(|linha| linha.get::<&str, _>("esquema_padrao"))
            .unwrap_or("dbo")
            .to_string(),
    };
    let mut ultimo = None;
    for linha in &linhas {
        let id = linha.get::<i32, _>("object_id");
        let nome = linha.get::<&str, _>("objeto").unwrap_or_default();
        let esquema = linha.get::<&str, _>("esquema").unwrap_or("dbo");
        if id != ultimo {
            let especie = linha.get::<&str, _>("especie").unwrap_or("").trim();
            catalogo.objetos.push((
                esquema.to_string(),
                Sugestao {
                    nome: nome.to_string(),
                    tipo: rotulo_do_tipo(especie).to_string(),
                    dono: (esquema != "dbo").then(|| esquema.to_string()),
                },
            ));
            ultimo = id;
        }
        if let Some(coluna) = ler_coluna(linha) {
            catalogo
                .colunas
                .entry(esquema.to_string())
                .or_default()
                .entry(nome.to_string())
                .or_default()
                .push(coluna);
        }
    }
    Ok(Some(catalogo))
}

/// Nomes do banco que começam pelo prefixo digitado.
///
/// A ordem põe o esquema padrão na frente: é o que se alcança escrevendo só o
/// nome da tabela, e portanto o que quase sempre se quer.
///
/// `esquema` é a segunda parte do nome de três partes — `Vendas.dbo.cli` —, e
/// quando vem preenchida corta a busca àquele esquema. É a diferença entre
/// oferecer o que o nome escrito alcança e oferecer o que ele não alcança: de
/// `rh.cli` só sai o que está em `rh`. A comparação é a do servidor, como a do
/// `LIKE` do prefixo: em collation que não diferencia caixa, `RH` acha `rh`.
/// `filtro_de_tabelas` chega preenchido na opção "apenas as filtradas"
/// (requisito 43) e vale **só para as linhas de tabela** (`o.type = 'U'`):
/// view, procedure, função e sinônimo não são da categoria Tabelas e passam
/// intactos, como no Oracle.
pub async fn sugerir(
    cliente: &mut Cliente,
    banco: Option<&str>,
    esquema: Option<&str>,
    prefixo: &str,
    filtro_de_tabelas: Option<&crate::filtro::Filtro>,
) -> Resultado<Vec<Sugestao>> {
    let catalogo = qualificar(banco, "sys");
    let so_do_esquema = if esquema.is_some() {
        " and e.name = @P2"
    } else {
        ""
    };
    // O prefixo é sempre o `@P1`; o esquema, quando existe, é o `@P2`. O filtro
    // continua a numeração de onde eles pararam.
    let recorte = match filtro_de_tabelas {
        Some(filtro) => filtro.condicao_tsql("o.name", if esquema.is_some() { 3 } else { 2 }),
        None => crate::filtro::CondicaoTsql::default(),
    };
    let condicao_do_filtro = if recorte.sql.is_empty() {
        String::new()
    } else {
        format!(" and (o.type <> 'U' or ({}))", recorte.sql)
    };

    // A collation do banco pode diferenciar maiúsculas. O cache de sugestões
    // não diferencia, então a consulta precisa ter a mesma regra: `TBL` deve
    // achar `tbl_503_nota_serv` antes de guardar o resultado.
    let sql = format!(
        "select top ({LIMITE_DE_SUGESTOES}) o.name as nome, o.type as tipo, e.name as esquema \
           from {catalogo}.objects o \
           join {catalogo}.schemas e on e.schema_id = o.schema_id \
          where o.is_ms_shipped = 0 and o.type in ({TIPOS}) \
            and o.name collate Latin1_General_CI_AS like @P1 escape '\\'{so_do_esquema}{condicao_do_filtro} \
          order by case when e.name = 'dbo' then 0 else 1 end, o.name"
    );

    let padrao = padrao_de_prefixo(prefixo);
    let mut parametros: Vec<&dyn ToSql> = vec![&padrao];
    if let Some(esquema) = esquema.as_ref() {
        parametros.push(esquema);
    }
    parametros.extend(recorte.valores.iter().map(|valor| valor as &dyn ToSql));
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    Ok(linhas
        .iter()
        .filter_map(|linha| {
            let nome = linha.get::<&str, _>("nome")?.to_string();
            let esquema = linha.get::<&str, _>("esquema").unwrap_or("dbo");
            Some(Sugestao {
                nome,
                tipo: rotulo_do_tipo(linha.get::<&str, _>("tipo").unwrap_or("").trim()).to_string(),
                // Como no Oracle: o dono só aparece quando não é o de casa.
                dono: (esquema != "dbo").then(|| esquema.to_string()),
            })
        })
        .collect())
}

/// Colunas de uma tabela, view ou função de tabela.
///
/// O nome do objeto vai em **parâmetro**, e não no texto da consulta: quem
/// resolve o nome é o `OBJECT_ID` do servidor, com as mesmas regras de esquema
/// padrão que a consulta do usuário teria. Não há nome a validar nem a escapar.
pub async fn colunas(
    cliente: &mut Cliente,
    banco: Option<&str>,
    objeto: &str,
) -> Resultado<Vec<Coluna>> {
    let catalogo = qualificar(banco, "sys");
    let sql = format!(
        "select c.name as nome, type_name(c.user_type_id) as tipo, \
                c.max_length as tamanho, c.precision as precisao, c.scale as escala, \
                c.is_nullable as aceita_nulo \
           from {catalogo}.columns c \
          where c.object_id = object_id(@P1) \
          order by c.column_id"
    );

    let alvo = nome_completo(banco, objeto);
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    Ok(linhas.iter().filter_map(ler_coluna).collect())
}

fn ler_coluna(linha: &Row) -> Option<Coluna> {
    Some(Coluna {
        nome: linha.get::<&str, _>("nome")?.to_string(),
        tipo: descrever_tipo(
            linha.get::<&str, _>("tipo").unwrap_or(""),
            linha.get::<i16, _>("tamanho").unwrap_or(0),
            linha.get::<u8, _>("precisao").unwrap_or(0),
            linha.get::<u8, _>("escala").unwrap_or(0),
        ),
        aceita_nulo: linha.get::<bool, _>("aceita_nulo").unwrap_or(true),
    })
}

/// `[Vendas].sys` — ou só `sys` quando não se sabe o banco.
pub fn qualificar(banco: Option<&str>, esquema: &str) -> String {
    match banco {
        Some(banco) => format!("{}.{esquema}", entre_colchetes(banco)),
        None => esquema.to_string(),
    }
}

/// O nome que o `OBJECT_ID` recebe, já apontado para o banco da aba.
///
/// `[Vendas]..cliente` é a forma de três partes com o esquema vazio: o servidor
/// resolve pelo esquema padrão do login naquele banco, que é exatamente o que
/// acontece quando o usuário escreve `from cliente`.
pub fn nome_completo(banco: Option<&str>, objeto: &str) -> String {
    let objeto = objeto.trim();
    let Some(banco) = banco else {
        return objeto.to_string();
    };
    let separador = if objeto.contains('.') { "." } else { ".." };
    format!("{}{separador}{objeto}", entre_colchetes(banco))
}

/// `EXEMPLO_` vira `EXEMPLO\_%`.
///
/// O sublinhado é literal também aqui: quem digita `EXEMPLO_` quer o caractere, e
/// não o curinga de uma posição do `LIKE`.
fn padrao_de_prefixo(prefixo: &str) -> String {
    format!(
        "{}%",
        prefixo
            .trim()
            .replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%")
            .replace('[', "\\[")
    )
}

/// O tipo da coluna como o T-SQL o escreve: `varchar(30)`, `numeric(10,2)`.
///
/// O `max_length` do catálogo é em **bytes**; nos tipos Unicode ele vale o dobro
/// dos caracteres declarados, e `-1` é o `(max)`.
pub fn descrever_tipo(tipo: &str, tamanho: i16, precisao: u8, escala: u8) -> String {
    match tipo {
        "char" | "varchar" | "binary" | "varbinary" => com_tamanho(tipo, i32::from(tamanho)),
        "nchar" | "nvarchar" => com_tamanho(
            tipo,
            if tamanho < 0 {
                -1
            } else {
                i32::from(tamanho) / 2
            },
        ),
        "decimal" | "numeric" => format!("{tipo}({precisao},{escala})"),
        "datetime2" | "datetimeoffset" | "time" => format!("{tipo}({escala})"),
        outro => outro.to_string(),
    }
}

fn com_tamanho(tipo: &str, tamanho: i32) -> String {
    if tamanho < 0 {
        return format!("{tipo}(max)");
    }
    format!("{tipo}({tamanho})")
}

/// O código de `sys.objects` traduzido para o rótulo que a lista do editor usa.
///
/// São as mesmas palavras que o Oracle devolve — `TABLE`, `VIEW`, `SYNONYM`,
/// `PROCEDURE`, `FUNCTION` —, porque é por elas que o editor decide onde cada
/// sugestão aparece (requisito 3). Traduzir aqui é o que permite que a regra de
/// lugar seja uma só para os dois bancos.
fn rotulo_do_tipo(tipo: &str) -> &'static str {
    match tipo {
        "U" => "TABLE",
        "V" => "VIEW",
        "SN" => "SYNONYM",
        "P" => "PROCEDURE",
        _ => "FUNCTION",
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_nome_do_objeto_vai_apontado_para_o_banco_da_aba() {
        assert_eq!(
            nome_completo(Some("Vendas"), "cliente"),
            "[Vendas]..cliente"
        );
        assert_eq!(
            nome_completo(Some("Vendas"), "rh.funcionario"),
            "[Vendas].rh.funcionario"
        );
        // Colchete no nome do banco não pode virar comando.
        assert_eq!(nome_completo(Some("a]b"), "t"), "[a]]b]..t");
    }

    #[test]
    fn o_sublinhado_do_prefixo_e_literal() {
        assert_eq!(padrao_de_prefixo("EXEMPLO_"), "EXEMPLO\\_%");
        assert_eq!(padrao_de_prefixo("tbl"), "tbl%");
        // O colchete é curinga de conjunto no LIKE do SQL Server, e um nome com
        // colchete não pode virar faixa de caracteres.
        assert_eq!(padrao_de_prefixo("a[b"), "a\\[b%");
    }

    #[test]
    fn o_tamanho_do_unicode_sai_em_caracteres() {
        assert_eq!(descrever_tipo("nvarchar", 60, 0, 0), "nvarchar(30)");
        assert_eq!(descrever_tipo("varchar", 30, 0, 0), "varchar(30)");
        assert_eq!(descrever_tipo("nvarchar", -1, 0, 0), "nvarchar(max)");
        assert_eq!(descrever_tipo("numeric", 9, 10, 2), "numeric(10,2)");
        assert_eq!(descrever_tipo("int", 4, 10, 0), "int");
    }
}
