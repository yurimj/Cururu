//! Leitura do texto SQL: separação de comandos e descoberta de binds.
//!
//! Serve ao modo script (requisito 5) e às bind variables (requisito 15).
//! Tudo aqui é análise de texto — não fala com banco nenhum, e por isso é
//! inteiramente coberto por testes.

use std::borrow::Cow;

use crate::plsql;

/// Substituição textual do cliente Oracle, antes da separação dos comandos.
/// O resolvedor permite usar a mesma leitura para pedir valores e executar.
/// Definições valem nesta execução; nunca são gravadas no perfil da conexão.
pub fn substituir_variaveis_oracle(
    texto: &str,
    mut resolver: impl FnMut(&str) -> String,
) -> String {
    let mut definidas = std::collections::HashMap::<String, String>::new();
    let mut ligada = true;
    let mut onde = Onde::Codigo;
    let mut saida = String::with_capacity(texto.len());
    for trecho in texto.split_inclusive('\n') {
        let linha = trecho.trim_end_matches(['\r', '\n']);
        let diretiva = linha.trim().trim_end_matches(';').trim();
        let palavras: Vec<&str> = diretiva.split_whitespace().collect();
        let mut retirada = false;
        if onde == Onde::Codigo {
            if palavras.len() == 3 && palavras[0].eq_ignore_ascii_case("SET")
                && palavras[1].eq_ignore_ascii_case("DEFINE")
                && (palavras[2].eq_ignore_ascii_case("ON") || palavras[2].eq_ignore_ascii_case("OFF")) {
                ligada = palavras[2].eq_ignore_ascii_case("ON");
                retirada = true;
            } else if palavras.first().is_some_and(|p| p.eq_ignore_ascii_case("DEFINE")) {
                if let Some((nome, valor)) = diretiva[6..].split_once('=') {
                    let valor = valor.trim();
                    let valor = if valor.len() >= 2 && ((valor.starts_with('\'') && valor.ends_with('\''))
                        || (valor.starts_with('"') && valor.ends_with('"'))) {
                        &valor[1..valor.len()-1]
                    } else { valor };
                    definidas.insert(nome.trim().to_uppercase(), valor.to_string());
                    retirada = true;
                }
            } else if palavras.first().is_some_and(|p| p.eq_ignore_ascii_case("UNDEFINE")) && palavras.len() > 1 {
                for nome in &palavras[1..] { definidas.remove(&nome.to_uppercase()); }
                retirada = true;
            }
        }
        if retirada {
            if trecho.ends_with('\n') { saida.push('\n'); }
            continue;
        }
        let caracteres: Vec<char> = trecho.chars().collect();
        let mut i = 0;
        while i < caracteres.len() {
            let c = caracteres[i];
            let proximo = caracteres.get(i + 1).copied();
            // Substitui também dentro de literais, como o SQL Developer.
            if ligada && c == '&' && !matches!(onde, Onde::ComentarioDeLinha | Onde::ComentarioDeBloco) {
                let inicio = i + if proximo == Some('&') { 2 } else { 1 };
                let mut fim = inicio;
                while fim < caracteres.len() && (caracteres[fim].is_ascii_alphanumeric() || matches!(caracteres[fim], '_' | '$' | '#')) { fim += 1; }
                if fim > inicio {
                    let nome: String = caracteres[inicio..fim].iter().collect();
                    let chave = nome.to_uppercase();
                    let valor = definidas.get(&chave).cloned().unwrap_or_else(|| resolver(&nome));
                    if proximo == Some('&') { definidas.insert(chave, valor.clone()); }
                    saida.push_str(&valor);
                    i = fim + usize::from(caracteres.get(fim) == Some(&'.'));
                    continue;
                }
            }
            match onde {
                Onde::Codigo => match (c, proximo) {
                    ('-', Some('-')) => onde = Onde::ComentarioDeLinha,
                    ('/', Some('*')) => { onde = Onde::ComentarioDeBloco; saida.push_str("/*"); i += 2; continue; }
                    ('\'', _) => onde = Onde::Texto,
                    ('"', _) => onde = Onde::Aspas,
                    _ => {}
                },
                Onde::Texto | Onde::Aspas if c == if onde == Onde::Texto { '\'' } else { '"' } => {
                    if proximo == Some(c) { saida.push(c); saida.push(c); i += 2; continue; }
                    onde = Onde::Codigo;
                }
                Onde::ComentarioDeLinha if c == '\n' => onde = Onde::Codigo,
                Onde::ComentarioDeBloco if c == '*' && proximo == Some('/') => {
                    onde = Onde::Codigo; saida.push_str("*/"); i += 2; continue;
                }
                _ => {}
            }
            saida.push(c);
            i += 1;
        }
    }
    saida
}

/// Um comando isolado dentro do texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comando {
    pub sql: String,
    /// Linha onde o comando começa, contando de 1. Serve para o painel de
    /// mensagens apontar o lugar certo (requisito 18).
    pub linha: usize,
}

/// Opções de cliente encontradas antes de um script Oracle.
///
/// Elas não são SQL e, portanto, não podem chegar à OCI. O texto devolvido por
/// [`preparar_script_oracle`] conserva uma quebra de linha no lugar de cada
/// diretiva para que a linha de um erro continue sendo a linha do editor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpcoesDoScriptOracle {
    pub rollback_ao_erro: bool,
    pub saida_dbms: Option<bool>,
}

/// Retira do texto as diretivas de SQL*Plus usadas também pelo SQL Developer e
/// devolve o comportamento que o executor deve aplicar na sessão.
///
/// Por enquanto são reconhecidas as formas que têm equivalente direto no
/// Cururu: `WHENEVER SQLERROR EXIT ... ROLLBACK` e `SET SERVEROUTPUT ON|OFF`.
/// As demais continuam seguindo ao Oracle, tornando uma incompatibilidade
/// visível em vez de fingir que uma opção foi atendida.
pub fn preparar_script_oracle(texto: &str) -> (String, OpcoesDoScriptOracle) {
    let mut opcoes = OpcoesDoScriptOracle::default();
    let mut preparado = String::with_capacity(texto.len());

    for trecho in texto.split_inclusive('\n') {
        let tem_quebra = trecho.ends_with('\n');
        let linha = trecho.trim_end_matches(['\r', '\n']);
        let sem_ponto_e_virgula = linha.trim().trim_end_matches(';').trim();
        let palavras: Vec<String> = sem_ponto_e_virgula
            .split_whitespace()
            .map(str::to_uppercase)
            .collect();

        let whenever_com_rollback = palavras.len() >= 4
            && palavras[0] == "WHENEVER"
            && palavras[1] == "SQLERROR"
            && palavras[2] == "EXIT"
            && palavras.last().is_some_and(|palavra| palavra == "ROLLBACK");
        let serveroutput =
            (palavras.len() >= 3 && palavras[0] == "SET" && palavras[1] == "SERVEROUTPUT")
                .then(|| palavras[2].as_str());

        if whenever_com_rollback {
            opcoes.rollback_ao_erro = true;
        } else if matches!(serveroutput, Some("ON")) {
            opcoes.saida_dbms = Some(true);
        } else if matches!(serveroutput, Some("OFF")) {
            opcoes.saida_dbms = Some(false);
        } else {
            preparado.push_str(linha);
        }

        if tem_quebra {
            preparado.push('\n');
        }
    }

    (preparado, opcoes)
}

/// Estado do varredor, para não confundir conteúdo de texto com código.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Onde {
    Codigo,
    Texto,
    Aspas,
    ComentarioDeLinha,
    ComentarioDeBloco,
}

