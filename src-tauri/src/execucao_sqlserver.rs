//! Execução de SQL no SQL Server (requisitos 5, 6, 7, 10, 14, 15, 17 e 21).
//!
//! O que a [`crate::execucao`] faz pelo Oracle, este módulo faz pelo SQL Server.
//! São dois arquivos e não um porque não há nada em comum abaixo da superfície:
//! o Oracle guarda um `ResultSet` num mapa e busca dele quando a aba pede mais
//! linhas; aqui o resultado é um **fluxo que empresta o cliente**, e o cliente
//! mora numa thread (ver [`crate::sqlserver`]). O tipo que sobe para a tela é o
//! mesmo, e isso é o que importa: a aba de consulta não sabe com que banco está
//! falando.
//!
//! ## O lote é o `GO`, não o `;`
//!
//! No T-SQL o `;` separa instruções **dentro** do lote; quem encerra o lote é a
//! linha com `GO`, marca do cliente que nem chega ao servidor. Cortar no `;`
//! partiria ao meio todo `IF … BEGIN … END` e toda variável declarada e usada
//! logo abaixo. A separação está em [`crate::sql_texto::separar_lotes`].
//!
//! Um lote pode devolver **vários conjuntos de resultados** — `select a; select
//! b;` é um lote só, com dois. O cursor fica no **primeiro** deles: é o único
//! que dá para deixar aberto, porque alcançar o segundo obriga a ler o primeiro
//! até o fim, e aí não haveria fetch parcial nenhum (requisito 10).

use std::time::Instant;

use futures_util::TryStreamExt;
use tiberius::{ColumnData, ColumnType, QueryItem, QueryStream, Row, ToSql};

use crate::erro::{Erro, Resultado};
use crate::execucao::{
    formatar_duracao, ColunaResultado, Linha, ResultadoConsulta, ResultadoDaExecucao,
    Transacao, AVISO_DE_TRANSACAO_ABERTA,
};
use crate::registro;
use crate::sql_texto::{self, Comando};
use crate::sqlserver::Cliente;

/// O fluxo de um lote que parou no meio, guardado para a aba pedir mais linhas.
///
/// Enquanto ele existe o cliente está emprestado, e a sessão não faz mais nada:
/// no TDS uma conexão atende um pedido de cada vez, e o resultado pendente tem
/// de sair da frente antes do próximo. É por isso que o autocomplete de uma aba
/// de SQL Server não usa a sessão dela, e sim a da conexão (requisito 3).
pub struct Cursor<'a> {
    fluxo: QueryStream<'a>,
}

impl Cursor<'_> {
    /// Mais um bloco de linhas do conjunto em que o cursor parou.
    ///
    /// O conjunto acabou quando o fluxo termina **ou** quando vem a descrição de
    /// outro conjunto: o segundo `select` do lote não continua o grid do
    /// primeiro.
    pub async fn buscar_mais(&mut self, quantidade: usize) -> Resultado<(Vec<Linha>, bool)> {
        let mut linhas = Vec::with_capacity(quantidade.min(1000));
        while linhas.len() < quantidade {
            match self.fluxo.try_next().await? {
                Some(QueryItem::Row(linha)) => linhas.push(converter_linha(&linha)),
                Some(QueryItem::Metadata(_)) | None => return Ok((linhas, false)),
            }
        }
        // Como no Oracle: só se sabe que há mais por ter enchido o bloco pedido.
        Ok((linhas, quantidade > 0))
    }

    /// Lê o que sobrou do fluxo, até o fim.
    ///
    /// É o preço de largar um grid pela metade: no TDS o resultado pendente
    /// precisa sair da frente antes do próximo pedido, e sair da frente é lê-lo.
    /// Fazer isso aqui, e não deixar o driver fazer sozinho no comando seguinte,
    /// é o que permite pôr um prazo nisso — ver `ESPERA_PARA_ESVAZIAR`.
    pub async fn esvaziar(&mut self) -> Resultado<()> {
        while self.fluxo.try_next().await?.is_some() {}
        Ok(())
    }
}

