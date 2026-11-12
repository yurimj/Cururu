//! SQL Server Agent (requisito 36): situação do serviço, trabalhos e cadastro.
//!
//! Tudo o que o Agent guarda mora no `msdb`, e é alcançado aqui por nome de três
//! partes (`msdb.dbo.…`). Nunca por `use msdb`: a sessão é a de uma aba
//! (requisito 7), e trocar o banco corrente por baixo de quem está trabalhando
//! nele seria mexer no estado da aba só para ler uma árvore.
//!
//! O cadastro de trabalho passa pelas mesmas procedures que o SSMS usa —
//! `sp_add_job`, `sp_add_jobstep`, `sp_add_jobschedule` e `sp_add_jobserver`. Não
//! há `insert` em tabela do `msdb`: as procedures são a interface suportada, e
//! escrever direto nas tabelas é o que quebra na próxima versão do servidor.

use serde::{Deserialize, Serialize};
use tiberius::ToSql;

use crate::erro::{Erro, Resultado};
use crate::registro;
use crate::sqlserver::Cliente;

/// Situação do serviço SQL Server Agent, como a árvore a mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EstadoDoAgente {
    Ativo,
    Parado,
    /// Não se conseguiu descobrir — falta de permissão, quase sempre.
    Desconhecido,
}

/// Como terminou a última execução de um trabalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultadoDoTrabalho {
    Falha,
    Sucesso,
    NovaTentativa,
    Cancelado,
    Desconhecido,
}

/// O que o Agent está fazendo com o trabalho **agora**.
///
/// São os códigos de `current_execution_status`, com os nomes que o SSMS lhes dá
/// na janela de atividade dos trabalhos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SituacaoDaExecucao {
    Executando,
    AguardandoThread,
    EntreTentativas,
    Parado,
    Suspenso,
    Finalizando,
    /// Não se conseguiu descobrir — nem o Agent nem o `msdb` responderam.
    Desconhecido,
}

/// Em que pé está a execução de um trabalho, para a tela de acompanhamento.
///
/// Junta as duas fontes que respondem a isso, porque nenhuma responde às duas
/// perguntas: o que está acontecendo agora só o processo do Agent sabe, e como
/// terminou só o histórico guarda.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecucaoDoTrabalho {
    /// `true` enquanto o Agent está com o trabalho na mão.
    pub executando: bool,
    pub situacao: SituacaoDaExecucao,
    /// A etapa em curso como o Agent a escreve — `2 (Carregar)`. Vazia quando o
    /// trabalho não está rodando.
    pub etapa: String,
    /// A instância do histórico da última execução **concluída**, ou 0 se o
    /// trabalho nunca rodou.
    ///
    /// É o que diz se a execução que a tela acompanha já terminou: o Agent
    /// grava uma instância nova no fim de cada execução, e comparar com a de
    /// antes do disparo evita tomar o resultado da execução anterior pelo desta
    /// — o Agent leva alguns instantes para pegar o trabalho, e nesse intervalo
    /// ele ainda não está em execução nem terminou.
    pub instancia: i32,
    pub resultado: ResultadoDoTrabalho,
    /// A mensagem que o Agent gravou no histórico — a mesma que o SSMS mostra.
    pub mensagem: String,
    /// `yyyy-MM-dd HH:mm:ss` do **início** dessa execução; é o que o histórico
    /// guarda, e a duração é contada dele.
    pub iniciou_em: Option<String>,
    /// Quanto ela durou, em segundos, segundo o próprio servidor.
    pub duracao: Option<i32>,
}

/// Um trabalho do Agent, como a árvore e a dica do mouse o mostram.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trabalho {
    pub id: String,
    pub nome: String,
    pub habilitado: bool,
    pub descricao: String,
    pub categoria: String,
    pub proprietario: String,
    /// `yyyy-MM-dd HH:mm:ss`; a tela formata no idioma dela.
    pub criado_em: Option<String>,
    pub modificado_em: Option<String>,
    /// Ausente quando o trabalho nunca rodou.
    pub ultima_execucao: Option<String>,
    pub resultado: ResultadoDoTrabalho,
}

/// Um trabalho por inteiro, com as etapas e as agendas dele.
///
/// É o que a tela de propriedades preenche e o que o DDL escreve — os dois
/// precisam exatamente da mesma leitura, e fazê-la duas vezes seria pagar duas
/// idas ao servidor pela mesma informação.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetalheDoTrabalho {
    pub id: String,
    pub nome: String,
    pub habilitado: bool,
    pub descricao: String,
    pub categoria: String,
    pub proprietario: String,
    pub criado_em: Option<String>,
    pub modificado_em: Option<String>,
    pub ultima_execucao: Option<String>,
    pub etapas: Vec<EtapaDeTrabalho>,
    pub agendas: Vec<AgendaDeTrabalho>,
}

/// O que a tela de Novo Trabalho precisa saber antes de abrir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DadosDeTrabalho {
    pub categorias: Vec<String>,
    /// Logins oferecidos como proprietário. Vem só com o próprio login quando a
    /// conta não tem permissão de enxergar os principais do servidor.
    pub logins: Vec<String>,
    pub login_atual: String,
    /// O nome do servidor: é o que a tela mostra em **Origem**.
    pub servidor: String,
}

/// Um trabalho a criar, como a tela o entrega.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovoTrabalho {
    pub nome: String,
    pub proprietario: Option<String>,
    pub categoria: Option<String>,
    pub descricao: String,
    pub habilitado: bool,
    pub etapas: Vec<EtapaDeTrabalho>,
    pub agendas: Vec<AgendaDeTrabalho>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EtapaDeTrabalho {
    pub nome: String,
    /// O `@subsystem` do `sp_add_jobstep`: `TSQL`, `CmdExec` ou `PowerShell`.
    pub tipo: String,
    /// Só vale no `TSQL`; nos outros a procedure recusa o parâmetro preenchido.
    pub banco: Option<String>,
    pub comando: String,
    /// 1 = sai com êxito, 2 = sai com falha, 3 = vai para a próxima etapa.
    pub ao_ter_exito: i32,
    pub ao_falhar: i32,
}

/// Uma agenda, nos mesmos números do `sp_add_jobschedule`.
///
/// A tela fala a língua da procedure de propósito. Traduzir "toda segunda às 3h"
/// para `freq_type = 8, freq_interval = 2` é trabalho de quem desenha a tela —
/// que é onde o SSMS também o faz —, e inventar aqui um vocabulário
/// intermediário só acrescentaria mais uma tradução para errar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgendaDeTrabalho {
    pub nome: String,
    pub habilitada: bool,
    pub freq_tipo: i32,
    pub freq_intervalo: i32,
    pub freq_subtipo: i32,
    pub freq_subintervalo: i32,
    pub freq_intervalo_relativo: i32,
    pub freq_fator: i32,
    /// `yyyyMMdd`.
    pub inicio_data: i32,
    pub fim_data: i32,
    /// `HHmmss`.
    pub inicio_hora: i32,
    pub fim_hora: i32,
}

/// Os três subsistemas oferecidos pela tela de etapa.
///
/// A lista existe para o erro de um valor errado nascer aqui, com o nome do
/// campo, em vez de chegar como uma mensagem do servidor sobre uma procedure que
/// quem usa nunca chamou.
const SUBSISTEMAS: [&str; 3] = ["TSQL", "CmdExec", "PowerShell"];

/// A conferência que vai depois de **cada** procedure do lote de criação.
///
/// `@@error` pega o erro de execução; `@retorno` pega a recusa por regra de
/// negócio, que as procedures do `msdb` sinalizam com `raiserror` e `return 1` —
/// e que não interrompe o lote sozinha. Ver [`criar`].
const CONFERIR: &str = " if (@@error <> 0 or @retorno <> 0) goto desfazer;";

/// O fecho do lote: confirma, ou desfaz tudo e reclama.
///
/// O `raiserror` do fim é rede de segurança: quando a procedure recusa, ela já
/// mandou a própria mensagem — que é a informativa, e é a que o driver guarda,
/// porque ele fica com a **primeira**. Este texto só aparece no caso em que uma
/// procedure devolvesse retorno diferente de zero sem dizer nada, e sem ele isso
/// viraria um "trabalho criado" com nada criado.
const DESFECHO: &str = " commit transaction; \
     return; \
     desfazer: \
     if @@trancount > 0 rollback transaction; \
     raiserror(N'O SQL Server recusou a operação; nada foi gravado no trabalho.', 16, 1);";

