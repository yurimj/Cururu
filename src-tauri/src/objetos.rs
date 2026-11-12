//! Árvore de objetos do banco (requisitos 2 e 4).
//!
//! O escopo da árvore é sempre o **schema do usuário conectado**: todas as
//! consultas usam as visões `USER_*`, nunca `ALL_*`. Foi decisão de projeto,
//! registrada em `requisitos.md` (requisito 2).
//!
//! A leitura de colunas é a exceção, e por um motivo: ela serve também ao
//! autocomplete, cujo alcance é o da consulta escrita no editor — que pode
//! citar sinônimo e tabela de outro dono. Ela não lê dicionário nenhum; usa o
//! describe do cursor, que enxerga exatamente o que a consulta enxergaria.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::filtro::Filtro;
use crate::registro;

/// As categorias da árvore.
///
/// As seis primeiras são as do Oracle. As duas últimas existem só no SQL
/// Server, que separa a função que devolve tabela da que devolve um valor —
/// como o SSMS separa, e por uma razão prática: uma entra no `FROM` e a outra
/// não. O Oracle não faz essa distinção e continua com a categoria `Funcoes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Categoria {
    Tabelas,
    Views,
    Pacotes,
    Procedures,
    Funcoes,
    Programador,
    /// Só no SQL Server: `IF` (em linha) e `TF` (de várias instruções).
    #[serde(rename = "funcoesTabela")]
    FuncoesTabela,
    /// Só no SQL Server: `FN` (escalar) e `AF` (agregação do CLR).
    #[serde(rename = "funcoesEscalares")]
    FuncoesEscalares,
}

impl Categoria {
    /// Tipo como o Oracle nomeia em `USER_OBJECTS` / `USER_SOURCE`.
    ///
    /// As duas categorias do SQL Server nunca chegam aqui — a árvore do Oracle
    /// não as monta —, mas responder `FUNCTION` é mais honesto do que um
    /// `unreachable!`: é o que elas são.
    pub fn tipo_oracle(self) -> &'static str {
        match self {
            Categoria::Tabelas => "TABLE",
            Categoria::Views => "VIEW",
            Categoria::Pacotes => "PACKAGE",
            Categoria::Procedures => "PROCEDURE",
            Categoria::Funcoes | Categoria::FuncoesTabela | Categoria::FuncoesEscalares => {
                "FUNCTION"
            }
            Categoria::Programador => "JOB",
        }
    }

    /// Tipos de `sys.objects` desta categoria, prontos para o `IN`.
    ///
    /// `None` nas categorias que o SQL Server não tem: pacote é construção do
    /// PL/SQL, o Programador é o Agent (que tem árvore própria) e `Funcoes`
    /// sozinha não diz se é de tabela ou escalar.
    pub fn tipos_sqlserver(self) -> Option<&'static str> {
        match self {
            Categoria::Tabelas => Some("'U'"),
            Categoria::Views => Some("'V'"),
            Categoria::Procedures => Some("'P'"),
            Categoria::FuncoesTabela => Some("'IF', 'TF'"),
            Categoria::FuncoesEscalares => Some("'FN', 'AF'"),
            _ => None,
        }
    }

    /// A opção "incluir sinônimos" só faz sentido para Tabelas e Views: são as
    /// únicas categorias que um sinônimo pode apontar de forma útil na árvore.
    pub fn aceita_sinonimos(self) -> bool {
        matches!(self, Categoria::Tabelas | Categoria::Views)
    }

    /// `USER_SCHEDULER_JOBS` não tem a coluna `GENERATED`.
    pub fn aceita_excluir_gerados(self) -> bool {
        self != Categoria::Programador
    }
}

/// Um objeto listado na árvore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Objeto {
    pub nome: String,
    /// `VALID` / `INVALID` para código; estado do job no Programador.
    pub situacao: Option<String>,
    /// O comentário da view, **por inteiro** (requisito 37).
    ///
    /// Vem inteiro, e não já encurtado, porque serve a dois usos: a árvore
    /// mostra os primeiros 50 caracteres ao lado do nome, e a tela de edição
    /// precisa do texto completo para não devolvê-lo truncado ao banco.
    ///
    /// Só as views o trazem. É nelas que o nome sozinho não diz o que o objeto
    /// é — uma tabela se explica pelas colunas, uma view não —, e é para elas
    /// que o comentário foi pedido. Nas demais categorias fica vazio, e nenhuma
    /// consulta a mais é feita por causa dele.
    pub comentario: Option<String>,
}

/// Uma coluna de tabela ou view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coluna {
    pub nome: String,
    /// Tipo já montado para exibição: `VARCHAR2(30)`, `NUMBER(10,2)`…
    pub tipo: String,
    pub aceita_nulo: bool,
}

/// Um subprograma de dentro de um pacote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subprograma {
    pub nome: String,
    /// `PROCEDURE` ou `FUNCTION`.
    pub especie: String,
}

/// Parte do fonte a abrir. Pacote tem as duas; o resto só tem corpo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParteDoFonte {
    Especificacao,
    Corpo,
}

/// Destino de um Ctrl+clique no editor SQL.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinoDaReferencia {
    pub categoria: Categoria,
    /// Inclui o dono quando o objeto não pertence ao usuário conectado.
    pub nome: String,
    /// Pacotes abrem o body quando ele existe; nos demais objetos é ignorado.
    pub parte: ParteDoFonte,
    /// Em que banco o objeto está — só no SQL Server, onde um nome de três
    /// partes leva a outro banco do servidor. No Oracle é sempre vazio.
    pub banco: Option<String>,
}

/// Condição do filtro sobre as colunas indicadas, se houver alguma.
fn condicao_de(
    filtro: Option<&Filtro>,
    coluna_nome: &str,
    coluna_situacao: &str,
) -> Option<crate::filtro::Condicao> {
    filtro
        .map(|f| f.condicao(coluna_nome, coluna_situacao))
        .filter(|c| !c.sql.is_empty())
}

/// Um pedaço da lista de objetos, do tamanho pedido.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagina {
    pub objetos: Vec<Objeto>,
    /// Há mais objetos além destes?
    pub tem_mais: bool,
}

/// Lista uma página de objetos de uma categoria, aplicando o filtro montado na
/// tela (requisito 4).
///
/// A listagem é paginada porque há schemas com dezenas de milhares de tabelas:
/// trazer tudo de uma vez custa tempo no banco e memória na tela, e a árvore
/// não tem como mostrar isso de uma vez de qualquer forma.
pub fn listar(
    conexao: &oracle::Connection,
    categoria: Categoria,
    filtro: Option<&Filtro>,
    deslocamento: u32,
    limite: u32,
) -> Resultado<Pagina> {
    let filtro = filtro.filter(|f| f.ativo());

    // `USER_OBJECTS` lê o dicionário direto. As visões específicas
    // (`USER_TABLES`, `USER_VIEWS`) juntam dezenas de tabelas internas para
    // trazer detalhes que a árvore não usa, e ficam lentas em schema grande.
    let (fonte, coluna_nome, coluna_situacao) = match categoria {
        Categoria::Programador => (
            "user_scheduler_jobs".to_string(),
            "job_name",
            "state",
        ),
        outra => (
            format!("user_objects WHERE object_type = '{}'", outra.tipo_oracle()),
            "object_name",
            "status",
        ),
    };

    let excluir_gerados =
        filtro.is_some_and(|f| f.excluir_gerados) && categoria.aceita_excluir_gerados();
    // Em base de ERP a tabela costuma ser de outro owner e alcançada por
    // sinônimo; incluí-los faz a árvore mostrar o que a aplicação enxerga.
    let com_sinonimos =
        filtro.is_some_and(|f| f.incluir_sinonimos) && categoria.aceita_sinonimos();

    let mut sql = if com_sinonimos {
        // Com a união, os critérios precisam ficar por fora: as duas origens
        // não têm as mesmas colunas.
        let condicao = condicao_de(filtro, "nome", "situacao");
        let mut interna = format!(
            "SELECT {coluna_nome} AS nome, {coluna_situacao} AS situacao FROM {fonte}"
        );
        if excluir_gerados {
            interna.push_str(" AND generated = 'N'");
        }
        interna
            .push_str(" UNION ALL SELECT synonym_name, CAST(NULL AS VARCHAR2(9)) FROM user_synonyms");

        let mut externa = format!("SELECT nome, situacao FROM ({interna})");
        if let Some(condicao) = &condicao {
            externa.push_str(&format!(" WHERE {}", condicao.sql));
        }
        externa.push_str(" ORDER BY nome");
        externa
    } else {
        // Caminho normal, e o que interessa para o desempenho: **um só nível**.
        // Embrulhar `USER_OBJECTS` numa consulta externa impedia o Oracle de
        // juntar os predicados, e uma filtragem passava de 2 s para 25 s.
        let condicao = condicao_de(filtro, coluna_nome, coluna_situacao);
        let mut unica = format!("SELECT {coluna_nome}, {coluna_situacao} FROM {fonte}");
        // O Programador não tem um predicado fixo antes; os demais têm o tipo.
        if categoria == Categoria::Programador {
            unica.push_str(" WHERE 1 = 1");
        }
        if excluir_gerados {
            unica.push_str(" AND generated = 'N'");
        }
        if let Some(condicao) = &condicao {
            unica.push_str(&format!(" AND {}", condicao.sql));
        }
        unica.push_str(&format!(" ORDER BY {coluna_nome}"));
        unica
    };

    // OFFSET/FETCH existe desde o 12.1, que é a versão mínima do projeto.
    sql.push_str(" OFFSET :deslocamento ROWS FETCH NEXT :limite ROWS ONLY");

    let condicao = if com_sinonimos {
        condicao_de(filtro, "nome", "situacao")
    } else {
        condicao_de(filtro, coluna_nome, coluna_situacao)
    };

    // Pede um a mais que o pedido: se ele vier, é porque há próxima página.
    let deslocamento = i64::from(deslocamento);
    let limite_consultado = i64::from(limite) + 1;

    let mut parametros: Vec<(&str, &dyn oracle::sql_type::ToSql)> = Vec::new();
    if let Some(condicao) = &condicao {
        for (nome, valor) in &condicao.valores {
            parametros.push((nome.as_str(), valor));
        }
    }
    parametros.push(("deslocamento", &deslocamento));
    parametros.push(("limite", &limite_consultado));

    let linhas = conexao.query_as_named::<(String, Option<String>)>(&sql, &parametros)?;
    let mut objetos: Vec<Objeto> = linhas
        .map(|linha| {
            let (nome, situacao) = linha?;
            Ok(Objeto {
                nome,
                situacao,
                comentario: None,
            })
        })
        .collect::<Resultado<Vec<_>>>()?;

    let tem_mais = objetos.len() > limite as usize;
    objetos.truncate(limite as usize);
    // Depois do `truncate`: o objeto a mais foi pedido só para saber se há
    // próxima página, e ninguém vai ler o comentário de um objeto que não
    // aparece.
    if categoria == Categoria::Views {
        anexar_comentarios(conexao, &mut objetos)?;
    }
    Ok(Pagina { objetos, tem_mais })
}