/// O que a execução de um lote produziu.
pub struct Execucao<'a> {
    pub resultados: Vec<ResultadoDaExecucao>,
    /// Presente quando o fluxo parou antes do fim.
    ///
    /// Vem preenchido mesmo quando a aba **não** vai pedir mais linhas — no modo
    /// script, por exemplo. Nesse caso ele não é cursor de ninguém, e quem chama
    /// tem de descartá-lo: o resultado pendente precisa sair da frente antes do
    /// próximo lote, e é quem chama que sabe pôr prazo nisso.
    pub cursor: Option<Cursor<'a>>,
    /// O lote não devolveu conjunto de resultados nenhum — é comando, e não
    /// consulta. Quantas linhas ele afetou e se deixou transação aberta não vêm
    /// no fluxo, e quem chama completa isso com [`ler_depois_do_comando`]
    /// depois de largar o empréstimo.
    pub precisa_contagem: bool,
}

/// Executa um lote e lê o primeiro bloco de linhas.
///
/// A leitura **sempre** para no primeiro bloco, como no Oracle (requisito 10);
/// `manter_cursor` decide só se a aba pode pedir o resto. No modo script ele é
/// falso: ali a saída é textual, mostra o primeiro bloco e mais nada — ler a
/// tabela inteira para jogá-la fora seria a rede toda por nada.
pub async fn executar<'a>(
    cliente: &'a mut Cliente,
    lote: &Comando,
    binds: &[(String, String)],
    tamanho_do_fetch: usize,
    manter_cursor: bool,
) -> Resultado<Execucao<'a>> {
    let inicio = Instant::now();
    let bloco = tamanho_do_fetch.max(1);

    let nomes: Vec<String> = binds.iter().map(|(nome, _)| nome.clone()).collect();
    let valores: Vec<&str> = binds.iter().map(|(_, valor)| valor.as_str()).collect();
    let parametros: Vec<&dyn ToSql> = valores.iter().map(|v| v as &dyn ToSql).collect();

    // Sem bind o lote vai como lote mesmo (`simple_query`), e não embrulhado em
    // `sp_executesql`. A diferença aparece no `USE`: dentro do `sp_executesql`
    // ele valeria só ali dentro e a aba continuaria no banco de antes, o que
    // seria mentir para quem digitou `use Vendas`.
    let mut fluxo = if parametros.is_empty() {
        cliente.simple_query(lote.sql.clone()).await?
    } else {
        let sql = sql_texto::substituir_binds_sqlserver(&lote.sql, &nomes);
        registro::informacao("executar_sql", format!("binds traduzidos: {sql}"));
        cliente.query(sql, &parametros).await?
    };

    let mut resultados: Vec<ResultadoDaExecucao> = Vec::new();
    let mut colunas: Option<Vec<ColunaResultado>> = None;
    let mut linhas: Vec<Linha> = Vec::new();
    let mut lidas: u64 = 0;
    let mut cheio = false;

    loop {
        // O erro do fluxo **não** sobe direto: no T-SQL o servidor manda os
        // conjuntos de resultados e só depois a falha do lote, e um `?` aqui
        // jogaria fora tudo o que já veio. O que chegou vai junto com o erro.
        let item = match fluxo.try_next().await {
            Ok(item) => item,
            Err(falha) => {
                if let Some(descricao) = colunas.take() {
                    resultados.push(resultado_de_consulta(
                        lote,
                        descricao,
                        std::mem::take(&mut linhas),
                        lidas,
                        false,
                        String::new(),
                        inicio,
                    ));
                }
                return Err(Erro::from(falha).apos(resultados));
            }
        };
        match item {
            Some(QueryItem::Metadata(descricao)) => {
                if let Some(anteriores) = colunas.take() {
                    resultados.push(resultado_de_consulta(
                        lote,
                        anteriores,
                        std::mem::take(&mut linhas),
                        lidas,
                        false,
                        String::new(),
                        inicio,
                    ));
                }
                colunas = Some(descrever_colunas(descricao.columns()));
                lidas = 0;
            }
            Some(QueryItem::Row(linha)) => {
                let descricao = colunas.get_or_insert_with(|| descrever_colunas(linha.columns()));
                lidas += 1;
                if linhas.len() < bloco {
                    linhas.push(converter_linha(&linha));
                }
                // Só o primeiro conjunto do lote pode ficar aberto: chegar ao
                // segundo exige ler o primeiro inteiro.
                if resultados.is_empty() && linhas.len() >= bloco {
                    let colunas = descricao.clone();
                    resultados.push(resultado_de_consulta(
                        lote,
                        colunas,
                        std::mem::take(&mut linhas),
                        lidas,
                        manter_cursor,
                        String::new(),
                        inicio,
                    ));
                    cheio = true;
                    break;
                }
            }
            None => {
                if let Some(descricao) = colunas.take() {
                    resultados.push(resultado_de_consulta(
                        lote,
                        descricao,
                        std::mem::take(&mut linhas),
                        lidas,
                        false,
                        String::new(),
                        inicio,
                    ));
                }
                break;
            }
        }
    }

    if cheio {
        return Ok(Execucao {
            resultados,
            cursor: Some(Cursor { fluxo }),
            precisa_contagem: false,
        });
    }

    let precisa_contagem = resultados.is_empty();
    if precisa_contagem {
        // Deixa a moldura pronta; a contagem entra depois, quando o cliente
        // estiver livre.
        resultados.push(ResultadoDaExecucao {
            comando: lote.sql.clone(),
            linha_no_editor: lote.linha,
            duracao_ms: inicio.elapsed().as_millis() as u64,
            consulta: None,
            linhas_afetadas: Some(0),
            transacao: Transacao::Inalterada,
            mensagem: String::new(),
            saida_dbms: Vec::new(),
            reconectado: false,
            erros_de_compilacao: Vec::new(),
        });
    }
    Ok(Execucao {
        resultados,
        cursor: None,
        precisa_contagem,
    })
}

