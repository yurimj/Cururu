//! Execução de SQL (requisitos 5, 6, 7, 8, 10, 14, 15 e 17).
//!
//! As consultas mantêm o cursor aberto para trazer as linhas em blocos — é o
//! que permite `select * from` sem `where` sem esperar o banco inteiro
//! (requisito 10). O cursor fica registrado aqui até a aba pedir mais linhas,
//! trocar de consulta ou fechar.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use oracle::sql_type::{OracleType, Timestamp};
use oracle::{ResultSet, Row};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::plsql::{self, ErroDeCompilacao, ObjetoPlsql, PedidoDeErros};
use crate::registro;
use crate::sessao::Sessao;
use crate::sql_texto::{self, EfeitoNoOracle};

/// Teto de linhas de `DBMS_OUTPUT` lidas após cada execução, para uma saída
/// enorme não travar a interface.
const MAXIMO_DE_LINHAS_DBMS: usize = 500;

/// O que se acrescenta à mensagem de um comando que ficou pendente.
///
/// Com o auto-commit desligado nada do que foi executado vale ainda, e fechar a
/// aba descarta tudo sem perguntar (requisito 12). Antes, a única pista era a
/// **ausência** do "(commit automático)" — dizer o contrário por omissão. No
/// SQL Server isso custa caro: ali o DDL também é transacional, e um `alter
/// procedure` executado com sucesso desaparece inteiro quando a aba fecha.
pub const AVISO_DE_TRANSACAO_ABERTA: &str = " — transação aberta, falta commit";

/// Como a transação da aba ficou depois de um comando (requisito 7).
///
/// É isto que acende e apaga o ✔ da barra, e não o tipo do resultado: no
/// Oracle um DDL não devolve linhas e mesmo assim não deixa nada para
/// confirmar — confirma sozinho, e leva junto o que estava pendente —, e um
/// `commit` digitado no editor fecha a transação como o botão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Transacao {
    /// Consulta, ou comando que não abre nem fecha transação.
    Inalterada,
    /// Há trabalho sem confirmar: só vale depois do commit.
    Aberta,
    /// Não sobrou nada pendente — nem o que estava antes do comando.
    Encerrada,
}

/// Uma coluna do resultado.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColunaResultado {
    pub nome: String,
    /// Tipo do Oracle, como aparece no cabeçalho do grid.
    pub tipo: String,
    /// Colunas numéricas são alinhadas à direita no grid.
    pub numerica: bool,
}

/// `None` representa `NULL`; `Some("")` é texto vazio. A distinção importa no
/// grid (requisito 20).
pub type Linha = Vec<Option<String>>;

/// O que uma execução devolve.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoDaExecucao {
    /// Comando executado, para o histórico (requisito 19).
    pub comando: String,
    /// Linha do editor onde o comando começa (requisito 18).
    pub linha_no_editor: usize,
    pub duracao_ms: u64,
    /// Presente quando o comando devolveu linhas.
    pub consulta: Option<ResultadoConsulta>,
    /// Presente quando o comando alterou dados.
    pub linhas_afetadas: Option<u64>,
    /// Como a transação da aba ficou depois deste comando.
    pub transacao: Transacao,
    /// Texto para o painel de mensagens.
    pub mensagem: String,
    /// Linhas de `DBMS_OUTPUT` produzidas por este comando (requisito 8).
    pub saida_dbms: Vec<String>,
    /// A conexão tinha caído e foi restabelecida antes deste comando rodar.
    ///
    /// Vem marcado só no primeiro resultado do lote. A tela avisa, porque a
    /// sessão nova é outra sessão do banco: o que estava aberto sem commit se
    /// perdeu junto com a conexão velha.
    pub reconectado: bool,
    /// Erros do objeto que este comando compilou, quando o `CREATE` o deixou
    /// inválido (requisito 13). A linha é contada no texto executado, para o
    /// clique na mensagem levar ao erro.
    pub erros_de_compilacao: Vec<ErroDeCompilacao>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoConsulta {
    /// Identificador do cursor, para pedir mais linhas.
    pub cursor_id: String,
    pub colunas: Vec<ColunaResultado>,
    pub linhas: Vec<Linha>,
    pub tem_mais: bool,
}

/// Bloco adicional de linhas de um cursor já aberto.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocoDeLinhas {
    pub linhas: Vec<Linha>,
    pub tem_mais: bool,
}