/// Preenche o comentário das views da página, numa consulta só (requisito 37).
///
/// É uma **segunda** consulta, e não uma junção com a listagem, pelo motivo
/// anotado em [`listar`]: aquela consulta é de um nível só, e embrulhá-la já
/// fez uma filtragem passar de 2 s para 25 s. Aqui os nomes já vieram, então
/// isto é um `IN` de no máximo uma página — e `USER_TAB_COMMENTS` é uma das
/// visões leves do dicionário, ao contrário de `USER_TABLES`.
///
/// Nome de view não vai por concatenação: cada um é um bind. São nomes vindos
/// do próprio dicionário, mas o `IN` montado à mão seria o único lugar do
/// módulo onde texto do banco voltaria como SQL.
fn anexar_comentarios(conexao: &oracle::Connection, objetos: &mut [Objeto]) -> Resultado<()> {
    if objetos.is_empty() {
        return Ok(());
    }

    let chaves: Vec<String> = (0..objetos.len()).map(|i| format!("n{i}")).collect();
    let marcadores: Vec<String> = chaves.iter().map(|chave| format!(":{chave}")).collect();
    let sql = format!(
        "SELECT table_name, comments FROM user_tab_comments \n          WHERE comments IS NOT NULL AND table_name IN ({})",
        marcadores.join(", ")
    );

    let parametros: Vec<(&str, &dyn oracle::sql_type::ToSql)> = chaves
        .iter()
        .zip(objetos.iter())
        .map(|(chave, objeto)| {
            (chave.as_str(), &objeto.nome as &dyn oracle::sql_type::ToSql)
        })
        .collect();

    let linhas = conexao.query_as_named::<(String, Option<String>)>(&sql, &parametros)?;
    let comentarios: std::collections::HashMap<String, String> = linhas
        .filter_map(|linha| match linha {
            Ok((nome, Some(comentario))) => Some(Ok((nome, comentario))),
            Ok(_) => None,
            Err(erro) => Some(Err(erro)),
        })
        .collect::<Result<_, _>>()?;

    for objeto in objetos.iter_mut() {
        objeto.comentario = comentarios.get(&objeto.nome).cloned();
    }
    Ok(())
}

/// Grava o comentário de uma view, ou o apaga quando o texto vem vazio
/// (requisito 37).
///
/// Duas coisas que valem ser ditas por escrito:
///
/// * **O texto não pode ir em bind.** `COMMENT ON` não aceita variável no lugar
///   do literal, então ele entra no comando — e a aspa simples que o usuário
///   digitar precisa ser dobrada, que é o que impede o comentário de fechar a
///   string e virar SQL.
/// * **`COMMENT ON` é DDL, e DDL no Oracle confirma sozinho.** A conexão é a
///   mesma que atende as abas de consulta, então gravar um comentário confirma
///   a transação aberta nelas. Não há como evitar pelo lado do cliente; é o
///   mesmo efeito de digitar o `COMMENT ON` na janela de consulta, que é como
///   isto era feito antes de existir a tela.
pub fn gravar_comentario(
    conexao: &oracle::Connection,
    nome: &str,
    comentario: &str,
) -> Resultado<()> {
    let nome = nome_de_objeto(nome)?;
    let texto = comentario.trim().replace('\'', "''");
    // Comentário vazio é como o Oracle apaga o que havia: não existe um
    // `DROP COMMENT`, e `IS ''` deixa a linha do dicionário com nulo.
    conexao.execute(&format!("COMMENT ON TABLE {nome} IS '{texto}'"), &[])?;
    Ok(())
}

/// Colunas de uma tabela ou view, na ordem em que existem no objeto.
///
/// Vem do **describe do cursor**, e não do dicionário: abre-se um `SELECT *`
/// que não devolve linha nenhuma e leem-se os nomes e tipos que o próprio
/// Oracle diz que aquela consulta teria. Custa um parse.
///
/// A leitura anterior era `USER_TAB_COLUMNS`, que junta meia dúzia de tabelas
/// internas do SYS. Em base de ERP isso custava **de 1,7 a 4 segundos para uma
/// tabela só** — tempo medido no log, e tempo demais: a sugestão chegava depois
/// de quem digitou já ter escrito a coluna à mão.
///
/// De quebra, o describe alcança o que as visões `USER_*` não alcançam:
/// sinônimo e tabela de outro dono. É o alcance certo aqui, porque quem
/// pergunta é o autocomplete, e a consulta no editor pode citar as duas coisas.
pub fn colunas(conexao: &oracle::Connection, objeto: &str) -> Resultado<Vec<Coluna>> {
    // Nome de objeto não vai em bind: entra no texto da consulta. A validação
    // aqui não é zelo, é o que impede o nome de virar SQL.
    let nome = nome_de_objeto(objeto)?;

    // `WHERE 1 = 0` é o que garante que nada seja lido: o cursor é aberto só
    // para ser descrito.
    let resultado = conexao.query(&format!("SELECT * FROM {nome} WHERE 1 = 0"), &[])?;

    Ok(resultado
        .column_info()
        .iter()
        .map(|coluna| Coluna {
            nome: coluna.name().to_string(),
            // O driver já monta `VARCHAR2(30)`, `NUMBER(10,2)`, `DATE` — o mesmo
            // texto que se montava à mão a partir do dicionário, e melhor para
            // os tipos que ficavam sem detalhe, como TIMESTAMP e INTERVAL.
            tipo: coluna.oracle_type().to_string(),
            aceita_nulo: coluna.nullable(),
        })
        .collect())
}

/// Aceita `TABELA` e `DONO.TABELA`, e nada além disso.
///
/// Sem aspas, como o resto do módulo: um objeto criado com nome entre aspas e
/// caixa mista continua fora do alcance, como já estava.
fn nome_de_objeto(objeto: &str) -> Resultado<String> {
    let nome = objeto.trim();
    let partes: Vec<&str> = nome.split('.').collect();

    let valido = (1..=2).contains(&partes.len()) && partes.iter().all(|parte| {
        let mut caracteres = parte.chars();
        caracteres
            .next()
            .is_some_and(|inicial| inicial.is_ascii_alphabetic() || inicial == '_')
            && caracteres.all(|c| c.is_ascii_alphanumeric() || "_$#".contains(c))
    });

    if !valido {
        return Err(Erro::novo(format!("nome de objeto inválido: {objeto}")));
    }
    // Em maiúsculas o texto da consulta se repete entre as tabelas pedidas de
    // formas diferentes, e o Oracle reaproveita o cursor do shared pool.
    Ok(nome.to_uppercase())
}

