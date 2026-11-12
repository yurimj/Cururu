//! Compilação de PL/SQL (requisito 13).
//!
//! Compilar no Oracle é executar o `CREATE OR REPLACE`. Quando há erro de
//! sintaxe o banco **não** recusa o comando: ele grava o objeto como inválido
//! e responde `ORA-24344` — que o driver entrega como **aviso**, e não como
//! erro: o `execute` volta `Ok`. Os erros de verdade ficam em `ALL_ERRORS`, e é
//! de lá que vem a lista com linha e coluna.
//!
//! O editor usa o mesmo caminho: um `CREATE` executado como script avisa quando
//! compilou com erro, e o `SHOW ERRORS` do SQL*Plus é atendido aqui.

use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::sql_texto;

/// Código que o Oracle devolve quando compilou com erros.
pub const SUCESSO_COM_ERRO_DE_COMPILACAO: i32 = 24344;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErroDeCompilacao {
    pub linha: u32,
    pub coluna: u32,
    pub texto: String,
    /// Aviso do compilador (`PLW-`): aparece na lista, mas não deixa o objeto
    /// inválido.
    pub aviso: bool,
}

/// Um objeto compilável, com o tipo escrito como o dicionário o escreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjetoPlsql {
    /// Como aparece em `ALL_ERRORS.TYPE`: `PROCEDURE`, `PACKAGE BODY`…
    pub tipo: String,
    /// Dono escrito no comando. Sem ele vale o schema corrente da sessão, que é
    /// onde o Oracle cria o objeto.
    pub dono: Option<String>,
    pub nome: String,
}

impl ObjetoPlsql {
    /// `PROCEDURE EXEMPLO.GRAV_LOG`, para mensagem e log.
    pub fn descricao(&self) -> String {
        match &self.dono {
            Some(dono) => format!("{} {dono}.{}", self.tipo, self.nome),
            None => format!("{} {}", self.tipo, self.nome),
        }
    }
}

/// Compila o fonte e devolve os erros encontrados. Lista vazia = compilou.
///
/// A linha de cada erro é contada no fonte recebido, e não no objeto: o Oracle
/// numera a partir da linha do tipo (`PROCEDURE …`), e o que vem antes dela —
/// um comentário de cabeçalho, um `CREATE OR REPLACE` sozinho na linha — deixava
/// o clique na mensagem algumas linhas acima do erro.
pub fn compilar(conexao: &oracle::Connection, fonte: &str) -> Resultado<Vec<ErroDeCompilacao>> {
    let fonte = fonte.trim().trim_end_matches('/').trim_end();
    let Some((objeto, linhas_antes)) = localizar_objeto(fonte) else {
        return Err(Erro::novo(
            "não reconheci o objeto: o fonte precisa começar com CREATE OR REPLACE \
             PACKAGE, PACKAGE BODY, PROCEDURE, FUNCTION, TRIGGER ou TYPE",
        ));
    };

    match conexao.execute(fonte, &[]) {
        Ok(_) => {}
        Err(erro) => {
            let compilou_com_erro = erro
                .db_error()
                .is_some_and(|banco| banco.code() == SUCESSO_COM_ERRO_DE_COMPILACAO);
            if !compilou_com_erro {
                return Err(erro.into());
            }
        }
    }

    let mut erros = erros_do_objeto(conexao, &objeto)?;
    for erro in &mut erros {
        erro.linha += linhas_antes as u32;
    }
    Ok(erros)
}

/// Erros de compilação do objeto, na ordem do compilador. A linha é a do
/// objeto, como o Oracle a conta.
///
/// `ALL_ERRORS`, e não `USER_ERRORS`: o `CREATE` pode ter nomeado outro dono
/// (`EXEMPLO.GRAV_LOG`), e o schema corrente da sessão pode não ser o do login.
pub fn erros_do_objeto(
    conexao: &oracle::Connection,
    objeto: &ObjetoPlsql,
) -> Resultado<Vec<ErroDeCompilacao>> {
    let sql = "SELECT line, position, text, attribute \
                 FROM all_errors \
                WHERE owner = NVL(:dono, SYS_CONTEXT('USERENV', 'CURRENT_SCHEMA')) \
                  AND name = :nome \
                  AND type = :tipo \
                ORDER BY sequence";
    let linhas = conexao.query_as_named::<(u32, u32, String, Option<String>)>(
        sql,
        &[
            ("dono", &objeto.dono),
            ("nome", &objeto.nome),
            ("tipo", &objeto.tipo),
        ],
    )?;

    linhas
        .map(|linha| {
            let (linha, coluna, texto, atributo) = linha?;
            Ok(ErroDeCompilacao {
                linha,
                coluna,
                texto: texto.trim_end().to_string(),
                aviso: atributo.as_deref() == Some("WARNING"),
            })
        })
        .collect()
}