/// O que a sessão conta depois de um comando.
pub struct DepoisDoComando {
    /// Quantas linhas o último comando do lote afetou.
    pub afetadas: Option<u64>,
    /// Quantas transações ficaram abertas na sessão (`@@TRANCOUNT`).
    pub transacoes: Option<i32>,
}

/// Pergunta à sessão o que o último comando do lote deixou.
///
/// O `@@ROWCOUNT` é da **sessão**, e não do lote: lido no lote seguinte, ele
/// ainda é o da última instrução executada. É a única forma de saber o número,
/// porque o driver descarta as marcas de fim de comando — que é justamente onde
/// o servidor manda essa contagem — ao devolver o fluxo de linhas.
///
/// O `@@TRANCOUNT` vem na mesma ida: é o servidor dizendo se sobrou transação,
/// e não o Cururu adivinhando pelo texto. A consulta não abre transação nem
/// com as implícitas ligadas, porque não lê tabela nenhuma.
pub async fn ler_depois_do_comando(cliente: &mut Cliente) -> DepoisDoComando {
    let linha = ler_contadores(cliente).await;
    let contador = |indice: usize| linha.as_ref().and_then(|linha| linha.get::<i32, _>(indice));
    DepoisDoComando {
        afetadas: contador(0).map(|valor| valor.max(0) as u64),
        transacoes: contador(1),
    }
}

async fn ler_contadores(cliente: &mut Cliente) -> Option<Row> {
    cliente
        .simple_query("select @@rowcount, @@trancount")
        .await
        .ok()?
        .into_row()
        .await
        .ok()?
}