/// Cursor guardado entre uma busca e outra.
struct CursorAberto {
    resultado: ResultSet<'static, Row>,
    colunas: Vec<ColunaResultado>,
}

/// Cursores abertos, por identificador.
#[derive(Default)]
pub struct Cursores {
    mapa: Mutex<HashMap<String, Arc<Mutex<CursorAberto>>>>,
}

impl Cursores {
    fn guardar(&self, id: String, cursor: CursorAberto) {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        mapa.insert(id, Arc::new(Mutex::new(cursor)));
    }

    fn obter(&self, id: &str) -> Resultado<Arc<Mutex<CursorAberto>>> {
        let mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        mapa.get(id)
            .cloned()
            .ok_or_else(|| Erro::novo("o resultado desta consulta não está mais disponível"))
    }

    pub fn fechar(&self, id: &str) {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        mapa.remove(id);
    }

    /// Descarta todos os cursores de uma sessão encerrada.
    pub fn fechar_da_sessao(&self, sessao_id: &str) {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        mapa.retain(|id, _| !id.starts_with(&format!("{sessao_id}:")));
    }

    /// Traz mais um bloco de linhas de um cursor aberto.
    pub fn buscar_mais(&self, id: &str, quantidade: usize) -> Resultado<BlocoDeLinhas> {
        let cursor = self.obter(id)?;
        let mut cursor = cursor.lock().unwrap_or_else(|e| e.into_inner());
        let colunas = cursor.colunas.clone();
        let (linhas, tem_mais) = ler_bloco(&mut cursor.resultado, &colunas, quantidade)?;
        if !tem_mais {
            drop(cursor);
            self.fechar(id);
        }
        Ok(BlocoDeLinhas { linhas, tem_mais })
    }
}