/// Separa o texto em comandos.
///
/// Regras, iguais às do SQL*Plus e do SQL Developer:
///
/// - `;` encerra um comando comum;
/// - uma linha contendo apenas `/` encerra um bloco PL/SQL;
/// - dentro de bloco PL/SQL o `;` **não** encerra — só a barra encerra;
/// - `SHOW ERRORS`, que é do SQL*Plus, termina na quebra de linha, com ou sem
///   `;`.
pub fn separar_comandos(texto: &str) -> Vec<Comando> {
    let mut comandos = Vec::new();
    let mut atual = String::new();
    let mut linha_inicial = 1usize;
    let mut linha_atual = 1usize;
    let mut onde = Onde::Codigo;

    let caracteres: Vec<char> = texto.chars().collect();
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];
        let proximo = caracteres.get(i + 1).copied();

        // Uma barra sozinha na linha encerra o comando, seja ele bloco ou não.
        if onde == Onde::Codigo && c == '/' && proximo != Some('*') && barra_sozinha(&caracteres, i)
        {
            guardar(&mut comandos, &mut atual, &mut linha_inicial, linha_atual);
            i = fim_da_linha(&caracteres, i);
            linha_atual += 1;
            linha_inicial = linha_atual;
            continue;
        }

        match onde {
            Onde::Codigo => match (c, proximo) {
                ('\'', _) => onde = Onde::Texto,
                ('"', _) => onde = Onde::Aspas,
                ('-', Some('-')) => onde = Onde::ComentarioDeLinha,
                ('/', Some('*')) => onde = Onde::ComentarioDeBloco,
                (';', _) if !ehBlocoPlsql(&atual) => {
                    guardar(&mut comandos, &mut atual, &mut linha_inicial, linha_atual);
                    i += 1;
                    if atual.is_empty() {
                        linha_inicial = linha_atual;
                    }
                    continue;
                }
                _ => {}
            },
            Onde::Texto if c == '\'' => onde = Onde::Codigo,
            Onde::Aspas if c == '"' => onde = Onde::Codigo,
            Onde::ComentarioDeLinha if c == '\n' => onde = Onde::Codigo,
            Onde::ComentarioDeBloco if c == '*' && proximo == Some('/') => {
                atual.push(c);
                atual.push('/');
                i += 2;
                onde = Onde::Codigo;
                continue;
            }
            _ => {}
        }

        // O comando do SQL*Plus é de uma linha só. Sem este corte, o `SHOW
        // ERRORS` escrito sem `;` levaria junto o `SELECT` da linha de baixo.
        if c == '\n' && onde == Onde::Codigo && plsql::e_mostrar_erros(&atual) {
            guardar(&mut comandos, &mut atual, &mut linha_inicial, linha_atual);
            linha_atual += 1;
            linha_inicial = linha_atual;
            i += 1;
            continue;
        }

        if c == '\n' {
            linha_atual += 1;
            if atual.trim().is_empty() {
                linha_inicial = linha_atual;
            }
        }
        atual.push(c);
        i += 1;
    }

    guardar(&mut comandos, &mut atual, &mut linha_inicial, linha_atual);
    comandos
}