/// Preenche a mensagem de um comando depois de a contagem chegar.
///
/// Aqui o aviso de transação aberta pesa mais que no Oracle: no SQL Server o
/// DDL é transacional, e um `alter procedure` que a mensagem deu por executado
/// se perde inteiro quando a aba fecha sem commit. Quem diz se sobrou
/// transação é o `@@TRANCOUNT`, e não o tipo do comando: com as implícitas
/// ligadas o DDL abre transação como o DML, um `commit` digitado fecha, e um
/// `begin tran` abre mesmo com o auto-commit ligado. Sem a resposta do
/// servidor, vale o que o auto-commit promete.
pub fn concluir_comando(
    resultado: &mut ResultadoDaExecucao,
    depois: DepoisDoComando,
    auto_commit: bool,
) {
    let duracao = formatar_duracao(resultado.duracao_ms);
    let aberta = depois
        .transacoes
        .map_or(!auto_commit, |transacoes| transacoes > 0);
    let (transacao, desfecho) = if aberta {
        (Transacao::Aberta, AVISO_DE_TRANSACAO_ABERTA)
    } else if auto_commit {
        (Transacao::Encerrada, " (commit automático)")
    } else {
        (Transacao::Encerrada, "")
    };
    resultado.transacao = transacao;
    match depois.afetadas {
        Some(quantidade) => {
            resultado.linhas_afetadas = Some(quantidade);
            resultado.mensagem =
                format!("{quantidade} linha(s) afetada(s) em {duracao}{desfecho}");
        }
        None => {
            resultado.linhas_afetadas = None;
            resultado.mensagem = format!("Comando executado em {duracao}{desfecho}");
        }
    }
}

/// Monta o resultado de um conjunto de linhas.
fn resultado_de_consulta(
    lote: &Comando,
    colunas: Vec<ColunaResultado>,
    linhas: Vec<Linha>,
    lidas: u64,
    tem_mais: bool,
    cursor_id: String,
    inicio: Instant,
) -> ResultadoDaExecucao {
    let duracao_ms = inicio.elapsed().as_millis() as u64;
    // Depois do primeiro conjunto o lote não deixa cursor, e o que passa do
    // bloco é lido e descartado — a leitura é obrigatória para alcançar o
    // conjunto seguinte. O grid mostraria menos linhas do que a contagem
    // promete, então a contagem diz quantas ele tem.
    let trazidas = linhas.len() as u64;
    let sobra = if tem_mais {
        " — há mais".to_string()
    } else if trazidas < lidas {
        format!(" — mostrando as primeiras {trazidas}")
    } else {
        String::new()
    };
    ResultadoDaExecucao {
        comando: lote.sql.clone(),
        linha_no_editor: lote.linha,
        duracao_ms,
        mensagem: format!(
            "{lidas} linha(s) obtida(s) em {}{sobra}",
            formatar_duracao(duracao_ms)
        ),
        consulta: Some(ResultadoConsulta {
            cursor_id,
            colunas,
            linhas,
            tem_mais,
        }),
        linhas_afetadas: None,
        transacao: Transacao::Inalterada,
        saida_dbms: Vec::new(),
        reconectado: false,
        erros_de_compilacao: Vec::new(),
    }
}

// --- Colunas e valores ------------------------------------------------------

fn descrever_colunas(colunas: &[tiberius::Column]) -> Vec<ColunaResultado> {
    colunas
        .iter()
        .map(|coluna| ColunaResultado {
            nome: coluna.name().to_string(),
            tipo: nome_do_tipo(coluna.column_type()).to_string(),
            numerica: e_numerica(coluna.column_type()),
        })
        .collect()
}