/// Executa um comando.
///
/// `binds` são os valores informados para as `:variaveis` (requisito 15).
pub fn executar(
    sessao: &Sessao,
    cursores: &Cursores,
    comando: &sql_texto::Comando,
    binds: &[(String, String)],
    tamanho_do_fetch: usize,
    manter_cursor: bool,
) -> Resultado<ResultadoDaExecucao> {
    let saida_dbms_ligada = sessao.saida_dbms.load(std::sync::atomic::Ordering::Relaxed);
    // A conexão é pega uma vez só: se ela cair no meio, quem troca por outra é
    // quem chamou (ver `sessao::com_reconexao` e `sessao::sem_repetir`), e não
    // se muda de conexão entre o `execute` e o `commit` do mesmo comando.
    let conexao = sessao.oracle()?;
    let inicio = Instant::now();
    let sql = &comando.sql;
    // `SHOW ERRORS` é do SQL*Plus: mandado ao Oracle, volta `ORA-00900`.
    if let Some(pedido) = plsql::pedido_de_erros(sql) {
        return mostrar_erros(sessao, &conexao, comando, pedido, inicio);
    }
    let sql_executavel = sql_texto::preparar_para_execucao(sql);
    // Quando o texto enviado difere do digitado — `EXEC` virando bloco anônimo,
    // `*` solto sendo qualificado —, isso fica registrado. Uma consulta que roda
    // diferente do que está na tela não pode ser descoberta por adivinhação.
    if matches!(sql_executavel, std::borrow::Cow::Owned(_)) {
        registro::informacao(
            "executar_sql",
            format!("texto ajustado para o Oracle: {sql_executavel}"),
        );
    }
    let parametros: Vec<(&str, &dyn oracle::sql_type::ToSql)> = binds
        .iter()
        .map(|(nome, valor)| (nome.as_str(), valor as &dyn oracle::sql_type::ToSql))
        .collect();

    if sql_texto::e_consulta(&sql_executavel) {
        let resultado = conexao.query_named(&sql_executavel, &parametros)?;
        let colunas = descrever_colunas(resultado.column_info());
        let mut resultado = resultado;
        let (linhas, tem_mais) = ler_bloco(&mut resultado, &colunas, tamanho_do_fetch)?;
        let quantidade = linhas.len();

        let cursor_id = format!("{}:{}", sessao.id, identificador());
        // Em modo script não se guarda cursor: a saída é textual e ninguém vai
        // pedir mais linhas.
        let tem_mais = tem_mais && manter_cursor;
        if tem_mais {
            cursores.guardar(
                cursor_id.clone(),
                CursorAberto {
                    resultado,
                    colunas: colunas.clone(),
                },
            );
        }

        let duracao_ms = inicio.elapsed().as_millis() as u64;
        return Ok(ResultadoDaExecucao {
            comando: sql.clone(),
            linha_no_editor: comando.linha,
            duracao_ms,
            mensagem: format!(
                "{quantidade} linha(s) obtida(s) em {}{}",
                formatar_duracao(duracao_ms),
                if tem_mais { " — há mais" } else { "" }
            ),
            consulta: Some(ResultadoConsulta {
                cursor_id,
                colunas,
                linhas,
                tem_mais,
            }),
            linhas_afetadas: None,
            transacao: Transacao::Inalterada,
            saida_dbms: ler_saida_dbms(&conexao, saida_dbms_ligada),
            reconectado: false,
            erros_de_compilacao: Vec::new(),
        });
    }

    let statement = conexao.execute_named(&sql_executavel, &parametros)?;
    // O `CREATE` que compila com erro não volta como erro: o driver entrega o
    // `ORA-24344` como aviso, e o `execute` diz que deu certo. Sem esta leitura
    // o objeto ficava inválido no banco, a tela dizia "executado" e o script
    // seguia em frente como se nada fosse.
    let compilou_com_erro = conexao
        .last_warning()
        .and_then(|aviso| aviso.db_error().map(|banco| banco.code()))
        == Some(plsql::SUCESSO_COM_ERRO_DE_COMPILACAO);
    let afetadas = statement.row_count().unwrap_or(0);
    let efeito = sql_texto::efeito_no_oracle(&sql_executavel);
    let compilado = plsql::localizar_objeto(&sql_executavel);
    if let Some((objeto, _)) = &compilado {
        sessao.lembrar_compilado(objeto.clone());
    }
    let erros_de_compilacao = if compilou_com_erro {
        erros_do_comando(&conexao, comando, compilado.as_ref())
    } else {
        Vec::new()
    };
    let auto_commit = sessao
        .auto_commit
        .load(std::sync::atomic::Ordering::Relaxed);

    // Requisito 7: com auto-commit desligado, o que o comando mudou só vale
    // depois do botão de commit. O DDL é a exceção que o próprio Oracle abre:
    // ele confirma sozinho, antes e depois de rodar, e leva junto o que estava
    // pendente na aba — pedir commit depois dele seria pedir o que já foi
    // feito. `COMMIT` e `ROLLBACK` digitados valem o mesmo que os botões.
    let mexe_em_dados = efeito == EfeitoNoOracle::PodeDeixarPendente;
    if mexe_em_dados && auto_commit {
        conexao.commit()?;
    }
    let duracao_ms = inicio.elapsed().as_millis() as u64;
    let duracao = formatar_duracao(duracao_ms);
    let (transacao, mensagem) = match efeito {
        EfeitoNoOracle::ConfirmaSozinho => (
            Transacao::Encerrada,
            format!("Comando executado em {duracao} (DDL: o Oracle confirma sozinho)"),
        ),
        EfeitoNoOracle::Commit => (Transacao::Encerrada, format!("Commit executado em {duracao}")),
        EfeitoNoOracle::Rollback => (
            Transacao::Encerrada,
            format!("Rollback executado em {duracao}"),
        ),
        EfeitoNoOracle::NaoMexe => (Transacao::Inalterada, format!("Comando executado em {duracao}")),
        EfeitoNoOracle::PodeDeixarPendente if auto_commit => (
            Transacao::Encerrada,
            format!("{afetadas} linha(s) afetada(s) em {duracao} (commit automático)"),
        ),
        EfeitoNoOracle::PodeDeixarPendente => (
            Transacao::Aberta,
            format!("{afetadas} linha(s) afetada(s) em {duracao}{AVISO_DE_TRANSACAO_ABERTA}"),
        ),
    };
    let mensagem = if compilou_com_erro {
        let objeto = compilado.as_ref().map(|(objeto, _)| objeto);
        let mensagem = mensagem_de_compilacao(objeto, &erros_de_compilacao, &duracao);
        registro::aviso("executar_sql", &mensagem);
        mensagem
    } else {
        mensagem
    };

    Ok(ResultadoDaExecucao {
        comando: sql.clone(),
        linha_no_editor: comando.linha,
        duracao_ms,
        mensagem,
        consulta: None,
        // Contar linhas só faz sentido para o que mexe em dados: o DDL e o
        // `commit` voltam zero, que não diz nada.
        linhas_afetadas: mexe_em_dados.then_some(afetadas),
        transacao,
        saida_dbms: ler_saida_dbms(&conexao, saida_dbms_ligada),
        reconectado: false,
        erros_de_compilacao,
    })
}