/// Subprogramas declarados dentro de um pacote.
pub fn subprogramas(conexao: &oracle::Connection, pacote: &str) -> Resultado<Vec<Subprograma>> {
    // Três correções de desempenho passaram por aqui, e todas foram a mesma
    // descoberta: **as visões `USER_*` de rotina custam caro quando filtradas
    // por nome, e são baratas quando filtradas por `object_id`**. Elas juntam
    // obj$, procedureinfo$ e argument$ com lógica de edição e de tipo no meio,
    // e o nome não chega até o índice; o número do objeto, sim.
    //
    // O caminho medido, sempre com DEM_INT (17 subprogramas) e com o log
    // separando cada etapa:
    //
    // - EXISTS correlacionado em `USER_ARGUMENTS`, um por rotina: 26,7 s.
    // - Uma leitura só de `USER_ARGUMENTS`, por `package_name`: 20,8 s.
    // - `USER_ARGUMENTS` por `object_id`: 28 ms — mas `USER_PROCEDURES` por
    //   `object_name` continuava levando 15,6 s, e era ela que ficou de fora
    //   das duas primeiras contas.
    //
    // Por isso agora o `object_id` vem antes de tudo, de `USER_OBJECTS`, que é
    // leitura de índice por nome; as outras duas leituras passam a ser por ele.
    // O ganho maior nem é no pacote: é no nome digitado que **não** é pacote —
    // e o editor pede subprogramas de todo nome com ponto atrás.
    let pacote = pacote.to_uppercase();
    let inicio = Instant::now();
    // O **formato** da consulta pesa mais que o filtro, e o log deste banco já
    // mostrou três formas na mesma família de visões, sempre com DEM_INT:
    //
    // - `user_procedures WHERE object_name = :nome` — 15,6 s.
    // - `user_objects WHERE object_name = :nome` — 14,8 s.
    // - `user_objects WHERE object_name LIKE :nome ESCAPE '\'` sem curinga,
    //   com `FETCH FIRST 1` — 14,4 s. Trocar a igualdade pelo `LIKE` sozinho
    //   não muda nada, e essa tentativa está registrada aqui para ninguém
    //   repeti-la.
    // - `user_objects WHERE object_name LIKE 'DEM%'`, que é a consulta do
    //   autocomplete — **1,2 s**, trazendo 173 nomes.
    //
    // O que a rápida tem de diferente é o curinga no fim: com um prefixo
    // seguido de `%` o otimizador faz varredura de faixa pelo índice de nome.
    // Sem curinga nenhum, o predicado com bind não vira faixa e o plano
    // degringola. Por isso aqui se pede o **prefixo** — o nome inteiro seguido
    // de `%` — e a escolha do nome exato acontece em memória, sobre um punhado
    // de linhas: `DEM\_INT%` traz `DEM_INT` e pouco mais.
    //
    // `PACKAGE BODY` fica de fora: as rotinas são atribuídas ao id da
    // especificação. `TYPE` entra porque os métodos dele aparecem em
    // `USER_PROCEDURES` do mesmo jeito, e era esse o alcance de antes.
    let padrao = format!("{}%", padrao_exato(&pacote));
    let candidatos: Vec<(String, i64)> = conexao
        .query_as_named::<(String, i64)>(
            "SELECT object_name, object_id \
               FROM user_objects \
              WHERE object_name LIKE :nome ESCAPE '\\' \
                AND object_type IN ('PACKAGE', 'TYPE') \
              FETCH FIRST 200 ROWS ONLY",
            &[("nome", &padrao)],
        )?
        .collect::<Result<_, _>>()?;
    anotar_demora("id", &pacote, inicio);

    let ids: Vec<i64> = candidatos
        .into_iter()
        .filter(|(nome, _)| *nome == pacote)
        .map(|(_, id)| id)
        .collect();

    // Nome que não é pacote nem tipo para aqui, e para barato: é o caso de toda
    // palavra seguida de ponto que o editor manda conferir.
    let Some(&object_id) = ids.first() else {
        return Ok(Vec::new());
    };

    // `procedure_name` é nulo na linha que representa o próprio pacote, daí o
    // filtro. O DISTINCT agrupa sobrecargas sob um nome só, como faz o SQL
    // Developer.
    let inicio = Instant::now();
    let mut subprogramas = nomes_do_pacote(conexao, object_id)?;
    anotar_demora("nomes", &pacote, inicio);

    // Se o id não trouxe nada, o nome ainda pode trazer: é a rede de segurança
    // da suposição de que as rotinas ficam sob o id da especificação. Lento,
    // mas a lista aparece — e o log diz que foi por aqui.
    if subprogramas.is_empty() {
        registro::aviso(
            "listar_subprogramas",
            format!("{pacote}: nada sob object_id {object_id}; relendo por nome"),
        );
        let inicio = Instant::now();
        subprogramas = nomes_do_pacote_por_nome(conexao, &pacote)?;
        anotar_demora("nomes por nome", &pacote, inicio);
    }
    if subprogramas.is_empty() {
        return Ok(Vec::new());
    }

    // Quem tem argumento na posição 0 tem valor de retorno, e portanto é
    // função. A junção com os nomes é pequena e acontece em memória.
    //
    // A falha aqui **não** derruba a lista: os nomes já estão lidos, e sem esta
    // leitura o que se perde é a distinção entre procedure e função — uma
    // etiqueta na lista. Deixar a lista sumir por causa dela seria trocar o
    // defeito grande pelo pequeno; o que não pode faltar é o log dizendo que
    // faltou.
    let inicio = Instant::now();
    match funcoes_do_pacote(conexao, object_id) {
        Ok(funcoes) => {
            anotar_demora("espécies", &pacote, inicio);
            for subprograma in &mut subprogramas {
                if funcoes.contains(&subprograma.nome) {
                    subprograma.especie = "FUNCTION".to_string();
                }
            }
        }
        Err(erro) => registro::aviso(
            "listar_subprogramas",
            format!("{pacote}: espécies não lidas, tudo como procedure: {erro}"),
        ),
    }
    Ok(subprogramas)
}

/// O nome escrito como padrão de `LIKE` que só casa com ele mesmo.
///
/// `DEM_INT` vira `DEM\_INT`: sem escapar, o sublinhado é o curinga de uma
/// posição do `LIKE` e o padrão casaria também com `DEMXINT`.
fn padrao_exato(nome: &str) -> String {
    nome.replace('\\', "\\\\").replace('_', "\\_")
}

/// Os subprogramas do pacote de `object_id`, todos como procedure por ora.
fn nomes_do_pacote(conexao: &oracle::Connection, object_id: i64) -> Resultado<Vec<Subprograma>> {
    let nomes = conexao.query_as_named::<String>(
        "SELECT DISTINCT procedure_name \
           FROM user_procedures \
          WHERE object_id = :id AND procedure_name IS NOT NULL \
          ORDER BY procedure_name",
        &[("id", &object_id)],
    )?;
    nomes
        .map(|linha| {
            Ok(Subprograma {
                nome: linha?,
                especie: "PROCEDURE".to_string(),
            })
        })
        .collect()
}

/// O mesmo, filtrando pelo nome — o caminho lento, só como rede de segurança.
fn nomes_do_pacote_por_nome(
    conexao: &oracle::Connection,
    pacote: &str,
) -> Resultado<Vec<Subprograma>> {
    let nomes = conexao.query_as_named::<String>(
        "SELECT DISTINCT procedure_name \
           FROM user_procedures \
          WHERE object_name = :pacote AND procedure_name IS NOT NULL \
          ORDER BY procedure_name",
        &[("pacote", &pacote)],
    )?;
    nomes
        .map(|linha| {
            Ok(Subprograma {
                nome: linha?,
                especie: "PROCEDURE".to_string(),
            })
        })
        .collect()
}

/// Os nomes que, dentro do pacote, são função — os que têm valor de retorno.
fn funcoes_do_pacote(
    conexao: &oracle::Connection,
    object_id: i64,
) -> Resultado<std::collections::HashSet<String>> {
    let nomes = conexao.query_as_named::<String>(
        "SELECT DISTINCT object_name \
           FROM user_arguments \
          WHERE object_id = :id AND position = 0",
        &[("id", &object_id)],
    )?;
    Ok(nomes.collect::<Result<_, _>>()?)
}

/// Acima disso a leitura de subprogramas vai para o log, dizendo qual das
/// consultas demorou.
const DEMORA_DE_SUBPROGRAMA_MS: u128 = 500;

/// Registra a consulta que passou do tempo esperado, nomeando qual foi.
///
/// São duas leituras de dicionário com custos muito diferentes. Sem separá-las
/// o log dizia apenas que a chamada inteira demorou — e foi por isso que a
/// primeira otimização mexeu na consulta errada.
fn anotar_demora(etapa: &str, pacote: &str, inicio: Instant) {
    let demora = inicio.elapsed().as_millis();
    if demora > DEMORA_DE_SUBPROGRAMA_MS {
        registro::informacao(
            "listar_subprogramas",
            format!("{pacote}: {etapa} em {demora} ms"),
        );
    }
}

/// Fonte do objeto, pronto para abrir no editor.
///
/// Para código PL/SQL vem de `USER_SOURCE`, que é exatamente o texto que o
/// banco guarda — é o que precisa ser editado e recompilado (requisito 13).
/// Para tabelas e views, monta-se o DDL.
pub fn fonte(
    conexao: &oracle::Connection,
    categoria: Categoria,
    nome: &str,
    parte: ParteDoFonte,
) -> Resultado<String> {
    let nome = nome.to_uppercase();

    match categoria {
        Categoria::Tabelas | Categoria::Views => ddl(conexao, categoria.tipo_oracle(), &nome),
        Categoria::Programador => ddl(conexao, "PROCOBJ", &nome),
        Categoria::Pacotes => {
            let tipo = match parte {
                ParteDoFonte::Especificacao => "PACKAGE",
                ParteDoFonte::Corpo => "PACKAGE BODY",
            };
            fonte_plsql(conexao, tipo, &nome)
        }
        Categoria::Procedures => fonte_plsql(conexao, "PROCEDURE", &nome),
        // As duas categorias do SQL Server não chegam aqui pela árvore; se
        // chegarem por um caminho novo, uma função é uma função.
        Categoria::Funcoes | Categoria::FuncoesTabela | Categoria::FuncoesEscalares => {
            fonte_plsql(conexao, "FUNCTION", &nome)
        }
    }
}

fn fonte_plsql(conexao: &oracle::Connection, tipo: &str, nome: &str) -> Resultado<String> {
    let partes = partes_do_identificador(nome, 2)?;
    let nome = partes.last().expect("identificador validado");
    let linhas = if partes.len() == 2 {
        let dono = &partes[0];
        conexao.query_as_named::<String>(
            "SELECT text FROM all_source \
             WHERE owner = :dono AND name = :nome AND type = :tipo \
             ORDER BY line",
            &[("dono", dono), ("nome", nome), ("tipo", &tipo.to_string())],
        )?
    } else {
        conexao.query_as_named::<String>(
            "SELECT text FROM user_source \
             WHERE name = :nome AND type = :tipo \
             ORDER BY line",
            &[("nome", nome), ("tipo", &tipo.to_string())],
        )?
    };

    let mut texto = String::new();
    for linha in linhas {
        texto.push_str(&linha?);
    }
    if texto.trim().is_empty() {
        return Err(Erro::novo(format!(
            "não há fonte de {tipo} para {nome} neste schema"
        )));
    }
    // O banco guarda "PACKAGE X AS…"; o que se compila é "CREATE OR REPLACE …".
    Ok(format!("CREATE OR REPLACE {}", texto.trim_start()))
}

fn ddl(conexao: &oracle::Connection, tipo: &str, nome: &str) -> Resultado<String> {
    let partes = partes_do_identificador(nome, 2)?;
    let nome = partes.last().expect("identificador validado");
    let texto = if partes.len() == 2 {
        conexao.query_row_as_named::<String>(
            "SELECT DBMS_METADATA.GET_DDL(:tipo, :nome, :dono) FROM dual",
            &[("tipo", &tipo.to_string()), ("nome", nome), ("dono", &partes[0])],
        )?
    } else {
        conexao.query_row_as_named::<String>(
            "SELECT DBMS_METADATA.GET_DDL(:tipo, :nome) FROM dual",
            &[("tipo", &tipo.to_string()), ("nome", nome)],
        )?
    };
    Ok(texto.trim().to_string())
}