// --- Situação do serviço ---------------------------------------------------

/// Situação do serviço SQL Server Agent.
///
/// Duas perguntas diferentes, porque nenhuma responde sozinha em todo servidor:
///
/// - `sys.dm_server_services` é a resposta direta, mas exige `VIEW SERVER STATE`
///   e não existe antes do SQL Server 2008 R2 SP1.
/// - A sessão `SQLAgent - Generic Refresher` em `sysprocesses` é o sinal
///   indireto que o SSMS usa há décadas. A presença dela **prova** que o serviço
///   está no ar; a ausência não prova o contrário, porque sem `VIEW SERVER
///   STATE` a conta só enxerga as próprias sessões.
///
/// Daí o terceiro estado. Dizer "parado" a quem apenas não tem permissão de
/// olhar seria mentir onde a mentira custa caro: o usuário iria procurar defeito
/// num serviço parado que está funcionando.
pub async fn estado(cliente: &mut Cliente) -> EstadoDoAgente {
    match servico(cliente).await {
        Ok(Some(estado)) => return estado,
        Ok(None) => {}
        Err(erro) => registro::informacao(
            "agente",
            format!("sys.dm_server_services indisponível ({erro}); tentando pelas sessões"),
        ),
    }

    match sessao_do_agente(cliente).await {
        Ok(true) => EstadoDoAgente::Ativo,
        Ok(false) => EstadoDoAgente::Desconhecido,
        Err(erro) => {
            registro::informacao("agente", format!("situação do serviço indeterminada: {erro}"));
            EstadoDoAgente::Desconhecido
        }
    }
}

/// O que o servidor diz do próprio serviço.
///
/// O `like '%Agent%'` cobre o nome traduzido das instalações em outras línguas —
/// "Agente SQL Server", em português, contém "Agent" —, e nenhum dos outros
/// serviços da lista (mecanismo, texto completo, navegador) tem essa palavra.
async fn servico(cliente: &mut Cliente) -> Resultado<Option<EstadoDoAgente>> {
    let linhas = cliente
        .simple_query(
            "select top 1 status_desc from sys.dm_server_services \
             where servicename like N'%Agent%'",
        )
        .await?
        .into_first_result()
        .await?;

    Ok(linhas.first().map(|linha| match linha.get::<&str, _>(0) {
        Some(situacao) if situacao.eq_ignore_ascii_case("Running") => EstadoDoAgente::Ativo,
        // "Stopped", "Stop Pending", "Paused"… nada disso atende a um trabalho
        // agendado, e é só isso que a árvore precisa saber.
        Some(_) => EstadoDoAgente::Parado,
        None => EstadoDoAgente::Desconhecido,
    }))
}

async fn sessao_do_agente(cliente: &mut Cliente) -> Resultado<bool> {
    let linha = cliente
        .simple_query(
            "select count(*) from master.dbo.sysprocesses \
             where program_name like N'SQLAgent - Generic Refresher%'",
        )
        .await?
        .into_row()
        .await?;
    Ok(linha
        .as_ref()
        .and_then(|linha| linha.get::<i32, _>(0))
        .is_some_and(|quantidade| quantidade > 0))
}

// --- Trabalhos -------------------------------------------------------------

/// Os trabalhos que este login enxerga.
///
/// `sysjobs_view` e não `sysjobs`: é a view que aplica a regra de visibilidade do
/// próprio Agent — quem é `sysadmin` vê todos, os demais veem os seus. Ler a
/// tabela direto mostraria a lista inteira a quem não deveria vê-la, ou
/// simplesmente seria recusada.
pub async fn listar(
    cliente: &mut Cliente,
    filtro: Option<&crate::filtro::Filtro>,
) -> Resultado<Vec<Trabalho>> {
    // O filtro é o mesmo das categorias da árvore (requisito 4), sobre o nome
    // do trabalho. A situação não entra: trabalho não tem estado inválido.
    let recorte = filtro
        .map(|filtro| filtro.condicao_tsql("t.name", 1))
        .unwrap_or_default();
    let sql = format!(
        "select cast(t.job_id as nvarchar(36)) as id, \
                    t.name as nome, \
                    t.enabled as habilitado, \
                    isnull(t.description, N'') as descricao, \
                    isnull(c.name, N'') as categoria, \
                    isnull(suser_sname(t.owner_sid), N'') as proprietario, \
                    convert(varchar(19), t.date_created, 120) as criado_em, \
                    convert(varchar(19), t.date_modified, 120) as modificado_em, \
                    isnull(s.last_run_date, 0) as data_execucao, \
                    isnull(s.last_run_time, 0) as hora_execucao, \
                    isnull(s.last_run_outcome, 5) as resultado \
             from msdb.dbo.sysjobs_view t \
             left join msdb.dbo.syscategories c on c.category_id = t.category_id \
             left join msdb.dbo.sysjobservers s on s.job_id = t.job_id{}
             order by t.name",
        if recorte.sql.is_empty() {
            String::new()
        } else {
            format!(" where {}", recorte.sql)
        }
    );

    let parametros: Vec<&dyn ToSql> = recorte
        .valores
        .iter()
        .map(|valor| valor as &dyn ToSql)
        .collect();
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    Ok(linhas
        .iter()
        .map(|linha| {
            let data = linha.get::<i32, _>("data_execucao").unwrap_or(0);
            let hora = linha.get::<i32, _>("hora_execucao").unwrap_or(0);
            Trabalho {
                id: texto(linha.get::<&str, _>("id")),
                nome: texto(linha.get::<&str, _>("nome")),
                habilitado: linha.get::<u8, _>("habilitado").unwrap_or(0) != 0,
                descricao: texto(linha.get::<&str, _>("descricao")),
                categoria: texto(linha.get::<&str, _>("categoria")),
                proprietario: texto(linha.get::<&str, _>("proprietario")),
                criado_em: linha.get::<&str, _>("criado_em").map(str::to_string),
                modificado_em: linha.get::<&str, _>("modificado_em").map(str::to_string),
                ultima_execucao: momento_da_execucao(data, hora),
                resultado: resultado_do_trabalho(linha.get::<u8, _>("resultado").unwrap_or(5)),
            }
        })
        .collect())
}

fn texto(valor: Option<&str>) -> String {
    valor.unwrap_or_default().to_string()
}

/// O Agent guarda data e hora como dois inteiros — `20260827` e `31105` —, e não
/// como `datetime`. Juntá-los é conta, não conversão de tipo.
fn momento_da_execucao(data: i32, hora: i32) -> Option<String> {
    if data <= 0 {
        return None;
    }
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        data / 10_000,
        (data / 100) % 100,
        data % 100,
        hora / 10_000,
        (hora / 100) % 100,
        hora % 100,
    ))
}

fn resultado_do_trabalho(codigo: u8) -> ResultadoDoTrabalho {
    match codigo {
        0 => ResultadoDoTrabalho::Falha,
        1 => ResultadoDoTrabalho::Sucesso,
        2 => ResultadoDoTrabalho::NovaTentativa,
        3 => ResultadoDoTrabalho::Cancelado,
        _ => ResultadoDoTrabalho::Desconhecido,
    }
}