/// Diz em que linha do texto executado o erro caiu (requisito 18).
///
/// O Oracle informa o deslocamento **dentro do comando**, em bytes, e a tela não
/// sabe onde cada comando começa: num script de vários comandos, aquele número
/// tomado como posição no editor levava o cursor para o comando errado.
pub fn situar_erro(mut erro: Erro, comando: &sql_texto::Comando) -> Erro {
    if erro.linha.is_some() {
        return erro;
    }
    // O deslocamento só vale quando o texto enviado é o digitado: o `EXEC` que
    // virou bloco anônimo conta posições de outro texto.
    let enviado_como_esta = matches!(
        sql_texto::preparar_para_execucao(&comando.sql),
        std::borrow::Cow::Borrowed(_)
    );
    let dentro = match erro.posicao {
        Some(posicao) if enviado_como_esta => {
            let fim = (posicao as usize).min(comando.sql.len());
            comando.sql.as_bytes()[..fim]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
        }
        _ => 0,
    };
    erro.linha = Some((comando.linha + dentro) as u32);
    erro
}

/// Os erros do objeto que o `CREATE` deixou inválido, com a linha contada no
/// texto executado.
fn erros_do_comando(
    conexao: &oracle::Connection,
    comando: &sql_texto::Comando,
    compilado: Option<&(ObjetoPlsql, usize)>,
) -> Vec<ErroDeCompilacao> {
    let Some((objeto, linhas_antes)) = compilado else {
        return Vec::new();
    };
    match plsql::erros_do_objeto(conexao, objeto) {
        Ok(mut erros) => {
            // O Oracle conta a partir da linha do tipo, que fica `linhas_antes`
            // abaixo de onde o comando começa.
            let deslocamento = (comando.linha + linhas_antes).saturating_sub(1) as u32;
            for erro in &mut erros {
                erro.linha += deslocamento;
            }
            erros
        }
        // O `CREATE` já valeu: não conseguir a lista não pode esconder isso.
        Err(falha) => {
            registro::aviso(
                "executar_sql",
                format!("erros de {} não lidos: {falha}", objeto.descricao()),
            );
            Vec::new()
        }
    }
}

/// O que dizer de um `CREATE` que o Oracle gravou com erro de compilação.
fn mensagem_de_compilacao(
    objeto: Option<&ObjetoPlsql>,
    erros: &[ErroDeCompilacao],
    duracao: &str,
) -> String {
    let descricao = objeto.map_or_else(|| "Objeto".to_string(), ObjetoPlsql::descricao);
    let graves = erros.iter().filter(|erro| !erro.aviso).count();
    match (graves, erros.len()) {
        // Com o `PLSQL_WARNINGS` ligado, o aviso sozinho não invalida nada.
        (0, avisos) if avisos > 0 => {
            format!("{descricao} compilado com {avisos} aviso(s) em {duracao}")
        }
        (0, _) => format!("{descricao} compilado com erros em {duracao}: o objeto ficou inválido"),
        (graves, _) => {
            format!("{descricao} compilado com {graves} erro(s) em {duracao}: o objeto ficou inválido")
        }
    }
}

