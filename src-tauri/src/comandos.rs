//! Comandos expostos à interface.
//!
//! Este módulo é só a fronteira: valida, delega, registra e traduz o
//! resultado. A lógica vive nos módulos de domínio.
//!
//! **Todo comando que fala com o banco é declarado `#[tauri::command(async)]`.**
//! Sem isso, o Tauri executa o comando na thread principal e a janela congela
//! enquanto a consulta corre — foi o que travou a expansão de Tabelas num
//! schema grande. O atributo `async` numa função síncrona faz o Tauri rodá-la
//! numa thread separada.

use std::fmt::Display;
use std::time::Instant;

use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::agente::{
    self, DadosDeTrabalho, DetalheDoTrabalho, EstadoDoAgente, ExecucaoDoTrabalho, NovoTrabalho,
    Trabalho,
};
use crate::arquivo;
use crate::atualizacao::{self, Andamento, Atualizacao};
use crate::backup;
use crate::erro::{Erro, Resultado};
use crate::execucao::{self, Cursores};
use crate::filtro::Filtro;
use crate::filtros_salvos::{FiltrosDaConexao, RepositorioDeFiltros};
use crate::importacao;
use crate::indice;
use crate::instantclient::Situacao;
use crate::metadados::{self, CacheDeMetadados, Sugestao};
use crate::mongo;
use crate::navegador;
use crate::objetos::{self, Categoria, Coluna, Pagina, ParteDoFonte, Subprograma};
use crate::perfis::{Perfil, Repositorio, TipoBanco};
use crate::plsql;
use crate::recuperacao;
use crate::registro;
use crate::sessao::{self, ResultadoTeste, Sessao, Sessoes};
use crate::sql_texto;
use crate::sqlserver;
use crate::xlsx;

#[tauri::command]
pub fn diagnostico_instantclient(situacao: State<'_, Situacao>) -> Situacao {
    situacao.inner().clone()
}

/// O idioma escolhido no instalador, para a primeira execução (requisito 31).
///
/// `None` quando não há escolha gravada — pasta portátil, ou instalação feita
/// por uma versão anterior a esta. Aí a tela decide sozinha pelo idioma da
/// máquina, como sempre decidiu.
///
/// Não é `async`: a janela espera por ele antes de montar a interface, e uma
/// leitura de registro custa microssegundos.
#[tauri::command]
pub fn idioma_do_instalador() -> Option<String> {
    crate::idioma_instalado::escolhido()
}

/// Arquivos que o Windows mandou abrir — duplo clique, **Abrir com** — e que a
/// interface ainda não abriu (requisito 12).
///
/// Quem pergunta **leva**: a fila sai vazia, e o mesmo arquivo não abre duas
/// vezes por ter sido perguntado duas vezes. Só registra quando há o que
/// entregar — a interface pergunta a cada subida e a cada aviso.
#[tauri::command]
pub fn arquivos_recebidos() -> Vec<String> {
    let arquivos = crate::instancia::retirar();
    if !arquivos.is_empty() {
        registro::informacao(
            "arquivos_recebidos",
            format!("{} arquivo(s) para a interface", arquivos.len()),
        );
    }
    arquivos
}

/// Caminho do arquivo de log, para a interface poder mostrá-lo.
#[tauri::command]
pub fn caminho_do_log() -> Option<String> {
    registro::caminho().map(|c| c.display().to_string())
}

