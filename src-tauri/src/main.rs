// Impede a janela de console no Windows fora do modo de depuração.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agente;
mod alertas;
mod arquivo;
mod atualizacao;
mod backup;
mod comandos;
mod dpapi;
mod erro;
mod execucao;
mod execucao_sqlserver;
mod filtro;
mod filtros_salvos;
mod idioma_instalado;
mod importacao;
mod indice;
mod instancia;
mod instantclient;
mod metadados;
mod metadados_sqlserver;
mod mongo;
mod navegador;
mod objetos;
mod objetos_sqlserver;
mod perfis;
mod plsql;
mod recuperacao;
mod registro;
mod relacoes;
mod sessao;
mod sql_texto;
mod sqlserver;
mod teams;
mod webview;
mod xlsx;

use std::time::Instant;

use tauri::Manager;

fn main() {
    let inicio = Instant::now();

    // Antes até do log: com outro Cururu aberto, o arquivo que o Windows mandou
    // abrir vai para a janela dele, e este processo sai sem mostrar nada
    // (requisito 12). Não há o que registrar aqui — quem registra é a janela
    // que recebe. Abrir sem arquivo segue adiante, como sempre.
    let pedido = instancia::pedido_da_linha_de_comando();
    let havia_outro = instancia::marcar_presenca();
    if havia_outro && !pedido.arquivos.is_empty() && instancia::entregar(&pedido.arquivos) {
        return;
    }

    // O log vem antes do resto: se algo falhar daqui em diante, precisa ficar
    // registrado (requisito 28).
    registro::iniciar();
    instancia::aceitar_pedido(pedido, havia_outro);

    // O Instant Client é registrado no driver logo em seguida: depois de
    // inicializado, o driver não aceita trocar a pasta da biblioteca.
    let instantclient = instantclient::preparar();
    match (&instantclient.diretorio, &instantclient.problema) {
        (_, Some(problema)) => registro::erro("instantclient", problema),
        (Some(diretorio), None) => registro::informacao("instantclient", diretorio),
        (None, None) => registro::aviso("instantclient", "situação indefinida"),
    }

    // As cópias de recuperação vencidas saem aqui, uma vez por execução: nunca
    // por temporizador, que ficaria varrendo pasta o dia inteiro à toa.
    recuperacao::limpar();

    // A versão do WebView2 sai antes de a janela nascer: com outro Cururu
    // aberto, esta janela precisa entrar no navegador dele, e quem decide a
    // versão é o carregador, na hora de criar a página (ver webview.rs).
    let contexto = tauri::generate_context!();
    webview::escolher_versao(&contexto.config().identifier);

    let aplicativo = tauri::Builder::default()
        // Usado só para o diálogo de "salvar como" da exportação.
        .plugin(tauri_plugin_dialog::init())
        // A primeira página carregada é a prova de que a janela veio: sem essa
        // linha no log, o processo parou antes, esperando o WebView2.
        .on_page_load(move |_, carga| {
            if matches!(carga.event(), tauri::webview::PageLoadEvent::Finished) {
                webview::pagina_carregada(inicio);
            }
        })
        .manage(instantclient)
        .manage(perfis::Repositorio::default())
        .manage(sessao::Sessoes::default())
        .manage(metadados::CacheDeMetadados::default())
        .manage(indice::RepositorioDeIndice::default())
        .manage(filtros_salvos::RepositorioDeFiltros::default())
        .manage(execucao::Cursores::default())
        .manage(alertas::Repositorio::default())
        .invoke_handler(tauri::generate_handler![
            alertas::listar_alertas,
            alertas::ler_logs_alerta,
            alertas::ler_webhook_alerta,
            alertas::salvar_alerta,
            alertas::excluir_alerta,
            alertas::verificar_alerta,
            alertas::notificar_alerta_teams,
            comandos::diagnostico_instantclient,
            comandos::caminho_do_log,
            comandos::versao_em_execucao,
            comandos::abrir_no_navegador,
            comandos::verificar_atualizacao,
            comandos::baixar_atualizacao,
            comandos::instalar_atualizacao,
            comandos::encerrar_aplicativo,
            comandos::listar_perfis,
            comandos::ler_senha_salva,
            comandos::salvar_perfil,
            comandos::excluir_perfil,
            comandos::exportar_backup,
            comandos::importar_backup,
            comandos::testar_conexao,
            comandos::testar_mongo,
            comandos::conectar_mongo,
            comandos::listar_bancos_mongo,
            comandos::listar_colecoes_mongo,
            comandos::consultar_mongo,
            comandos::campos_mongo,
            comandos::conectar,
            comandos::conectar_irma,
            comandos::listar_bancos,
            comandos::banco_da_sessao,
            comandos::usar_banco,
            comandos::estado_do_agente,
            comandos::listar_trabalhos,
            comandos::dados_de_trabalho,
            comandos::criar_trabalho,
            comandos::detalhe_do_trabalho,
            comandos::ddl_do_trabalho,
            comandos::atualizar_trabalho,
            comandos::habilitar_trabalho,
            comandos::renomear_trabalho,
            comandos::excluir_trabalho,
            comandos::iniciar_trabalho,
            comandos::parar_trabalho,
            comandos::execucao_do_trabalho,
            comandos::desconectar,
            comandos::listar_objetos,
            comandos::listar_colunas,
            comandos::listar_subprogramas,
            comandos::indexar_objetos,
            comandos::sugerir_relacoes,
            comandos::fonte_do_objeto,
            comandos::gravar_comentario,
            comandos::resolver_referencia,
            comandos::preparar_navegacao,
            comandos::referencias_navegaveis,
            comandos::idioma_do_instalador,
            comandos::sugerir_objetos,
            comandos::atualizar_metadados,
            comandos::ler_filtros,
            comandos::gravar_filtro,
            comandos::analisar_binds,
            comandos::executar_sql,
            comandos::buscar_mais_linhas,
            comandos::fechar_cursor,
            comandos::cancelar_execucao,
            comandos::confirmar_transacao,
            comandos::desfazer_transacao,
            comandos::definir_auto_commit,
            comandos::definir_saida_dbms,
            comandos::compilar,
            comandos::gravar_texto,
            comandos::gravar_planilha,
            comandos::ler_texto,
            comandos::arquivos_recebidos,
            comandos::gravar_recuperacao,
            comandos::listar_recuperacoes,
            comandos::ler_recuperacao,
            comandos::descartar_recuperacao,
            comandos::procurar_conexoes_importaveis,
        ])
        .build(contexto)
        .expect("falha ao iniciar o Cururu");

    // A janela que recebe arquivos dos outros processos nasce aqui: na thread
    // principal, cujo laço de eventos é que lhe entrega as mensagens, e antes
    // de a página carregar — o que chegar cedo espera na fila.
    instancia::escutar(aplicativo.handle().clone());

    aplicativo.run(|app, evento| match evento {
        // Pedido o fechamento, arquivo novo não é mais aceito: aceitar e fechar
        // em seguida seria perdê-lo. Quem o mandou abre a própria janela.
        tauri::RunEvent::ExitRequested { .. } => instancia::recusar_arquivos(),
        // Ao sair (janela fechada ou atualização), encerra as sessões
        // abertas: o logoff limpo libera os locks na hora. Um fechamento
        // forçado (crash, Gerenciador de Tarefas) não passa por aqui — nesse
        // caso os locks ficam com o servidor até ele perceber a conexão
        // morta, o que pode demorar.
        tauri::RunEvent::Exit => {
            instancia::recusar_arquivos();
            app.state::<sessao::Sessoes>().encerrar_todas();
            registro::informacao("aplicativo", "sessões encerradas na saída");
        }
        _ => {}
    });

    registro::informacao("aplicativo", "Cururu encerrado");
}