/// Atende o `SHOW ERRORS` do SQL*Plus: os erros do objeto, com linha e coluna
/// contadas nele, como o SQL*Plus e o SQL Developer mostram.
fn mostrar_erros(
    sessao: &Sessao,
    conexao: &oracle::Connection,
    comando: &sql_texto::Comando,
    pedido: PedidoDeErros,
    inicio: Instant,
) -> Resultado<ResultadoDaExecucao> {
    let objeto = match pedido {
        PedidoDeErros::Do(objeto) => Some(objeto),
        PedidoDeErros::DoUltimo => sessao.ultimo_compilado(),
        PedidoDeErros::Invalido => {
            return Err(Erro::novo(
                "SHOW ERRORS pede o tipo e o nome do objeto (SHOW ERRORS PROCEDURE DONO.NOME), \
                 ou nada, para o último compilado nesta aba",
            ));
        }
    };
    let (mensagem, consulta) = match objeto {
        None => ("Nenhum objeto foi compilado nesta aba".to_string(), None),
        Some(objeto) => {
            let erros = plsql::erros_do_objeto(conexao, &objeto)?;
            if erros.is_empty() {
                (format!("Sem erros de compilação em {}", objeto.descricao()), None)
            } else {
                (
                    format!("{} erro(s) de compilação em {}", erros.len(), objeto.descricao()),
                    Some(tabela_de_erros(&erros)),
                )
            }
        }
    };

    Ok(ResultadoDaExecucao {
        comando: comando.sql.clone(),
        linha_no_editor: comando.linha,
        duracao_ms: inicio.elapsed().as_millis() as u64,
        consulta,
        linhas_afetadas: None,
        transacao: Transacao::Inalterada,
        mensagem,
        saida_dbms: Vec::new(),
        reconectado: false,
        erros_de_compilacao: Vec::new(),
    })
}

/// A lista no formato do SQL*Plus: `LINE/COL` e `ERROR`.
fn tabela_de_erros(erros: &[ErroDeCompilacao]) -> ResultadoConsulta {
    let coluna = |nome: &str| ColunaResultado {
        nome: nome.to_string(),
        tipo: "VARCHAR2".to_string(),
        numerica: false,
    };
    ResultadoConsulta {
        // Nada fica aberto no banco: a lista inteira já veio.
        cursor_id: String::new(),
        colunas: vec![coluna("LINE/COL"), coluna("ERROR")],
        linhas: erros
            .iter()
            .map(|erro| {
                vec![
                    Some(format!("{}/{}", erro.linha, erro.coluna)),
                    Some(erro.texto.clone()),
                ]
            })
            .collect(),
        tem_mais: false,
    }
}

/// Lê até `quantidade` linhas, dizendo se o cursor ainda tem mais.
fn ler_bloco(
    resultado: &mut ResultSet<'static, Row>,
    colunas: &[ColunaResultado],
    quantidade: usize,
) -> Resultado<(Vec<Linha>, bool)> {
    let mut linhas = Vec::with_capacity(quantidade.min(1000));
    for _ in 0..quantidade {
        match resultado.next() {
            Some(Ok(linha)) => linhas.push(converter_linha(&linha, colunas)),
            Some(Err(erro)) => return Err(erro.into()),
            None => return Ok((linhas, false)),
        }
    }
    // Só se sabe que há mais quando o cursor ainda não acabou; o teste seguro é
    // ter enchido o bloco pedido.
    Ok((linhas, quantidade > 0))
}

fn descrever_colunas(info: &[oracle::ColumnInfo]) -> Vec<ColunaResultado> {
    info.iter()
        .map(|coluna| ColunaResultado {
            nome: coluna.name().to_string(),
            tipo: coluna.oracle_type().to_string(),
            numerica: e_numerica(coluna.oracle_type()),
        })
        .collect()
}

fn e_numerica(tipo: &OracleType) -> bool {
    matches!(
        tipo,
        OracleType::Number(..)
            | OracleType::Float(..)
            | OracleType::BinaryFloat
            | OracleType::BinaryDouble
            | OracleType::Int64
            | OracleType::UInt64
    )
}

fn converter_linha(linha: &Row, colunas: &[ColunaResultado]) -> Linha {
    (0..colunas.len())
        .map(|indice| valor_para_texto(linha, indice))
        .collect()
}

/// Converte um valor para o texto que aparece no grid.
///
/// Datas e números saem no padrão brasileiro (requisito 21). Conteúdo binário
/// vira uma descrição, porque não há como mostrá-lo numa célula.
fn valor_para_texto(linha: &Row, indice: usize) -> Option<String> {
    let tipo = linha.column_info().get(indice)?.oracle_type().clone();

    match tipo {
        OracleType::Date
        | OracleType::Timestamp(_)
        | OracleType::TimestampTZ(_)
        | OracleType::TimestampLTZ(_) => match linha.get::<usize, Option<Timestamp>>(indice) {
            Ok(Some(momento)) => Some(formatar_data(&momento)),
            Ok(None) => None,
            Err(_) => como_texto(linha, indice),
        },
        OracleType::BLOB | OracleType::LongRaw | OracleType::Raw(_) | OracleType::BFILE => {
            match linha.get::<usize, Option<Vec<u8>>>(indice) {
                Ok(Some(bytes)) => Some(format!("(binário, {} bytes)", bytes.len())),
                Ok(None) => None,
                Err(_) => Some("(binário)".to_string()),
            }
        }
        _ if e_numerica(&tipo) => como_texto(linha, indice).map(|texto| decimal_brasileiro(&texto)),
        _ => como_texto(linha, indice),
    }
}