/// Identificação da compilação em execução.
///
/// Existe para responder sem dúvida a "estou olhando a versão nova?": vem da
/// data de modificação do próprio executável, então nunca fica desatualizada.
#[tauri::command]
pub fn versao_em_execucao() -> String {
    let momento = std::env::current_exe()
        .and_then(|caminho| caminho.metadata())
        .and_then(|dados| dados.modified())
        .ok();

    match momento {
        Some(quando) => format!(
            "{} · compilado {}",
            env!("CARGO_PKG_VERSION"),
            registro::formatar(quando)
        ),
        None => env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// Abre um endereço no navegador padrão — o link do autor, na tela Sobre.
#[tauri::command(async)]
pub fn abrir_no_navegador(endereco: String) -> Resultado<()> {
    registrar("abrir_no_navegador", &endereco, || {
        navegador::abrir(&endereco)
    })
}

// --- Atualização -----------------------------------------------------------

/// De quantos em quantos bytes o andamento do download é informado à tela.
///
/// O instalador passa de 100 MB e desce em blocos de 64 KB; avisar a cada bloco
/// seriam milhares de mensagens para mover uma barra que tem poucos pixels.
const PASSO_DO_ANDAMENTO: u64 = 1024 * 1024;

/// Procura no repositório público de releases uma versão mais nova que esta.
///
/// `None` quer dizer que já se está na versão mais recente — é resposta boa, e
/// não erro.
#[tauri::command(async)]
pub fn verificar_atualizacao() -> Resultado<Option<Atualizacao>> {
    registrar("verificar_atualizacao", "", atualizacao::verificar)
}

/// Baixa o instalador da versão nova, informando o andamento pelo canal.
#[tauri::command(async)]
pub fn baixar_atualizacao(endereco: String, andamento: Channel<Andamento>) -> Resultado<()> {
    registrar("baixar_atualizacao", &endereco, || {
        let mut ultimo_aviso = 0u64;
        atualizacao::baixar(&endereco, |baixados, total| {
            let terminou = total > 0 && baixados >= total;
            if baixados - ultimo_aviso >= PASSO_DO_ANDAMENTO || terminou {
                ultimo_aviso = baixados;
                let _ = andamento.send(Andamento { baixados, total });
            }
        })
        .map(|_| ())
    })
}

/// Executa o instalador baixado. Quem o baixou foi o backend, e é ele quem
/// sabe o caminho: a tela não informa qual programa rodar.
#[tauri::command(async)]
pub fn instalar_atualizacao() -> Resultado<()> {
    registrar("instalar_atualizacao", "", atualizacao::instalar)
}

/// Fecha o Cururu para o instalador poder substituir os arquivos em uso.
#[tauri::command]
pub fn encerrar_aplicativo(app: tauri::AppHandle, sessoes: State<'_, Sessoes>) {
    registro::informacao("aplicativo", "encerrando a pedido da atualização");
    // Fecha as sessões antes de sair: o logoff limpo libera os locks na hora,
    // em vez de deixá-los pendurados até o servidor perceber a conexão morta —
    // importante porque agora há uma sessão por aba de consulta.
    sessoes.encerrar_todas();
    app.exit(0);
}

// --- Conexões --------------------------------------------------------------

#[tauri::command(async)]
pub fn listar_perfis(repositorio: State<'_, Repositorio>) -> Resultado<Vec<Perfil>> {
    registrar("listar_perfis", "", || repositorio.listar())
}

#[tauri::command(async)]
pub fn ler_senha_salva(id: String, repositorio: State<'_, Repositorio>) -> Resultado<Option<String>> {
    repositorio.senha_salva(&id)
}

#[tauri::command(async)]
pub fn salvar_perfil(perfil: Perfil, repositorio: State<'_, Repositorio>) -> Resultado<Perfil> {
    registrar("salvar_perfil", &perfil.nome, || {
        repositorio.salvar(perfil.clone())
    })
}

#[tauri::command(async)]
pub fn excluir_perfil(
    id: String,
    repositorio: State<'_, Repositorio>,
    filtros: State<'_, RepositorioDeFiltros>,
    alertas: State<'_, crate::alertas::Repositorio>,
) -> Resultado<()> {
    registrar("excluir_perfil", &id, || {
        repositorio.excluir(&id)?;
        // Sem a conexão, os filtros dela viram lixo no arquivo.
        filtros.esquecer(&id)?;
        alertas.esquecer(&id)
    })
}

#[tauri::command(async)]
pub fn exportar_backup(
    caminho: String,
    botoes: Vec<backup::BotaoRapido>,
    atalhos: std::collections::HashMap<String, (String, String)>,
    repositorio: State<'_, Repositorio>,
) -> Resultado<()> {
    registrar("exportar_backup", &caminho, || {
        backup::exportar(
            std::path::Path::new(&caminho),
            botoes.clone(),
            atalhos.clone(),
            &repositorio,
        )
    })
}

#[tauri::command(async)]
pub fn importar_backup(
    caminho: String,
    repositorio: State<'_, Repositorio>,
) -> Resultado<backup::ConteudoRestaurado> {
    registrar("importar_backup", &caminho, || {
        backup::importar(std::path::Path::new(&caminho), &repositorio)
    })
}

// --- Filtros salvos (requisito 4) ------------------------------------------

/// Filtros guardados de uma conexão, por categoria.
#[tauri::command(async)]
pub fn ler_filtros(
    perfil_id: String,
    filtros: State<'_, RepositorioDeFiltros>,
) -> Resultado<FiltrosDaConexao> {
    registrar("ler_filtros", &perfil_id, || filtros.ler(&perfil_id))
}

/// Guarda o filtro de uma categoria. Sem filtro, apaga o que estava guardado.
#[tauri::command(async)]
pub fn gravar_filtro(
    perfil_id: String,
    categoria: Categoria,
    filtro: Option<Filtro>,
    filtros: State<'_, RepositorioDeFiltros>,
    indice: State<'_, indice::RepositorioDeIndice>,
) -> Resultado<()> {
    let detalhe = format!("{perfil_id} {categoria:?}");
    registrar("gravar_filtro", detalhe, || {
        filtros.gravar(&perfil_id, categoria, filtro.clone())?;
        if let Some(filtro) = filtro.as_ref().filter(|filtro| filtro.ativo()) {
            indice.preparar_listagem(
                &indice::chave(&perfil_id, None),
                categoria,
                Some(filtro),
            );
        }
        Ok(())
    })
}

#[tauri::command(async)]
pub fn testar_conexao(
    perfil: Perfil,
    repositorio: State<'_, Repositorio>,
    situacao: State<'_, Situacao>,
) -> Resultado<ResultadoTeste> {
    registrar("testar_conexao", &perfil.nome, || {
        exigir_instantclient(&situacao, &perfil)?;
        let completo = repositorio.com_senhas(&perfil)?;
        Ok(sessao::testar(&completo))
    })
}

#[tauri::command]
pub async fn testar_mongo(perfil: Perfil, repositorio: State<'_, Repositorio>) -> Resultado<ResultadoTeste> {
    let inicio = Instant::now();
    let resultado = mongo::registrar("testar_mongo", async {
        let completo = perfil_mongo(&perfil, &repositorio)?;
        mongo::Sessao::abrir(&completo).await
    })
    .await;
    Ok(ResultadoTeste {
        sucesso: resultado.is_ok(),
        mensagem: resultado
            .err()
            .map_or_else(|| "Com êxito".to_string(), |erro| erro.mensagem),
        versao_banco: None,
        duracao_ms: inicio.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
pub async fn conectar_mongo(
    perfil: Perfil,
    repositorio: State<'_, Repositorio>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<String> {
    mongo::registrar("conectar_mongo", async {
        let mut completo = perfil_mongo(&perfil, &repositorio)?;
        let conexao = mongo::Sessao::abrir(&completo).await?;
        completo.senha = None;
        completo.senha_proxy = None;
        Ok(sessoes.registrar(Sessao::nova_mongo(
            identificador_de_sessao(),
            completo,
            conexao,
        )))
    })
    .await
}

fn perfil_mongo(perfil: &Perfil, repositorio: &Repositorio) -> Resultado<Perfil> {
    if perfil.tipo_banco != TipoBanco::MongoDb {
        return Err(Erro::novo("esta conexão não é MongoDB"));
    }
    repositorio.com_senhas(perfil)
}

#[tauri::command]
pub async fn listar_bancos_mongo(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Vec<String>> {
    mongo::registrar("listar_bancos_mongo", async {
        sessoes.obter(&sessao_id)?.mongo()?.bancos().await
    })
    .await
}

#[tauri::command]
pub async fn listar_colecoes_mongo(
    sessao_id: String,
    banco: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Vec<String>> {
    mongo::registrar("listar_colecoes_mongo", async {
        sessoes.obter(&sessao_id)?.mongo()?.colecoes(&banco).await
    })
    .await
}

#[tauri::command]
pub async fn consultar_mongo(
    sessao_id: String,
    banco: String,
    colecao: String,
    filtro: String,
    limite: u32,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Vec<serde_json::Value>> {
    mongo::registrar("consultar_mongo", async {
        sessoes
            .obter(&sessao_id)?
            .mongo()?
            .consultar(&banco, &colecao, &filtro, limite)
            .await
    })
    .await
}

#[tauri::command]
pub async fn campos_mongo(
    sessao_id: String,
    banco: String,
    colecao: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Vec<String>> {
    mongo::registrar("campos_mongo", async {
        sessoes.obter(&sessao_id)?.mongo()?.campos(&banco, &colecao).await
    })
    .await
}

/// Abre a conexão e devolve o identificador da sessão criada.
#[tauri::command(async)]
pub fn conectar(
    perfil: Perfil,
    aplicativo: AppHandle,
    repositorio: State<'_, Repositorio>,
    situacao: State<'_, Situacao>,
    sessoes: State<'_, Sessoes>,
    indice: State<'_, indice::RepositorioDeIndice>,
    filtros: State<'_, RepositorioDeFiltros>,
    cache: State<'_, CacheDeMetadados>,
) -> Resultado<String> {
    registrar("conectar", &perfil.nome, || {
        exigir_instantclient(&situacao, &perfil)?;
        let completo = repositorio.com_senhas(&perfil)?;
        if completo.tipo_banco == TipoBanco::Oracle {
            // Quando `conectar` devolve, a árvore já pode ser expandida. Ler os
            // JSONs aqui impede a primeira expansão de pagar desserialização.
            let chave = indice::chave(&completo.id, None);
            indice.preparar(&chave);
            match filtros.ler(&completo.id) {
                Ok(filtros) => indice.preparar_listagens(&chave, &filtros),
                Err(erro) => registro::aviso("indice", format!("filtros não preaquecidos: {erro}")),
            }
        }

        // A sessão fica com as credenciais resolvidas: é com elas que as sessões
        // irmãs das abas (requisito 7) nascem sem repedir a senha, e é com elas
        // que a própria sessão se reabre quando a conexão cai.
        let sessao = match completo.tipo_banco {
            TipoBanco::Oracle => {
                let conexao = sessao::abrir(&completo)?;
                Sessao::nova(identificador_de_sessao(), completo, conexao)
            }
            // Sem banco indicado, a sessão nova se aponta para o primeiro da
            // lista — o mesmo que a árvore mostra logo abaixo de "Banco de
            // Dados".
            // A sessão da conexão abre com auto-commit ligado, qualquer que
            // seja o do perfil: é ela que atende árvore e autocomplete, e com o
            // auto-commit desligado o SQL Server abriria uma transação implícita
            // já na primeira leitura do catálogo — transação que nunca teria
            // commit, porque o usuário não executou nada. Quem respeita o
            // interruptor é a sessão da aba (requisito 7).
            TipoBanco::SqlServer => {
                let conexao = sqlserver::Sessao::abrir(&completo, None, true)?;
                let id = identificador_de_sessao();
                if let Some(banco) = conexao.banco() {
                    cache.preparar_catalogo_sqlserver(&aplicativo, &id, &completo, &banco);
                }
                Sessao::nova_sqlserver(id, completo, conexao)
            }
            TipoBanco::MongoDb => return Err(Erro::novo("use a conexão do MongoDB")),
        };
        Ok(sessoes.registrar(sessao))
    })
}

/// Abre outra sessão na mesma conexão — uma por aba de consulta (requisito 7).
///
/// Cada aba executa na sua própria sessão para que uma consulta pesada numa aba
/// não bloqueie as outras nem o autocomplete. Reaproveita as credenciais
/// retidas no `conectar`, então não repergunta a senha. Recebe o id da sessão
/// de metadados — a que a árvore abriu.
/// `banco` abre a aba num banco específico, e não no da conexão: é o caminho de
/// "Nova Consulta" sobre um banco da árvore (SQL Server). Vazio, a aba nasce
/// onde a conexão está.
#[tauri::command(async)]
pub fn conectar_irma(
    sessao_id: String,
    banco: Option<String>,
    aplicativo: AppHandle,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
) -> Resultado<String> {
    let detalhe = match &banco {
        Some(banco) => format!("{sessao_id} em {banco}"),
        None => sessao_id.clone(),
    };
    registrar("conectar_irma", detalhe, || {
        let origem = sessoes.obter(&sessao_id)?;
        let perfil = origem.perfil.clone();

        let sessao = match perfil.tipo_banco {
            TipoBanco::Oracle => {
                let conexao = sessao::abrir(&perfil)?;
                // A aba nova nasce com o mesmo auto-commit da conexão, igual ao
                // que a barra de ferramentas mostra ao abrir.
                Sessao::nova(identificador_de_sessao(), perfil, conexao)
            }
            // Sem banco indicado, a aba nasce no mesmo banco em que a conexão
            // está: além de ser o que se espera, poupa a ida ao servidor para
            // listar e escolher.
            TipoBanco::SqlServer => {
                let alvo = match banco.clone() {
                    Some(banco) => Some(banco),
                    None => origem.sqlserver()?.banco(),
                };
                // A aba nasce com o auto-commit do perfil, igual ao que a
                // barra de ferramentas mostra ao abrir.
                let auto_commit = perfil.auto_commit;
                let conexao = sqlserver::Sessao::abrir(&perfil, alvo, auto_commit)?;
                if let Some(banco) = conexao.banco() {
                    cache.preparar_catalogo_sqlserver(&aplicativo, &sessao_id, &perfil, &banco);
                }
                Sessao::nova_sqlserver(identificador_de_sessao(), perfil, conexao)
            }
            TipoBanco::MongoDb => return Err(Erro::novo("o MongoDB usa a sessão da conexão")),
        };
        Ok(sessoes.registrar(sessao))
    })
}

/// Bancos do servidor a que a conexão tem acesso (requisito 2, SQL Server).
///
/// `incluir_sistema` separa os dois usos: a árvore da conexão esconde `master`,
/// `tempdb`, `model` e `msdb`; a lista da aba de consulta os oferece, porque
/// consultar o catálogo do `master` é trabalho legítimo de quem administra.
#[tauri::command(async)]
pub fn listar_bancos(
    sessao_id: String,
    incluir_sistema: bool,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Vec<String>> {
    let detalhe = format!(
        "{sessao_id}{}",
        if incluir_sistema {
            " (com sistema)"
        } else {
            ""
        }
    );
    registrar("listar_bancos", detalhe, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .bancos(incluir_sistema)
    })
}

/// Troca o banco em que a aba executa.
///
/// Vale só para a sessão indicada, que é a de uma aba (requisito 7): trocar o
/// banco numa aba não mexe nas outras nem na conexão da árvore, do mesmo jeito
/// que o `USE` do SSMS vale só para a janela em que foi dado.
#[tauri::command(async)]
pub fn usar_banco(
    sessao_id: String,
    banco: String,
    sessao_metadados: Option<String>,
    aplicativo: AppHandle,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
) -> Resultado<()> {
    registrar("usar_banco", format!("{sessao_id} → {banco}"), || {
        sessoes.obter(&sessao_id)?.sqlserver()?.usar_banco(&banco)?;
        if let Some(origem) = sessao_metadados.and_then(|id| sessoes.obter(&id).ok()) {
            cache.preparar_catalogo_sqlserver(&aplicativo, &origem.id, &origem.perfil, &banco);
        }
        Ok(())
    })
}

/// Banco em que a sessão está agora.
///
/// A barra da aba de consulta o mostra no lugar em que o Oracle mostra a Saída
/// DBMS. Devolve `None` para conexões Oracle, que não têm banco corrente.
#[tauri::command(async)]
pub fn banco_da_sessao(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Option<String>> {
    let sessao = sessoes.obter(&sessao_id)?;
    Ok(match sessao.perfil.tipo_banco {
        TipoBanco::Oracle => None,
        TipoBanco::SqlServer => sessao.sqlserver()?.banco(),
        TipoBanco::MongoDb => None,
    })
}

// --- SQL Server Agent (requisito 36) --------------------------------------

/// Situação do serviço SQL Server Agent.
///
/// A árvore usa isto para desenhar o nó desconectado quando o serviço está
/// parado. Responde `desconhecido` quando a conta não tem permissão de olhar o
/// serviço — que não é a mesma coisa que estar parado.
#[tauri::command(async)]
pub fn estado_do_agente(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<EstadoDoAgente> {
    registrar("estado_do_agente", &sessao_id, || {
        sessoes.obter(&sessao_id)?.sqlserver()?.estado_do_agente()
    })
}

/// Trabalhos do Agent, pendurados direto no nó SQL Server Agent.
#[tauri::command(async)]
pub fn listar_trabalhos(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
    filtros: State<'_, RepositorioDeFiltros>,
) -> Resultado<Vec<Trabalho>> {
    registrar("listar_trabalhos", &sessao_id, || {
        let sessao = sessoes.obter(&sessao_id)?;
        // O filtro dos trabalhos é guardado na chave do Programador, que é a
        // categoria de job do outro banco: uma conexão é de um banco só, e a
        // tela do filtro já chama o alvo de "Job" nas duas (requisito 4).
        let filtro = filtros
            .ler(&sessao.perfil.id)?
            .remove(&objetos::Categoria::Programador);
        sessao.sqlserver()?.trabalhos(filtro.as_ref())
    })
}

/// Categorias, logins e nome do servidor: o que a tela de Novo Trabalho precisa
/// saber antes de abrir.
#[tauri::command(async)]
pub fn dados_de_trabalho(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<DadosDeTrabalho> {
    registrar("dados_de_trabalho", &sessao_id, || {
        sessoes.obter(&sessao_id)?.sqlserver()?.dados_de_trabalho()
    })
}

/// Cria um trabalho com as etapas e agendas informadas — tudo ou nada.
#[tauri::command(async)]
pub fn criar_trabalho(
    sessao_id: String,
    trabalho: NovoTrabalho,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let detalhe = format!("{sessao_id}: {}", trabalho.nome);
    registrar("criar_trabalho", detalhe, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .criar_trabalho(trabalho)
    })
}

/// Um trabalho por inteiro, para a tela de propriedades.
#[tauri::command(async)]
pub fn detalhe_do_trabalho(
    sessao_id: String,
    trabalho_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<DetalheDoTrabalho> {
    registrar("detalhe_do_trabalho", &trabalho_id, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .detalhe_do_trabalho(&trabalho_id)
    })
}

/// O script que recria o trabalho, no formato do "Script Job as CREATE" do SSMS.
///
/// A leitura é a mesma da tela de propriedades; o que muda é só o que se faz com
/// ela. Gerar o texto aqui, e não na tela, é o que deixa o formato testável sem
/// um servidor por perto.
#[tauri::command(async)]
pub fn ddl_do_trabalho(
    sessao_id: String,
    trabalho_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<String> {
    registrar("ddl_do_trabalho", &trabalho_id, || {
        let detalhe = sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .detalhe_do_trabalho(&trabalho_id)?;
        Ok(agente::ddl(&detalhe))
    })
}

/// Grava o que a tela de propriedades mudou — tudo ou nada.
#[tauri::command(async)]
pub fn atualizar_trabalho(
    sessao_id: String,
    trabalho_id: String,
    trabalho: NovoTrabalho,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let detalhe = format!("{trabalho_id}: {}", trabalho.nome);
    registrar("atualizar_trabalho", detalhe, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .atualizar_trabalho(&trabalho_id, trabalho)
    })
}

/// Liga ou desliga o trabalho, sem tocar em mais nada dele.
#[tauri::command(async)]
pub fn habilitar_trabalho(
    sessao_id: String,
    trabalho_id: String,
    habilitado: bool,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let detalhe = format!("{trabalho_id} → {habilitado}");
    registrar("habilitar_trabalho", detalhe, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .habilitar_trabalho(&trabalho_id, habilitado)
    })
}

#[tauri::command(async)]
pub fn renomear_trabalho(
    sessao_id: String,
    trabalho_id: String,
    nome: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let detalhe = format!("{trabalho_id} → {nome}");
    registrar("renomear_trabalho", detalhe, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .renomear_trabalho(&trabalho_id, &nome)
    })
}

#[tauri::command(async)]
pub fn excluir_trabalho(
    sessao_id: String,
    trabalho_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    registrar("excluir_trabalho", &trabalho_id, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .excluir_trabalho(&trabalho_id)
    })
}

/// Manda o Agent rodar o trabalho, opcionalmente a partir de uma etapa.
///
/// O comando **agenda** a execução e volta na hora: quem executa é o serviço, e
/// ele não avisa quando termina.
#[tauri::command(async)]
pub fn iniciar_trabalho(
    sessao_id: String,
    trabalho_id: String,
    etapa: Option<String>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let detalhe = match &etapa {
        Some(etapa) => format!("{trabalho_id} na etapa {etapa}"),
        None => trabalho_id.clone(),
    };
    registrar("iniciar_trabalho", detalhe, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .iniciar_trabalho(&trabalho_id, etapa)
    })
}

#[tauri::command(async)]
pub fn parar_trabalho(
    sessao_id: String,
    trabalho_id: String,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    registrar("parar_trabalho", &trabalho_id, || {
        sessoes
            .obter(&sessao_id)?
            .sqlserver()?
            .parar_trabalho(&trabalho_id)
    })
}

/// Em que pé está a execução do trabalho, para a tela de acompanhamento.
///
/// **Não passa pelo `registrar`**, e é o único comando do Agent que não passa: a
/// tela pergunta de segundo em segundo enquanto o trabalho roda, e uma linha de
/// log por pergunta afogaria o arquivo justamente nas horas em que ele mais é
/// lido.
///
/// Vai para o log o que o `iniciar_trabalho` não tinha como dizer: **o fim**,
/// com o resultado e a mensagem do Agent. É para isso que serve
/// `instancia_conhecida` — a instância do histórico que a tela viu antes de
/// disparar. Sem ela não há como distinguir o fim desta execução do resultado da
/// anterior, e o log registraria um fim a cada pergunta enquanto o Agent ainda
/// não pegou o trabalho.
#[tauri::command(async)]
pub fn execucao_do_trabalho(
    sessao_id: String,
    trabalho_id: String,
    instancia_conhecida: Option<i32>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<ExecucaoDoTrabalho> {
    let inicio = Instant::now();
    let resultado = sessoes
        .obter(&sessao_id)
        .and_then(|sessao| sessao.sqlserver()?.execucao_do_trabalho(&trabalho_id));

    match &resultado {
        Ok(execucao)
            if !execucao.executando
                && instancia_conhecida.is_some_and(|antes| execucao.instancia > antes) =>
        {
            registro::informacao(
                "execucao_do_trabalho",
                format!(
                    "{trabalho_id} — terminou {:?} em {} s: {}",
                    execucao.resultado,
                    execucao.duracao.unwrap_or(0),
                    execucao.mensagem.trim()
                ),
            );
        }
        Ok(_) => {}
        Err(erro) => registro::erro(
            "execucao_do_trabalho",
            format!(
                "{trabalho_id} — falhou em {} ms: {erro}",
                inicio.elapsed().as_millis()
            ),
        ),
    }
    resultado
}

#[tauri::command(async)]
pub fn desconectar(
    id: String,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
    cursores: State<'_, Cursores>,
) {
    registro::informacao("desconectar", &id);
    cache.invalidar(&id);
    cursores.fechar_da_sessao(&id);
    sessoes.encerrar(&id);
}

// --- Árvore de objetos (requisitos 2 e 4) ---------------------------------

/// Uma página de objetos de uma categoria.
///
/// `banco` só existe no SQL Server, onde a árvore pendura as categorias de cada
/// banco — e onde o filtro da tela ainda não vale: os critérios do requisito 4
/// são montados em SQL do Oracle, com bind nomeado.
#[tauri::command(async)]
pub fn listar_objetos(
    sessao_id: String,
    categoria: Categoria,
    banco: Option<String>,
    filtro: Option<Filtro>,
    deslocamento: u32,
    limite: u32,
    atualizar: Option<bool>,
    indice: State<'_, indice::RepositorioDeIndice>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Pagina> {
    let detalhe = format!(
        "{categoria:?}{} {} critério(s) a partir de {deslocamento} ({limite} por vez)",
        banco
            .as_deref()
            .map(|b| format!(" em {b}"))
            .unwrap_or_default(),
        filtro.as_ref().map_or(0, |f| f.criterios.len())
    );
    registrar("listar_objetos", detalhe, || {
        let sessao = sessoes.obter(&sessao_id)?;
        if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
            return sessao.sqlserver()?.listar_objetos(
                banco.as_deref(),
                categoria,
                filtro.as_ref(),
                deslocamento,
                limite,
            );
        }
        let chave = indice::chave(&sessao.perfil.id, None);
        if matches!(categoria, Categoria::Tabelas | Categoria::Views) {
            if !atualizar.unwrap_or(false) {
                if let Some(pagina) =
                    indice.listar_relacoes(&chave, categoria, filtro.as_ref(), deslocamento, limite)
                {
                    registro::informacao(
                        "listar_relacoes_indice",
                        format!("{} {categoria:?}: {} objeto(s)", sessao.perfil.nome, pagina.objetos.len()),
                    );
                    return Ok(pagina);
                }
            } else {
                let mut relacoes = sessao::com_reconexao(&sessao, |conexao| {
                    let mut relacoes = crate::relacoes::ler(conexao, false, None, None, None)?;
                    crate::relacoes::anexar_comentarios_de_views(conexao, &mut relacoes)?;
                    Ok(relacoes)
                })?;
                // A variável é mutável apenas durante a leitura dos comentários.
                indice.guardar_relacoes(&chave, false, std::mem::take(&mut relacoes));
                if let Some(pagina) =
                    indice.listar_relacoes(&chave, categoria, filtro.as_ref(), deslocamento, limite)
                {
                    return Ok(pagina);
                }
            }
        }
        if categoria == Categoria::Procedures {
            if !atualizar.unwrap_or(false) {
                if let Some(pagina) =
                    indice.listar_procedures(&chave, filtro.as_ref(), deslocamento, limite)
                {
                    registro::informacao(
                        "listar_procedures_indice",
                        format!("{}: {} procedure(s)", sessao.perfil.nome, pagina.objetos.len()),
                    );
                    return Ok(pagina);
                }
            } else {
                let procedures = sessao::com_reconexao(&sessao, |conexao| {
                    let mut procedures = Vec::new();
                    for linha in conexao.query(
                        "SELECT object_name, status, generated FROM user_objects \
                         WHERE object_type = 'PROCEDURE' ORDER BY object_name",
                        &[],
                    )? {
                        let linha = linha?;
                        procedures.push(indice::PacoteIndexado {
                            nome: linha.get(0)?,
                            situacao: linha.get(1)?,
                            gerado: linha.get(2)?,
                        });
                    }
                    Ok(procedures)
                })?;
                indice.guardar_procedures(&chave, procedures);
                if let Some(pagina) =
                    indice.listar_procedures(&chave, filtro.as_ref(), deslocamento, limite)
                {
                    return Ok(pagina);
                }
            }
        }
        if categoria == Categoria::Pacotes {
            if !atualizar.unwrap_or(false) {
                if let Some(pagina) =
                    indice.listar_pacotes(&chave, filtro.as_ref(), deslocamento, limite)
                {
                    registro::informacao(
                        "listar_pacotes_indice",
                        format!("{}: {} pacote(s)", sessao.perfil.nome, pagina.objetos.len()),
                    );
                    return Ok(pagina);
                }
            }
            let pacotes = sessao::com_reconexao(&sessao, |conexao| {
                let mut pacotes = Vec::new();
                for linha in conexao.query("SELECT object_name, status, generated FROM user_objects WHERE object_type = 'PACKAGE' ORDER BY object_name", &[])? {
                    let linha = linha?;
                    pacotes.push(indice::PacoteIndexado { nome: linha.get(0)?, situacao: linha.get(1)?, gerado: linha.get(2)? });
                }
                Ok(pacotes)
            })?;
            indice.guardar_pacotes(&chave, pacotes);
            if let Some(pagina) =
                indice.listar_pacotes(&chave, filtro.as_ref(), deslocamento, limite)
            {
                return Ok(pagina);
            }
        }
        sessao::com_reconexao(&sessao, |conexao| {
            objetos::listar(conexao, categoria, filtro.as_ref(), deslocamento, limite)
        })
    })
}

/// Colunas de um objeto, para a árvore e para o autocomplete.
///
/// `banco` só existe no SQL Server, e diz de qual banco são as colunas: quem
/// responde é a sessão da **conexão**, e a aba pode estar em outro banco
/// (requisito 3). No Oracle vem vazio.
#[tauri::command(async)]
pub fn listar_colunas(
    sessao_id: String,
    objeto: String,
    banco: Option<String>,
    para_editor: Option<bool>,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
) -> Resultado<Vec<Coluna>> {
    // Sem `registrar`: com as colunas guardadas por sessão, a maior parte das
    // chamadas não vai ao banco e encheria o log de "ok em 0 ms". Quem registra
    // é o cache, e só quando a leitura acontece de verdade.
    //
    // O `inspect_err` da sessão segue o mesmo motivo do `sugerir_objetos`: sem
    // ele, uma sessão que não resolve derruba o autocomplete em silêncio, e o
    // sintoma na tela é idêntico ao de "não achou nada".
    let sessao = sessoes.obter(&sessao_id).inspect_err(|erro| {
        registro::erro(
            "listar_colunas",
            format!("sessão {sessao_id} indisponível: {erro}"),
        );
    })?;
    if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        let conexao = sessao.sqlserver()?;
        if para_editor == Some(true) {
            if let Some(alvo) = banco.as_deref() {
                if let Some(colunas) = cache.colunas_no_catalogo(&sessao_id, alvo, &objeto) {
                    return Ok(colunas);
                }
            }
        }
        return cache.colunas_sqlserver(&sessao_id, conexao, banco.as_deref(), &objeto);
    }
    sessao::com_reconexao(&sessao, |conexao| {
        cache.colunas(&sessao_id, conexao, &objeto)
    })
}

#[tauri::command(async)]
pub fn listar_subprogramas(
    sessao_id: String,
    pacote: String,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
    indice: State<'_, indice::RepositorioDeIndice>,
) -> Resultado<Vec<Subprograma>> {
    // Sem `registrar`, pelo mesmo motivo de `listar_colunas`: com os
    // subprogramas guardados por sessão, a maior parte das chamadas não vai ao
    // banco. Quem registra é o cache, e só quando a leitura acontece.
    let sessao = sessoes.obter(&sessao_id)?;

    // O índice em disco vem antes do banco (requisito 40). Um pacote raramente
    // ganha rotina nova, e esta leitura custa de 14 a 23 segundos neste
    // dicionário: pagá-la a cada abertura do aplicativo é o que o índice evita.
    let chave = indice::chave(&sessao.perfil.id, None);
    if let Some(guardados) = indice.subprogramas(&chave, &pacote) {
        return Ok(guardados);
    }

    let lidos = sessao::com_reconexao(&sessao, |conexao| {
        cache.subprogramas(&sessao_id, conexao, &pacote)
    })?;
    // Lista vazia também fica gravada: é a resposta de todo nome com ponto
    // atrás que não é pacote, e é ela que evita repetir a leitura cara.
    indice.guardar_subprogramas(&chave, &pacote, &lidos);
    Ok(lidos)
}

/// O fonte do objeto — ou, nas tabelas, o DDL que o recria.
///
/// `parte` só faz sentido no Oracle, onde o pacote tem especificação e corpo
/// separados; no SQL Server o módulo é um só e o parâmetro é ignorado.
#[tauri::command(async)]
pub fn fonte_do_objeto(
    sessao_id: String,
    categoria: Categoria,
    nome: String,
    parte: ParteDoFonte,
    banco: Option<String>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<String> {
    registrar(
        "fonte_do_objeto",
        format!("{categoria:?} {nome} {parte:?}"),
        || {
            let sessao = sessoes.obter(&sessao_id)?;
            if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
                return sessao
                    .sqlserver()?
                    .fonte_do_objeto(banco.as_deref(), categoria, &nome);
            }
            sessao::com_reconexao(&sessao, |conexao| {
                objetos::fonte(conexao, categoria, &nome, parte)
            })
        },
    )
}

/// Grava o comentário de uma view; texto vazio apaga o que havia (requisito 37).
///
/// Não passa por [`sessao::com_reconexao`], e de propósito: aquele caminho
/// repete a operação quando a conexão caiu, e repetir sozinho só é seguro em
/// leitura. Uma gravação repetida em silêncio é o tipo de coisa que ninguém
/// quer descobrir depois.
///
/// `banco` só existe no SQL Server, como no resto da árvore. No Oracle vem
/// vazio, e o alcance é o schema do usuário conectado (requisito 2).
#[tauri::command(async)]
pub fn gravar_comentario(
    sessao_id: String,
    nome: String,
    comentario: String,
    banco: Option<String>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let detalhe = format!(
        "{nome}{} — {}",
        banco
            .as_deref()
            .map(|b| format!(" em {b}"))
            .unwrap_or_default(),
        if comentario.trim().is_empty() {
            "apagando".to_string()
        } else {
            format!("{} caractere(s)", comentario.trim().chars().count())
        }
    );
    registrar("gravar_comentario", detalhe, || {
        let sessao = sessoes.obter(&sessao_id)?;
        if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
            return sessao
                .sqlserver()?
                .gravar_comentario(banco.as_deref(), &nome, &comentario);
        }
        let conexao = sessao.oracle()?;
        objetos::gravar_comentario(&conexao, &nome, &comentario)
    })
}

/// Descobre o destino de um Ctrl+clique no editor SQL.
///
/// Roda na conexão de navegação, como o sublinhado: na de metadados, a
/// estrutura da tabela aberta em seguida esperava na fila atrás dele — no log,
/// 44 s para ler 25 colunas.
#[tauri::command(async)]
pub fn resolver_referencia(
    sessao_id: String,
    referencia: String,
    // Só o SQL Server usa: é o banco em que a aba está, e é nele que o nome
    // escrito na consulta é resolvido.
    banco: Option<String>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Option<objetos::DestinoDaReferencia>> {
    registrar("resolver_referencia", &referencia, || {
        let sessao = sessoes.obter(&sessao_id)?;
        if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
            return sessao
                .sqlserver()?
                .resolver_referencia(banco.as_deref(), &referencia);
        }
        sessao
            .com_conexao_de_navegacao(|conexao| objetos::resolver_referencia(conexao, &referencia))
    })
}

/// Abre e prepara a conexão de navegação antes do primeiro Ctrl (requisito
/// 22).
///
/// A tela chama ao abrir uma aba de consulta do Oracle, sem esperar a resposta.
/// A segunda aba da mesma conexão encontra tudo pronto e volta na hora. O
/// sucesso fica no log pelas linhas `[navegacao]`; aqui só se registra a falha.
#[tauri::command(async)]
pub fn preparar_navegacao(sessao_id: String, sessoes: State<'_, Sessoes>) -> Resultado<()> {
    let sessao = sessoes.obter(&sessao_id)?;
    if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        return Ok(());
    }
    sessao
        .preparar_navegacao(objetos::preparar_resolucao)
        .inspect_err(|erro| {
            registro::erro("preparar_navegacao", format!("sessão {sessao_id}: {erro}"));
        })
}

/// Quais nomes do editor o Ctrl+clique abre — os que ficam sublinhados com o
/// Ctrl apertado (requisito 22).
///
/// Roda na conexão de navegação da sessão, e não na de metadados: ver
/// [`sessao::Sessao::com_conexao_de_navegacao`]. A linha do log diz quantos
/// nomes vieram e quantos têm destino, que é o que separa "demorou" de "não
/// achou nada".
#[tauri::command(async)]
pub fn referencias_navegaveis(
    sessao_id: String,
    referencias: Vec<String>,
    // Só o SQL Server usa, como em `resolver_referencia`.
    banco: Option<String>,
    sessoes: State<'_, Sessoes>,
) -> Resultado<Vec<String>> {
    let inicio = Instant::now();
    let resultado = sessoes.obter(&sessao_id).and_then(|sessao| {
        if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
            return sessao
                .sqlserver()?
                .referencias_navegaveis(banco.as_deref(), &referencias);
        }
        sessao.com_conexao_de_navegacao(|conexao| {
            objetos::referencias_navegaveis(conexao, &referencias)
        })
    });

    let ms = inicio.elapsed().as_millis();
    match &resultado {
        Ok(navegaveis) => registro::informacao(
            "referencias_navegaveis",
            format!(
                "{} nome(s), {} com destino — ok em {ms} ms",
                referencias.len(),
                navegaveis.len()
            ),
        ),
        Err(erro) => registro::erro(
            "referencias_navegaveis",
            format!("{} nome(s) — falhou em {ms} ms: {erro}", referencias.len()),
        ),
    }
    resultado
}

// --- Autocomplete (requisitos 3 e 23) --------------------------------------

/// Nomes do schema que começam pelo prefixo digitado.
///
/// `banco` e `esquema` são as duas primeiras partes do nome de três partes do
/// SQL Server (`Vendas.dbo.cli`), e chegam preenchidos quando o editor as vê
/// escritas. O Oracle não as usa: lá o dono aparece ao lado de cada nome, sem
/// precisar ser escrito para o nome ser sugerido.
#[tauri::command(async)]
pub fn sugerir_objetos(
    sessao_id: String,
    prefixo: String,
    banco: Option<String>,
    esquema: Option<String>,
    // `apenas_rotinas` diz que o cursor está fora de lugar de tabela: ali só
    // cabe o que se chama. Com isso o índice responde pelo grupo das rotinas,
    // que está sempre inteiro, e `EXECUTE ` abre a lista sem prefixo nenhum.
    apenas_rotinas: bool,
    // A opção **Tabelas** das Configurações (requisito 43): com ela ligada, só
    // as tabelas que passam pelo filtro da conexão são sugeridas.
    apenas_tabelas_filtradas: bool,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
    indice: State<'_, indice::RepositorioDeIndice>,
    filtros: State<'_, RepositorioDeFiltros>,
) -> Resultado<Vec<Sugestao>> {
    // Sem `registrar`: isto roda a cada tecla digitada e encheria o log. As
    // buscas demoradas são registradas de dentro do próprio cache.
    //
    // O `inspect_err` aqui não é decoração: sem ele, uma sessão que não resolve
    // derruba o autocomplete a cada tecla sem deixar rastro nenhum no log, e o
    // sintoma na tela é idêntico ao de "não achou nada".
    let sessao = sessoes.obter(&sessao_id).inspect_err(|erro| {
        registro::erro(
            "sugerir_objetos",
            format!("sessão {sessao_id} indisponível: {erro}"),
        );
    })?;
    // O filtro só é lido quando vai ser usado: em "todas" não há recorte a
    // aplicar, e ir ao disco por ele a cada tecla seria trabalho jogado fora.
    // Vale para os dois bancos — o que muda é o SQL que sai dele.
    let filtro = if apenas_tabelas_filtradas {
        filtros
            .ler(&sessao.perfil.id)
            .unwrap_or_default()
            .remove(&Categoria::Tabelas)
            .filter(|f| f.ativo())
    } else {
        None
    };
    let achadas = if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        cache.sugerir_sqlserver(
            &sessao_id,
            sessao.sqlserver()?,
            banco.as_deref(),
            esquema.as_deref(),
            &prefixo,
            filtro.as_ref(),
            apenas_tabelas_filtradas,
        )
    } else {
        // O índice local responde só **fora de lugar de tabela** (requisito
        // 40): ali só cabe o que se chama, e as rotinas estão nele inteiras. No
        // lugar de tabela ele responderia pela metade — view, sinônimo e objeto
        // de outro dono não estão nele —, então ali vale a consulta por
        // prefixo, com o recorte da opção Tabelas dentro do próprio SQL.
        let do_indice = if apenas_rotinas {
            indice.rotinas_que_comecam_com(&indice::chave(&sessao.perfil.id, None), &prefixo)
        } else {
            None
        };
        let chave_da_sessao = metadados::chave_de_sugestoes(
            &sessao_id,
            do_indice.is_some(),
            apenas_tabelas_filtradas,
        );
        sessao::com_reconexao(&sessao, |conexao| {
            cache.sugerir(
                &chave_da_sessao,
                conexao,
                &prefixo,
                do_indice.as_deref(),
                filtro.as_ref(),
            )
        })
    };
    achadas.inspect_err(|erro| {
        registro::erro("sugerir_objetos", format!("falha ao sugerir: {erro}"));
    })
}

/// Duas fontes independentes do autocomplete Oracle. A fonte local não
/// adquire conexão nem espera a fila de consultas do dicionário.
#[tauri::command(async)]
pub fn sugerir_relacoes(
    sessao_id: String,
    prefixo: String,
    esquema: Option<String>,
    local: bool,
    apenas_tabelas_filtradas: bool,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
    indice: State<'_, indice::RepositorioDeIndice>,
    filtros: State<'_, RepositorioDeFiltros>,
) -> Resultado<Vec<Sugestao>> {
    let sessao = sessoes.obter(&sessao_id)?;
    if sessao.perfil.tipo_banco != TipoBanco::Oracle {
        return Ok(Vec::new());
    }
    let filtro = if apenas_tabelas_filtradas {
        filtros
            .ler(&sessao.perfil.id)?
            .remove(&Categoria::Tabelas)
            .filter(|f| f.ativo())
    } else {
        None
    };
    let inicio = std::time::Instant::now();
    let resultado = if local {
        let achadas = indice.sugerir_relacoes(
            &indice::chave(&sessao.perfil.id, None),
            &prefixo,
            esquema.as_deref(),
            filtro.as_ref(),
        );
        registro::informacao(
            "sugerir_relacoes_indice",
            format!(
                "{} {prefixo:?}: {} nome(s) em {} ms",
                sessao.perfil.nome,
                achadas.len(),
                inicio.elapsed().as_millis()
            ),
        );
        Ok(achadas)
    } else {
        sessao::com_reconexao(&sessao, |conexao| {
            cache.sugerir_relacoes(
                &sessao_id,
                conexao,
                &prefixo,
                esquema.as_deref(),
                filtro.as_ref(),
            )
        })
    };
    resultado.inspect_err(|erro| registro::erro("sugerir_relacoes", erro))
}

/// Lê os objetos do usuário e guarda no índice local (requisito 40).
///
/// Chamado pela tela **sem esperar resposta**, logo depois de conectar. O JSON
/// anterior já atende o autocomplete enquanto esta conferência procura objetos
/// novos e substitui o retrato em memória e em disco.
///
/// **Abre uma conexão só para isto, e a fecha ao terminar.** A conexão de
/// metadados atende um pedido por vez, e esta leitura levou 29 segundos num
/// schema grande: feita ali, ela seguraria o autocomplete e a árvore por todo
/// esse tempo — foi o que aconteceu, e está no log ao lado de uma sugestão que
/// esperou 25 segundos.
#[tauri::command(async)]
pub fn indexar_objetos(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
    indice: State<'_, indice::RepositorioDeIndice>,
    filtros: State<'_, RepositorioDeFiltros>,
) -> Resultado<usize> {
    let sessao = sessoes.obter(&sessao_id)?;
    // Só Oracle por ora: é lá que a leitura do dicionário custa caro, e o SQL
    // Server ainda precisaria da chave por banco.
    if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        return Ok(0);
    }
    let chave = indice::chave(&sessao.perfil.id, None);

    let inicio = std::time::Instant::now();
    indice.preparar(&chave);
    let propria = sessao::abrir(&sessao.perfil)?;
    // Publica os nomes consultáveis antes das rotinas de packages, cuja leitura
    // é mais demorada. Uma etapa que falha preserva o retrato anterior.
    for outros in [false, true] {
        let etapa = std::time::Instant::now();
        match crate::relacoes::ler(&propria, outros, None, None, None) {
            Ok(mut relacoes) => {
                if !outros {
                    if let Err(erro) = crate::relacoes::anexar_comentarios_de_views(&propria, &mut relacoes) {
                        registro::erro("indexar_relacoes", format!("{}: comentários de views: {erro}", sessao.perfil.nome));
                    }
                }
                registro::informacao(
                    "indexar_relacoes",
                    format!(
                        "{} outros={outros}: {} nome(s) em {} ms",
                        sessao.perfil.nome,
                        relacoes.len(),
                        etapa.elapsed().as_millis()
                    ),
                );
                indice.guardar_relacoes(&chave, outros, relacoes);
            }
            Err(erro) => registro::erro(
                "indexar_relacoes",
                format!("{} outros={outros}: {erro}", sessao.perfil.nome),
            ),
        }
    }
    let acervo = metadados::acervo_do_usuario(&propria)?;
    registro::informacao(
        "indexar_objetos",
        format!(
            "{}: {} rotina(s) e {} pacote(s) detalhado(s) em {} ms",
            sessao.perfil.nome,
            acervo.rotinas.len(),
            acervo.subprogramas.len(),
            inicio.elapsed().as_millis(),
        ),
    );
    let guardados = acervo.rotinas.len();
    indice.guardar_acervo(&chave, &acervo);
    // A atualização substituiu o retrato e invalidou as listas antigas. Refaz
    // os filtros ativos agora, ainda em background, para a expansão continuar
    // sendo apenas uma paginação do resultado pronto.
    match filtros.ler(&sessao.perfil.id) {
        Ok(filtros) => indice.preparar_listagens(&chave, &filtros),
        Err(erro) => registro::aviso("indice", format!("filtros não reaquecidos: {erro}")),
    }
    // Uma sugestão pedida enquanto a conferência rodava pode ter ficado no
    // cache da sessão. O retrato novo precisa vencer esse resultado antigo.
    cache.invalidar(&sessao_id);
    Ok(guardados)
}

/// Descarta o cache de nomes da sessão, para reler após criar objetos.
#[tauri::command(async)]
pub fn atualizar_metadados(
    sessao_id: String,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
    indice: State<'_, indice::RepositorioDeIndice>,
) {
    registro::informacao("atualizar_metadados", &sessao_id);
    cache.invalidar(&sessao_id);
    // O índice em disco também precisa sair: ele guarda o que existia da última
    // vez, e é justamente isso que deixa de valer depois de criar ou recompilar
    // um objeto.
    if let Ok(sessao) = sessoes.obter(&sessao_id) {
        indice.esquecer(&indice::chave(&sessao.perfil.id, None));
    }
}

// --- Execução de SQL (requisitos 5, 14, 15 e 17) ---------------------------

/// Nomes das bind variables de um texto, para a tela pedir os valores.
#[tauri::command]
pub fn analisar_binds(texto: String, substituicoes_oracle: Option<bool>) -> Vec<String> {
    let mut nomes: Vec<String> = Vec::new();
    let texto = if substituicoes_oracle == Some(true) {
        let substituido = sql_texto::substituir_variaveis_oracle(&texto, |nome| {
            let nome = format!("&{nome}");
            if !nomes.iter().any(|existente| existente.eq_ignore_ascii_case(&nome)) {
                nomes.push(nome.clone());
            }
            nome
        });
        sql_texto::preparar_script_oracle(&substituido).0
    } else { texto };
    let comandos = sql_texto::separar_comandos(&texto);
    for comando in &comandos {
        for nome in sql_texto::binds(&comando.sql) {
            if !nomes
                .iter()
                .any(|existente| existente.eq_ignore_ascii_case(&nome))
            {
                nomes.push(nome);
            }
        }
    }
    nomes
}

/// Executa o texto informado.
///
/// A regra de o que executar (seleção ou tela inteira) é da interface: aqui
/// chega só o texto que deve rodar. `modo_script` muda o que se faz com o
/// resultado, não o que se executa.
#[tauri::command(async)]
pub fn executar_sql(
    sessao_id: String,
    texto: String,
    modo_script: bool,
    binds: Vec<(String, String)>,
    tamanho_do_fetch: u32,
    sessoes: State<'_, Sessoes>,
    cursores: State<'_, Cursores>,
) -> Resultado<Vec<execucao::ResultadoDaExecucao>> {
    let sessao = sessoes.obter(&sessao_id)?;
    // A separação é do dialeto: no Oracle o `;` encerra o comando; no T-SQL ele
    // separa instruções **dentro** do lote, e quem encerra é a linha com `GO`
    // (ver `sql_texto::separar_lotes`).
    let sql_server = sessao.perfil.tipo_banco == TipoBanco::SqlServer;
    let (texto, opcoes_oracle) = if sql_server || !modo_script {
        (texto, sql_texto::OpcoesDoScriptOracle::default())
    } else {
        let mut faltantes = Vec::new();
        let texto = sql_texto::substituir_variaveis_oracle(&texto, |nome| {
            let chave = format!("&{nome}");
            match binds.iter().find(|(n, _)| n.eq_ignore_ascii_case(&chave)) {
                Some((_, valor)) => valor.clone(),
                None => { faltantes.push(chave.clone()); chave }
            }
        });
        if !faltantes.is_empty() {
            return registrar("executar_sql", &sessao.perfil.nome, || {
                Err(Erro::novo(format!("variáveis de substituição sem valor: {}", faltantes.join(", "))))
            });
        }
        sql_texto::preparar_script_oracle(&texto)
    };
    let binds: Vec<_> = binds.into_iter().filter(|(nome, _)| !nome.starts_with('&')).collect();
    let comandos = if sql_server {
        sql_texto::separar_lotes(&texto)
    } else {
        sql_texto::separar_comandos(&texto)
    };
    if comandos.is_empty() {
        return Err(Erro::novo("não há comando para executar"));
    }
    // A conexão vai no log: um script que falhou precisa dizer se foi em
    // produção ou em homologação, sem depender de cruzar ids de sessão.
    let detalhe = format!(
        "{}: {} {}{}",
        sessao.perfil.nome,
        comandos.len(),
        if sql_server { "lote(s)" } else { "comando(s)" },
        if modo_script { " em script" } else { "" }
    );

    if sql_server {
        // O cursor do SQL Server mora na thread da sessão, e não no mapa de
        // cursores: ele empresta o cliente e não sobrevive fora dela. Por isso a
        // execução inteira — inclusive o fetch parcial — passa pelo canal da
        // sessão, e não por este módulo.
        return registrar("executar_sql", detalhe, || {
            sessao.sqlserver()?.executar(
                &sessao_id,
                comandos,
                binds,
                tamanho_do_fetch as usize,
                !modo_script,
            )
        });
    }

    registrar("executar_sql", detalhe, || {
        if let Some(ligada) = opcoes_oracle.saida_dbms {
            let conexao = sessao.oracle()?;
            let tamanho = sessao
                .buffer_dbms
                .load(std::sync::atomic::Ordering::Relaxed)
                .max(20_000);
            if ligada {
                execucao::ligar_saida_dbms(&conexao, tamanho)?;
            } else {
                execucao::desligar_saida_dbms(&conexao)?;
            }
            sessao
                .buffer_dbms
                .store(tamanho, std::sync::atomic::Ordering::Relaxed);
            sessao
                .saida_dbms
                .store(ligada, std::sync::atomic::Ordering::Relaxed);
            registro::informacao(
                "diretiva_sqlplus",
                format!("SET SERVEROUTPUT {}", if ligada { "ON" } else { "OFF" }),
            );
        }

        let lote = || -> Resultado<Vec<execucao::ResultadoDaExecucao>> {
            let mut resultados = Vec::with_capacity(comandos.len());
            for (indice, comando) in comandos.iter().enumerate() {
                match execucao::executar(
                    &sessao,
                    &cursores,
                    comando,
                    &binds,
                    tamanho_do_fetch as usize,
                    !modo_script,
                ) {
                    Ok(resultado) => resultados.push(resultado),
                    // O que rodou antes da falha volta junto com ela, como no
                    // SQL Server: um `CREATE` que valeu antes de o script parar
                    // já está no banco, e a tela não pode mostrar só o erro.
                    Err(erro) => {
                        registro::erro(
                            "executar_sql",
                            format!(
                                "parou no comando {} de {}, linha {}: {}",
                                indice + 1,
                                comandos.len(),
                                comando.linha,
                                sql_texto::sem_comentarios_iniciais(&comando.sql)
                                    .lines()
                                    .next()
                                    .unwrap_or("")
                            ),
                        );
                        if opcoes_oracle.rollback_ao_erro {
                            match sessao.oracle()?.rollback() {
                                Ok(()) => registro::informacao(
                                    "diretiva_sqlplus",
                                    "WHENEVER SQLERROR: rollback executado",
                                ),
                                Err(erro_rollback) => registro::erro(
                                    "diretiva_sqlplus",
                                    format!("WHENEVER SQLERROR: rollback falhou: {erro_rollback}"),
                                ),
                            }
                        }
                        return Err(execucao::situar_erro(erro, comando).apos(resultados));
                    }
                }
            }
            Ok(resultados)
        };

        let conexao = sessao.oracle()?;
        let erro = match lote() {
            Ok(resultados) => return Ok(resultados),
            Err(erro) => erro,
        };
        if !sessao::e_falha_de_conexao(&erro, &conexao) {
            return Err(erro);
        }

        // Aqui está o caso da VPN que cai e volta: a conexão morreu, mas a aba
        // não precisa morrer com ela. Reabre-se a sessão na hora, para esta
        // tentativa ou para a próxima.
        registro::aviso(
            "executar_sql",
            format!("a conexão da sessão {sessao_id} caiu: {erro}"),
        );
        // Os cursores abertos eram da conexão que morreu; o grid que pedir mais
        // linhas vai ouvir que aquele resultado acabou, o que é a verdade.
        cursores.fechar_da_sessao(&sessao_id);
        sessao.reconectar(&conexao)?;

        // Repete-se sozinho só o lote que é todo de consulta: reexecutar um
        // `select` não muda nada no banco. Havendo qualquer comando que altera
        // dados, a sessão volta de pé mas o lote não roda de novo — não há como
        // saber se ele chegou a valer antes de a conexão cair, e essa decisão é
        // do usuário.
        let so_consultas = comandos
            .iter()
            .all(|comando| sql_texto::e_consulta(&sql_texto::preparar_para_execucao(&comando.sql)));
        if !so_consultas {
            return Err(erro.reconectado());
        }

        let mut resultados = lote()?;
        if let Some(primeiro) = resultados.first_mut() {
            primeiro.reconectado = true;
        }
        Ok(resultados)
    })
}

/// Traz mais linhas de uma consulta já executada (requisito 10).
///
/// O identificador do cursor começa pelo da sessão que o abriu, e é por ele que
/// se sabe a quem perguntar: no Oracle o cursor está no mapa deste processo; no
/// SQL Server ele está aberto dentro da thread da sessão.
#[tauri::command(async)]
pub fn buscar_mais_linhas(
    cursor_id: String,
    quantidade: u32,
    sessoes: State<'_, Sessoes>,
    cursores: State<'_, Cursores>,
) -> Resultado<execucao::BlocoDeLinhas> {
    if let Some(sessao) = sessao_do_cursor(&cursor_id, &sessoes) {
        return sessao
            .sqlserver()?
            .buscar_mais(&cursor_id, quantidade as usize);
    }
    cursores.buscar_mais(&cursor_id, quantidade as usize)
}

#[tauri::command(async)]
pub fn fechar_cursor(
    cursor_id: String,
    sessoes: State<'_, Sessoes>,
    cursores: State<'_, Cursores>,
) {
    if let Some(sessao) = sessao_do_cursor(&cursor_id, &sessoes) {
        if let Ok(sqlserver) = sessao.sqlserver() {
            let _ = sqlserver.fechar_cursor(&cursor_id);
        }
        return;
    }
    cursores.fechar(&cursor_id);
}

/// A sessão de SQL Server dona de um cursor, quando é o caso.
///
/// Devolve vazio para cursor do Oracle e para sessão que já foi encerrada — nos
/// dois casos quem responde é o mapa de cursores, que sabe dizer que aquele
/// resultado acabou.
fn sessao_do_cursor(cursor_id: &str, sessoes: &Sessoes) -> Option<std::sync::Arc<Sessao>> {
    let (sessao_id, _) = cursor_id.split_once(':')?;
    let sessao = sessoes.obter(sessao_id).ok()?;
    (sessao.perfil.tipo_banco == TipoBanco::SqlServer).then_some(sessao)
}

/// Interrompe o que estiver rodando na sessão (requisito 14).
///
/// Não é `async` de propósito: precisa correr agora, sem esperar a vez numa
/// fila atrás da consulta que se quer interromper.
#[tauri::command]
pub fn cancelar_execucao(sessao_id: String, sessoes: State<'_, Sessoes>) -> Resultado<()> {
    let sessao = sessoes.obter(&sessao_id)?;
    registro::aviso("cancelar_execucao", &sessao_id);
    if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        // No SQL Server interromper é largar a conexão e abrir outra: o driver
        // não expõe o sinal de atenção do TDS, e largar o socket faz o servidor
        // abortar a consulta do mesmo jeito. A sessão nova é outra sessão do
        // banco, e o próximo resultado avisa a tela.
        return sessao.sqlserver()?.cancelar();
    }
    sessao.oracle()?.break_execution()?;
    Ok(())
}