/// Resolve um identificador clicado sem confundir alias de coluna com objeto.
///
/// Quem responde é o próprio Oracle (ver [`DESTINO_PELO_ORACLE`]): pelo
/// dicionário, o clique em `TABELA_EXEMPLO` levou 21 s no PRD, e três cliques
/// seguidos, sem nada na tela, viraram 21, 34 e 38 s. O dicionário fica de
/// reserva para o banco que recusar o bloco.
pub fn resolver_referencia(
    conexao: &oracle::Connection,
    referencia: &str,
) -> Resultado<Option<DestinoDaReferencia>> {
    let partes = partes_do_identificador(referencia, 3)?;
    match destino_pelo_oracle(conexao, &partes.join(".")) {
        Ok(destino) => Ok(destino),
        // Conexão caída não é recusa do bloco: quem cuida dela é a reconexão.
        Err(erro) if crate::sessao::e_falha_de_conexao(&erro, conexao) => Err(erro),
        Err(erro) => {
            registro::aviso(
                "resolver_referencia",
                format!("o Oracle recusou o bloco ({erro}); indo pelo dicionário"),
            );
            resolver_pelo_dicionario(conexao, &partes)
        }
    }
}

/// O destino do nome segundo o [`DESTINO_PELO_ORACLE`].
fn destino_pelo_oracle(
    conexao: &oracle::Connection,
    nome: &str,
) -> Resultado<Option<DestinoDaReferencia>> {
    let mut comando = conexao.statement(DESTINO_PELO_ORACLE).build()?;
    comando.execute_named(&[
        ("nome", &nome.to_string()),
        ("categoria", &oracle::sql_type::OracleType::Varchar2(20)),
        ("destino", &oracle::sql_type::OracleType::Varchar2(400)),
        ("corpo", &oracle::sql_type::OracleType::Int64),
    ])?;
    let Some(categoria) = comando.bind_value::<&str, Option<String>>("categoria")? else {
        return Ok(None);
    };
    let categoria = match categoria.as_str() {
        "tabelas" => Categoria::Tabelas,
        "views" => Categoria::Views,
        "procedures" => Categoria::Procedures,
        "funcoes" => Categoria::Funcoes,
        "pacotes" => Categoria::Pacotes,
        outra => {
            return Err(Erro::novo(format!(
                "categoria inesperada no destino do clique: {outra}"
            )))
        }
    };
    let nome = comando
        .bind_value::<&str, Option<String>>("destino")?
        .unwrap_or_default();
    let corpo = comando.bind_value::<&str, i64>("corpo")?;
    Ok(Some(DestinoDaReferencia {
        categoria,
        nome,
        // Pacote sem corpo abre a especificação; nos demais a parte é ignorada.
        parte: if categoria == Categoria::Pacotes && corpo == 0 {
            ParteDoFonte::Especificacao
        } else {
            ParteDoFonte::Corpo
        },
        // O Oracle não tem banco: o alcance é o schema do usuário conectado.
        banco: None,
    }))
}

/// A resolução pelo dicionário: a reserva de [`destino_pelo_oracle`].
///
/// Um nome simples procura no schema conectado e depois nos sinônimos privados
/// e públicos. Dois nomes podem ser `PACOTE.MEMBRO` ou `DONO.OBJETO`; três são
/// `DONO.PACOTE.MEMBRO`. Só os tipos que têm destino definido na interface são
/// devolvidos.
fn resolver_pelo_dicionario(
    conexao: &oracle::Connection,
    partes: &[String],
) -> Resultado<Option<DestinoDaReferencia>> {
    let usuario = conexao.query_row_as::<String>("SELECT USER FROM dual", &[])?;

    match partes {
        [nome] => {
            if let Some(destino) = resolver_objeto(conexao, &usuario, nome, false)? {
                return Ok(Some(destino));
            }
            resolver_sinonimo(conexao, &usuario, nome)
        }
        [primeiro, segundo] => {
            // `USUARIO.OBJETO` é inequivocamente dono + objeto. Nos demais
            // casos, pacote + membro é a leitura natural de `PKG.PROC()`.
            if primeiro == &usuario {
                return resolver_objeto(conexao, primeiro, segundo, false);
            }
            if let Some(destino) = resolver_membro_de_pacote(
                conexao,
                &usuario,
                primeiro,
                segundo,
                false,
            )? {
                return Ok(Some(destino));
            }
            if let Some((dono, pacote)) = alvo_do_sinonimo(conexao, &usuario, primeiro)? {
                if let Some(destino) =
                    resolver_membro_de_pacote(conexao, &dono, &pacote, segundo, true)?
                {
                    return Ok(Some(destino));
                }
            }
            resolver_objeto(conexao, primeiro, segundo, true)
        }
        [dono, pacote, membro] => {
            resolver_membro_de_pacote(conexao, dono, pacote, membro, dono != &usuario)
        }
        _ => unreachable!("quantidade de partes validada"),
    }
}

fn resolver_objeto(
    conexao: &oracle::Connection,
    dono: &str,
    nome: &str,
    qualificar: bool,
) -> Resultado<Option<DestinoDaReferencia>> {
    // O caminho comum fica em USER_OBJECTS. Em bases grandes, ALL_OBJECTS pode
    // levar vários segundos mesmo com owner + nome exatos (15,9 s medidos no
    // log); não há motivo para pagá-lo por um objeto do schema conectado.
    let mut linhas = if qualificar {
        conexao.query_as_named::<String>(
            "SELECT object_type FROM all_objects \
             WHERE owner = :dono AND object_name = :nome \
               AND object_type IN ('TABLE', 'VIEW', 'PROCEDURE', 'FUNCTION') \
             ORDER BY CASE object_type \
               WHEN 'TABLE' THEN 1 WHEN 'VIEW' THEN 2 \
               WHEN 'PROCEDURE' THEN 3 ELSE 4 END",
            &[("dono", &dono.to_string()), ("nome", &nome.to_string())],
        )?
    } else {
        conexao.query_as_named::<String>(
            "SELECT object_type FROM user_objects \
             WHERE object_name = :nome \
               AND object_type IN ('TABLE', 'VIEW', 'PROCEDURE', 'FUNCTION') \
             ORDER BY CASE object_type \
               WHEN 'TABLE' THEN 1 WHEN 'VIEW' THEN 2 \
               WHEN 'PROCEDURE' THEN 3 ELSE 4 END",
            &[("nome", &nome.to_string())],
        )?
    };
    let Some(tipo) = linhas.next() else {
        return Ok(None);
    };
    let categoria = match tipo?.as_str() {
        "TABLE" => Categoria::Tabelas,
        "VIEW" => Categoria::Views,
        "PROCEDURE" => Categoria::Procedures,
        "FUNCTION" => Categoria::Funcoes,
        _ => unreachable!("tipo filtrado na consulta"),
    };
    Ok(Some(DestinoDaReferencia {
        categoria,
        nome: if qualificar {
            format!("{dono}.{nome}")
        } else {
            nome.to_string()
        },
        banco: None,
        parte: ParteDoFonte::Corpo,
    }))
}

fn resolver_membro_de_pacote(
    conexao: &oracle::Connection,
    dono: &str,
    pacote: &str,
    membro: &str,
    qualificar: bool,
) -> Resultado<Option<DestinoDaReferencia>> {
    let mut membros = if qualificar {
        conexao.query_as_named::<String>(
            "SELECT 'S' FROM all_procedures \
             WHERE owner = :dono AND object_name = :pacote AND procedure_name = :membro \
             FETCH FIRST 1 ROW ONLY",
            &[
                ("dono", &dono.to_string()),
                ("pacote", &pacote.to_string()),
                ("membro", &membro.to_string()),
            ],
        )?
    } else {
        conexao.query_as_named::<String>(
            "SELECT 'S' FROM user_procedures \
             WHERE object_name = :pacote AND procedure_name = :membro \
             FETCH FIRST 1 ROW ONLY",
            &[("pacote", &pacote.to_string()), ("membro", &membro.to_string())],
        )?
    };
    let Some(existe) = membros.next() else {
        return Ok(None);
    };
    existe?;

    let mut corpos = if qualificar {
        conexao.query_as_named::<String>(
            "SELECT 'S' FROM all_objects \
             WHERE owner = :dono AND object_name = :pacote AND object_type = 'PACKAGE BODY' \
             FETCH FIRST 1 ROW ONLY",
            &[("dono", &dono.to_string()), ("pacote", &pacote.to_string())],
        )?
    } else {
        conexao.query_as_named::<String>(
            "SELECT 'S' FROM user_objects \
             WHERE object_name = :pacote AND object_type = 'PACKAGE BODY' \
             FETCH FIRST 1 ROW ONLY",
            &[("pacote", &pacote.to_string())],
        )?
    };
    let parte = if let Some(corpo) = corpos.next() {
        corpo?;
        ParteDoFonte::Corpo
    } else {
        ParteDoFonte::Especificacao
    };

    Ok(Some(DestinoDaReferencia {
        categoria: Categoria::Pacotes,
        nome: if qualificar {
            format!("{dono}.{pacote}")
        } else {
            pacote.to_string()
        },
        parte,
        banco: None,
    }))
}

fn resolver_sinonimo(
    conexao: &oracle::Connection,
    usuario: &str,
    sinonimo: &str,
) -> Resultado<Option<DestinoDaReferencia>> {
    let Some((dono, nome)) = alvo_do_sinonimo(conexao, usuario, sinonimo)? else {
        return Ok(None);
    };
    resolver_objeto(conexao, &dono, &nome, dono != usuario)
}