/// O tipo como o T-SQL o escreve, para o cabeçalho do grid.
///
/// As formas `…n` (`Intn`, `Bitn`, `Floatn`, `Decimaln`) são a codificação de
/// comprimento variável que o TDS usa para a coluna que aceita nulo; o tipo
/// declarado continua sendo o mesmo, e é o nome dele que vai para a tela.
fn nome_do_tipo(tipo: ColumnType) -> &'static str {
    match tipo {
        ColumnType::Null => "null",
        ColumnType::Bit | ColumnType::Bitn => "bit",
        ColumnType::Int1 => "tinyint",
        ColumnType::Int2 => "smallint",
        ColumnType::Int4 | ColumnType::Intn => "int",
        ColumnType::Int8 => "bigint",
        ColumnType::Float4 => "real",
        ColumnType::Float8 | ColumnType::Floatn => "float",
        ColumnType::Money | ColumnType::Money4 => "money",
        ColumnType::Decimaln | ColumnType::Numericn => "numeric",
        ColumnType::Datetime | ColumnType::Datetimen => "datetime",
        ColumnType::Datetime4 => "smalldatetime",
        ColumnType::Daten => "date",
        ColumnType::Timen => "time",
        ColumnType::Datetime2 => "datetime2",
        ColumnType::DatetimeOffsetn => "datetimeoffset",
        ColumnType::Guid => "uniqueidentifier",
        ColumnType::BigVarBin => "varbinary",
        ColumnType::BigBinary => "binary",
        ColumnType::BigVarChar => "varchar",
        ColumnType::BigChar => "char",
        ColumnType::NVarchar => "nvarchar",
        ColumnType::NChar => "nchar",
        ColumnType::Xml => "xml",
        ColumnType::Udt => "udt",
        ColumnType::Text => "text",
        ColumnType::Image => "image",
        ColumnType::NText => "ntext",
        ColumnType::SSVariant => "sql_variant",
    }
}

fn e_numerica(tipo: ColumnType) -> bool {
    matches!(
        tipo,
        ColumnType::Int1
            | ColumnType::Int2
            | ColumnType::Int4
            | ColumnType::Int8
            | ColumnType::Intn
            | ColumnType::Float4
            | ColumnType::Float8
            | ColumnType::Floatn
            | ColumnType::Money
            | ColumnType::Money4
            | ColumnType::Decimaln
            | ColumnType::Numericn
    )
}

fn converter_linha(linha: &Row) -> Linha {
    linha.cells().map(|(_, valor)| texto_do_valor(valor)).collect()
}

/// Converte um valor para o texto que aparece na célula.
///
/// `None` é `NULL` e o grid o distingue de texto vazio (requisito 20). Datas e
/// números saem no padrão brasileiro (requisito 21), e conteúdo binário vira uma
/// descrição, porque não há como mostrá-lo numa célula.
fn texto_do_valor(valor: &ColumnData<'_>) -> Option<String> {
    match valor {
        ColumnData::U8(v) => v.map(|n| n.to_string()),
        ColumnData::I16(v) => v.map(|n| n.to_string()),
        ColumnData::I32(v) => v.map(|n| n.to_string()),
        ColumnData::I64(v) => v.map(|n| n.to_string()),
        ColumnData::F32(v) => v.map(|n| decimal_brasileiro(&n.to_string())),
        ColumnData::F64(v) => v.map(|n| decimal_brasileiro(&n.to_string())),
        // O SSMS mostra `1` e `0` numa coluna `bit`, e não `true`/`false`: é o
        // que se escreve de volta num `where`.
        ColumnData::Bit(v) => v.map(|ligado| if ligado { "1" } else { "0" }.to_string()),
        ColumnData::String(v) => v.as_ref().map(|texto| texto.to_string()),
        ColumnData::Guid(v) => v.map(|id| id.to_string().to_uppercase()),
        ColumnData::Binary(v) => v
            .as_ref()
            .map(|bytes| format!("(binário, {} bytes)", bytes.len())),
        ColumnData::Numeric(v) => v.map(decimal_de_numeric),
        ColumnData::Xml(v) => v.as_ref().map(|xml| xml.to_string()),
        ColumnData::DateTime(v) => v.map(|momento| {
            // Dias desde 1900-01-01 e trezentos avos de segundo.
            formatar_momento(
                dias_desde_1900(i64::from(momento.days())),
                i64::from(momento.seconds_fragments()) / 300,
            )
        }),
        ColumnData::SmallDateTime(v) => v.map(|momento| {
            formatar_momento(
                dias_desde_1900(i64::from(momento.days())),
                i64::from(momento.seconds_fragments()) * 60,
            )
        }),
        ColumnData::Date(v) => v.map(|data| formatar_data(dias_desde_ano_um(i64::from(data.days())))),
        ColumnData::Time(v) => v.map(|hora| formatar_hora(segundos_do_tempo(&hora))),
        ColumnData::DateTime2(v) => v.map(|momento| {
            formatar_momento(
                dias_desde_ano_um(i64::from(momento.date().days())),
                segundos_do_tempo(&momento.time()),
            )
        }),
        ColumnData::DateTimeOffset(v) => v.map(|momento| {
            // O valor vem em UTC com o deslocamento à parte; a tela mostra a hora
            // do fuso gravado, que é a que o usuário escreveu.
            let deslocamento = i64::from(momento.offset()) * 60;
            let dias = dias_desde_ano_um(i64::from(momento.datetime2().date().days()));
            let segundos = segundos_do_tempo(&momento.datetime2().time()) + deslocamento;
            let (dias, segundos) = normalizar(dias, segundos);
            formatar_momento(dias, segundos)
        }),
    }
}