/// Nomes dos binds (`:nome`) presentes no comando, na ordem de aparição e sem
/// repetir. Ignora o que está dentro de texto e de comentário.
pub fn binds(sql: &str) -> Vec<String> {
    let mut nomes: Vec<String> = Vec::new();
    let palavras = palavras_iniciais(sql, 16);
    let e_trigger = indice_do_tipo_no_create(&palavras).is_some_and(|indice| {
        palavras[indice].1.eq_ignore_ascii_case("TRIGGER")
    });
    let mut onde = Onde::Codigo;
    let caracteres: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];
        let proximo = caracteres.get(i + 1).copied();

        match onde {
            Onde::Codigo => match (c, proximo) {
                ('\'', _) => onde = Onde::Texto,
                ('"', _) => onde = Onde::Aspas,
                ('-', Some('-')) => onde = Onde::ComentarioDeLinha,
                ('/', Some('*')) => onde = Onde::ComentarioDeBloco,
                // `::` é conversão de tipo, não bind.
                (':', Some(':')) => {
                    i += 2;
                    continue;
                }
                (':', Some(p)) if p.is_alphabetic() || p == '_' => {
                    let mut nome = String::new();
                    let mut j = i + 1;
                    while let Some(&letra) = caracteres.get(j) {
                        if letra.is_alphanumeric() || letra == '_' || letra == '$' || letra == '#' {
                            nome.push(letra);
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    // :NEW.coluna e :OLD.coluna são pseudorregistros do
                    // trigger, resolvidos pelo Oracle ao dispará-lo.
                    if e_trigger
                        && matches!(nome.to_ascii_uppercase().as_str(), "NEW" | "OLD")
                        && caracteres.get(j) == Some(&'.')
                    {
                        i = j;
                        continue;
                    }
                    if !nomes.iter().any(|existente| existente.eq_ignore_ascii_case(&nome)) {
                        nomes.push(nome);
                    }
                    i = j;
                    continue;
                }
                _ => {}
            },
            Onde::Texto if c == '\'' => onde = Onde::Codigo,
            Onde::Aspas if c == '"' => onde = Onde::Codigo,
            Onde::ComentarioDeLinha if c == '\n' => onde = Onde::Codigo,
            Onde::ComentarioDeBloco if c == '*' && proximo == Some('/') => {
                i += 2;
                onde = Onde::Codigo;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    nomes
}

/// O comando devolve linhas (é `SELECT` ou `WITH`)?
pub fn e_consulta(sql: &str) -> bool {
    let primeira = primeira_palavra(sql);
    primeira == "SELECT" || primeira == "WITH"
}

/// O que um comando faz com a transação no Oracle, pelas palavras que o abrem.
///
/// O Oracle confirma sozinho todo DDL — antes de rodá-lo e depois —, e esse
/// primeiro commit leva junto o que estava pendente na sessão. `ALTER SESSION`
/// e `ALTER SYSTEM` começam como DDL e não são: não mexem na transação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfeitoNoOracle {
    /// DDL: não sobra nada para confirmar, nem o que vinha de antes.
    ConfirmaSozinho,
    /// `COMMIT` digitado no editor.
    Commit,
    /// `ROLLBACK` digitado no editor, inteiro — sem `TO SAVEPOINT`.
    Rollback,
    /// Não abre nem fecha transação: `ALTER SESSION`, `SAVEPOINT`,
    /// `SET TRANSACTION`, `ROLLBACK TO SAVEPOINT`.
    NaoMexe,
    /// DML, bloco PL/SQL e o resto. Um bloco que só consultou também cai aqui:
    /// dizer que sobrou o que não sobrou custa um clique; o contrário custa o
    /// trabalho.
    PodeDeixarPendente,
}

pub fn efeito_no_oracle(sql: &str) -> EfeitoNoOracle {
    let palavras: Vec<String> = sem_comentarios_iniciais(sql)
        .split(|c: char| c.is_whitespace() || c == ';' || c == '(')
        .filter(|palavra| !palavra.is_empty())
        .take(3)
        .map(str::to_uppercase)
        .collect();
    let palavra = |indice: usize| palavras.get(indice).map_or("", String::as_str);
    match palavra(0) {
        "ALTER" if matches!(palavra(1), "SESSION" | "SYSTEM") => EfeitoNoOracle::NaoMexe,
        "CREATE" | "ALTER" | "DROP" | "TRUNCATE" | "RENAME" | "COMMENT" | "GRANT" | "REVOKE"
        | "ANALYZE" | "AUDIT" | "NOAUDIT" | "PURGE" | "FLASHBACK" | "ASSOCIATE"
        | "DISASSOCIATE" => EfeitoNoOracle::ConfirmaSozinho,
        "COMMIT" => EfeitoNoOracle::Commit,
        "ROLLBACK" if palavra(1) == "TO" || (palavra(1) == "WORK" && palavra(2) == "TO") => {
            EfeitoNoOracle::NaoMexe
        }
        "ROLLBACK" => EfeitoNoOracle::Rollback,
        "SAVEPOINT" | "SET" => EfeitoNoOracle::NaoMexe,
        _ => EfeitoNoOracle::PodeDeixarPendente,
    }
}

/// Traduz comandos próprios do SQL Developer/SQL*Plus para SQL ou PL/SQL que o
/// Oracle aceite diretamente pela OCI.
///
/// `EXEC` e `EXECUTE` são atalhos do cliente, não instruções reconhecidas pelo
/// banco. O texto original continua no histórico; só a cópia enviada ao Oracle
/// é envolvida num bloco anônimo.
pub fn preparar_para_execucao(sql: &str) -> Cow<'_, str> {
    if let Some(bloco) = como_bloco_anonimo(sql) {
        return Cow::Owned(bloco);
    }
    if let Some(qualificado) = qualificar_asterisco(sql) {
        return Cow::Owned(qualificado);
    }
    Cow::Borrowed(sql)
}

/// `EXEC minha_proc(1)` vira `BEGIN minha_proc(1); END;`.
fn como_bloco_anonimo(sql: &str) -> Option<String> {
    let limpo = sem_comentarios_iniciais(sql);
    let fim_da_palavra = limpo.find(char::is_whitespace).unwrap_or(limpo.len());
    let palavra = &limpo[..fim_da_palavra];
    if !palavra.eq_ignore_ascii_case("EXEC") && !palavra.eq_ignore_ascii_case("EXECUTE") {
        return None;
    }

    let chamada = limpo[fim_da_palavra..].trim();
    if chamada.is_empty() {
        return None;
    }
    Some(format!("BEGIN {chamada}; END;"))
}

// --- Asterisco solto na lista de seleção ------------------------------------

/// Palavras que encerram a lista de tabelas do `FROM`.
const FIM_DA_LISTA_DE_TABELAS: &[&str] = &[
    "WHERE", "GROUP", "ORDER", "HAVING", "CONNECT", "START", "UNION", "MINUS", "INTERSECT",
    "MODEL", "FETCH", "OFFSET", "FOR", "WITH", "PIVOT", "UNPIVOT", "SAMPLE", "PARTITION",
];

/// Palavras que começam uma nova tabela na lista, sem vírgula.
const LIGACOES: &[&str] = &[
    "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "NATURAL",
];

/// Qualifica o `*` solto de uma lista de seleção que tem outros itens.
///
/// `SELECT id_registro, * FROM financeiro` é aceito por MySQL, PostgreSQL e SQL
/// Server e recusado pelo Oracle com `ORA-00923`: ali o asterisco sozinho só
/// vale como **único** item da lista. O que o Oracle aceita é o asterisco
/// qualificado — `financeiro.*` —, e é isso que se manda no lugar.
///
/// **A troca só alcança texto que o Oracle já recusa hoje, e essa é a garantia
/// que a torna segura.** Um `*` cujo token anterior é `SELECT` ou `,` não tem
/// operando à esquerda; um cujo token seguinte é `,` ou `FROM` não tem operando
/// à direita. Nos dois casos ele não pode ser multiplicação — `preco * 2` e
/// `COUNT(*)` ficam de fora por construção. Tirando `SELECT * FROM`, que é
/// válido e fica intocado, o que sobra são exatamente as formas que hoje param
/// no `ORA-00923`. Nenhuma consulta que funciona muda de comportamento.
///
/// Na dúvida, não mexe: qualquer coisa que este código não entenda por inteiro
/// — subconsulta, view em linha, item estranho na lista de tabelas — devolve
/// `None`, o texto vai como está e o erro do Oracle aparece como antes.
fn qualificar_asterisco(sql: &str) -> Option<String> {
    if !primeira_palavra(sql).eq_ignore_ascii_case("SELECT") {
        return None;
    }

    let mascarado = mascarar(sql);
    let posicao = asterisco_da_lista(&mascarado)?;
    let qualificadores = qualificadores_do_from(&mascarado)?;

    let original: Vec<char> = sql.chars().collect();
    let mut novo: String = original[..posicao].iter().collect();
    novo.push_str(
        &qualificadores
            .iter()
            .map(|nome| format!("{nome}.*"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    novo.extend(&original[posicao + 1..]);
    Some(novo)
}

/// Copia o texto trocando por espaço tudo que não é código — literal, aspas
/// duplas e comentário —, mantendo o comprimento para as posições continuarem
/// valendo no original.
fn mascarar(sql: &str) -> Vec<char> {
    let caracteres: Vec<char> = sql.chars().collect();
    let mut saida = caracteres.clone();
    let mut onde = Onde::Codigo;
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];
        let proximo = caracteres.get(i + 1).copied();

        match onde {
            Onde::Codigo => match (c, proximo) {
                ('\'', _) => {
                    onde = Onde::Texto;
                    saida[i] = ' ';
                }
                ('"', _) => {
                    onde = Onde::Aspas;
                    saida[i] = ' ';
                }
                ('-', Some('-')) => {
                    onde = Onde::ComentarioDeLinha;
                    saida[i] = ' ';
                }
                ('/', Some('*')) => {
                    onde = Onde::ComentarioDeBloco;
                    saida[i] = ' ';
                }
                _ => {}
            },
            Onde::Texto => {
                saida[i] = ' ';
                if c == '\'' {
                    onde = Onde::Codigo;
                }
            }
            Onde::Aspas => {
                saida[i] = ' ';
                if c == '"' {
                    onde = Onde::Codigo;
                }
            }
            Onde::ComentarioDeLinha => {
                if c == '\n' {
                    onde = Onde::Codigo;
                } else {
                    saida[i] = ' ';
                }
            }
            Onde::ComentarioDeBloco => {
                saida[i] = ' ';
                if c == '*' && proximo == Some('/') {
                    saida[i + 1] = ' ';
                    i += 2;
                    onde = Onde::Codigo;
                    continue;
                }
            }
        }
        i += 1;
    }
    saida
}

/// Posição do único `*` que é item de lista de seleção — e não multiplicação,
/// nem `COUNT(*)`, nem o `*` sozinho que o Oracle já aceita.
fn asterisco_da_lista(caracteres: &[char]) -> Option<usize> {
    let mut encontrado = None;

    for (i, c) in caracteres.iter().enumerate() {
        if *c != '*' {
            continue;
        }
        let antes = token_antes(caracteres, i);
        let depois = token_depois(caracteres, i);

        let sem_operando_a_esquerda = antes == "SELECT" || antes == ",";
        let sem_operando_a_direita = depois == "," || depois == "FROM";
        if !sem_operando_a_esquerda || !sem_operando_a_direita {
            continue;
        }
        // `SELECT * FROM`: válido como está, não se mexe.
        if antes == "SELECT" && depois == "FROM" {
            continue;
        }
        // Mais de um asterisco solto não é forma que se saiba corrigir.
        if encontrado.is_some() {
            return None;
        }
        encontrado = Some(i);
    }
    encontrado
}

/// Com o que qualificar o asterisco: o apelido de cada tabela do `FROM`, ou o
/// nome quando não há apelido, na ordem em que aparecem.
fn qualificadores_do_from(caracteres: &[char]) -> Option<Vec<String>> {
    let tokens = tokenizar(caracteres);

    // Subconsulta em qualquer lugar tira a certeza sobre a qual `FROM` o
    // asterisco pertence. Igual para mais de um `FROM`.
    if tokens.iter().filter(|t| t.chave == "SELECT").count() != 1 {
        return None;
    }
    let inicio = tokens.iter().position(|t| t.chave == "FROM")?;
    if tokens.iter().filter(|t| t.chave == "FROM").count() != 1 {
        return None;
    }

    let mut itens: Vec<Vec<String>> = vec![Vec::new()];
    let mut ignorando = false;

    for token in &tokens[inicio + 1..] {
        if FIM_DA_LISTA_DE_TABELAS.contains(&token.chave.as_str()) {
            break;
        }
        // Uma view em linha ou uma junção entre parênteses não se resolve aqui.
        if token.chave == "(" {
            return None;
        }
        if token.chave == "," || LIGACOES.contains(&token.chave.as_str()) {
            if !itens.last().is_some_and(Vec::is_empty) {
                itens.push(Vec::new());
            }
            ignorando = false;
            continue;
        }
        // A condição da junção não faz parte da lista de tabelas.
        if token.chave == "ON" || token.chave == "USING" {
            ignorando = true;
            continue;
        }
        if ignorando || token.chave == "AS" {
            continue;
        }
        itens.last_mut()?.push(token.texto.clone());
    }

    let mut qualificadores = Vec::new();
    for item in itens {
        if item.is_empty() {
            continue;
        }
        // `tabela` ou `tabela apelido`; qualquer outra forma é desconhecida.
        if item.len() > 2 || !item.iter().all(|palavra| e_identificador(palavra)) {
            return None;
        }
        qualificadores.push(item.last()?.clone());
    }

    if qualificadores.is_empty() {
        None
    } else {
        Some(qualificadores)
    }
}

/// Uma palavra ou um sinal avulso do comando.
struct Token {
    /// Em maiúsculas, para comparar com palavra reservada.
    chave: String,
    /// Como estava escrito — é esta grafia que volta para o texto enviado.
    texto: String,
}

/// Palavras e sinais avulsos, na ordem. Só o que interessa para achar a lista
/// de tabelas.
fn tokenizar(caracteres: &[char]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let texto = if e_de_identificador(c) {
            let inicio = i;
            while i < caracteres.len() && e_de_identificador(caracteres[i]) {
                i += 1;
            }
            caracteres[inicio..i].iter().collect::<String>()
        } else {
            i += 1;
            c.to_string()
        };
        tokens.push(Token {
            chave: texto.to_uppercase(),
            texto,
        });
    }
    tokens
}

/// O token colado antes da posição: palavra em maiúsculas, ou o próprio sinal.
fn token_antes(caracteres: &[char], posicao: usize) -> String {
    let mut fim = posicao;
    while fim > 0 && caracteres[fim - 1].is_whitespace() {
        fim -= 1;
    }
    if fim == 0 {
        return String::new();
    }
    if !e_de_identificador(caracteres[fim - 1]) {
        return caracteres[fim - 1].to_string();
    }
    let mut inicio = fim;
    while inicio > 0 && e_de_identificador(caracteres[inicio - 1]) {
        inicio -= 1;
    }
    caracteres[inicio..fim].iter().collect::<String>().to_uppercase()
}

/// O token colado depois da posição: palavra em maiúsculas, ou o próprio sinal.
fn token_depois(caracteres: &[char], posicao: usize) -> String {
    let mut inicio = posicao + 1;
    while inicio < caracteres.len() && caracteres[inicio].is_whitespace() {
        inicio += 1;
    }
    if inicio >= caracteres.len() {
        return String::new();
    }
    if !e_de_identificador(caracteres[inicio]) {
        return caracteres[inicio].to_string();
    }
    let mut fim = inicio;
    while fim < caracteres.len() && e_de_identificador(caracteres[fim]) {
        fim += 1;
    }
    caracteres[inicio..fim].iter().collect::<String>().to_uppercase()
}

fn e_de_identificador(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || c == '#' || c == '.'
}

/// `financeiro` e `exemplo.financeiro` passam; qualquer outra coisa, não.
fn e_identificador(palavra: &str) -> bool {
    !palavra.is_empty()
        && palavra.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && palavra.chars().all(e_de_identificador)
        && palavra.split('.').count() <= 2
        && !palavra.split('.').any(str::is_empty)
}

/// Tipos cujo `CREATE` traz corpo com `;` dentro, e por isso só termina na
/// barra.
const TIPOS_COM_CORPO: &[&str] = &[
    "PACKAGE",
    "PROCEDURE",
    "FUNCTION",
    "TRIGGER",
    "TYPE",
    "LIBRARY",
    "JAVA",
];

/// Palavras que o Oracle aceita entre `CREATE [OR REPLACE]` e o tipo do objeto.
const MODIFICADORES_DO_CREATE: &[&str] = &[
    // Edição: o SQL Developer escreve `EDITIONABLE` em todo fonte que exporta.
    "EDITIONABLE",
    "NONEDITIONABLE",
    "EDITIONING",
    // View: `[NO] FORCE`.
    "FORCE",
    "NOFORCE",
    "NO",
    // Java: `AND { RESOLVE | COMPILE } [NOFORCE]`.
    "AND",
    "RESOLVE",
    "COMPILE",
];

/// O comando é um bloco PL/SQL, em que `;` não encerra a execução?
///
/// No `CREATE`, quem decide é o tipo do objeto — e ele não tem posição fixa:
/// `CREATE PROCEDURE` o traz na segunda palavra, `CREATE OR REPLACE
/// EDITIONABLE PROCEDURE` na quinta. Olhar só as quatro primeiras, como se
/// fazia, cortava o corpo do procedure no primeiro `;` e mandava o cabeçalho
/// sozinho ao banco, que o gravava inválido por cima do objeto bom.
#[allow(non_snake_case)]
fn ehBlocoPlsql(sql: &str) -> bool {
    let palavras = palavras_iniciais(sql, 16);
    let Some((_, primeira)) = palavras.first() else {
        return false;
    };
    if primeira.eq_ignore_ascii_case("DECLARE") || primeira.eq_ignore_ascii_case("BEGIN") {
        return true;
    }
    indice_do_tipo_no_create(&palavras).is_some_and(|indice| {
        TIPOS_COM_CORPO
            .iter()
            .any(|tipo| palavras[indice].1.eq_ignore_ascii_case(tipo))
    })
}

/// As primeiras palavras do comando, depois dos comentários iniciais, com a
/// posição de cada uma — em bytes — no texto recebido.
pub(crate) fn palavras_iniciais(sql: &str, limite: usize) -> Vec<(usize, &str)> {
    let resto = sem_comentarios_iniciais(sql);
    // `resto` é o fim de `sql`: a diferença de tamanho é onde ele começa.
    let base = sql.len() - resto.len();
    resto
        .split_whitespace()
        .take(limite)
        .map(|palavra| (base + (palavra.as_ptr() as usize - resto.as_ptr() as usize), palavra))
        .collect()
}

/// Em que palavra está o tipo do objeto de um `CREATE`, depois do `OR REPLACE`
/// e dos modificadores. `None` quando o comando não é `CREATE`.
pub(crate) fn indice_do_tipo_no_create(palavras: &[(usize, &str)]) -> Option<usize> {
    let e = |indice: usize, esperada: &str| {
        palavras
            .get(indice)
            .is_some_and(|(_, palavra)| palavra.eq_ignore_ascii_case(esperada))
    };
    if !e(0, "CREATE") {
        return None;
    }
    let mut indice = if e(1, "OR") && e(2, "REPLACE") { 3 } else { 1 };
    while palavras.get(indice).is_some_and(|(_, palavra)| {
        MODIFICADORES_DO_CREATE
            .iter()
            .any(|modificador| palavra.eq_ignore_ascii_case(modificador))
    }) {
        indice += 1;
    }
    (indice < palavras.len()).then_some(indice)
}

fn primeira_palavra(sql: &str) -> String {
    sem_comentarios_iniciais(sql)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_uppercase()
}

/// Descarta comentários do começo do comando, para achar a primeira palavra.
pub(crate) fn sem_comentarios_iniciais(sql: &str) -> &str {
    let mut resto = sql.trim_start();
    loop {
        if let Some(sem) = resto.strip_prefix("--") {
            resto = sem.split_once('\n').map(|(_, r)| r).unwrap_or("").trim_start();
        } else if let Some(sem) = resto.strip_prefix("/*") {
            resto = sem.split_once("*/").map(|(_, r)| r).unwrap_or("").trim_start();
        } else {
            return resto;
        }
    }
}

/// A barra está sozinha na linha (só espaços antes e depois)?
fn barra_sozinha(caracteres: &[char], posicao: usize) -> bool {
    let antes_ok = caracteres[..posicao]
        .iter()
        .rev()
        .take_while(|c| **c != '\n')
        .all(|c| c.is_whitespace());
    let depois_ok = caracteres[posicao + 1..]
        .iter()
        .take_while(|c| **c != '\n')
        .all(|c| c.is_whitespace());
    antes_ok && depois_ok
}

fn fim_da_linha(caracteres: &[char], posicao: usize) -> usize {
    let mut i = posicao;
    while i < caracteres.len() && caracteres[i] != '\n' {
        i += 1;
    }
    i + 1
}

fn guardar(comandos: &mut Vec<Comando>, atual: &mut String, linha_inicial: &mut usize, linha_atual: usize) {
    let sql = atual.trim().to_string();
    atual.clear();
    if !sql.is_empty() {
        comandos.push(Comando {
            sql,
            linha: *linha_inicial,
        });
    }
    *linha_inicial = linha_atual;
}

// --- SQL Server: lotes e binds ----------------------------------------------

/// Estado do varredor do T-SQL.
///
/// Separado do [`Onde`] do Oracle porque o T-SQL tem duas coisas que o PL/SQL
/// não tem: o identificador entre colchetes (`[Minha Tabela]`) e o comentário de
/// bloco **aninhado** — `/* /* */ */` é um comentário só, e fechar no primeiro
/// `*/` deixaria o resto do texto passando por código.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OndeTsql {
    Codigo,
    Texto,
    Colchete,
    Aspas,
    ComentarioDeLinha,
    /// Guarda a profundidade, porque o comentário de bloco do T-SQL aninha.
    ComentarioDeBloco(u32),
}