fn alvo_do_sinonimo(
    conexao: &oracle::Connection,
    usuario: &str,
    sinonimo: &str,
) -> Resultado<Option<(String, String)>> {
    let mut linhas = conexao.query_as_named::<(String, String, Option<String>)>(
        "SELECT table_owner, table_name, db_link FROM all_synonyms \
         WHERE (owner = :usuario OR owner = 'PUBLIC') AND synonym_name = :sinonimo \
         ORDER BY CASE WHEN owner = 'PUBLIC' THEN 1 ELSE 0 END",
        &[("usuario", &usuario.to_string()), ("sinonimo", &sinonimo.to_string())],
    )?;
    let Some(linha) = linhas.next() else {
        return Ok(None);
    };
    let (dono, nome, db_link) = linha?;
    // Metadados de um objeto remoto exigiriam abrir a conexão do database
    // link; Ctrl+clique não deve fazê-lo implicitamente.
    Ok(db_link.is_none().then_some((dono, nome)))
}

// --- Nomes que o Ctrl+clique abre, em lote (requisito 22) -------------------

/// A função PL/SQL que confere se o membro existe no pacote, comum aos dois
/// blocos de resolução.
///
/// É macro, e não constante, porque `concat!` só junta literais. O bloco que a
/// usa precisa ter declarado `usuario` antes dela.
macro_rules! membro_existe_em_plsql {
    () => {
        r#"
  FUNCTION membro_existe(p_esquema VARCHAR2, p_pacote VARCHAR2, p_membro VARCHAR2)
    RETURN BOOLEAN
  IS
    a DBMS_DESCRIBE.NUMBER_TABLE; b DBMS_DESCRIBE.NUMBER_TABLE; c DBMS_DESCRIBE.NUMBER_TABLE;
    d DBMS_DESCRIBE.VARCHAR2_TABLE; e DBMS_DESCRIBE.NUMBER_TABLE; f DBMS_DESCRIBE.NUMBER_TABLE;
    g DBMS_DESCRIBE.NUMBER_TABLE; h DBMS_DESCRIBE.NUMBER_TABLE; i DBMS_DESCRIBE.NUMBER_TABLE;
    j DBMS_DESCRIBE.NUMBER_TABLE; k DBMS_DESCRIBE.NUMBER_TABLE; l DBMS_DESCRIBE.NUMBER_TABLE;
    existe PLS_INTEGER;
  BEGIN
    DBMS_DESCRIBE.DESCRIBE_PROCEDURE(
      p_esquema || '.' || p_pacote || '.' || p_membro, NULL, NULL,
      a, b, c, d, e, f, g, h, i, j, k, l);
    RETURN TRUE;
  EXCEPTION WHEN OTHERS THEN
    IF SQLCODE <> -20003 THEN
      RETURN FALSE;
    END IF;
    IF p_esquema = usuario THEN
      SELECT COUNT(*) INTO existe FROM user_procedures
       WHERE object_name = p_pacote AND procedure_name = p_membro AND ROWNUM = 1;
    ELSE
      SELECT COUNT(*) INTO existe FROM all_procedures
       WHERE owner = p_esquema AND object_name = p_pacote AND procedure_name = p_membro
         AND ROWNUM = 1;
    END IF;
    RETURN existe > 0;
  END;
"#
    };
}

/// Pergunta ao próprio Oracle quais nomes têm destino, sem passar pelo
/// dicionário.
///
/// `DBMS_UTILITY.NAME_RESOLVE` é a resolução de nomes do banco — a mesma de um
/// `SELECT` ou de uma chamada, com sinônimo privado e público — e responde da
/// memória do servidor. Medido no HML com 53 nomes: 132 ms, contra 23 s das
/// consultas a `ALL_SYNONYMS`, `ALL_OBJECTS` e `ALL_PROCEDURES`, e com a mesma
/// resposta nome a nome.
///
/// O que se confirmou no 19c, e que o bloco usa:
///
/// - **Contexto 1** (PL/SQL): procedure e função de nível de schema voltam sem a
///   primeira parte, com tipo 7 e 8 — têm destino. Pacote volta com tipo 9, e
///   sozinho não tem destino. Com membro, o `NAME_RESOLVE` **não** confere se o
///   membro existe — `PKG.NAO_EXISTE` volta igual a `PKG.PROC` —, e quem confere
///   é o `DBMS_DESCRIBE`, que responde `-20001` quando ele não existe. Pacote
///   inválido o `DBMS_DESCRIBE` não descreve (`-20003`); só aí vale a lista do
///   dicionário, como no clique.
/// - Sem PL/SQL com o nome, **contexto 0**: tipo 2 é tabela — e materialized
///   view —, tipo 4 é view, os dois com destino. Sequence, tipo e o que não
///   existe voltam erro.
/// - Sinônimo de outro banco volta com o link preenchido, e o Oracle não abre o
///   link para isso (é o que a documentação do `NAME_RESOLVE` diz; o HML não
///   tem sinônimo assim para conferir). Fica sem destino, como no clique.
///
/// Não há `SELECT ... WHERE 1 = 0` para confirmar tabela e view, de propósito:
/// o parse abriria o link de um sinônimo remoto e recompilaria a view inválida
/// só por olhá-la, e o contexto 0 já dá a mesma resposta.
const RESOLUCAO_PELO_ORACLE: &str = concat!(
    r#"
DECLARE
  nomes    CONSTANT VARCHAR2(32767) := :nomes;
  usuario  CONSTANT VARCHAR2(128) := SYS_CONTEXT('USERENV', 'SESSION_USER');
  achados  VARCHAR2(32767);
  inicio   PLS_INTEGER := 1;
  fim      PLS_INTEGER;
  nome     VARCHAR2(400);
  navega   BOOLEAN;
  esquema  VARCHAR2(128);
  parte1   VARCHAR2(128);
  parte2   VARCHAR2(128);
  link     VARCHAR2(128);
  tipo     NUMBER;
  numero   NUMBER;
"#,
    membro_existe_em_plsql!(),
    r#"
BEGIN
  WHILE inicio <= NVL(LENGTH(nomes), 0) LOOP
    fim := INSTR(nomes, ',', inicio);
    IF fim = 0 THEN fim := LENGTH(nomes) + 1; END IF;
    nome := SUBSTR(nomes, inicio, fim - inicio);
    inicio := fim + 1;
    navega := FALSE;

    IF REGEXP_LIKE(nome, '^[A-Z_][A-Z0-9_$#]*(\.[A-Z_][A-Z0-9_$#]*){0,2}$') THEN
      BEGIN
        DBMS_UTILITY.NAME_RESOLVE(nome, 1, esquema, parte1, parte2, link, tipo, numero);
        IF link IS NULL THEN
          IF parte1 IS NULL AND tipo IN (7, 8) THEN
            navega := TRUE;
          ELSIF parte1 IS NOT NULL AND parte2 IS NOT NULL AND tipo = 9 THEN
            navega := membro_existe(esquema, parte1, parte2);
          END IF;
        END IF;
      EXCEPTION WHEN OTHERS THEN
        BEGIN
          DBMS_UTILITY.NAME_RESOLVE(nome, 0, esquema, parte1, parte2, link, tipo, numero);
          navega := link IS NULL AND tipo IN (2, 4);
        EXCEPTION WHEN OTHERS THEN NULL;
        END;
      END;
    END IF;

    IF navega THEN
      achados := achados || nome || ',';
    END IF;
  END LOOP;
  :achados := RTRIM(achados, ',');
END;
"#
);

/// O destino de um nome só, pelo mesmo caminho de [`RESOLUCAO_PELO_ORACLE`]: é
/// o que o Ctrl+clique e o **Ir para definição** abrem.
///
/// Devolve a categoria da árvore, o nome como a árvore o abre — com o dono
/// quando não é o usuário conectado — e, no pacote, se ele tem corpo. Só essa
/// última pergunta vai ao dicionário, e só para membro de pacote: o
/// `NAME_RESOLVE` não distingue pacote com e sem corpo.
const DESTINO_PELO_ORACLE: &str = concat!(
    r#"
DECLARE
  nome      CONSTANT VARCHAR2(400) := :nome;
  usuario   CONSTANT VARCHAR2(128) := SYS_CONTEXT('USERENV', 'SESSION_USER');
  esquema   VARCHAR2(128);
  parte1    VARCHAR2(128);
  parte2    VARCHAR2(128);
  link      VARCHAR2(128);
  tipo      NUMBER;
  numero    NUMBER;
  categoria VARCHAR2(20);
  dono      VARCHAR2(128);
  objeto    VARCHAR2(128);
  corpo     PLS_INTEGER := 0;
"#,
    membro_existe_em_plsql!(),
    r#"
BEGIN
  BEGIN
    DBMS_UTILITY.NAME_RESOLVE(nome, 1, esquema, parte1, parte2, link, tipo, numero);
    IF link IS NULL THEN
      IF parte1 IS NULL AND tipo IN (7, 8) THEN
        categoria := CASE tipo WHEN 7 THEN 'procedures' ELSE 'funcoes' END;
        dono := esquema;
        objeto := parte2;
      ELSIF parte1 IS NOT NULL AND parte2 IS NOT NULL AND tipo = 9 THEN
        IF membro_existe(esquema, parte1, parte2) THEN
          categoria := 'pacotes';
          dono := esquema;
          objeto := parte1;
          IF esquema = usuario THEN
            SELECT COUNT(*) INTO corpo FROM user_objects
             WHERE object_name = parte1 AND object_type = 'PACKAGE BODY' AND ROWNUM = 1;
          ELSE
            SELECT COUNT(*) INTO corpo FROM all_objects
             WHERE owner = esquema AND object_name = parte1 AND object_type = 'PACKAGE BODY'
               AND ROWNUM = 1;
          END IF;
        END IF;
      END IF;
    END IF;
  EXCEPTION WHEN OTHERS THEN
    BEGIN
      DBMS_UTILITY.NAME_RESOLVE(nome, 0, esquema, parte1, parte2, link, tipo, numero);
      IF link IS NULL AND tipo IN (2, 4) THEN
        categoria := CASE tipo WHEN 2 THEN 'tabelas' ELSE 'views' END;
        dono := esquema;
        objeto := parte1;
      END IF;
    EXCEPTION WHEN OTHERS THEN NULL;
    END;
  END;
  :categoria := categoria;
  :destino := CASE WHEN dono = usuario THEN objeto ELSE dono || '.' || objeto END;
  :corpo := corpo;
END;
"#
);