/// Tipos que o `CREATE` compila e que deixam erro em `ALL_ERRORS`.
const TIPOS_COMPILAVEIS: &[&str] = &["PACKAGE", "PROCEDURE", "FUNCTION", "TRIGGER", "TYPE", "VIEW"];

/// Descobre tipo, dono e nome do objeto a partir do cabeçalho do fonte, e
/// quantas linhas do texto vêm antes da linha do tipo — que é a linha 1 do
/// objeto para o Oracle.
///
/// Comentários antes do `CREATE` são aceitos: o script que chega do editor os
/// traz junto com o comando.
pub fn localizar_objeto(fonte: &str) -> Option<(ObjetoPlsql, usize)> {
    let palavras = sql_texto::palavras_iniciais(fonte, 16);
    let mut i = sql_texto::indice_do_tipo_no_create(&palavras)?;
    let e = |indice: usize, esperada: &str| {
        palavras
            .get(indice)
            .is_some_and(|(_, palavra)| palavra.eq_ignore_ascii_case(esperada))
    };

    let (posicao_do_tipo, tipo) = palavras[i];
    let tipo = tipo.to_ascii_uppercase();
    if !TIPOS_COMPILAVEIS.contains(&tipo.as_str()) {
        return None;
    }
    i += 1;

    // "PACKAGE BODY X" e "TYPE BODY X" têm o corpo como tipo próprio.
    let tipo_completo = if e(i, "BODY") {
        i += 1;
        format!("{tipo} BODY")
    } else {
        tipo
    };
    // Oracle 23ai: CREATE PROCEDURE IF NOT EXISTS nome.
    if e(i, "IF") && e(i + 1, "NOT") && e(i + 2, "EXISTS") {
        i += 3;
    }

    let (dono, nome) = separar_nome(palavras.get(i)?.1)?;
    let linhas_antes = fonte[..posicao_do_tipo].matches('\n').count();
    Some((
        ObjetoPlsql {
            tipo: tipo_completo,
            dono,
            nome,
        },
        linhas_antes,
    ))
}

// --- SHOW ERRORS -----------------------------------------------------------

/// O que um `SHOW ERRORS` pediu.
#[derive(Debug, PartialEq, Eq)]
pub enum PedidoDeErros {
    /// `SHOW ERRORS` sozinho: o último objeto que a aba compilou, como no
    /// SQL*Plus.
    DoUltimo,
    /// `SHOW ERRORS PROCEDURE dono.nome`.
    Do(ObjetoPlsql),
    /// Começa como `SHOW ERRORS`, mas o resto não se entende.
    Invalido,
}

/// Tipos aceitos depois de `SHOW ERRORS`, na forma do SQL*Plus.
const TIPOS_DO_SHOW: &[&str] = &[
    "FUNCTION",
    "PROCEDURE",
    "PACKAGE",
    "TRIGGER",
    "VIEW",
    "TYPE",
    "DIMENSION",
    "HIERARCHY",
    "LIBRARY",
];

const TIPOS_DO_SHOW_EM_DUAS_PALAVRAS: &[(&str, &str)] = &[
    ("PACKAGE", "BODY"),
    ("TYPE", "BODY"),
    ("JAVA", "SOURCE"),
    ("JAVA", "CLASS"),
    ("ANALYTIC", "VIEW"),
    ("ATTRIBUTE", "DIMENSION"),
];

/// Reconhece o `SHOW ERRORS` do SQL*Plus e do SQL Developer.
///
/// Não é SQL: mandado ao Oracle ele volta `ORA-00900`. Vale a abreviação do
/// SQL*Plus — `SHO ERR` é o mesmo comando — e o comentário no fim da linha.
pub fn pedido_de_erros(sql: &str) -> Option<PedidoDeErros> {
    let resto = sql_texto::sem_comentarios_iniciais(sql);
    let linha = resto.lines().next().unwrap_or("");
    let linha = linha.split("--").next().unwrap_or("");
    let linha = linha.trim().trim_end_matches(';').trim_end();
    let palavras: Vec<&str> = linha.split_whitespace().collect();

    let [mostrar, erros, argumentos @ ..] = palavras.as_slice() else {
        return None;
    };
    if !e_abreviacao(mostrar, "SHOW", 3) || !e_abreviacao(erros, "ERRORS", 3) {
        return None;
    }
    if argumentos.is_empty() {
        return Some(PedidoDeErros::DoUltimo);
    }

    let (tipo, nome) = match argumentos {
        [primeira, segunda, nome]
            if TIPOS_DO_SHOW_EM_DUAS_PALAVRAS
                .iter()
                .any(|(a, b)| igual(primeira, a) && igual(segunda, b)) =>
        {
            (format!("{primeira} {segunda}").to_ascii_uppercase(), *nome)
        }
        [tipo, nome] if TIPOS_DO_SHOW.iter().any(|conhecido| igual(tipo, conhecido)) => {
            (tipo.to_ascii_uppercase(), *nome)
        }
        _ => return Some(PedidoDeErros::Invalido),
    };
    Some(match separar_nome(nome) {
        Some((dono, nome)) => PedidoDeErros::Do(ObjetoPlsql { tipo, dono, nome }),
        None => PedidoDeErros::Invalido,
    })
}