// --- Transação (requisito 7) -----------------------------------------------

#[tauri::command(async)]
pub fn confirmar_transacao(sessao_id: String, sessoes: State<'_, Sessoes>) -> Resultado<()> {
    registrar("commit", &sessao_id, || {
        encerrar_transacao(&sessao_id, &sessoes, true)
    })
}

#[tauri::command(async)]
pub fn desfazer_transacao(sessao_id: String, sessoes: State<'_, Sessoes>) -> Resultado<()> {
    registrar("rollback", &sessao_id, || {
        encerrar_transacao(&sessao_id, &sessoes, false)
    })
}

fn encerrar_transacao(sessao_id: &str, sessoes: &Sessoes, confirmar: bool) -> Resultado<()> {
    let sessao = sessoes.obter(sessao_id)?;
    if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        return sessao.sqlserver()?.transacao(confirmar);
    }
    sessao::sem_repetir(&sessao, |conexao| {
        if confirmar {
            conexao.commit()?;
        } else {
            conexao.rollback()?;
        }
        Ok(())
    })
}

/// Liga ou desliga o auto-commit da aba (requisito 7).
///
/// No Oracle isto é só um interruptor do Cururu: quem dá o `commit` depois de um
/// DML é a camada de execução. No SQL Server o auto-commit é **estado do
/// servidor** (`SET IMPLICIT_TRANSACTIONS`), e desligá-lo é um comando que vai
/// para a conexão — por isso aqui o caminho é outro.
#[tauri::command(async)]
pub fn definir_auto_commit(
    sessao_id: String,
    ligado: bool,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    let sessao = sessoes.obter(&sessao_id)?;
    sessao
        .auto_commit
        .store(ligado, std::sync::atomic::Ordering::Relaxed);
    registro::informacao("auto_commit", format!("{sessao_id} = {ligado}"));
    if sessao.perfil.tipo_banco == TipoBanco::SqlServer {
        return sessao.sqlserver()?.definir_auto_commit(ligado);
    }
    Ok(())
}