/// Quantos caracteres vão numa pergunta ao Oracle.
///
/// O bloco recebe e devolve a lista num `VARCHAR2`, que no PL/SQL para em
/// 32767 bytes; os nomes são ASCII, então caractere e byte são a mesma conta.
const TAMANHO_DA_PERGUNTA: usize = 30_000;

/// O maior nome que o Oracle aceita: três partes de 128 bytes e dois pontos.
/// Maior que isso não é nome de nada, e estouraria a variável do bloco.
const TAMANHO_MAXIMO_DE_NOME: usize = 3 * 128 + 2;

/// Tipos que o Ctrl+clique abre por conta própria — os de [`resolver_objeto`].
const TIPOS_COM_DESTINO: &str = "'TABLE', 'VIEW', 'PROCEDURE', 'FUNCTION'";

/// Quantos nomes vão numa rodada de consultas ao dicionário.
///
/// Cada consulta leva a rodada inteira na lista do `IN`, e a de trios leva três
/// valores por nome: com 300, a maior fica em 900 binds.
const NOMES_POR_RODADA: usize = 300;

/// Uma referência que passou pela validação.
struct Referencia<'a> {
    /// Como veio da tela — é o que volta para ela.
    original: &'a String,
    /// Em maiúsculas, uma por parte.
    partes: Vec<String>,
    /// As partes unidas por ponto: é o nome que vai ao banco.
    nome: String,
}

/// Das referências recebidas, as que o Ctrl+clique sabe abrir.
///
/// É [`resolver_referencia`] para muitos nomes de uma vez, e existe para o
/// sublinhado que aparece com o Ctrl apertado (requisito 22). Quem responde é o
/// próprio Oracle (ver [`RESOLUCAO_PELO_ORACLE`]); o dicionário fica de reserva
/// para o banco que recusar o bloco — sem permissão no `DBMS_DESCRIBE`, por
/// exemplo —, e então a resposta é a mesma, só que em segundos.
///
/// Nome que nem chega a ser identificador válido não é erro: só não leva a
/// lugar nenhum, que é o que o clique também responderia.
///
/// Devolve as referências como vieram.
pub fn referencias_navegaveis(
    conexao: &oracle::Connection,
    referencias: &[String],
) -> Resultado<Vec<String>> {
    let validas: Vec<Referencia> = referencias
        .iter()
        .filter(|referencia| referencia.trim().len() <= TAMANHO_MAXIMO_DE_NOME)
        .filter_map(|referencia| {
            let partes = partes_do_identificador(referencia, 3).ok()?;
            let nome = partes.join(".");
            Some(Referencia {
                original: referencia,
                partes,
                nome,
            })
        })
        .collect();

    let mut navegaveis = Vec::new();
    for rodada in rodadas(&validas, |referencia| referencia.nome.len(), TAMANHO_DA_PERGUNTA) {
        let nomes: Vec<&str> = rodada.iter().map(|referencia| referencia.nome.as_str()).collect();
        let achados = match pelo_oracle(conexao, &nomes) {
            Ok(achados) => achados,
            // Conexão caída não é recusa do bloco: quem cuida dela é a
            // reconexão, e o dicionário cairia do mesmo jeito.
            Err(erro) if crate::sessao::e_falha_de_conexao(&erro, conexao) => return Err(erro),
            Err(erro) => {
                registro::aviso(
                    "referencias_navegaveis",
                    format!("o Oracle recusou o bloco ({erro}); indo pelo dicionário"),
                );
                pelo_dicionario(conexao, rodada)?
            }
        };
        navegaveis.extend(
            rodada
                .iter()
                .filter(|referencia| achados.contains(&referencia.nome))
                .map(|referencia| referencia.original.clone()),
        );
    }
    Ok(navegaveis)
}

/// Deixa o bloco de resolução pronto numa conexão recém-aberta.
///
/// A primeira execução do bloco é a mais cara — o Oracle compila o PL/SQL e
/// carrega os pacotes na sessão —, e é melhor pagá-la ao abrir a aba do que no
/// primeiro Ctrl. O banco que recusar o bloco fica sabido já aqui, no log.
pub fn preparar_resolucao(conexao: &oracle::Connection) {
    let inicio = Instant::now();
    match pelo_oracle(conexao, &["DUAL"]) {
        Ok(_) => registro::informacao(
            "navegacao",
            format!(
                "bloco de resolução preparado em {} ms",
                inicio.elapsed().as_millis()
            ),
        ),
        Err(erro) if crate::sessao::e_falha_de_conexao(&erro, conexao) => registro::aviso(
            "navegacao",
            format!("o bloco de resolução não foi preparado: a conexão caiu ({erro})"),
        ),
        Err(erro) => registro::aviso(
            "navegacao",
            format!(
                "o Oracle recusou o bloco de resolução ({erro}); o sublinhado do Ctrl vai pelo dicionário"
            ),
        ),
    }
}

/// Os nomes da lista que, segundo o Oracle, têm destino.
fn pelo_oracle(conexao: &oracle::Connection, nomes: &[&str]) -> Resultado<HashSet<String>> {
    let mut comando = conexao.statement(RESOLUCAO_PELO_ORACLE).build()?;
    comando.execute_named(&[
        ("nomes", &nomes.join(",")),
        ("achados", &oracle::sql_type::OracleType::Varchar2(32767)),
    ])?;
    let achados = comando.bind_value::<&str, Option<String>>("achados")?;
    Ok(achados
        .unwrap_or_default()
        .split(',')
        .filter(|nome| !nome.is_empty())
        .map(str::to_string)
        .collect())
}

/// A mesma pergunta pelo dicionário: a reserva de [`pelo_oracle`].
///
/// São as regras de [`resolver_referencia`] (ver [`navegavel`]), com no máximo
/// seis consultas por rodada: cada uma pergunta de uma vez por todos os nomes
/// que precisam dela.
fn pelo_dicionario(
    conexao: &oracle::Connection,
    referencias: &[Referencia],
) -> Resultado<HashSet<String>> {
    let usuario = conexao.query_row_as::<String>("SELECT USER FROM dual", &[])?;
    let mut achados = HashSet::new();
    for rodada in referencias.chunks(NOMES_POR_RODADA) {
        let partes: Vec<&[String]> = rodada.iter().map(|referencia| referencia.partes.as_slice()).collect();
        let fatos = levantar_fatos(conexao, &usuario, &partes)?;
        achados.extend(
            rodada
                .iter()
                .filter(|referencia| navegavel(&referencia.partes, &usuario, &fatos))
                .map(|referencia| referencia.nome.clone()),
        );
    }
    Ok(achados)
}

/// Parte a lista em rodadas cujo tamanho somado — com o separador de cada item
/// — não passa do limite. Item maior que o limite vai sozinho.
fn rodadas<T>(itens: &[T], tamanho: impl Fn(&T) -> usize, limite: usize) -> Vec<&[T]> {
    let mut rodadas = Vec::new();
    let mut inicio = 0;
    let mut soma = 0;
    for (indice, item) in itens.iter().enumerate() {
        let custo = tamanho(item) + 1;
        if indice > inicio && soma + custo > limite {
            rodadas.push(&itens[inicio..indice]);
            inicio = indice;
            soma = 0;
        }
        soma += custo;
    }
    if inicio < itens.len() {
        rodadas.push(&itens[inicio..]);
    }
    rodadas
}

/// O que o dicionário respondeu sobre os nomes de uma rodada.
///
/// Os nomes compostos ficam com as partes unidas por ponto — `PACOTE.MEMBRO`,
/// `DONO.OBJETO`. Não há como confundir: o ponto não cabe num nome sem aspas,
/// e nome com aspas não chega até aqui.
#[derive(Debug, Default)]
struct Fatos {
    /// Tabelas, views, procedures e funções do schema conectado.
    proprios: HashSet<String>,
    /// Membros de pacote do schema conectado, como `PACOTE.MEMBRO`.
    membros_proprios: HashSet<String>,
    /// Para onde aponta cada sinônimo, com o privado à frente do público.
    /// `None` é o sinônimo de outro banco, que o clique não segue.
    sinonimos: HashMap<String, Option<(String, String)>>,
    /// Os donos que existem, entre os citados nos nomes compostos.
    donos: HashSet<String>,
    /// Membros de pacote de qualquer dono visível, como `DONO.PACOTE.MEMBRO`.
    membros: HashSet<String>,
    /// Tabelas, views, procedures e funções de qualquer dono visível, como
    /// `DONO.OBJETO`.
    objetos: HashSet<String>,
}

impl Fatos {
    /// Dono e objeto para onde o sinônimo aponta, quando é deste banco.
    fn alvo(&self, sinonimo: &str) -> Option<&(String, String)> {
        self.sinonimos.get(sinonimo)?.as_ref()
    }
}

/// As regras de [`resolver_referencia`], aplicadas ao que a rodada levantou.
///
/// Lá a ordem dos passos decide **qual** destino é aberto, porque o primeiro
/// achado encerra a busca. Aqui só importa se algum deles vale.
fn navegavel(partes: &[String], usuario: &str, fatos: &Fatos) -> bool {
    match partes {
        [nome] => {
            fatos.proprios.contains(nome)
                || fatos
                    .alvo(nome)
                    .is_some_and(|(dono, objeto)| fatos.objetos.contains(&format!("{dono}.{objeto}")))
        }
        // `USUARIO.OBJETO` é dono + objeto, e lá não se tenta mais nada.
        [dono, nome] if dono == usuario => fatos.proprios.contains(nome),
        [primeiro, segundo] => {
            fatos.membros_proprios.contains(&format!("{primeiro}.{segundo}"))
                || fatos.alvo(primeiro).is_some_and(|(dono, pacote)| {
                    fatos.membros.contains(&format!("{dono}.{pacote}.{segundo}"))
                })
                || fatos.objetos.contains(&format!("{primeiro}.{segundo}"))
        }
        [dono, pacote, membro] if dono == usuario => {
            fatos.membros_proprios.contains(&format!("{pacote}.{membro}"))
        }
        [dono, pacote, membro] => fatos.membros.contains(&format!("{dono}.{pacote}.{membro}")),
        _ => false,
    }
}