/// Separa o texto em **lotes** do T-SQL, na regra do SSMS.
///
/// A diferença para [`separar_comandos`] não é detalhe: no Oracle o `;` encerra
/// um comando; no T-SQL ele **separa instruções dentro do mesmo lote**, e quem
/// encerra o lote é a linha com `GO`. Cortar no `;` partiria ao meio todo
/// `IF … BEGIN … END`, todo `WHILE`, toda variável declarada e usada logo
/// abaixo — que é como o T-SQL é escrito.
///
/// `GO` não é comando do servidor: é marca do cliente, e vai embora aqui, como
/// no SSMS. A contagem opcional (`GO 5`) é aceita na linha, mas não repete o
/// lote — repetir seria inventar comportamento que ninguém pediu, e o número
/// aparece raramente fora de script de carga.
pub fn separar_lotes(texto: &str) -> Vec<Comando> {
    let mut lotes = Vec::new();
    let mut atual = String::new();
    let mut linha_inicial = 1usize;
    let mut onde = OndeTsql::Codigo;

    for (indice, linha) in texto.split('\n').enumerate() {
        let numero = indice + 1;

        // O `GO` só encerra quando está em código: dentro de literal ou de
        // comentário ele é conteúdo.
        if onde == OndeTsql::Codigo && e_separador_de_lote(linha) {
            guardar(&mut lotes, &mut atual, &mut linha_inicial, numero + 1);
            continue;
        }

        onde = avancar_no_tsql(linha, onde);
        // Comentário de linha não atravessa a quebra; os outros estados sim.
        if onde == OndeTsql::ComentarioDeLinha {
            onde = OndeTsql::Codigo;
        }

        if atual.is_empty() && linha.trim().is_empty() {
            // Linha em branco antes de o lote começar: o número do lote é o da
            // primeira linha com conteúdo, e não o do espaço acima dela.
            linha_inicial = numero + 1;
            continue;
        }
        atual.push_str(linha);
        atual.push('\n');
    }

    let ultima = texto.split('\n').count() + 1;
    guardar(&mut lotes, &mut atual, &mut linha_inicial, ultima);
    lotes
}