/// `1234.56` → `1234,56`. Sem separador de milhar, como no SQL Developer.
fn decimal_brasileiro(numero: &str) -> String {
    numero.replace('.', ",")
}

/// O `Display` do driver escreve `-1.-50` para um decimal negativo, e `5.` para
/// um sem casas — por isso o número é montado aqui.
fn decimal_de_numeric(numero: tiberius::numeric::Numeric) -> String {
    let escala = numero.scale() as usize;
    let inteiro = numero.int_part();
    let decimal = numero.dec_part();
    let sinal = if numero.value() < 0 { "-" } else { "" };
    if escala == 0 {
        return format!("{sinal}{}", inteiro.unsigned_abs());
    }
    format!(
        "{sinal}{},{:0largura$}",
        inteiro.unsigned_abs(),
        decimal.unsigned_abs(),
        largura = escala
    )
}

// --- Datas ------------------------------------------------------------------

/// Dias entre 1900-01-01 e 1970-01-01.
const DIAS_ATE_1900: i64 = 25_567;
/// Dias entre 0001-01-01 e 1970-01-01.
const DIAS_ATE_ANO_UM: i64 = 719_162;

fn dias_desde_1900(dias: i64) -> i64 {
    dias - DIAS_ATE_1900
}

fn dias_desde_ano_um(dias: i64) -> i64 {
    dias - DIAS_ATE_ANO_UM
}

/// Segundos inteiros de um valor `time`, cuja precisão vem na escala.
fn segundos_do_tempo(hora: &tiberius::time::Time) -> i64 {
    let divisor = 10i64.pow(u32::from(hora.scale()));
    (hora.increments() as i64) / divisor.max(1)
}

/// Passa para o dia seguinte ou anterior quando o deslocamento de fuso
/// atravessa a meia-noite.
fn normalizar(dias: i64, segundos: i64) -> (i64, i64) {
    let dia = 86_400;
    (dias + segundos.div_euclid(dia), segundos.rem_euclid(dia))
}

/// `29/07/2026`, e a hora só quando ela existe — como no Oracle.
fn formatar_momento(dias: i64, segundos: i64) -> String {
    let data = formatar_data(dias);
    if segundos == 0 {
        return data;
    }
    format!("{data} {}", formatar_hora(segundos))
}

fn formatar_data(dias: i64) -> String {
    let (ano, mes, dia) = civil(dias);
    format!("{dia:02}/{mes:02}/{ano:04}")
}

fn formatar_hora(segundos: i64) -> String {
    let segundos = segundos.rem_euclid(86_400);
    format!(
        "{:02}:{:02}:{:02}",
        segundos / 3600,
        (segundos % 3600) / 60,
        segundos % 60
    )
}

/// Data civil a partir do número de dias desde 1970-01-01.
///
/// É o algoritmo de Howard Hinnant, o mesmo que as bibliotecas de data usam.
/// Está aqui para não trazer uma dependência de calendário inteira só para
/// escrever `dd/mm/aaaa` — o driver entrega dia e segundo, e nada mais.
fn civil(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let ano = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (ano + i64::from(mes <= 2), mes, dia)
}