/// Faz as consultas de uma rodada — só as que algum nome precisa.
fn levantar_fatos(
    conexao: &oracle::Connection,
    usuario: &str,
    rodada: &[&[String]],
) -> Resultado<Fatos> {
    let mut fatos = Fatos::default();
    let alheio = |dono: &String| dono != usuario;

    // O schema conectado vem primeiro: é o caminho comum e o mais barato.
    let nomes = unicos(rodada.iter().copied().filter_map(|partes| match partes {
        [nome] => Some(vec![nome.clone()]),
        [dono, nome] if !alheio(dono) => Some(vec![nome.clone()]),
        _ => None,
    }));
    fatos.proprios = existentes(
        conexao,
        &format!(
            "SELECT object_name FROM user_objects \
             WHERE object_type IN ({TIPOS_COM_DESTINO}) AND object_name IN"
        ),
        &nomes,
    )?;

    let pares_proprios = unicos(rodada.iter().copied().filter_map(|partes| match partes {
        [primeiro, segundo] if alheio(primeiro) => Some(vec![primeiro.clone(), segundo.clone()]),
        [dono, pacote, membro] if !alheio(dono) => Some(vec![pacote.clone(), membro.clone()]),
        _ => None,
    }));
    fatos.membros_proprios = existentes(
        conexao,
        "SELECT object_name, procedure_name FROM user_procedures \
         WHERE (object_name, procedure_name) IN",
        &pares_proprios,
    )?;

    // O nome solto que não é do schema pode ser sinônimo, e a primeira parte
    // de `X.Y` pode ser o sinônimo de um pacote.
    let sinonimos = unicos(rodada.iter().copied().filter_map(|partes| match partes {
        [nome] if !fatos.proprios.contains(nome) => Some(vec![nome.clone()]),
        [primeiro, _] if alheio(primeiro) => Some(vec![primeiro.clone()]),
        _ => None,
    }));
    if !sinonimos.is_empty() {
        let sql = format!(
            "SELECT owner, synonym_name, table_owner, table_name, db_link FROM all_synonyms \
             WHERE owner IN (USER, 'PUBLIC') AND synonym_name IN ({})",
            marcadores(sinonimos.len(), 1)
        );
        let linhas = conexao
            .query_as::<LinhaDeSinonimo>(&sql, &valores(&sinonimos))?
            .collect::<Result<Vec<_>, _>>()?;
        fatos.sinonimos = escolher_sinonimos(linhas, usuario);
    }

    // Consulta barata que poupa as caras: em `c.nome` o `c` é quase sempre um
    // apelido, e não há por que perguntar a `ALL_OBJECTS` por um dono C.
    let donos = unicos(rodada.iter().copied().filter_map(|partes| match partes {
        [dono, _] | [dono, _, _] if alheio(dono) => Some(vec![dono.clone()]),
        _ => None,
    }));
    fatos.donos = existentes(
        conexao,
        "SELECT username FROM all_users WHERE username IN",
        &donos,
    )?;

    // Membros de pacote de outro dono: pelo sinônimo do pacote, ou escritos
    // como `DONO.PACOTE.MEMBRO`.
    let trios = unicos(rodada.iter().copied().filter_map(|partes| match partes {
        [primeiro, membro] if alheio(primeiro) => fatos
            .alvo(primeiro)
            .map(|(dono, pacote)| vec![dono.clone(), pacote.clone(), membro.clone()]),
        [dono, pacote, membro] if fatos.donos.contains(dono) => {
            Some(vec![dono.clone(), pacote.clone(), membro.clone()])
        }
        _ => None,
    }));
    fatos.membros = existentes(
        conexao,
        "SELECT owner, object_name, procedure_name FROM all_procedures \
         WHERE (owner, object_name, procedure_name) IN",
        &trios,
    )?;

    // Objetos de outro dono: para onde os sinônimos apontam, e `DONO.OBJETO`.
    let pares_alheios = unicos(rodada.iter().copied().filter_map(|partes| match partes {
        [nome] if !fatos.proprios.contains(nome) => fatos
            .alvo(nome)
            .map(|(dono, objeto)| vec![dono.clone(), objeto.clone()]),
        [dono, objeto] if fatos.donos.contains(dono) => Some(vec![dono.clone(), objeto.clone()]),
        _ => None,
    }));
    fatos.objetos = existentes(
        conexao,
        &format!(
            "SELECT owner, object_name FROM all_objects \
             WHERE object_type IN ({TIPOS_COM_DESTINO}) AND (owner, object_name) IN"
        ),
        &pares_alheios,
    )?;

    Ok(fatos)
}

/// Dono, nome, dono do destino, nome do destino e database link.
type LinhaDeSinonimo = (String, String, Option<String>, Option<String>, Option<String>);

/// Um destino por sinônimo: o privado vence o público, como em
/// [`alvo_do_sinonimo`], e o que aponta para outro banco fica sem destino.
fn escolher_sinonimos(
    linhas: Vec<LinhaDeSinonimo>,
    usuario: &str,
) -> HashMap<String, Option<(String, String)>> {
    let mut escolhidos = HashMap::new();
    let mut publicos = Vec::new();
    for (dono, sinonimo, dono_do_alvo, alvo, link) in linhas {
        let destino = match (dono_do_alvo, alvo, link) {
            (Some(dono_do_alvo), Some(alvo), None) => Some((dono_do_alvo, alvo)),
            _ => None,
        };
        if dono == usuario {
            escolhidos.insert(sinonimo, destino);
        } else {
            publicos.push((sinonimo, destino));
        }
    }
    for (sinonimo, destino) in publicos {
        escolhidos.entry(sinonimo).or_insert(destino);
    }
    escolhidos
}

/// Os grupos que a consulta devolveu, com as partes unidas por ponto.
///
/// `consulta` termina no `IN`, e a lista de marcadores é posta aqui, do
/// tamanho dos grupos. Sem grupo nenhum não há o que perguntar, e nada vai ao
/// banco.
fn existentes(
    conexao: &oracle::Connection,
    consulta: &str,
    grupos: &[Vec<String>],
) -> Resultado<HashSet<String>> {
    let Some(largura) = grupos.first().map(Vec::len) else {
        return Ok(HashSet::new());
    };
    let sql = format!("{consulta} ({})", marcadores(grupos.len(), largura));
    let mut achados = HashSet::new();
    for linha in conexao.query(&sql, &valores(grupos))? {
        let linha = linha?;
        let partes = (0..largura)
            .map(|coluna| linha.get::<usize, String>(coluna))
            .collect::<Result<Vec<_>, _>>()?;
        achados.insert(partes.join("."));
    }
    Ok(achados)
}

/// Os valores dos grupos, na ordem dos marcadores.
fn valores(grupos: &[Vec<String>]) -> Vec<&dyn oracle::sql_type::ToSql> {
    grupos
        .iter()
        .flatten()
        .map(|valor| valor as &dyn oracle::sql_type::ToSql)
        .collect()
}