// --- Saída DBMS (requisito 8) ----------------------------------------------

#[tauri::command(async)]
pub fn definir_saida_dbms(
    sessao_id: String,
    ligada: bool,
    tamanho_do_buffer: u32,
    sessoes: State<'_, Sessoes>,
) -> Resultado<()> {
    registrar("saida_dbms", format!("{sessao_id} = {ligada}"), || {
        let sessao = sessoes.obter(&sessao_id)?;
        // A vontade do usuário fica registrada antes de valer no banco: é ela
        // que faz a saída voltar ligada se a sessão precisar ser reaberta.
        sessao
            .buffer_dbms
            .store(tamanho_do_buffer, std::sync::atomic::Ordering::Relaxed);
        sessao
            .saida_dbms
            .store(ligada, std::sync::atomic::Ordering::Relaxed);
        sessao::sem_repetir(&sessao, |conexao| {
            if ligada {
                execucao::ligar_saida_dbms(conexao, tamanho_do_buffer)
            } else {
                execucao::desligar_saida_dbms(conexao)
            }
        })
    })
}

// --- PL/SQL (requisito 13) -------------------------------------------------

#[tauri::command(async)]
pub fn compilar(
    sessao_id: String,
    fonte: String,
    sessoes: State<'_, Sessoes>,
    cache: State<'_, CacheDeMetadados>,
) -> Resultado<Vec<plsql::ErroDeCompilacao>> {
    registrar("compilar", "", || {
        let sessao = sessoes.obter(&sessao_id)?;
        let erros = sessao::sem_repetir(&sessao, |conexao| plsql::compilar(conexao, &fonte))?;
        // Compilar pode ter criado um objeto novo: o autocomplete precisa vê-lo.
        cache.invalidar(&sessao_id);
        Ok(erros)
    })
}