fn como_texto(linha: &Row, indice: usize) -> Option<String> {
    match linha.get::<usize, Option<String>>(indice) {
        Ok(valor) => valor,
        Err(_) => Some("(valor não convertido)".to_string()),
    }
}

/// `1234.56` → `1234,56`. Sem separador de milhar, como no SQL Developer.
fn decimal_brasileiro(numero: &str) -> String {
    numero.replace('.', ",")
}

fn formatar_data(momento: &Timestamp) -> String {
    let data = format!(
        "{:02}/{:02}/{:04}",
        momento.day(),
        momento.month(),
        momento.year()
    );
    if momento.hour() == 0 && momento.minute() == 0 && momento.second() == 0 {
        return data;
    }
    format!(
        "{data} {:02}:{:02}:{:02}",
        momento.hour(),
        momento.minute(),
        momento.second()
    )
}

// --- Saída DBMS (requisito 8) ----------------------------------------------

/// Liga o `DBMS_OUTPUT` da sessão com o tamanho de buffer pedido.
pub fn ligar_saida_dbms(conexao: &oracle::Connection, tamanho_do_buffer: u32) -> Resultado<()> {
    conexao.execute(
        "begin dbms_output.enable(:tamanho); end;",
        &[&(tamanho_do_buffer as i64)],
    )?;
    Ok(())
}

pub fn desligar_saida_dbms(conexao: &oracle::Connection) -> Resultado<()> {
    conexao.execute("begin dbms_output.disable; end;", &[])?;
    Ok(())
}

/// Esvazia o buffer do `DBMS_OUTPUT` acumulado desde a última leitura.
fn ler_saida_dbms(conexao: &oracle::Connection, ligada: bool) -> Vec<String> {
    if !ligada {
        return Vec::new();
    }
    let mut linhas = Vec::new();
    for _ in 0..MAXIMO_DE_LINHAS_DBMS {
        let Ok(mut statement) = conexao
            .statement("begin dbms_output.get_line(:linha, :estado); end;")
            .build()
        else {
            break;
        };
        if statement
            .execute(&[
                &oracle::sql_type::OracleType::Varchar2(32767),
                &oracle::sql_type::OracleType::Int64,
            ])
            .is_err()
        {
            break;
        }
        let estado = statement.bind_value::<usize, i64>(2).unwrap_or(1);
        if estado != 0 {
            break;
        }
        match statement.bind_value::<usize, Option<String>>(1) {
            Ok(Some(texto)) => linhas.push(texto),
            Ok(None) => linhas.push(String::new()),
            Err(_) => break,
        }
    }
    linhas
}

fn identificador() -> String {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("c{:x}{:x}", agora.as_secs(), agora.subsec_nanos())
}

/** Duração apresentada ao usuário: segundos e, a partir de 60 s, minutos. */
pub fn formatar_duracao(duracao_ms: u64) -> String {
    if duracao_ms < 60_000 {
        return formatar_segundos(duracao_ms);
    }

    let minutos = duracao_ms / 60_000;
    let restante = duracao_ms % 60_000;
    if restante == 0 {
        format!("{minutos} min")
    } else {
        format!("{minutos} min {}", formatar_segundos(restante))
    }
}