/// O banco em que a sessão está agora.
///
/// Serve ao `use Vendas` digitado no editor: o lote troca o banco da aba por
/// dentro, e a lista da barra precisa parar de mostrar o banco de antes.
pub async fn banco_corrente(cliente: &mut Cliente) -> Option<String> {
    cliente
        .simple_query("select db_name()")
        .await
        .ok()?
        .into_row()
        .await
        .ok()??
        .get::<&str, _>(0)
        .map(str::to_string)
}

/// O lote pode ter trocado o banco corrente?
///
/// Basta a suspeita: confirmar custa uma pergunta curta ao servidor, e a
/// alternativa — perguntar depois de toda execução — custaria uma ida e volta
/// por `Ctrl+Enter`.
pub fn parece_trocar_banco(sql: &str) -> bool {
    sql.split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|palavra| palavra.eq_ignore_ascii_case("use"))
}

/// Erro do servidor com a linha corrigida para a posição no editor.
///
/// O SQL Server conta a linha **dentro do lote**; o editor conta desde o começo
/// do texto. Sem esta soma, o salto do requisito 18 levaria à linha errada em
/// todo lote que não fosse o primeiro.
pub fn com_linha_do_editor(erro: Erro, lote: &Comando) -> Erro {
    match erro.linha {
        Some(linha) => {
            let absoluta = lote.linha as u32 + linha.saturating_sub(1);
            Erro {
                linha: Some(absoluta),
                ..erro
            }
        }
        None => erro,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn data_do_sql_server_sai_no_padrao_brasileiro() {
        // 2026-07-29 é o dia 20_663 desde 1970-01-01.
        assert_eq!(formatar_data(20_663), "29/07/2026");
        assert_eq!(formatar_data(0), "01/01/1970");
        assert_eq!(formatar_data(-1), "31/12/1969");
    }

    #[test]
    fn as_bases_de_dia_dos_tipos_de_data_batem() {
        // O `datetime` conta desde 1900-01-01; o `date`, desde 0001-01-01. Os
        // dois precisam cair no mesmo 1º de janeiro de 1900.
        assert_eq!(formatar_data(dias_desde_1900(0)), "01/01/1900");
        assert_eq!(formatar_data(dias_desde_ano_um(0)), "01/01/0001");
        assert_eq!(formatar_data(dias_desde_ano_um(693_595)), "01/01/1900");
    }

    #[test]
    fn hora_zero_nao_aparece() {
        assert_eq!(formatar_momento(20_663, 0), "29/07/2026");
        assert_eq!(formatar_momento(20_663, 14 * 3600 + 5 * 60 + 9), "29/07/2026 14:05:09");
    }

    #[test]
    fn decimal_negativo_nao_sai_com_dois_sinais() {
        let numero = tiberius::numeric::Numeric::new_with_scale(-150, 2);
        assert_eq!(decimal_de_numeric(numero), "-1,50");
        let inteiro = tiberius::numeric::Numeric::new_with_scale(42, 0);
        assert_eq!(decimal_de_numeric(inteiro), "42");
        let quebrado = tiberius::numeric::Numeric::new_with_scale(-5, 1);
        assert_eq!(decimal_de_numeric(quebrado), "-0,5");
    }

    #[test]
    fn o_use_no_texto_e_percebido() {
        assert!(parece_trocar_banco("use Vendas"));
        assert!(parece_trocar_banco("USE [Vendas];
select 1"));
        assert!(!parece_trocar_banco("select * from usuario"));
        assert!(!parece_trocar_banco("select used from t"));
    }

    #[test]
    fn fuso_que_atravessa_a_meia_noite_muda_o_dia() {
        let (dias, segundos) = normalizar(20_663, -3600);
        assert_eq!(formatar_momento(dias, segundos), "28/07/2026 23:00:00");
    }
}