/// Um trabalho por inteiro: o geral, as etapas e as agendas, numa ida só.
pub async fn detalhe(cliente: &mut Cliente, id: &str) -> Resultado<DetalheDoTrabalho> {
    let identificador = id.to_string();
    let mut resultados = cliente
        .query(
            "select cast(t.job_id as nvarchar(36)) as id, \
                    t.name as nome, \
                    t.enabled as habilitado, \
                    isnull(t.description, N'') as descricao, \
                    isnull(c.name, N'') as categoria, \
                    isnull(suser_sname(t.owner_sid), N'') as proprietario, \
                    convert(varchar(19), t.date_created, 120) as criado_em, \
                    convert(varchar(19), t.date_modified, 120) as modificado_em, \
                    isnull(s.last_run_date, 0) as data_execucao, \
                    isnull(s.last_run_time, 0) as hora_execucao \
             from msdb.dbo.sysjobs_view t \
             left join msdb.dbo.syscategories c on c.category_id = t.category_id \
             left join msdb.dbo.sysjobservers s on s.job_id = t.job_id \
             where t.job_id = cast(@P1 as uniqueidentifier); \
             select e.step_name, e.subsystem, e.command, e.database_name, \
                    e.on_success_action, e.on_fail_action \
             from msdb.dbo.sysjobsteps e \
             where e.job_id = cast(@P1 as uniqueidentifier) order by e.step_id; \
             select h.name, h.enabled, h.freq_type, h.freq_interval, h.freq_subday_type, \
                    h.freq_subday_interval, h.freq_relative_interval, h.freq_recurrence_factor, \
                    h.active_start_date, h.active_end_date, h.active_start_time, \
                    h.active_end_time \
             from msdb.dbo.sysschedules h \
             join msdb.dbo.sysjobschedules a on a.schedule_id = h.schedule_id \
             where a.job_id = cast(@P1 as uniqueidentifier) order by h.name",
            &[&identificador],
        )
        .await?
        .into_results()
        .await?
        .into_iter();

    let geral = resultados.next().unwrap_or_default();
    let Some(linha) = geral.first() else {
        // Um trabalho excluído por outra pessoa entre a árvore listar e o clique
        // chegar. Dizer isso vale mais que abrir uma tela em branco.
        return Err(Erro::novo(
            "o trabalho não existe mais; atualize o SQL Server Agent",
        ));
    };

    let data = linha.get::<i32, _>("data_execucao").unwrap_or(0);
    let hora = linha.get::<i32, _>("hora_execucao").unwrap_or(0);
    Ok(DetalheDoTrabalho {
        id: texto(linha.get::<&str, _>("id")),
        nome: texto(linha.get::<&str, _>("nome")),
        habilitado: linha.get::<u8, _>("habilitado").unwrap_or(0) != 0,
        descricao: texto(linha.get::<&str, _>("descricao")),
        categoria: texto(linha.get::<&str, _>("categoria")),
        proprietario: texto(linha.get::<&str, _>("proprietario")),
        criado_em: linha.get::<&str, _>("criado_em").map(str::to_string),
        modificado_em: linha.get::<&str, _>("modificado_em").map(str::to_string),
        ultima_execucao: momento_da_execucao(data, hora),
        etapas: resultados
            .next()
            .unwrap_or_default()
            .iter()
            .map(|linha| EtapaDeTrabalho {
                nome: texto(linha.get::<&str, _>("step_name")),
                tipo: texto(linha.get::<&str, _>("subsystem")),
                banco: linha.get::<&str, _>("database_name").map(str::to_string),
                comando: texto(linha.get::<&str, _>("command")),
                ao_ter_exito: i32::from(linha.get::<u8, _>("on_success_action").unwrap_or(1)),
                ao_falhar: i32::from(linha.get::<u8, _>("on_fail_action").unwrap_or(2)),
            })
            .collect(),
        agendas: resultados
            .next()
            .unwrap_or_default()
            .iter()
            .map(|linha| {
                let inteiro = |coluna: &str| linha.get::<i32, _>(coluna).unwrap_or(0);
                AgendaDeTrabalho {
                    nome: texto(linha.get::<&str, _>("name")),
                    habilitada: inteiro("enabled") != 0,
                    freq_tipo: inteiro("freq_type"),
                    freq_intervalo: inteiro("freq_interval"),
                    freq_subtipo: inteiro("freq_subday_type"),
                    freq_subintervalo: inteiro("freq_subday_interval"),
                    freq_intervalo_relativo: inteiro("freq_relative_interval"),
                    freq_fator: inteiro("freq_recurrence_factor"),
                    inicio_data: inteiro("active_start_date"),
                    fim_data: inteiro("active_end_date"),
                    inicio_hora: inteiro("active_start_time"),
                    fim_hora: inteiro("active_end_time"),
                }
            })
            .collect(),
    })
}

// --- Dados do formulário ---------------------------------------------------

/// Categorias, logins, login corrente e nome do servidor, numa ida só.
pub async fn dados(cliente: &mut Cliente) -> Resultado<DadosDeTrabalho> {
    let mut resultados = cliente
        .simple_query(
            "select isnull(suser_sname(), N''); \
             select isnull(cast(serverproperty('ServerName') as nvarchar(128)), N''); \
             select name from msdb.dbo.syscategories where category_class = 1 order by name",
        )
        .await?
        .into_results()
        .await?
        .into_iter();

    let login_atual = primeiro_texto(resultados.next());
    let servidor = primeiro_texto(resultados.next());
    let categorias = resultados
        .next()
        .unwrap_or_default()
        .iter()
        .filter_map(|linha| linha.get::<&str, _>(0))
        .map(str::to_string)
        .collect();

    Ok(DadosDeTrabalho {
        logins: logins(cliente, &login_atual).await,
        login_atual,
        servidor,
        categorias,
    })
}