// --- Arquivos (requisitos 12 e 16) -----------------------------------------

#[tauri::command(async)]
pub fn gravar_texto(caminho: String, conteudo: String) -> Resultado<()> {
    registrar("gravar_texto", &caminho, || {
        // O BOM já vem no conteúdo quando o formato pede.
        std::fs::write(&caminho, conteudo.as_bytes())?;
        Ok(())
    })
}

/// Grava o resultado do grid como planilha do Excel (requisito 16).
///
/// A planilha é montada aqui, e não na interface, porque o `.xlsx` é binário:
/// atravessar a ponte seria mandar de volta um vetor de números em JSON, maior
/// que o próprio arquivo. Daqui vão só os dados, que já são texto.
#[tauri::command(async)]
pub fn gravar_planilha(
    caminho: String,
    colunas: Vec<String>,
    linhas: Vec<Vec<Option<String>>>,
) -> Resultado<()> {
    registrar("gravar_planilha", &caminho, || {
        if linhas.len() > xlsx::MAXIMO_DE_LINHAS || colunas.len() > xlsx::MAXIMO_DE_COLUNAS {
            registro::aviso(
                "gravar_planilha",
                format!(
                    "{} linhas e {} colunas não cabem numa planilha do Excel: vão as primeiras {} e {}",
                    linhas.len(),
                    colunas.len(),
                    linhas.len().min(xlsx::MAXIMO_DE_LINHAS),
                    colunas.len().min(xlsx::MAXIMO_DE_COLUNAS),
                ),
            );
        }
        std::fs::write(&caminho, xlsx::montar(&colunas, &linhas))?;
        Ok(())
    })
}