/// O comando é um `SHOW ERRORS`?
pub fn e_mostrar_erros(sql: &str) -> bool {
    pedido_de_erros(sql).is_some()
}

/// `SHO` é `SHOW`, e `ERR` é `ERRORS`: o SQL*Plus aceita qualquer começo da
/// palavra a partir do tamanho mínimo.
fn e_abreviacao(palavra: &str, completa: &str, minimo: usize) -> bool {
    palavra.len() >= minimo
        && completa
            .get(..palavra.len())
            .is_some_and(|inicio| inicio.eq_ignore_ascii_case(palavra))
}

fn igual(palavra: &str, esperada: &str) -> bool {
    palavra.eq_ignore_ascii_case(esperada)
}

/// `dono.nome` em partes, cada uma em maiúsculas — salvo a escrita entre aspas,
/// que o Oracle guarda como está.
fn separar_nome(texto: &str) -> Option<(Option<String>, String)> {
    // `PROCEDURE X(A NUMBER)` chega com os parâmetros colados no nome.
    let texto = texto.split(['(', ';']).next().unwrap_or("");

    let mut partes = Vec::new();
    let mut entre_aspas = false;
    let mut inicio = 0;
    for (posicao, c) in texto.char_indices() {
        match c {
            '"' => entre_aspas = !entre_aspas,
            '.' if !entre_aspas => {
                partes.push(&texto[inicio..posicao]);
                inicio = posicao + 1;
            }
            _ => {}
        }
    }
    partes.push(&texto[inicio..]);

    match partes.as_slice() {
        [nome] => Some((None, normalizar_nome(nome)?)),
        [dono, nome] => Some((Some(normalizar_nome(dono)?), normalizar_nome(nome)?)),
        _ => None,
    }
}