fn formatar_segundos(duracao_ms: u64) -> String {
    let inteiros = duracao_ms / 1_000;
    let fracao = duracao_ms % 1_000;
    if fracao == 0 {
        return format!("{inteiros} s");
    }

    let decimais = format!("{fracao:03}").trim_end_matches('0').to_string();
    format!("{inteiros},{decimais} s")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numero_sai_com_virgula_decimal() {
        assert_eq!(decimal_brasileiro("1234.56"), "1234,56");
        assert_eq!(decimal_brasileiro("42"), "42");
        assert_eq!(decimal_brasileiro("-0.5"), "-0,5");
    }

    #[test]
    fn data_sem_hora_nao_mostra_hora() {
        let momento = Timestamp::new(2026, 7, 29, 0, 0, 0, 0).unwrap();
        assert_eq!(formatar_data(&momento), "29/07/2026");
    }

    #[test]
    fn data_com_hora_mostra_hora() {
        let momento = Timestamp::new(2026, 7, 29, 14, 5, 9, 0).unwrap();
        assert_eq!(formatar_data(&momento), "29/07/2026 14:05:09");
    }

    fn comando(sql: &str, linha: usize) -> sql_texto::Comando {
        sql_texto::Comando {
            sql: sql.to_string(),
            linha,
        }
    }

    #[test]
    fn erro_sem_posicao_cai_na_linha_do_comando() {
        // O segundo pedaço do GRAV_LOG partido: o Oracle não diz posição.
        let erro = situar_erro(
            Erro::novo("ORA-00900: invalid SQL statement"),
            &comando("V_MSG    VARCHAR2(4000)", 13),
        );
        assert_eq!(erro.linha, Some(13));
    }

    #[test]
    fn a_posicao_do_oracle_conta_dentro_do_comando() {
        let erro = Erro {
            posicao: Some(16),
            ..Erro::novo("ORA-00942: table or view does not exist")
        };
        let erro = situar_erro(erro, &comando("select a\n  from nao_existe", 30));
        assert_eq!(erro.linha, Some(31));
    }

    #[test]
    fn exec_nao_usa_a_posicao_do_bloco_anonimo() {
        let erro = Erro {
            posicao: Some(12),
            ..Erro::novo("ORA-06550")
        };
        assert_eq!(situar_erro(erro, &comando("exec p(\n 1)", 5)).linha, Some(5));
    }

    fn erro_de_compilacao(aviso: bool) -> ErroDeCompilacao {
        ErroDeCompilacao {
            linha: 8,
            coluna: 29,
            texto: "PLS-00103: Encountered the symbol \"end-of-file\"".to_string(),
            aviso,
        }
    }

    #[test]
    fn compilacao_com_erro_diz_que_o_objeto_ficou_invalido() {
        let objeto = ObjetoPlsql {
            tipo: "PROCEDURE".to_string(),
            dono: Some("EXEMPLO".to_string()),
            nome: "GRAV_LOG".to_string(),
        };
        assert_eq!(
            mensagem_de_compilacao(Some(&objeto), &[erro_de_compilacao(false)], "0,1 s"),
            "PROCEDURE EXEMPLO.GRAV_LOG compilado com 1 erro(s) em 0,1 s: o objeto ficou inválido"
        );
        // Sem a lista, a mensagem ainda não pode dizer que deu certo.
        assert_eq!(
            mensagem_de_compilacao(None, &[], "0,1 s"),
            "Objeto compilado com erros em 0,1 s: o objeto ficou inválido"
        );
        assert_eq!(
            mensagem_de_compilacao(Some(&objeto), &[erro_de_compilacao(true)], "0,1 s"),
            "PROCEDURE EXEMPLO.GRAV_LOG compilado com 1 aviso(s) em 0,1 s"
        );
    }

    #[test]
    fn show_errors_sai_no_formato_do_sql_plus() {
        let tabela = tabela_de_erros(&[erro_de_compilacao(false)]);
        let nomes: Vec<&str> = tabela.colunas.iter().map(|c| c.nome.as_str()).collect();
        assert_eq!(nomes, ["LINE/COL", "ERROR"]);
        assert_eq!(tabela.linhas[0][0].as_deref(), Some("8/29"));
        assert!(!tabela.tem_mais);
    }

    #[test]
    fn duracao_da_interface_usa_segundos_ou_minutos() {
        assert_eq!(formatar_duracao(0), "0 s");
        assert_eq!(formatar_duracao(35), "0,035 s");
        assert_eq!(formatar_duracao(1_500), "1,5 s");
        assert_eq!(formatar_duracao(59_999), "59,999 s");
        assert_eq!(formatar_duracao(60_000), "1 min");
        assert_eq!(formatar_duracao(61_500), "1 min 1,5 s");
        assert_eq!(formatar_duracao(120_000), "2 min");
    }
}