/// A linha é um `GO` — sozinho, com contagem ou com comentário atrás?
fn e_separador_de_lote(linha: &str) -> bool {
    let limpo = match linha.trim().split_once("--") {
        Some((antes, _)) => antes.trim(),
        None => linha.trim(),
    };
    let mut partes = limpo.split_whitespace();
    let Some(primeira) = partes.next() else {
        return false;
    };
    if !primeira.eq_ignore_ascii_case("GO") {
        return false;
    }
    match partes.next() {
        None => true,
        Some(contagem) => contagem.parse::<u32>().is_ok() && partes.next().is_none(),
    }
}

/// Percorre uma linha atualizando o estado do varredor.
fn avancar_no_tsql(linha: &str, inicial: OndeTsql) -> OndeTsql {
    percorrer_codigo_tsql(linha, inicial, |_| {})
}

/// Confere se o SELECT pode ficar dentro dos parênteses de uma tabela derivada.
pub(crate) fn pode_embutir_consulta(sql: &str, sqlserver: bool) -> bool {
    let mut profundidade = 0;
    let mut valido = true;
    let mut conferir = |c| {
        match c {
            '(' => profundidade += 1,
            ')' => profundidade -= 1,
            ';' => valido = false,
            _ => {}
        }
        valido &= profundidade >= 0;
    };
    if sqlserver {
        let mut onde = OndeTsql::Codigo;
        for linha in sql.split(['\r', '\n']) {
            onde = percorrer_codigo_tsql(linha, onde, &mut conferir);
            if onde == OndeTsql::ComentarioDeLinha {
                onde = OndeTsql::Codigo;
            }
        }
        valido &= onde == OndeTsql::Codigo;
    } else {
        mascarar(sql).into_iter().for_each(&mut conferir);
    }
    valido && profundidade == 0
}