/// Lê um arquivo de texto — o Carregar Consulta SQL do menu.
#[tauri::command(async)]
pub fn ler_texto(caminho: String) -> Resultado<String> {
    registrar("ler_texto", &caminho, || arquivo::ler(&caminho))
}

// --- Cópia de recuperação (requisito 12) ------------------------------------

/// Grava a cópia de socorro de uma aba.
///
/// **Não registra em log a cada gravação**: ela acontece a cada pausa da
/// digitação, e encheria o arquivo de log com centenas de linhas iguais por
/// hora, escondendo o que interessa. Só a falha é registrada.
#[tauri::command(async)]
pub fn gravar_recuperacao(
    id: String,
    texto: String,
    descricao: recuperacao::Descricao,
) -> Resultado<()> {
    let resultado = recuperacao::gravar(&id, &texto, &descricao);
    if let Err(erro) = &resultado {
        registro::erro("gravar_recuperacao", format!("{id} — {erro}"));
    }
    resultado
}

#[tauri::command(async)]
pub fn listar_recuperacoes() -> Resultado<Vec<recuperacao::Entrada>> {
    registrar("listar_recuperacoes", "", recuperacao::listar)
}

#[tauri::command(async)]
pub fn ler_recuperacao(id: String) -> Resultado<String> {
    registrar("ler_recuperacao", &id, || recuperacao::texto(&id))
}