fn normalizar_nome(parte: &str) -> Option<String> {
    match parte.strip_prefix('"').and_then(|p| p.strip_suffix('"')) {
        Some(dentro) => (!dentro.is_empty()).then(|| dentro.to_string()),
        None => (!parte.is_empty()).then(|| parte.to_uppercase()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn objeto(tipo: &str, dono: Option<&str>, nome: &str) -> ObjetoPlsql {
        ObjetoPlsql {
            tipo: tipo.into(),
            dono: dono.map(Into::into),
            nome: nome.into(),
        }
    }

    fn identificar(fonte: &str) -> Option<ObjetoPlsql> {
        localizar_objeto(fonte).map(|(objeto, _)| objeto)
    }

    #[test]
    fn reconhece_procedure() {
        assert_eq!(
            identificar("create or replace procedure calcula is begin null; end;"),
            Some(objeto("PROCEDURE", None, "CALCULA"))
        );
    }

    #[test]
    fn reconhece_corpo_de_pacote() {
        assert_eq!(
            identificar("CREATE OR REPLACE PACKAGE BODY PKG_FATURA AS END;"),
            Some(objeto("PACKAGE BODY", None, "PKG_FATURA"))
        );
    }

    #[test]
    fn reconhece_especificacao_de_pacote() {
        assert_eq!(
            identificar("create package pkg_x as end;"),
            Some(objeto("PACKAGE", None, "PKG_X"))
        );
    }

    #[test]
    fn ignora_editionable() {
        assert_eq!(
            identificar("CREATE OR REPLACE EDITIONABLE FUNCTION f RETURN NUMBER IS BEGIN RETURN 1; END;"),
            Some(objeto("FUNCTION", None, "F"))
        );
        assert_eq!(
            identificar("create nonEditionable package body p as end;"),
            Some(objeto("PACKAGE BODY", None, "P"))
        );
    }

    #[test]
    fn guarda_o_schema_do_nome() {
        assert_eq!(
            identificar("create or replace procedure meu_schema.p is begin null; end;"),
            Some(objeto("PROCEDURE", Some("MEU_SCHEMA"), "P"))
        );
    }

    #[test]
    fn nome_entre_aspas_fica_como_esta() {
        assert_eq!(
            identificar("create or replace procedure \"Dono\".\"minha_proc\" is begin null; end;"),
            Some(objeto("PROCEDURE", Some("Dono"), "minha_proc"))
        );
    }

    #[test]
    fn parametros_colados_no_nome_nao_entram_no_nome() {
        assert_eq!(
            identificar("create procedure p(a number) is begin null; end;"),
            Some(objeto("PROCEDURE", None, "P"))
        );
    }

    #[test]
    fn view_forcada_tambem_se_reconhece() {
        assert_eq!(
            identificar("CREATE OR REPLACE FORCE EDITIONABLE VIEW v AS SELECT 1 x FROM dual"),
            Some(objeto("VIEW", None, "V"))
        );
    }

    #[test]
    fn if_not_exists_nao_vira_nome() {
        assert_eq!(
            identificar("create procedure if not exists p is begin null; end;"),
            Some(objeto("PROCEDURE", None, "P"))
        );
    }

    #[test]
    fn recusa_o_que_nao_e_objeto_compilavel() {
        assert!(identificar("select 1 from dual").is_none());
        assert!(identificar("create table t (a number)").is_none());
        assert!(identificar("begin null; end;").is_none());
        assert!(identificar("create or replace public synonym s for t").is_none());
    }

    #[test]
    fn a_linha_um_do_objeto_e_a_do_tipo() {
        // O script do GRAV_LOG: quatro linhas de comentário antes do CREATE.
        let fonte = "-- um\n-- dois\n-- três\n-- quatro\n\
                     CREATE OR REPLACE EDITIONABLE PROCEDURE EXEMPLO.GRAV_LOG\n(\n  IN_MSG IN VARCHAR2\n) IS\nBEGIN NULL; END;";
        assert_eq!(
            localizar_objeto(fonte),
            Some((objeto("PROCEDURE", Some("EXEMPLO"), "GRAV_LOG"), 4))
        );
        // O tipo numa linha abaixo do CREATE: é dela que o Oracle conta.
        assert_eq!(
            localizar_objeto("create or replace\nprocedure p is begin null; end;").map(|(_, l)| l),
            Some(1)
        );
    }

    #[test]
    fn descricao_leva_o_dono_quando_ha() {
        assert_eq!(
            objeto("PROCEDURE", Some("EXEMPLO"), "GRAV_LOG").descricao(),
            "PROCEDURE EXEMPLO.GRAV_LOG"
        );
        assert_eq!(objeto("PACKAGE BODY", None, "P").descricao(), "PACKAGE BODY P");
    }

    // --- SHOW ERRORS ---------------------------------------------------------

    #[test]
    fn show_errors_sozinho_e_do_ultimo_compilado() {
        for sql in ["SHOW ERRORS", "show errors;", "SHO ERR", "sho err  -- depois do create"] {
            assert_eq!(pedido_de_erros(sql), Some(PedidoDeErros::DoUltimo), "{sql}");
        }
    }

    #[test]
    fn show_errors_com_tipo_e_nome() {
        assert_eq!(
            pedido_de_erros("SHOW ERRORS PROCEDURE EXEMPLO.GRAV_LOG"),
            Some(PedidoDeErros::Do(objeto("PROCEDURE", Some("EXEMPLO"), "GRAV_LOG")))
        );
        assert_eq!(
            pedido_de_erros("show errors package body pkg_fatura;"),
            Some(PedidoDeErros::Do(objeto("PACKAGE BODY", None, "PKG_FATURA")))
        );
        assert_eq!(
            pedido_de_erros("-- conferência\nSHOW ERR TRIGGER \"Dono\".trg"),
            Some(PedidoDeErros::Do(objeto("TRIGGER", Some("Dono"), "TRG")))
        );
    }

    #[test]
    fn show_errors_que_nao_se_entende_e_invalido() {
        assert_eq!(pedido_de_erros("SHOW ERRORS GRAV_LOG"), Some(PedidoDeErros::Invalido));
        assert_eq!(
            pedido_de_erros("SHOW ERRORS PROCEDURE"),
            Some(PedidoDeErros::Invalido)
        );
        assert_eq!(
            pedido_de_erros("SHOW ERRORS TABLE T"),
            Some(PedidoDeErros::Invalido)
        );
    }

    #[test]
    fn outros_show_e_outras_palavras_nao_sao_show_errors() {
        for sql in [
            "SHOW USER",
            "SH ERRORS",
            "SHOWS ERRORS",
            "SHOW ER",
            "SHOW ERRORSX",
            "select show, errors from t",
            "",
        ] {
            assert_eq!(pedido_de_erros(sql), None, "{sql}");
        }
    }
}