fn percorrer_codigo_tsql(linha: &str, inicial: OndeTsql, mut visitar: impl FnMut(char)) -> OndeTsql {
    let caracteres: Vec<char> = linha.chars().collect();
    let mut onde = inicial;
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];
        let proximo = caracteres.get(i + 1).copied();

        match onde {
            OndeTsql::Codigo => match (c, proximo) {
                ('\'', _) => onde = OndeTsql::Texto,
                ('[', _) => onde = OndeTsql::Colchete,
                ('"', _) => onde = OndeTsql::Aspas,
                ('-', Some('-')) => return OndeTsql::ComentarioDeLinha,
                ('/', Some('*')) => {
                    onde = OndeTsql::ComentarioDeBloco(1);
                    i += 2;
                    continue;
                }
                _ => visitar(c),
            },
            // `''` é a aspa escapada: fecha e reabre, o que dá no mesmo.
            OndeTsql::Texto if c == '\'' => onde = OndeTsql::Codigo,
            OndeTsql::Colchete if c == ']' => {
                if proximo == Some(']') {
                    i += 2;
                    continue;
                }
                onde = OndeTsql::Codigo;
            }
            OndeTsql::Aspas if c == '"' => onde = OndeTsql::Codigo,
            OndeTsql::ComentarioDeBloco(nivel) => match (c, proximo) {
                ('/', Some('*')) => {
                    onde = OndeTsql::ComentarioDeBloco(nivel + 1);
                    i += 2;
                    continue;
                }
                ('*', Some('/')) => {
                    onde = if nivel <= 1 {
                        OndeTsql::Codigo
                    } else {
                        OndeTsql::ComentarioDeBloco(nivel - 1)
                    };
                    i += 2;
                    continue;
                }
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    onde
}

/// Troca os `:nome` do texto pelos `@P1`, `@P2`… que o driver do SQL Server usa.
///
/// O marcador de bind continua sendo o do Oracle — `:nome` (requisito 15) — e
/// não o `@nome` do T-SQL. Não é teimosia: `@nome` **já significa outra coisa**
/// no T-SQL, que é a variável local declarada com `DECLARE`. Tratar `@x` como
/// bind faria todo script com `DECLARE @x int` parar para perguntar um valor
/// que o próprio script já dá. O `:nome`, esse, não é sintaxe válida em lugar
/// nenhum do T-SQL: não há o que confundir.
///
/// A posição vem da lista de nomes que a tela preencheu, então um mesmo nome
/// repetido no texto vira o mesmo `@Pn`.
pub fn substituir_binds_sqlserver(sql: &str, nomes: &[String]) -> String {
    let mut saida = String::with_capacity(sql.len());
    let mut onde = Onde::Codigo;
    let caracteres: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];
        let proximo = caracteres.get(i + 1).copied();

        match onde {
            Onde::Codigo => match (c, proximo) {
                ('\'', _) => onde = Onde::Texto,
                ('"', _) => onde = Onde::Aspas,
                ('-', Some('-')) => onde = Onde::ComentarioDeLinha,
                ('/', Some('*')) => onde = Onde::ComentarioDeBloco,
                (':', Some(':')) => {
                    saida.push_str("::");
                    i += 2;
                    continue;
                }
                (':', Some(p)) if p.is_alphabetic() || p == '_' => {
                    let mut nome = String::new();
                    let mut j = i + 1;
                    while let Some(&letra) = caracteres.get(j) {
                        if letra.is_alphanumeric() || letra == '_' || letra == '$' || letra == '#' {
                            nome.push(letra);
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    match nomes.iter().position(|n| n.eq_ignore_ascii_case(&nome)) {
                        Some(posicao) => saida.push_str(&format!("@P{}", posicao + 1)),
                        // Bind sem valor informado segue como está: o servidor
                        // recusa com a mensagem dele, que diz mais do que um
                        // `@P0` inventado aqui.
                        None => {
                            saida.push(':');
                            saida.push_str(&nome);
                        }
                    }
                    i = j;
                    continue;
                }
                _ => {}
            },
            Onde::Texto if c == '\'' => onde = Onde::Codigo,
            Onde::Aspas if c == '"' => onde = Onde::Codigo,
            Onde::ComentarioDeLinha if c == '\n' => onde = Onde::Codigo,
            Onde::ComentarioDeBloco if c == '*' && proximo == Some('/') => {
                saida.push_str("*/");
                i += 2;
                onde = Onde::Codigo;
                continue;
            }
            _ => {}
        }
        saida.push(c);
        i += 1;
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn substituicoes_respeitam_define_comentarios_e_ponto_de_concatenacao() {
        let texto = "-- &ignorada\nDEFINE dono = 'EXEMPLO'\nSET DEFINE ON\nselect '&dono..DEM', '&nome', 'd''água &nome' from dual;\n/* &ignorada\nSET DEFINE OFF\n*/\nSET DEFINE OFF\nselect '&literal' from dual;\nSET DEFINE ON\nUNDEFINE dono\nselect '&&dono', '&dono' from dual;";
        let mut pedidos = Vec::new();
        let preparado = substituir_variaveis_oracle(texto, |nome| {
            pedidos.push(nome.to_string());
            "HOM".into()
        });
        assert_eq!(pedidos, ["nome", "nome", "dono"]);
        assert!(preparado.contains("select 'EXEMPLO.DEM', 'HOM', 'd''água HOM'"));
        assert!(preparado.contains("select '&literal'"));
        assert!(preparado.contains("select 'HOM', 'HOM'"));
        assert!(preparado.contains("/* &ignorada\nSET DEFINE OFF\n*/"));
        assert_eq!(preparado.lines().count(), texto.lines().count());
    }

    #[test]
    fn script_ficticio_conserva_os_seis_blocos() {
        let texto = r#"WHENEVER SQLERROR EXIT SQL.SQLCODE ROLLBACK
SET DEFINE ON
DECLARE
    esperado VARCHAR2(128) := UPPER('&BASE_TESTE');
BEGIN
    NULL;
END;
/
DECLARE
    PROCEDURE adicionar IS
    BEGIN
        NULL;
    END;
BEGIN
    adicionar;
END;
/
BEGIN
    NULL;
END;
/
CREATE OR REPLACE TRIGGER EXEMPLO.TRG_ITENS
BEFORE INSERT ON EXEMPLO.ITENS
FOR EACH ROW
BEGIN
    :NEW.NOME := TRIM(:NEW.NOME);
END;
/
CREATE OR REPLACE PROCEDURE EXEMPLO.NORMALIZAR AS
BEGIN
    NULL;
END;
/
BEGIN
    NULL;
END;
/
"#;
        let mut pedidos = Vec::new();
        let substituido = substituir_variaveis_oracle(texto, |nome| {
            pedidos.push(nome.to_string());
            "BASE_HOM".into()
        });
        let (preparado, opcoes) = preparar_script_oracle(&substituido);
        assert_eq!(pedidos, ["BASE_TESTE"]);
        assert!(opcoes.rollback_ao_erro);
        let comandos = separar_comandos(&preparado);
        assert_eq!(comandos.len(), 6);
        assert!(comandos[0].sql.contains("UPPER('BASE_HOM')"));
        assert!(comandos[1].sql.contains("PROCEDURE adicionar"));
        assert!(comandos[3].sql.contains("CREATE OR REPLACE TRIGGER"));
        assert!(comandos[4].sql.contains("CREATE OR REPLACE PROCEDURE"));
        assert!(comandos.iter().all(|comando| binds(&comando.sql).is_empty()));
        assert_eq!(preparado.lines().count(), texto.lines().count());
    }

    #[test]
    fn diretivas_do_sql_developer_sao_retiradas_sem_mudar_as_linhas() {
        let texto = "WHENEVER SQLERROR EXIT SQL.SQLCODE ROLLBACK\r\n\
SET SERVEROUTPUT ON;\n\
select 1 from dual;";
        let (preparado, opcoes) = preparar_script_oracle(texto);

        assert!(opcoes.rollback_ao_erro);
        assert_eq!(opcoes.saida_dbms, Some(true));
        assert_eq!(preparado, "\n\nselect 1 from dual;");
        assert_eq!(separar_comandos(&preparado)[0].linha, 3);
    }

    #[test]
    fn serveroutput_off_e_reconhecido_sem_engolir_outro_set() {
        let texto = "set serveroutput off\nset timing on\nselect 1 from dual;";
        let (preparado, opcoes) = preparar_script_oracle(texto);

        assert_eq!(opcoes.saida_dbms, Some(false));
        assert_eq!(preparado, "\nset timing on\nselect 1 from dual;");
    }

    // --- SQL Server: lotes ---------------------------------------------------

    fn lotes(texto: &str) -> Vec<String> {
        separar_lotes(texto).into_iter().map(|lote| lote.sql).collect()
    }

    #[test]
    fn o_ponto_e_virgula_nao_separa_lote_no_tsql() {
        // No T-SQL o `;` separa instruções dentro do lote. Cortar nele partiria
        // ao meio a variável declarada e usada logo abaixo.
        let texto = "declare @x int = 1;
select @x";
        assert_eq!(lotes(texto), ["declare @x int = 1;
select @x"]);
    }

    #[test]
    fn o_go_separa_lotes() {
        let texto = "select 1
GO
select 2
go
select 3";
        assert_eq!(lotes(texto), ["select 1", "select 2", "select 3"]);
    }

    #[test]
    fn o_go_aceita_contagem_e_comentario_atras() {
        assert_eq!(lotes("select 1
GO 5
select 2"), ["select 1", "select 2"]);
        assert_eq!(lotes("select 1
go -- fim
select 2"), ["select 1", "select 2"]);
    }

    #[test]
    fn go_no_meio_da_linha_nao_separa() {
        // `go` só encerra o lote quando está sozinho na linha; aqui é o nome de
        // uma coluna e de uma tabela.
        assert_eq!(lotes("select go from cargo"), ["select go from cargo"]);
    }

    #[test]
    fn go_dentro_de_texto_ou_comentario_nao_separa() {
        let literal = "select 'linha
GO
fim'";
        assert_eq!(lotes(literal), [literal]);
        let comentario = "/*
GO
*/
select 1";
        assert_eq!(lotes(comentario), [comentario]);
    }

    #[test]
    fn comentario_de_bloco_do_tsql_aninha() {
        // Fechar no primeiro `*/` deixaria o `GO` de baixo passando por código.
        let texto = "/* fora /* dentro */
GO
ainda comentado */
select 1";
        assert_eq!(lotes(texto), [texto]);
    }

    #[test]
    fn colchete_protege_o_nome_do_objeto() {
        let texto = "select * from [minha ' tabela]
GO
select 2";
        assert_eq!(lotes(texto), ["select * from [minha ' tabela]", "select 2"]);
    }

    #[test]
    fn o_lote_guarda_a_linha_em_que_comeca() {
        let separados = separar_lotes("select 1
GO

select 2");
        assert_eq!(separados[0].linha, 1);
        assert_eq!(separados[1].linha, 4);
    }

    // --- SQL Server: binds ---------------------------------------------------

    #[test]
    fn os_binds_viram_parametros_posicionais() {
        let nomes = vec!["nome".to_string(), "idade".to_string()];
        assert_eq!(
            substituir_binds_sqlserver("select * from p where n = :nome and i > :idade", &nomes),
            "select * from p where n = @P1 and i > @P2"
        );
    }

    #[test]
    fn o_mesmo_bind_repetido_vira_o_mesmo_parametro() {
        let nomes = vec!["id".to_string()];
        assert_eq!(
            substituir_binds_sqlserver("select :id, :ID", &nomes),
            "select @P1, @P1"
        );
    }

    #[test]
    fn a_variavel_do_tsql_nao_e_bind() {
        // `@x` é variável local do T-SQL, declarada logo acima: trocá-la faria
        // todo script com `DECLARE` parar para pedir um valor que ele já tem.
        let nomes = vec!["limite".to_string()];
        let texto = "declare @x int = 10; select * from t where a = @x and b = :limite";
        assert_eq!(
            substituir_binds_sqlserver(texto, &nomes),
            "declare @x int = 10; select * from t where a = @x and b = @P1"
        );
    }

    #[test]
    fn bind_dentro_de_texto_ou_comentario_fica_como_esta() {
        let nomes = vec!["nome".to_string()];
        let texto = "select 'oi :nome' -- e :nome
from t where n = :nome";
        assert_eq!(
            substituir_binds_sqlserver(texto, &nomes),
            "select 'oi :nome' -- e :nome
from t where n = @P1"
        );
    }

    #[test]
    fn bind_sem_valor_informado_segue_intocado() {
        // O servidor recusa com a mensagem dele, que diz mais do que um `@P0`
        // inventado aqui.
        assert_eq!(
            substituir_binds_sqlserver("select :perdido", &[]),
            "select :perdido"
        );
    }

    fn sqls(texto: &str) -> Vec<String> {
        separar_comandos(texto).into_iter().map(|c| c.sql).collect()
    }

    /// O que sai para o Oracle.
    fn enviado(sql: &str) -> String {
        preparar_para_execucao(sql).into_owned()
    }

    /// O texto passou intocado?
    fn intocado(sql: &str) -> bool {
        matches!(preparar_para_execucao(sql), Cow::Borrowed(_))
    }

    #[test]
    fn asterisco_solto_na_lista_vira_asterisco_qualificado() {
        // A forma que o Oracle recusa, nas duas ordens.
        assert_eq!(
            enviado("SELECT id_registro, * FROM financeiro"),
            "SELECT id_registro, financeiro.* FROM financeiro"
        );
        assert_eq!(
            enviado("SELECT *, id_registro FROM financeiro"),
            "SELECT financeiro.*, id_registro FROM financeiro"
        );
        // Com apelido, o apelido é que qualifica.
        assert_eq!(
            enviado("SELECT *, s.id_registro FROM financeiro s"),
            "SELECT s.*, s.id_registro FROM financeiro s"
        );
        // Duas tabelas: o asterisco vale pelas duas, na ordem do FROM.
        assert_eq!(
            enviado("SELECT *, a.x FROM cliente c, pedido p"),
            "SELECT c.*, p.*, a.x FROM cliente c, pedido p"
        );
        assert_eq!(
            enviado("SELECT *, c.x FROM cliente c JOIN pedido p ON p.id = c.id"),
            "SELECT c.*, p.*, c.x FROM cliente c JOIN pedido p ON p.id = c.id"
        );
        // Dono no nome da tabela.
        assert_eq!(
            enviado("SELECT *, x FROM exemplo.financeiro"),
            "SELECT exemplo.financeiro.*, x FROM exemplo.financeiro"
        );
    }

    #[test]
    fn asterisco_que_o_oracle_ja_aceita_nao_e_tocado() {
        // Estes precisam sair idênticos: são consultas que hoje funcionam, e
        // mexer nelas seria mudar resultado de quem não pediu nada.
        assert!(intocado("SELECT * FROM financeiro"));
        assert!(intocado("SELECT s.* FROM financeiro s"));
        assert!(intocado("SELECT s.*, s.id_registro FROM financeiro s"));
        assert!(intocado("SELECT COUNT(*), nome FROM financeiro GROUP BY nome"));
        assert!(intocado("SELECT nome, COUNT(*) FROM financeiro GROUP BY nome"));
    }

    #[test]
    fn multiplicacao_nunca_e_confundida_com_asterisco_de_lista() {
        assert!(intocado("SELECT preco * 2, nome FROM produto"));
        assert!(intocado("SELECT nome, preco * quantidade FROM item"));
        assert!(intocado("SELECT nome, (a + b) * 2 FROM t"));
        assert!(intocado("SELECT nome, 2 * (a + b) FROM t"));
    }

    #[test]
    fn na_duvida_o_texto_vai_como_esta() {
        // Subconsulta: não dá para saber a qual FROM o asterisco pertence.
        assert!(intocado("SELECT *, x FROM t WHERE id IN (SELECT id FROM u)"));
        assert!(intocado("SELECT *, x FROM (SELECT a, b FROM t)"));
        // View em linha na lista de tabelas.
        assert!(intocado("SELECT *, x FROM (t)"));
        // Sem FROM não há o que qualificar.
        assert!(intocado("SELECT *, 1"));
        // Não é SELECT.
        assert!(intocado("UPDATE t SET a = 1"));
    }

    #[test]
    fn asterisco_dentro_de_texto_e_de_comentario_nao_conta() {
        assert!(intocado("SELECT 'a, * FROM x', nome FROM produto"));
        assert!(intocado("SELECT nome /* , * FROM x */ FROM produto"));
        assert!(intocado("SELECT nome -- , * FROM x\n FROM produto"));
        // O comentário some da máscara, mas continua no texto enviado.
        assert_eq!(
            enviado("SELECT *, nome /* tudo */ FROM produto"),
            "SELECT produto.*, nome /* tudo */ FROM produto"
        );
    }

    #[test]
    fn separa_comandos_por_ponto_e_virgula() {
        assert_eq!(
            sqls("select 1 from dual; select 2 from dual;"),
            ["select 1 from dual", "select 2 from dual"]
        );
    }

    #[test]
    fn comando_sem_ponto_e_virgula_final_conta() {
        assert_eq!(sqls("select 1 from dual"), ["select 1 from dual"]);
    }

    #[test]
    fn ponto_e_virgula_dentro_de_texto_nao_separa() {
        assert_eq!(
            sqls("select 'a;b' from dual"),
            ["select 'a;b' from dual"]
        );
    }

    #[test]
    fn ponto_e_virgula_em_comentario_nao_separa() {
        assert_eq!(
            sqls("select 1 from dual -- comentário; com ponto e vírgula\n"),
            ["select 1 from dual -- comentário; com ponto e vírgula"]
        );
        assert_eq!(
            sqls("select 1 /* ; */ from dual"),
            ["select 1 /* ; */ from dual"]
        );
    }

    #[test]
    fn bloco_plsql_so_termina_na_barra() {
        let texto = "begin\n  null;\n  null;\nend;\n/\nselect 1 from dual;";
        assert_eq!(
            sqls(texto),
            ["begin\n  null;\n  null;\nend;", "select 1 from dual"]
        );
    }

    #[test]
    fn declare_tambem_e_bloco() {
        let texto = "declare\n  x number;\nbegin\n  x := 1;\nend;\n/";
        assert_eq!(sqls(texto).len(), 1);
        assert!(sqls(texto)[0].starts_with("declare"));
    }

    #[test]
    fn create_procedure_e_bloco() {
        let texto = "create or replace procedure p is\nbegin\n  null;\nend;\n/";
        assert_eq!(sqls(texto).len(), 1);
    }

    #[test]
    fn create_table_nao_e_bloco() {
        assert_eq!(
            sqls("create table t (a number); select 1 from dual;").len(),
            2
        );
    }

    /// O script que o SQL Developer rodava e o Cururu partia em nove pedaços:
    /// o `EDITIONABLE` empurrava o `PROCEDURE` para a quinta palavra.
    const SCRIPT_DO_GRAV_LOG: &str = "\
-- Aplicar ANTES de publicar a aplicacao que envia IN_METODO e IN_LOG.
-- Mantem o COMMIT da procedure atual; nao adiciona transacao autonoma.
CREATE OR REPLACE EDITIONABLE PROCEDURE EXEMPLO.GRAV_LOG
(
    IN_MSG    IN VARCHAR2,
    IN_COMMIT IN BOOLEAN DEFAULT TRUE
) IS
    V_METODO VARCHAR2(32767);
    V_MSG    VARCHAR2(4000);

    FUNCTION LIMITAR_BYTES(P_TEXTO VARCHAR2, P_LIMITE PLS_INTEGER)
        RETURN VARCHAR2 IS
        V_TEXTO VARCHAR2(32767) := SUBSTR(P_TEXTO, 1, P_LIMITE);
    BEGIN
        RETURN V_TEXTO;
    END;
BEGIN
    V_MSG := LIMITAR_BYTES(IN_MSG, 4000);
    IF IN_COMMIT THEN
        COMMIT;
    END IF;
END;
/
SHOW ERRORS PROCEDURE EXEMPLO.GRAV_LOG;

SELECT OBJECT_NAME, STATUS
  FROM ALL_OBJECTS
 WHERE OWNER = 'EXEMPLO' AND OBJECT_NAME = 'GRAV_LOG';

-- Validacao apos publicar a aplicacao.
SELECT ID, MSG FROM EXEMPLO.Z_LOG WHERE ROWNUM <= 30;
";

    #[test]
    fn create_editionable_so_termina_na_barra() {
        let comandos = separar_comandos(SCRIPT_DO_GRAV_LOG);
        let resumo: Vec<(usize, &str)> = comandos
            .iter()
            .map(|c| (c.linha, sem_comentarios_iniciais(&c.sql).lines().next().unwrap_or("")))
            .collect();
        assert_eq!(
            resumo,
            [
                (1, "CREATE OR REPLACE EDITIONABLE PROCEDURE EXEMPLO.GRAV_LOG"),
                (24, "SHOW ERRORS PROCEDURE EXEMPLO.GRAV_LOG"),
                (26, "SELECT OBJECT_NAME, STATUS"),
                (30, "SELECT ID, MSG FROM EXEMPLO.Z_LOG WHERE ROWNUM <= 30"),
            ]
        );
        // O corpo vai inteiro, até o END do procedure.
        assert!(comandos[0].sql.ends_with("    END IF;\nEND;"));
    }

    #[test]
    fn create_com_modificadores_e_bloco() {
        for texto in [
            "create or replace noneditionable package body p as\n  x number;\nend;\n/\nselect 1 from dual;",
            "CREATE EDITIONABLE TRIGGER t BEFORE INSERT ON x FOR EACH ROW BEGIN NULL; END;\n/\nselect 1 from dual;",
            "create or replace\neditionable\nfunction f return number is begin return 1; end;\n/\nselect 1 from dual;",
            "create or replace and compile java source named \"Oi\" as\npublic class Oi { int x; }\n/\nselect 1 from dual;",
            "-- cabeçalho; com ponto e vírgula\ncreate or replace editionable type t as object (a number, b number);\n/\nselect 1 from dual;",
        ] {
            assert_eq!(sqls(texto).len(), 2, "{texto}");
        }
    }

    #[test]
    fn create_de_objeto_sem_corpo_nao_e_bloco() {
        for texto in [
            "create or replace force editionable view v as select 1 x from dual; select 1 from dual;",
            "create or replace public synonym s for t; select 1 from dual;",
            "create or replace editionable synonym s for t; select 1 from dual;",
            "create table type (package number); select 1 from dual;",
            "create index procedure on t (a); select 1 from dual;",
        ] {
            assert_eq!(sqls(texto).len(), 2, "{texto}");
        }
    }

    #[test]
    fn show_errors_termina_na_quebra_de_linha() {
        assert_eq!(
            sqls("SHOW ERRORS\nSELECT 1 FROM DUAL;"),
            ["SHOW ERRORS", "SELECT 1 FROM DUAL"]
        );
        assert_eq!(
            sqls("sho err -- confere\nselect 1 from dual"),
            ["sho err -- confere", "select 1 from dual"]
        );
        assert_eq!(
            sqls("show errors;\nselect 1 from dual;"),
            ["show errors", "select 1 from dual"]
        );
        let comandos = separar_comandos("begin null; end;\n/\n-- confere\nSHOW ERRORS\n\nselect 1 from dual");
        let linhas: Vec<usize> = comandos.iter().map(|c| c.linha).collect();
        assert_eq!(linhas, [1, 3, 6]);
        assert_eq!(comandos[1].sql, "-- confere\nSHOW ERRORS");
    }

    #[test]
    fn show_errors_em_comentario_ou_no_meio_do_comando_nao_corta() {
        assert_eq!(
            sqls("-- SHOW ERRORS\nselect 1\nfrom dual;"),
            ["-- SHOW ERRORS\nselect 1\nfrom dual"]
        );
        assert_eq!(
            sqls("select 'SHOW ERRORS'\nfrom dual;"),
            ["select 'SHOW ERRORS'\nfrom dual"]
        );
    }

    #[test]
    fn palavras_iniciais_dao_a_posicao_no_texto_recebido() {
        let texto = "-- nota\n  create  or replace";
        let palavras = palavras_iniciais(texto, 2);
        assert_eq!(palavras, [(10, "create"), (18, "or")]);
        assert_eq!(&texto[10..16], "create");
    }

    #[test]
    fn guarda_a_linha_em_que_o_comando_comeca() {
        let comandos = separar_comandos("select 1 from dual;\n\nselect 2 from dual;");
        assert_eq!(comandos[0].linha, 1);
        assert_eq!(comandos[1].linha, 3);
    }

    #[test]
    fn acha_os_binds() {
        assert_eq!(binds("select * from t where a = :codigo and b = :nome"), ["codigo", "nome"]);
    }

    #[test]
    fn bind_repetido_aparece_uma_vez_so() {
        assert_eq!(binds("select :a, :a, :A from dual"), ["a"]);
    }

    #[test]
    fn nao_confunde_texto_e_comentario_com_bind() {
        assert!(binds("select ':nao' from dual").is_empty());
        assert!(binds("select 1 from dual -- :nao").is_empty());
        assert!(binds("select 1 /* :nao */ from dual").is_empty());
    }

    #[test]
    fn dois_pontos_seguidos_nao_e_bind() {
        assert!(binds("select 1::number from dual").is_empty());
    }

    #[test]
    fn pseudorregistros_de_trigger_nao_sao_binds() {
        let script = "CREATE OR REPLACE TRIGGER EXEMPLO.TRG_CLIENTES_AI
AFTER INSERT ON EXEMPLO.CLIENTES
FOR EACH ROW
BEGIN
  INSERT INTO EXEMPLO.CLIENTES_COPIA (ID_CLIENTE, DOCUMENTO)
  SELECT :NEW.ID_CLIENTE, :new.DOCUMENTO FROM DUAL
  WHERE NOT EXISTS (SELECT 1 FROM EXEMPLO.CLIENTES_COPIA P
                    WHERE P.ID_CLIENTE = :OLD.ID_CLIENTE);
END;
/
SHOW ERRORS TRIGGER EXEMPLO.TRG_CLIENTES_AI;";
        let comandos = separar_comandos(script);
        assert_eq!(comandos.len(), 2);
        assert!(binds(&comandos[0].sql).is_empty());
        assert!(binds(&comandos[1].sql).is_empty());
        assert_eq!(binds("select :NEW.ID_CLIENTE from dual"), ["NEW"]);
        assert_eq!(binds("create trigger t before insert on x for each row begin :NEW.x := :valor; end;"), ["valor"]);
    }

    #[test]
    fn reconhece_consulta() {
        assert!(e_consulta("select * from dual"));
        assert!(e_consulta("  WITH x AS (select 1 from dual) select * from x"));
        assert!(e_consulta("-- comentário\nselect 1 from dual"));
        assert!(!e_consulta("update t set a = 1"));
        assert!(!e_consulta("begin null; end;"));
    }

    #[test]
    fn execute_vira_bloco_plsql() {
        assert_eq!(
            preparar_para_execucao("EXECUTE INCLUI_BP_DEMVENDA"),
            "BEGIN INCLUI_BP_DEMVENDA; END;"
        );
        assert_eq!(
            preparar_para_execucao("exec pacote.procedimento(:codigo, 'X')"),
            "BEGIN pacote.procedimento(:codigo, 'X'); END;"
        );
    }

    #[test]
    fn execute_aceita_comentario_antes_e_nao_confunde_outra_palavra() {
        assert_eq!(
            preparar_para_execucao("-- chamada\nEXEC p"),
            "BEGIN p; END;"
        );
        assert!(matches!(
            preparar_para_execucao("execution_plan"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn separador_tira_o_terminador_do_comando() {
        assert_eq!(sqls("select 1 from dual;"), ["select 1 from dual"]);
        assert_eq!(sqls("begin null; end;\n/"), ["begin null; end;"]);
    }

    // --- Oracle: o que o comando faz com a transação ------------------------

    #[test]
    fn ddl_do_oracle_confirma_sozinho() {
        for sql in [
            "alter table cliente add (email varchar2(200))",
            "CREATE OR REPLACE PROCEDURE p AS BEGIN NULL; END;",
            "drop table t purge",
            "truncate table log_execucao",
            "comment on column cliente.nome is 'Nome'",
            "grant select on cliente to relatorio",
            "-- ajuste de nome\nrename t1 to t2",
            "/* índice */ create index ix_cliente on cliente (nome)",
        ] {
            assert_eq!(efeito_no_oracle(sql), EfeitoNoOracle::ConfirmaSozinho, "{sql}");
        }
    }

    #[test]
    fn alter_session_e_savepoint_nao_mexem_na_transacao() {
        for sql in [
            "alter session set nls_date_format = 'dd/mm/yyyy'",
            "ALTER SYSTEM FLUSH SHARED_POOL",
            "savepoint antes_da_carga",
            "set transaction read only",
            "rollback to savepoint antes_da_carga",
            "ROLLBACK WORK TO antes_da_carga",
        ] {
            assert_eq!(efeito_no_oracle(sql), EfeitoNoOracle::NaoMexe, "{sql}");
        }
    }

    #[test]
    fn commit_e_rollback_digitados_encerram_a_transacao() {
        assert_eq!(efeito_no_oracle("commit"), EfeitoNoOracle::Commit);
        assert_eq!(efeito_no_oracle("COMMIT WORK"), EfeitoNoOracle::Commit);
        assert_eq!(efeito_no_oracle("rollback"), EfeitoNoOracle::Rollback);
        assert_eq!(efeito_no_oracle("rollback work"), EfeitoNoOracle::Rollback);
    }

    #[test]
    fn dml_e_bloco_podem_deixar_pendente() {
        for sql in [
            "update cliente set ativo = 'N'",
            "insert into t values (1)",
            "delete from t",
            "merge into t using s on (t.id = s.id) when matched then update set t.c = s.c",
            "begin carga.processar; end;",
            "lock table t in exclusive mode",
        ] {
            assert_eq!(efeito_no_oracle(sql), EfeitoNoOracle::PodeDeixarPendente, "{sql}");
        }
    }
}