fn primeiro_texto(linhas: Option<Vec<tiberius::Row>>) -> String {
    linhas
        .and_then(|linhas| {
            linhas
                .first()
                .and_then(|linha| linha.get::<&str, _>(0))
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// Logins que podem ser donos de um trabalho.
///
/// Melhor esforço: `sys.server_principals` exige `VIEW ANY DEFINITION`, e sem ela
/// a consulta devolve só o próprio login — ou nada. Falhar aqui não pode impedir
/// o cadastro, porque um trabalho do próprio login não precisa de lista nenhuma.
/// Por isso o retorno não é `Resultado`.
async fn logins(cliente: &mut Cliente, login_atual: &str) -> Vec<String> {
    let consulta = cliente
        .simple_query(
            "select name from sys.server_principals \
             where type in ('S', 'U', 'G') and is_disabled = 0 and name not like N'##%' \
             order by name",
        )
        .await;

    let linhas = match consulta {
        Ok(consulta) => consulta.into_first_result().await.unwrap_or_default(),
        Err(erro) => {
            registro::informacao("agente", format!("logins não listados: {erro}"));
            Vec::new()
        }
    };

    let mut nomes: Vec<String> = linhas
        .iter()
        .filter_map(|linha| linha.get::<&str, _>(0))
        .map(str::to_string)
        .collect();
    // O próprio login é o padrão da tela, e precisa estar na lista mesmo quando
    // a consulta acima não respondeu nada.
    if !login_atual.is_empty() && !nomes.iter().any(|nome| nome == login_atual) {
        nomes.insert(0, login_atual.to_string());
    }
    nomes
}

// --- Criação de trabalho ---------------------------------------------------

/// Cria o trabalho, as etapas e as agendas — tudo ou nada.
///
/// Um lote só, e não uma chamada por procedure, porque as quatro precisam valer
/// juntas: um trabalho que ficou sem etapa, ou sem `sp_add_jobserver`, aparece na
/// lista e nunca roda.
///
/// **O `set xact_abort on` sozinho não basta**, e essa é a armadilha do lote. As
/// procedures do `msdb` recusam entrada com `raiserror` seguido de `return 1`, e
/// erro levantado por `raiserror` **não** interrompe o lote nem derruba a
/// transação — o `xact_abort` só age sobre erro de execução. Sem conferir o
/// retorno de cada procedure, uma etapa recusada deixava o lote seguir até o
/// `commit`, e o trabalho era gravado sem ela: exatamente o trabalho que existe e
/// nunca roda que este lote existe para evitar. Daí o `@retorno` conferido a cada
/// chamada, que é o mesmo que o script gerado pelo SSMS faz.
pub async fn criar(cliente: &mut Cliente, trabalho: &NovoTrabalho) -> Resultado<()> {
    validar(trabalho)?;
    let (sql, lote) = montar_lote(trabalho);
    cliente.execute(sql, &lote.referencias()).await?;
    registro::informacao(
        "agente",
        format!(
            "trabalho criado: {} ({} etapa(s), {} agenda(s))",
            trabalho.nome.trim(),
            trabalho.etapas.len(),
            trabalho.agendas.len()
        ),
    );
    Ok(())
}

/// Grava o que a tela de propriedades mudou — também tudo ou nada.
///
/// **Etapas e agendas são trocadas por inteiro**, não conciliadas uma a uma. É a
/// diferença mais visível para o SSMS, e é deliberada: casar o que está na tela
/// com o que está no servidor exigiria identificar cada etapa entre as duas
/// listas, e a tela não tem esse identificador — ela deixa reordenar, renomear e
/// excluir livremente. Trocar tudo dá o mesmo resultado observável com uma
/// fração do código que pode errar.
///
/// A consequência real é uma só: a agenda perde o `schedule_id`. Como o
/// `sp_detach_schedule` só apaga a agenda que **nenhum outro trabalho** usa, uma
/// agenda compartilhada continua de pé para os outros — o que se perde é o
/// compartilhamento com este trabalho, que a tela também não sabe montar.
pub async fn atualizar(cliente: &mut Cliente, id: &str, trabalho: &NovoTrabalho) -> Resultado<()> {
    validar(trabalho)?;

    let mut lote = Lote::novo();
    let mut sql = String::from(
        "set nocount on; \
         set xact_abort on; \
         declare @retorno int = 0; \
         declare @agenda int; \
         declare @trabalho uniqueidentifier = cast(",
    );
    sql.push_str(&lote.texto(id));
    sql.push_str(
        " as uniqueidentifier); \
         begin transaction; \
         exec @retorno = msdb.dbo.sp_update_job @job_id = @trabalho, @new_name = ",
    );
    sql.push_str(&lote.texto(trabalho.nome.trim()));
    sql.push_str(", @enabled = ");
    sql.push_str(&lote.numero(i32::from(trabalho.habilitado)));
    // Descrição vazia vai como texto vazio, e não como nulo: no `sp_update_job`
    // o nulo quer dizer "não mexa neste campo", e apagar a descrição ficaria
    // impossível.
    sql.push_str(", @description = ");
    sql.push_str(&lote.texto(trabalho.descricao.trim()));
    sql.push_str(", @owner_login_name = ");
    sql.push_str(&lote.texto_opcional(trabalho.proprietario.as_deref().and_then(vazio_e_nulo)));
    sql.push_str(", @category_name = ");
    sql.push_str(&lote.texto_opcional(trabalho.categoria.as_deref().and_then(vazio_e_nulo)));
    sql.push(';');
    sql.push_str(CONFERIR);

    // `@step_id = 0` apaga **todas** as etapas de uma vez, e é documentado.
    sql.push_str(" exec @retorno = msdb.dbo.sp_delete_jobstep @job_id = @trabalho, @step_id = 0;");
    sql.push_str(CONFERIR);

    // As agendas saem uma a uma porque não há equivalente do `@step_id = 0`.
    sql.push_str(
        " while exists (select 1 from msdb.dbo.sysjobschedules where job_id = @trabalho) \
         begin \
           select top 1 @agenda = schedule_id from msdb.dbo.sysjobschedules \
             where job_id = @trabalho; \
           exec @retorno = msdb.dbo.sp_detach_schedule @job_id = @trabalho, \
             @schedule_id = @agenda, @delete_unused_schedule = 1; \
           if (@@error <> 0 or @retorno <> 0) goto desfazer; \
         end",
    );

    acrescentar_etapas(&mut sql, &mut lote, &trabalho.etapas);
    acrescentar_agendas(&mut sql, &mut lote, &trabalho.agendas);

    // A etapa inicial volta a ser a primeira: as antigas já não existem, e um
    // `start_step_id` apontando para etapa que saiu deixaria o trabalho travado.
    sql.push_str(" exec @retorno = msdb.dbo.sp_update_job @job_id = @trabalho, @start_step_id = 1;");
    sql.push_str(CONFERIR);
    sql.push_str(DESFECHO);

    cliente.execute(sql, &lote.referencias()).await?;
    registro::informacao(
        "agente",
        format!(
            "trabalho {id} atualizado: {} ({} etapa(s), {} agenda(s))",
            trabalho.nome.trim(),
            trabalho.etapas.len(),
            trabalho.agendas.len()
        ),
    );
    Ok(())
}

/// O lote de criação e os parâmetros dele, separados de quem os executa para o
/// teste poder conferir a forma do comando sem um servidor por perto.
fn montar_lote(trabalho: &NovoTrabalho) -> (String, Lote) {
    let mut lote = Lote::novo();
    let mut sql = String::from(
        "set nocount on; \
         set xact_abort on; \
         declare @retorno int = 0; \
         declare @trabalho uniqueidentifier; \
         begin transaction; \
         exec @retorno = msdb.dbo.sp_add_job @job_name = ",
    );
    sql.push_str(&lote.texto(trabalho.nome.trim()));
    sql.push_str(", @enabled = ");
    sql.push_str(&lote.numero(i32::from(trabalho.habilitado)));
    sql.push_str(", @description = ");
    sql.push_str(&lote.texto_opcional(vazio_e_nulo(&trabalho.descricao)));
    sql.push_str(", @owner_login_name = ");
    sql.push_str(&lote.texto_opcional(trabalho.proprietario.as_deref().and_then(vazio_e_nulo)));
    sql.push_str(", @category_name = ");
    sql.push_str(&lote.texto_opcional(trabalho.categoria.as_deref().and_then(vazio_e_nulo)));
    sql.push_str(", @job_id = @trabalho output;");
    sql.push_str(CONFERIR);

    acrescentar_etapas(&mut sql, &mut lote, &trabalho.etapas);
    acrescentar_agendas(&mut sql, &mut lote, &trabalho.agendas);

    // Sem `sp_add_jobserver` o trabalho existe e nunca roda: é ela que diz ao
    // Agent local que aquele trabalho é dele.
    sql.push_str(
        " exec @retorno = msdb.dbo.sp_add_jobserver @job_id = @trabalho, \
          @server_name = N'(local)';",
    );
    sql.push_str(CONFERIR);
    sql.push_str(DESFECHO);
    (sql, lote)
}

// --- DDL do trabalho -------------------------------------------------------

/// O script que recria o trabalho, no formato do "Script Job as CREATE" do SSMS.
///
/// É texto para ler e guardar, não um lote para o Cururu executar: por isso vale
/// a pena ser o script inteiro, com `USE [msdb]`, transação, conferência de
/// retorno e `GO` — quem copiar isto para outro servidor precisa que rode lá.
///
/// A categoria vai com o `if not exists` que o SSMS também gera: um trabalho
/// levado para outro servidor costuma chegar antes da categoria dele.
pub fn ddl(trabalho: &DetalheDoTrabalho) -> String {
    let mut script = String::new();
    script.push_str("USE [msdb]\nGO\n\n");
    script.push_str(&format!(
        "/****** Trabalho [{}] — gerado pelo Cururu ******/\n",
        trabalho.nome
    ));
    script.push_str("BEGIN TRANSACTION\nDECLARE @retorno INT = 0\nDECLARE @trabalho UNIQUEIDENTIFIER\n\n");

    if !trabalho.categoria.is_empty() {
        script.push_str(&format!(
            "IF NOT EXISTS (SELECT name FROM msdb.dbo.syscategories \
             WHERE name = {} AND category_class = 1)\nBEGIN\n\
             \x20 EXEC @retorno = msdb.dbo.sp_add_category @class = N'JOB', @type = N'LOCAL', \
             @name = {}\n\
             \x20 IF (@@ERROR <> 0 OR @retorno <> 0) GOTO Desfazer\nEND\n\n",
            literal(&trabalho.categoria),
            literal(&trabalho.categoria)
        ));
    }

    script.push_str(&format!(
        "EXEC @retorno = msdb.dbo.sp_add_job\n\
         \x20 @job_name = {},\n\
         \x20 @enabled = {},\n\
         \x20 @description = {},\n\
         \x20 @owner_login_name = {},\n\
         \x20 @category_name = {},\n\
         \x20 @job_id = @trabalho OUTPUT\n\
         IF (@@ERROR <> 0 OR @retorno <> 0) GOTO Desfazer\n\n",
        literal(&trabalho.nome),
        i32::from(trabalho.habilitado),
        literal_opcional(vazio_e_nulo(&trabalho.descricao)),
        literal_opcional(vazio_e_nulo(&trabalho.proprietario)),
        literal_opcional(vazio_e_nulo(&trabalho.categoria)),
    ));

    for (ordem, etapa) in trabalho.etapas.iter().enumerate() {
        script.push_str(&format!(
            "EXEC @retorno = msdb.dbo.sp_add_jobstep\n\
             \x20 @job_id = @trabalho,\n\
             \x20 @step_id = {},\n\
             \x20 @step_name = {},\n\
             \x20 @subsystem = {},\n\
             \x20 @database_name = {},\n\
             \x20 @on_success_action = {},\n\
             \x20 @on_fail_action = {},\n\
             \x20 @command = {}\n\
             IF (@@ERROR <> 0 OR @retorno <> 0) GOTO Desfazer\n\n",
            ordem + 1,
            literal(&etapa.nome),
            literal(&etapa.tipo),
            literal_opcional(banco_da_etapa(etapa)),
            etapa.ao_ter_exito,
            etapa.ao_falhar,
            literal(&etapa.comando),
        ));
    }

    for agenda in &trabalho.agendas {
        script.push_str(&format!(
            "EXEC @retorno = msdb.dbo.sp_add_jobschedule\n\
             \x20 @job_id = @trabalho,\n\
             \x20 @name = {},\n\
             \x20 @enabled = {},\n\
             \x20 @freq_type = {},\n\
             \x20 @freq_interval = {},\n\
             \x20 @freq_subday_type = {},\n\
             \x20 @freq_subday_interval = {},\n\
             \x20 @freq_relative_interval = {},\n\
             \x20 @freq_recurrence_factor = {},\n\
             \x20 @active_start_date = {},\n\
             \x20 @active_end_date = {},\n\
             \x20 @active_start_time = {},\n\
             \x20 @active_end_time = {}\n\
             IF (@@ERROR <> 0 OR @retorno <> 0) GOTO Desfazer\n\n",
            literal(&agenda.nome),
            i32::from(agenda.habilitada),
            agenda.freq_tipo,
            agenda.freq_intervalo,
            agenda.freq_subtipo,
            agenda.freq_subintervalo,
            agenda.freq_intervalo_relativo,
            agenda.freq_fator,
            agenda.inicio_data,
            agenda.fim_data,
            agenda.inicio_hora,
            agenda.fim_hora,
        ));
    }

    script.push_str(
        "EXEC @retorno = msdb.dbo.sp_add_jobserver @job_id = @trabalho, \
         @server_name = N'(local)'\n\
         IF (@@ERROR <> 0 OR @retorno <> 0) GOTO Desfazer\n\n\
         COMMIT TRANSACTION\n\
         GOTO Fim\n\
         Desfazer:\n\
         \x20 IF (@@TRANCOUNT > 0) ROLLBACK TRANSACTION\n\
         Fim:\n\
         GO\n",
    );
    script
}

/// `O'Brien` vira `N'O''Brien'`, que é a única forma segura de um literal.
fn literal(texto: &str) -> String {
    format!("N'{}'", texto.replace('\'', "''"))
}

fn literal_opcional(texto: Option<&str>) -> String {
    texto.map_or_else(|| "NULL".to_string(), literal)
}

/// `@database_name` só existe no T-SQL: nos outros subsistemas a procedure
/// recusa o parâmetro preenchido.
fn banco_da_etapa(etapa: &EtapaDeTrabalho) -> Option<&str> {
    if etapa.tipo.eq_ignore_ascii_case("TSQL") {
        etapa.banco.as_deref().and_then(vazio_e_nulo)
    } else {
        None
    }
}

// --- Manutenção do trabalho ------------------------------------------------

/// O `declare` que precede **toda** chamada de procedure do Agent.
///
/// O trabalho é identificado por um `uniqueidentifier`, e o driver manda o
/// parâmetro como texto. Converter dentro da própria chamada — `@job_id =
/// cast(@P1 as uniqueidentifier)` — parece natural e é recusado pelo servidor
/// com `Incorrect syntax near '@P1'`: numa chamada de procedure o T-SQL aceita
/// **constante ou variável** como valor de parâmetro, nunca uma expressão.
///
/// Não é detalhe de estilo. Era isso que deixava **Iniciar**, **Parar**,
/// **Renomear**, **Excluir**, **Habilitar** e **Desabilitar** sem efeito nenhum:
/// o comando ia, o servidor recusava o lote inteiro no parse, e o trabalho nunca
/// chegava a ser tocado. Em `select` o mesmo `cast` é válido, porque ali ele
/// está numa expressão de `where` — por isso a leitura sempre funcionou e só a
/// escrita falhava.
const DECLARA_TRABALHO: &str =
    "declare @trabalho uniqueidentifier = cast(@P1 as uniqueidentifier); ";

/// Liga ou desliga o trabalho, sem tocar em mais nada dele.
pub async fn definir_habilitado(cliente: &mut Cliente, id: &str, ligado: bool) -> Resultado<()> {
    let identificador = id.to_string();
    cliente
        .execute(
            format!(
                "{DECLARA_TRABALHO}exec msdb.dbo.sp_update_job @job_id = @trabalho, \
                 @enabled = @P2"
            ),
            &[&identificador, &i32::from(ligado)],
        )
        .await?;
    registro::informacao(
        "agente",
        format!("trabalho {id} {}", if ligado { "habilitado" } else { "desabilitado" }),
    );
    Ok(())
}

pub async fn renomear(cliente: &mut Cliente, id: &str, nome: &str) -> Resultado<()> {
    let novo = nome.trim().to_string();
    if novo.is_empty() {
        return Err(Erro::novo("informe o nome do trabalho"));
    }
    let identificador = id.to_string();
    cliente
        .execute(
            format!(
                "{DECLARA_TRABALHO}exec msdb.dbo.sp_update_job @job_id = @trabalho, \
                 @new_name = @P2"
            ),
            &[&identificador, &novo],
        )
        .await?;
    registro::informacao("agente", format!("trabalho {id} renomeado para {novo}"));
    Ok(())
}

/// Exclui o trabalho e, junto, as agendas que ficarem sem dono.
///
/// `@delete_unused_schedule = 1` não é agressivo: ele só apaga a agenda que
/// **nenhum outro trabalho** usa. Sem isto, cada exclusão deixaria uma agenda
/// órfã no `msdb` para sempre.
pub async fn excluir(cliente: &mut Cliente, id: &str) -> Resultado<()> {
    let identificador = id.to_string();
    cliente
        .execute(
            format!(
                "{DECLARA_TRABALHO}exec msdb.dbo.sp_delete_job @job_id = @trabalho, \
                 @delete_unused_schedule = 1"
            ),
            &[&identificador],
        )
        .await?;
    registro::informacao("agente", format!("trabalho {id} excluído"));
    Ok(())
}

/// Manda o Agent rodar o trabalho, opcionalmente a partir de uma etapa.
///
/// O comando **agenda** a execução e volta na hora: quem executa é o serviço, e
/// ele não avisa quando termina. Com o serviço parado, o servidor recusa com a
/// mensagem certa — que é a resposta honesta e vai para a barra de status.
pub async fn iniciar(cliente: &mut Cliente, id: &str, etapa: Option<&str>) -> Resultado<()> {
    let identificador = id.to_string();
    match etapa {
        Some(etapa) => {
            let nome = etapa.to_string();
            cliente
                .execute(
                    format!(
                        "{DECLARA_TRABALHO}exec msdb.dbo.sp_start_job @job_id = @trabalho, \
                         @step_name = @P2"
                    ),
                    &[&identificador, &nome],
                )
                .await?;
            registro::informacao("agente", format!("trabalho {id} iniciado na etapa {nome}"));
        }
        None => {
            cliente
                .execute(
                    format!("{DECLARA_TRABALHO}exec msdb.dbo.sp_start_job @job_id = @trabalho"),
                    &[&identificador],
                )
                .await?;
            registro::informacao("agente", format!("trabalho {id} iniciado"));
        }
    }
    Ok(())
}

pub async fn parar(cliente: &mut Cliente, id: &str) -> Resultado<()> {
    let identificador = id.to_string();
    cliente
        .execute(
            format!("{DECLARA_TRABALHO}exec msdb.dbo.sp_stop_job @job_id = @trabalho"),
            &[&identificador],
        )
        .await?;
    registro::informacao("agente", format!("trabalho {id} interrompido"));
    Ok(())
}

/// Em que pé está a execução do trabalho, para a tela de acompanhamento.
///
/// São duas leituras porque são duas perguntas, e nenhuma fonte responde às
/// duas: o que está acontecendo agora só o Agent sabe, e como terminou só o
/// histórico guarda. É a mesma ida do SSMS, que também pergunta as duas coisas
/// para desenhar a janela de atividade.
pub async fn execucao(cliente: &mut Cliente, id: &str) -> Resultado<ExecucaoDoTrabalho> {
    let (situacao, etapa) = match corrente(cliente, id).await {
        Ok(estado) => estado,
        Err(erro) => {
            registro::informacao(
                "agente",
                format!("sp_help_job indisponível ({erro}); tentando pela atividade do msdb"),
            );
            atividade(cliente, id).await?
        }
    };

    let ultima = ultima_execucao(cliente, id).await?;
    Ok(ExecucaoDoTrabalho {
        // `suspenso` e `finalizando` contam como em andamento: o trabalho ainda
        // é do Agent, e a tela não pode anunciar um fim que não houve.
        executando: !matches!(
            situacao,
            SituacaoDaExecucao::Parado | SituacaoDaExecucao::Desconhecido
        ),
        situacao,
        etapa,
        instancia: ultima.as_ref().map(|u| u.instancia).unwrap_or(0),
        resultado: ultima
            .as_ref()
            .map(|u| u.resultado)
            .unwrap_or(ResultadoDoTrabalho::Desconhecido),
        mensagem: ultima
            .as_ref()
            .map(|u| u.mensagem.clone())
            .unwrap_or_default(),
        iniciou_em: ultima.as_ref().and_then(|u| u.iniciou_em.clone()),
        duracao: ultima.as_ref().map(|u| u.duracao),
    })
}

/// O estado corrente pela mesma porta que o SSMS usa.
///
/// `sp_help_job` pergunta ao processo do Agent, e não a uma tabela: é a única
/// fonte que sabe a etapa **em curso**. O `msdb` guarda a última etapa
/// concluída, que durante a etapa 2 ainda diz 1 — e mostrar a etapa errada
/// enquanto o trabalho roda é pior do que não mostrar nenhuma.
async fn corrente(cliente: &mut Cliente, id: &str) -> Resultado<(SituacaoDaExecucao, String)> {
    let identificador = id.to_string();
    let linhas = cliente
        .query(
            format!(
                "{DECLARA_TRABALHO}exec msdb.dbo.sp_help_job @job_id = @trabalho, \
                 @job_aspect = 'JOB'"
            ),
            &[&identificador],
        )
        .await?
        .into_first_result()
        .await?;

    let Some(linha) = linhas.first() else {
        return Err(Erro::novo("o Agent não respondeu sobre este trabalho"));
    };
    let codigo = linha.get::<i32, _>("current_execution_status").unwrap_or(4);
    let etapa = linha
        .get::<&str, _>("current_execution_step")
        .unwrap_or_default()
        .trim();
    // Parado, o Agent responde `0 (unknown)` — que não é etapa nenhuma e na tela
    // seria pior do que o espaço em branco.
    let etapa = if etapa.starts_with("0 (") {
        String::new()
    } else {
        etapa.to_string()
    };
    Ok((situacao_da_execucao(codigo), etapa))
}

/// A reserva: o que o `msdb` registrou da execução corrente.
///
/// Serve a quem não pode executar `sp_help_job`. A resposta é mais pobre — a
/// etapa é a última concluída, não a em curso —, mas traz o que a tela mais
/// precisa saber: se ainda está rodando.
///
/// Só a sessão corrente do Agent entra na conta. Um Agent reiniciado no meio de
/// uma execução deixa a linha antiga sem `stop_execution_date` para sempre, e
/// sem esse recorte a tela acompanharia até o fim dos tempos um trabalho que
/// morreu com o serviço.
async fn atividade(cliente: &mut Cliente, id: &str) -> Resultado<(SituacaoDaExecucao, String)> {
    let identificador = id.to_string();
    let linhas = cliente
        .query(
            "select top 1 case when a.stop_execution_date is null then 1 else 0 end as rodando, \
                    isnull(cast(e.step_id as nvarchar(11)) + N' (' + e.step_name + N')', N'') \
                      as etapa \
             from msdb.dbo.sysjobactivity a \
             left join msdb.dbo.sysjobsteps e \
                    on e.job_id = a.job_id and e.step_id = a.last_executed_step_id \
             where a.job_id = cast(@P1 as uniqueidentifier) \
               and a.start_execution_date is not null \
               and a.session_id = (select max(s.session_id) from msdb.dbo.syssessions s) \
             order by a.start_execution_date desc",
            &[&identificador],
        )
        .await?
        .into_first_result()
        .await?;

    let Some(linha) = linhas.first() else {
        // Sem atividade nesta sessão do Agent, o trabalho não está rodando.
        return Ok((SituacaoDaExecucao::Parado, String::new()));
    };
    if linha.get::<i32, _>("rodando").unwrap_or(0) == 0 {
        return Ok((SituacaoDaExecucao::Parado, String::new()));
    }
    Ok((
        SituacaoDaExecucao::Executando,
        texto(linha.get::<&str, _>("etapa")),
    ))
}

/// A última execução concluída, direto do histórico.
///
/// `step_id = 0` é a linha de resumo do trabalho — a que o SSMS mostra quando se
/// pergunta por que ele falhou. As linhas das etapas ficam de fora: a tela
/// precisa do veredito, não do caminho.
async fn ultima_execucao(cliente: &mut Cliente, id: &str) -> Resultado<Option<UltimaExecucao>> {
    let identificador = id.to_string();
    let linhas = cliente
        .query(
            "select top 1 h.instance_id as instancia, \
                    cast(isnull(h.run_status, 5) as tinyint) as resultado, \
                    isnull(h.message, N'') as mensagem, \
                    isnull(h.run_date, 0) as data_execucao, \
                    isnull(h.run_time, 0) as hora_execucao, \
                    isnull(h.run_duration, 0) as duracao \
             from msdb.dbo.sysjobhistory h \
             where h.job_id = cast(@P1 as uniqueidentifier) and h.step_id = 0 \
             order by h.instance_id desc",
            &[&identificador],
        )
        .await?
        .into_first_result()
        .await?;

    Ok(linhas.first().map(|linha| UltimaExecucao {
        instancia: linha.get::<i32, _>("instancia").unwrap_or(0),
        resultado: resultado_do_trabalho(linha.get::<u8, _>("resultado").unwrap_or(5)),
        mensagem: texto(linha.get::<&str, _>("mensagem")),
        iniciou_em: momento_da_execucao(
            linha.get::<i32, _>("data_execucao").unwrap_or(0),
            linha.get::<i32, _>("hora_execucao").unwrap_or(0),
        ),
        duracao: segundos(linha.get::<i32, _>("duracao").unwrap_or(0)),
    }))
}

struct UltimaExecucao {
    instancia: i32,
    resultado: ResultadoDoTrabalho,
    mensagem: String,
    iniciou_em: Option<String>,
    duracao: i32,
}

fn situacao_da_execucao(codigo: i32) -> SituacaoDaExecucao {
    match codigo {
        1 => SituacaoDaExecucao::Executando,
        2 => SituacaoDaExecucao::AguardandoThread,
        3 => SituacaoDaExecucao::EntreTentativas,
        4 => SituacaoDaExecucao::Parado,
        5 => SituacaoDaExecucao::Suspenso,
        7 => SituacaoDaExecucao::Finalizando,
        _ => SituacaoDaExecucao::Desconhecido,
    }
}

/// A duração do histórico é `HHMMSS` lido como número — `130` é um minuto e
/// trinta segundos, e não cento e trinta de nada.
fn segundos(duracao: i32) -> i32 {
    (duracao / 10_000) * 3_600 + ((duracao / 100) % 100) * 60 + duracao % 100
}

/// As etapas do lote, na ordem em que a tela as deixou.
fn acrescentar_etapas(sql: &mut String, lote: &mut Lote, etapas: &[EtapaDeTrabalho]) {
    for (ordem, etapa) in etapas.iter().enumerate() {
        sql.push_str(" exec @retorno = msdb.dbo.sp_add_jobstep @job_id = @trabalho, @step_id = ");
        sql.push_str(&lote.numero(ordem as i32 + 1));
        sql.push_str(", @step_name = ");
        sql.push_str(&lote.texto(etapa.nome.trim()));
        sql.push_str(", @subsystem = ");
        sql.push_str(&lote.texto(&etapa.tipo));
        sql.push_str(", @command = ");
        sql.push_str(&lote.texto(&etapa.comando));
        sql.push_str(", @database_name = ");
        // `@database_name` só existe para o T-SQL: nos outros subsistemas a
        // procedure recusa o parâmetro preenchido.
        sql.push_str(&lote.texto_opcional(if etapa.tipo.eq_ignore_ascii_case("TSQL") {
            etapa.banco.as_deref().and_then(vazio_e_nulo)
        } else {
            None
        }));
        sql.push_str(", @on_success_action = ");
        sql.push_str(&lote.numero(etapa.ao_ter_exito));
        sql.push_str(", @on_fail_action = ");
        sql.push_str(&lote.numero(etapa.ao_falhar));
        sql.push(';');
        sql.push_str(CONFERIR);
    }
}

/// As agendas do lote.
fn acrescentar_agendas(sql: &mut String, lote: &mut Lote, agendas: &[AgendaDeTrabalho]) {
    for agenda in agendas {
        sql.push_str(" exec @retorno = msdb.dbo.sp_add_jobschedule @job_id = @trabalho, @name = ");
        sql.push_str(&lote.texto(agenda.nome.trim()));
        sql.push_str(", @enabled = ");
        sql.push_str(&lote.numero(i32::from(agenda.habilitada)));
        sql.push_str(", @freq_type = ");
        sql.push_str(&lote.numero(agenda.freq_tipo));
        sql.push_str(", @freq_interval = ");
        sql.push_str(&lote.numero(agenda.freq_intervalo));
        sql.push_str(", @freq_subday_type = ");
        sql.push_str(&lote.numero(agenda.freq_subtipo));
        sql.push_str(", @freq_subday_interval = ");
        sql.push_str(&lote.numero(agenda.freq_subintervalo));
        sql.push_str(", @freq_relative_interval = ");
        sql.push_str(&lote.numero(agenda.freq_intervalo_relativo));
        sql.push_str(", @freq_recurrence_factor = ");
        sql.push_str(&lote.numero(agenda.freq_fator));
        sql.push_str(", @active_start_date = ");
        sql.push_str(&lote.numero(agenda.inicio_data));
        sql.push_str(", @active_end_date = ");
        sql.push_str(&lote.numero(agenda.fim_data));
        sql.push_str(", @active_start_time = ");
        sql.push_str(&lote.numero(agenda.inicio_hora));
        sql.push_str(", @active_end_time = ");
        sql.push_str(&lote.numero(agenda.fim_hora));
        sql.push(';');
        sql.push_str(CONFERIR);
    }
}

/// O que a tela deveria ter barrado, barrado de novo aqui.
///
/// A tela é a primeira barreira, não a única: o comando é chamável por quem
/// quiser, e sem isto o servidor recusaria com uma mensagem sobre uma procedure
/// que quem usa nunca chamou.
fn validar(trabalho: &NovoTrabalho) -> Resultado<()> {
    if trabalho.nome.trim().is_empty() {
        return Err(Erro::novo("informe o nome do trabalho"));
    }
    // O servidor aceita trabalho sem etapa, e ele nunca faz nada. Pior: na
    // edição, o `@start_step_id = 1` do fim do lote não teria etapa para
    // apontar, e a mensagem falaria de um parâmetro que ninguém preencheu.
    if trabalho.etapas.is_empty() {
        return Err(Erro::novo(
            "um trabalho sem etapa nunca faz nada; acrescente ao menos uma",
        ));
    }
    for etapa in &trabalho.etapas {
        if etapa.nome.trim().is_empty() {
            return Err(Erro::novo("toda etapa precisa de um nome"));
        }
        if etapa.comando.trim().is_empty() {
            return Err(Erro::novo(format!(
                "a etapa {} está sem comando",
                etapa.nome.trim()
            )));
        }
        if !SUBSISTEMAS
            .iter()
            .any(|tipo| tipo.eq_ignore_ascii_case(&etapa.tipo))
        {
            return Err(Erro::novo(format!(
                "tipo de etapa desconhecido: {}",
                etapa.tipo
            )));
        }
    }
    for agenda in &trabalho.agendas {
        if agenda.nome.trim().is_empty() {
            return Err(Erro::novo("toda agenda precisa de um nome"));
        }
    }
    Ok(())
}

fn vazio_e_nulo(texto: &str) -> Option<&str> {
    let limpo = texto.trim();
    (!limpo.is_empty()).then_some(limpo)
}

/// Os parâmetros do lote, na ordem em que os `@P1…@Pn` aparecem no texto.
///
/// O driver nomeia os parâmetros pela **posição** na lista, então o texto e a
/// lista precisam crescer juntos — daí cada `texto`/`numero` devolver o marcador
/// que deve ser escrito no lugar. Montar as duas coisas separadamente seria a
/// maneira mais fácil de desalinhar um parâmetro no meio de vinte.
struct Lote {
    valores: Vec<Valor>,
}

enum Valor {
    Texto(Option<String>),
    Numero(i32),
}

impl Lote {
    fn novo() -> Self {
        Lote {
            valores: Vec::new(),
        }
    }

    fn texto(&mut self, valor: &str) -> String {
        self.acrescentar(Valor::Texto(Some(valor.to_string())))
    }

    fn texto_opcional(&mut self, valor: Option<&str>) -> String {
        self.acrescentar(Valor::Texto(valor.map(str::to_string)))
    }

    fn numero(&mut self, valor: i32) -> String {
        self.acrescentar(Valor::Numero(valor))
    }

    fn acrescentar(&mut self, valor: Valor) -> String {
        self.valores.push(valor);
        format!("@P{}", self.valores.len())
    }

    fn referencias(&self) -> Vec<&dyn ToSql> {
        self.valores
            .iter()
            .map(|valor| match valor {
                Valor::Texto(texto) => texto as &dyn ToSql,
                Valor::Numero(numero) => numero as &dyn ToSql,
            })
            .collect()
    }

    /// Os textos que foram parar no lote — só o teste precisa olhá-los.
    #[cfg(test)]
    fn textos(&self) -> Vec<String> {
        self.valores
            .iter()
            .filter_map(|valor| match valor {
                Valor::Texto(texto) => texto.clone(),
                Valor::Numero(_) => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn execucao_nunca_ocorrida_nao_vira_data() {
        assert_eq!(momento_da_execucao(0, 0), None);
    }

    #[test]
    fn data_e_hora_do_agent_viram_um_momento_so() {
        assert_eq!(
            momento_da_execucao(20_260_827, 31_105).as_deref(),
            Some("2026-08-27 03:11:05")
        );
        // Meia-noite: a hora vem como zero, e os dois-pontos não podem sumir.
        assert_eq!(
            momento_da_execucao(20_260_101, 0).as_deref(),
            Some("2026-01-01 00:00:00")
        );
    }

    #[test]
    fn resultado_desconhecido_cobre_o_codigo_que_nao_existe() {
        assert_eq!(resultado_do_trabalho(1), ResultadoDoTrabalho::Sucesso);
        assert_eq!(resultado_do_trabalho(0), ResultadoDoTrabalho::Falha);
        assert_eq!(resultado_do_trabalho(9), ResultadoDoTrabalho::Desconhecido);
    }

    fn trabalho_minimo() -> NovoTrabalho {
        NovoTrabalho {
            nome: "Expurgo diário".to_string(),
            proprietario: None,
            categoria: None,
            descricao: String::new(),
            habilitado: true,
            etapas: Vec::new(),
            agendas: Vec::new(),
        }
    }

    fn etapa() -> EtapaDeTrabalho {
        EtapaDeTrabalho {
            nome: "Limpar".to_string(),
            tipo: "TSQL".to_string(),
            banco: Some("master".to_string()),
            comando: "delete from log where data < dateadd(day, -30, getdate())".to_string(),
            ao_ter_exito: 1,
            ao_falhar: 2,
        }
    }

    #[test]
    fn trabalho_sem_nome_e_recusado_antes_de_ir_ao_servidor() {
        let mut trabalho = trabalho_minimo();
        trabalho.nome = "   ".to_string();
        assert!(validar(&trabalho).is_err());
    }

    #[test]
    fn etapa_sem_comando_ou_com_tipo_estranho_e_recusada() {
        let mut sem_comando = trabalho_minimo();
        sem_comando.etapas = vec![EtapaDeTrabalho {
            comando: "  ".to_string(),
            ..etapa()
        }];
        assert!(validar(&sem_comando).is_err());

        let mut tipo_errado = trabalho_minimo();
        tipo_errado.etapas = vec![EtapaDeTrabalho {
            tipo: "ANALYSISQUERY".to_string(),
            ..etapa()
        }];
        assert!(validar(&tipo_errado).is_err());
    }

    /// O marcador devolvido tem de ser sempre o índice do valor que acabou de
    /// entrar: é isso que amarra o texto do lote à lista de parâmetros.
    #[test]
    fn os_marcadores_seguem_a_ordem_dos_valores() {
        let mut lote = Lote::novo();
        assert_eq!(lote.texto("um"), "@P1");
        assert_eq!(lote.numero(2), "@P2");
        assert_eq!(lote.texto_opcional(None), "@P3");
        assert_eq!(lote.referencias().len(), 3);
    }

    /// A conferência depois de cada procedure é o que impede o trabalho pela
    /// metade, e é fácil de perder ao mexer no lote.
    ///
    /// Sem ela, uma etapa recusada por `raiserror` — que é como as procedures do
    /// `msdb` recusam entrada — deixava o lote seguir até o `commit`, e o
    /// trabalho ficava gravado **sem** aquela etapa. O `set xact_abort on` não
    /// cobre esse caso: ele age sobre erro de execução, não sobre `raiserror`.
    #[test]
    fn cada_procedure_do_lote_tem_a_sua_conferencia() {
        let mut trabalho = trabalho_minimo();
        trabalho.etapas = vec![etapa(), etapa()];
        trabalho.agendas = vec![AgendaDeTrabalho {
            nome: "Diária".to_string(),
            habilitada: true,
            freq_tipo: 4,
            freq_intervalo: 1,
            freq_subtipo: 1,
            freq_subintervalo: 0,
            freq_intervalo_relativo: 0,
            freq_fator: 0,
            inicio_data: 20_260_827,
            fim_data: 99_991_231,
            inicio_hora: 30000,
            fim_hora: 235_959,
        }];

        let (sql, lote) = montar_lote(&trabalho);
        // Uma chamada de procedure por linha do lote: job + 2 etapas + agenda +
        // jobserver = 5, e uma conferência para cada uma.
        assert_eq!(sql.matches("exec @retorno = msdb.dbo.").count(), 5);
        assert_eq!(sql.matches(CONFERIR).count(), 5);
        assert!(sql.contains("goto desfazer"));
        assert!(sql.contains("rollback transaction"));
        // O último parâmetro precisa ter marcador no texto: um desalinhamento
        // aqui manda um valor para o campo errado.
        let ultimo = format!("@P{}", lote.referencias().len());
        assert!(sql.contains(&ultimo), "faltou o marcador {ultimo} no lote");
    }

    /// Etapa que não é T-SQL não pode levar banco: a procedure recusa.
    #[test]
    fn banco_so_acompanha_a_etapa_de_tsql() {
        let mut trabalho = trabalho_minimo();
        trabalho.etapas = vec![EtapaDeTrabalho {
            tipo: "CmdExec".to_string(),
            banco: Some("master".to_string()),
            ..etapa()
        }];
        let (_, lote) = montar_lote(&trabalho);
        assert!(
            !lote.textos().contains(&"master".to_string()),
            "o banco foi junto numa etapa de CmdExec"
        );
    }

    #[test]
    fn trabalho_sem_etapa_e_recusado() {
        // O servidor aceitaria, e o trabalho nunca faria nada.
        assert!(validar(&trabalho_minimo()).is_err());
    }

    fn detalhe_de_exemplo() -> DetalheDoTrabalho {
        DetalheDoTrabalho {
            id: "DD3A04EA-C2B8-45EF-BD17-85D0AB89E956".to_string(),
            nome: "Expurgo d'água".to_string(),
            habilitado: true,
            descricao: "Limpa o log".to_string(),
            categoria: "[Uncategorized (Local)]".to_string(),
            proprietario: "sa".to_string(),
            criado_em: Some("2026-08-27 15:11:05".to_string()),
            modificado_em: None,
            ultima_execucao: None,
            etapas: vec![
                etapa(),
                EtapaDeTrabalho {
                    nome: "Copiar".to_string(),
                    tipo: "CmdExec".to_string(),
                    banco: Some("master".to_string()),
                    comando: "cmd /c echo ok".to_string(),
                    ao_ter_exito: 1,
                    ao_falhar: 2,
                },
            ],
            agendas: vec![AgendaDeTrabalho {
                nome: "Semanal".to_string(),
                habilitada: true,
                freq_tipo: 8,
                freq_intervalo: 34,
                freq_subtipo: 1,
                freq_subintervalo: 0,
                freq_intervalo_relativo: 0,
                freq_fator: 2,
                inicio_data: 20_260_827,
                fim_data: 99_991_231,
                inicio_hora: 33000,
                fim_hora: 235_959,
            }],
        }
    }

    /// O DDL é script para copiar e rodar em outro servidor, não enfeite: ele
    /// precisa trazer as quatro procedures, a conferência de retorno e a
    /// transação — senão quem o executa lá recria um trabalho pela metade.
    #[test]
    fn o_ddl_recria_o_trabalho_inteiro() {
        let script = ddl(&detalhe_de_exemplo());
        assert!(script.starts_with("USE [msdb]\nGO"));
        assert_eq!(script.matches("sp_add_job\n").count(), 1);
        assert_eq!(script.matches("sp_add_jobstep").count(), 2);
        assert_eq!(script.matches("sp_add_jobschedule").count(), 1);
        assert_eq!(script.matches("sp_add_jobserver").count(), 1);
        // Uma conferência por procedure, mais a da categoria.
        assert_eq!(script.matches("GOTO Desfazer").count(), 6);
        assert!(script.contains("ROLLBACK TRANSACTION"));
        assert!(script.ends_with("GO\n"));
    }

    /// Apóstrofo em nome de trabalho não pode fechar o literal do script.
    #[test]
    fn o_ddl_dobra_o_apostrofo_do_literal() {
        let script = ddl(&detalhe_de_exemplo());
        assert!(script.contains("N'Expurgo d''água'"), "{script}");
        assert!(!script.contains("N'Expurgo d'água'"));
    }

    /// O banco só acompanha a etapa de T-SQL, também no script.
    #[test]
    fn o_ddl_nao_leva_banco_em_etapa_que_nao_e_tsql() {
        let script = ddl(&detalhe_de_exemplo());
        // A etapa 1 é TSQL e leva `master`; a 2 é CmdExec e vai com NULL, mesmo
        // tendo banco guardado.
        assert_eq!(script.matches("@database_name = N'master'").count(), 1);
        assert_eq!(script.matches("@database_name = NULL").count(), 1);
    }

    #[test]
    fn literal_nulo_nao_vira_texto_vazio() {
        assert_eq!(literal_opcional(None), "NULL");
        assert_eq!(literal_opcional(Some("x")), "N'x'");
    }

    #[test]
    fn texto_em_branco_vira_nulo_e_nao_string_vazia() {
        // `@category_name = N''` seria uma categoria chamada "", que não existe;
        // nulo é o que faz a procedure usar o padrão dela.
        assert_eq!(vazio_e_nulo("   "), None);
        assert_eq!(vazio_e_nulo(" Expurgo "), Some("Expurgo"));
    }
}