#[tauri::command(async)]
pub fn descartar_recuperacao(id: String) -> Resultado<()> {
    registrar("descartar_recuperacao", &id, || recuperacao::descartar(&id))
}

// --- Importação de conexões (requisito 25) ---------------------------------

#[tauri::command(async)]
pub fn procurar_conexoes_importaveis() -> Resultado<Vec<importacao::ConexaoImportavel>> {
    registrar("procurar_conexoes_importaveis", "", importacao::procurar)
}

// --- Apoio -----------------------------------------------------------------

/// Executa a operação medindo o tempo e registrando o desfecho (requisito 28).
fn registrar<T>(
    origem: &str,
    detalhe: impl Display,
    operacao: impl FnOnce() -> Resultado<T>,
) -> Resultado<T> {
    let inicio = Instant::now();
    let resultado = operacao();
    let ms = inicio.elapsed().as_millis();

    match &resultado {
        Ok(_) => registro::informacao(origem, format!("{detalhe} — ok em {ms} ms")),
        Err(erro) => registro::erro(origem, format!("{detalhe} — falhou em {ms} ms: {erro}")),
    }
    resultado
}

/// Sem Instant Client não há o que tentar: melhor uma mensagem clara aqui do
/// que um `DPI-1047` cru vindo do driver.
/// O Instant Client é exigência do Oracle, e só dele.
///
/// Um perfil de SQL Server numa máquina sem Instant Client precisa chegar até o
/// driver e voltar com o erro dele; barrar aqui responderia "falta o Oracle
/// Instant Client" a quem nunca pediu Oracle.
fn exigir_instantclient(situacao: &Situacao, perfil: &Perfil) -> Resultado<()> {
    if perfil.tipo_banco != TipoBanco::Oracle || situacao.utilizavel() {
        return Ok(());
    }
    Err(Erro::novo(situacao.problema.clone().unwrap_or_else(|| {
        "o Oracle Instant Client não está disponível".to_string()
    })))
}

fn identificador_de_sessao() -> String {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("s{:x}{:x}", agora.as_secs(), agora.subsec_nanos())
}