/// `:1, :2` para dois nomes soltos; `(:1, :2), (:3, :4)` para dois pares.
///
/// Cada marcador aparece uma vez só: o bind vai por posição, e um nome
/// repetido deixaria a contagem de posições por conta do driver.
fn marcadores(grupos: usize, largura: usize) -> String {
    (0..grupos)
        .map(|grupo| {
            let posicoes: Vec<String> = (1..=largura)
                .map(|coluna| format!(":{}", grupo * largura + coluna))
                .collect();
            if largura == 1 {
                posicoes.join(", ")
            } else {
                format!("({})", posicoes.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Os grupos sem repetição: o mesmo nome costuma aparecer várias vezes na
/// tela.
fn unicos(grupos: impl Iterator<Item = Vec<String>>) -> Vec<Vec<String>> {
    let mut lista: Vec<Vec<String>> = grupos.collect();
    lista.sort();
    lista.dedup();
    lista
}

fn partes_do_identificador(nome: &str, maximo: usize) -> Resultado<Vec<String>> {
    let partes: Vec<String> = nome
        .trim()
        .split('.')
        .map(|parte| parte.to_uppercase())
        .collect();
    let valido = (1..=maximo).contains(&partes.len()) && partes.iter().all(|parte| {
        let mut caracteres = parte.chars();
        caracteres
            .next()
            .is_some_and(|inicial| inicial.is_ascii_alphabetic() || inicial == '_')
            && caracteres.all(|c| c.is_ascii_alphanumeric() || "_$#".contains(c))
    });
    if !valido {
        return Err(Erro::novo(format!("nome de objeto inválido: {nome}")));
    }
    Ok(partes)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn padrao_exato_escapa_o_sublinhado() {
        // Sem escapar, `DEM_INT` casaria com `DEMXINT` — e o id lido seria de
        // outro objeto, com a lista de subprogramas de outro pacote junto.
        assert_eq!(padrao_exato("DEM_INT"), "DEM\\_INT");
        assert_eq!(padrao_exato("FINANCEIRO"), "FINANCEIRO");
        assert_eq!(padrao_exato("A_B_C"), "A\\_B\\_C");
    }

    #[test]
    fn padrao_exato_nao_tem_curinga() {
        // É igualdade escrita como `LIKE`: um `%` aqui traria o pacote errado.
        assert!(!padrao_exato("DEM_INT").contains('%'));
    }

    #[test]
    fn nome_de_objeto_aceita_tabela_e_dono_ponto_tabela() {
        assert_eq!(nome_de_objeto("financeiro").unwrap(), "FINANCEIRO");
        assert_eq!(nome_de_objeto("  exemplo.financeiro  ").unwrap(), "EXEMPLO.FINANCEIRO");
        assert_eq!(nome_de_objeto("TB_2$#").unwrap(), "TB_2$#");
    }

    #[test]
    fn nome_de_objeto_recusa_o_que_viraria_sql() {
        // O nome entra no texto da consulta; estes são os que não podem passar.
        assert!(nome_de_objeto("financeiro; DROP TABLE x").is_err());
        assert!(nome_de_objeto("financeiro WHERE 1=1 OR 1=1").is_err());
        assert!(nome_de_objeto("(SELECT 1 FROM dual)").is_err());
        assert!(nome_de_objeto("\"Tabela\"").is_err());
        assert!(nome_de_objeto("").is_err());
        assert!(nome_de_objeto("2tabela").is_err());
        assert!(nome_de_objeto("a.b.c").is_err());
        assert!(nome_de_objeto("a..b").is_err());
        assert!(nome_de_objeto("dono.").is_err());
    }

    /// O que um dicionário de mentira responderia a uma rodada, com o usuário
    /// `EU` conectado e os objetos do ERP no schema `EXEMPLO`.
    fn fatos_de_exemplo() -> Fatos {
        let conjunto = |itens: &[&str]| -> HashSet<String> {
            itens.iter().map(|item| item.to_string()).collect()
        };
        let alvo = |dono: &str, objeto: &str| Some((dono.to_string(), objeto.to_string()));
        Fatos {
            proprios: conjunto(&["LOG_ENTRY", "PRC_LOCAL"]),
            membros_proprios: conjunto(&["PKG_LOCAL.GERAR"]),
            sinonimos: HashMap::from([
                ("FINANCEIRO".to_string(), alvo("EXEMPLO", "FINANCEIRO")),
                ("SEQ_PEDIDO".to_string(), alvo("EXEMPLO", "SEQ_PEDIDO")),
                ("PKG_EXEMPLO".to_string(), alvo("EXEMPLO", "PKG_EXEMPLO")),
                ("REMOTA".to_string(), None),
            ]),
            donos: conjunto(&["EXEMPLO"]),
            membros: conjunto(&["EXEMPLO.PKG_EXEMPLO.CALCULAR", "EXEMPLO.PKG_OUTRO.ENVIAR"]),
            // Pacote não entra: a consulta só traz os tipos que o clique abre.
            objetos: conjunto(&["EXEMPLO.FINANCEIRO", "EXEMPLO.VW_DEM_MOVIMENTOS"]),
        }
    }

    fn navega(referencia: &str) -> bool {
        let partes = partes_do_identificador(referencia, 3).unwrap();
        navegavel(&partes, "EU", &fatos_de_exemplo())
    }

    #[test]
    fn nome_solto_navega_pelo_schema_ou_pelo_sinonimo() {
        assert!(navega("log_entry"));
        assert!(navega("FINANCEIRO"));
        // O sinônimo existe, mas aponta para o que o clique não abre sozinho.
        assert!(!navega("SEQ_PEDIDO"));
        // Sinônimo de pacote: sozinho, o clique não sabe para onde ir.
        assert!(!navega("PKG_EXEMPLO"));
        assert!(!navega("REMOTA"));
        // Coluna, apelido ou palavra qualquer.
        assert!(!navega("IDFIN"));
    }

    #[test]
    fn nome_com_ponto_vale_pelas_tres_leituras_do_clique() {
        assert!(navega("PKG_LOCAL.GERAR"));
        assert!(navega("pkg_exemplo.calcular"));
        assert!(navega("EXEMPLO.VW_DEM_MOVIMENTOS"));
        assert!(!navega("PKG_EXEMPLO.INEXISTENTE"));
        // `f.idfin`: o `F` é apelido — nem dono, nem pacote, nem sinônimo.
        assert!(!navega("F.IDFIN"));
    }

    #[test]
    fn o_proprio_usuario_como_dono_so_olha_o_proprio_schema() {
        assert!(navega("EU.LOG_ENTRY"));
        assert!(!navega("EU.FINANCEIRO"));
        assert!(navega("EU.PKG_LOCAL.GERAR"));
        assert!(!navega("EU.PKG_EXEMPLO.CALCULAR"));
    }

    #[test]
    fn tres_partes_sao_membro_de_pacote_de_outro_dono() {
        assert!(navega("EXEMPLO.PKG_OUTRO.ENVIAR"));
        assert!(!navega("EXEMPLO.FINANCEIRO.IDFIN"));
    }

    #[test]
    fn o_sinonimo_privado_vence_o_publico() {
        let linha = |dono: &str, nome: &str, alvo: (&str, &str), link: Option<&str>| {
            (
                dono.to_string(),
                nome.to_string(),
                Some(alvo.0.to_string()),
                Some(alvo.1.to_string()),
                link.map(str::to_string),
            )
        };
        let escolhidos = escolher_sinonimos(
            vec![
                linha("PUBLIC", "DUAL", ("SYS", "DUAL"), None),
                linha("PUBLIC", "CLIENTE", ("EXEMPLO", "CLIENTE"), None),
                linha("EU", "CLIENTE", ("EU", "CLIENTE_V2"), None),
                linha("PUBLIC", "REMOTO", ("EXEMPLO", "REMOTO"), None),
                linha("EU", "REMOTO", ("X", "Y"), Some("OUTRO_BANCO")),
            ],
            "EU",
        );
        let alvo = |dono: &str, objeto: &str| Some((dono.to_string(), objeto.to_string()));
        assert_eq!(escolhidos["DUAL"], alvo("SYS", "DUAL"));
        assert_eq!(escolhidos["CLIENTE"], alvo("EU", "CLIENTE_V2"));
        // O privado aponta para outro banco e ainda assim vence: é o que o
        // clique encontra primeiro, e ele não segue database link.
        assert_eq!(escolhidos["REMOTO"], None);
    }

    #[test]
    fn marcadores_numeram_na_ordem_dos_valores() {
        assert_eq!(marcadores(2, 1), ":1, :2");
        assert_eq!(marcadores(2, 2), "(:1, :2), (:3, :4)");
        assert_eq!(marcadores(1, 3), "(:1, :2, :3)");
    }

    #[test]
    fn o_bloco_de_destino_so_tem_os_seus_binds() {
        assert_eq!(
            binds_do_bloco(DESTINO_PELO_ORACLE),
            vec!["nome", "categoria", "destino", "corpo"]
        );
    }

    #[test]
    fn os_dois_blocos_conferem_o_membro_do_mesmo_jeito() {
        for bloco in [RESOLUCAO_PELO_ORACLE, DESTINO_PELO_ORACLE] {
            assert!(bloco.contains("FUNCTION membro_existe"));
            assert!(bloco.contains("DBMS_DESCRIBE.DESCRIBE_PROCEDURE"));
        }
    }

    /// Os `:nome` do bloco, na ordem em que aparecem, sem os `:=`.
    fn binds_do_bloco(bloco: &str) -> Vec<&str> {
        bloco
            .split(':')
            .skip(1)
            .filter(|resto| !resto.starts_with('='))
            .map(|resto| resto.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or(""))
            .collect()
    }

    #[test]
    fn rodadas_respeitam_o_tamanho_da_pergunta() {
        // Cada item custa o próprio tamanho mais o separador.
        let itens = [4, 4, 4, 10, 1];
        let partidas = rodadas(&itens, |tamanho| *tamanho, 10);
        assert_eq!(partidas, vec![&[4, 4][..], &[4][..], &[10][..], &[1][..]]);
    }

    #[test]
    fn rodadas_sem_itens_ou_que_cabem_de_uma_vez() {
        assert!(rodadas(&[] as &[usize], |tamanho| *tamanho, 10).is_empty());
        assert_eq!(rodadas(&[1, 2, 3], |tamanho| *tamanho, 100), vec![&[1, 2, 3][..]]);
    }

    #[test]
    fn o_bloco_so_tem_os_dois_binds() {
        // O driver trata todo `:nome` como bind; um a mais faria o Oracle
        // recusar o bloco e mandar tudo para o caminho lento.
        assert_eq!(binds_do_bloco(RESOLUCAO_PELO_ORACLE), vec!["nomes", "achados"]);
    }

    #[test]
    fn nomes_repetidos_vao_uma_vez_so() {
        let grupos = unicos(
            [vec!["B".to_string()], vec!["A".to_string()], vec!["B".to_string()]].into_iter(),
        );
        assert_eq!(grupos, vec![vec!["A".to_string()], vec!["B".to_string()]]);
    }

    #[test]
    fn sinonimos_so_para_tabelas_e_views() {
        assert!(Categoria::Tabelas.aceita_sinonimos());
        assert!(Categoria::Views.aceita_sinonimos());
        assert!(!Categoria::Pacotes.aceita_sinonimos());
        assert!(!Categoria::Programador.aceita_sinonimos());
    }

    #[test]
    fn programador_nao_tem_coluna_de_objetos_gerados() {
        assert!(Categoria::Tabelas.aceita_excluir_gerados());
        assert!(Categoria::Funcoes.aceita_excluir_gerados());
        assert!(!Categoria::Programador.aceita_excluir_gerados());
    }

    #[test]
    fn referencia_aceita_objeto_pacote_e_dono() {
        assert_eq!(partes_do_identificador("tabela", 3).unwrap(), ["TABELA"]);
        assert_eq!(
            partes_do_identificador("pkg.proc", 3).unwrap(),
            ["PKG", "PROC"]
        );
        assert_eq!(
            partes_do_identificador("exemplo.pkg.proc", 3).unwrap(),
            ["EXEMPLO", "PKG", "PROC"]
        );
    }

    #[test]
    fn referencia_recusa_texto_que_nao_e_identificador() {
        assert!(partes_do_identificador("a.b.c.d", 3).is_err());
        assert!(partes_do_identificador("pkg.proc()", 3).is_err());
        assert!(partes_do_identificador("a..b", 3).is_err());
        assert!(partes_do_identificador("1tabela", 3).is_err());
    }
}
