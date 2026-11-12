//! Conexões abertas com o banco.
//!
//! Cada conexão bem-sucedida vira uma sessão registrada aqui, identificada por
//! um id que a interface usa nos comandos seguintes (árvore de objetos,
//! execução de SQL, commit/rollback).
//!
//! A sessão também sabe **se reabrir**. Uma VPN que cai e volta deixa a conexão
//! do driver morta: o socket já não vai a lugar nenhum, e todo comando dali em
//! diante falha com `ORA-03113` ou parecido. Antes, a aba nessa situação só
//! voltava a funcionar sendo fechada e aberta de novo; agora ela reconecta
//! sozinha na tentativa seguinte (ver [`com_reconexao`] e [`sem_repetir`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::perfis::{Atribuicao, IdentificacaoBanco, Perfil, TipoBanco, TipoConexao};
use crate::plsql::ObjetoPlsql;
use crate::registro;

/// Uma conexão aberta.
pub struct Sessao {
    pub id: String,
    /// Credenciais já resolvidas desta sessão, guardadas em memória pela vida
    /// dela. O disco continua só com o DPAPI.
    ///
    /// Servem para dois fins: abrir as sessões irmãs das abas — uma sessão por
    /// aba de consulta (requisito 7) — sem repedir a senha, e **reabrir esta
    /// mesma sessão** quando a conexão cai.
    pub perfil: Perfil,
    /// Auto-commit **do Cururu**, não do driver (requisito 7).
    ///
    /// O driver só aceita trocar o auto-commit com acesso exclusivo à conexão,
    /// e a conexão precisa ficar compartilhada para o botão Parar poder
    /// interromper uma consulta em andamento. Então o auto-commit fica aqui: a
    /// execução dá `commit` explícito depois de um DML quando isto está ligado.
    pub auto_commit: AtomicBool,
    /// Saída DBMS ligada nesta sessão (requisito 8).
    pub saida_dbms: AtomicBool,
    /// Tamanho pedido no `dbms_output.enable`. Fica guardado porque o
    /// `DBMS_OUTPUT` é estado da sessão do banco, e não do Cururu: depois de uma
    /// reconexão ele precisa ser religado do mesmo jeito.
    pub buffer_dbms: AtomicU32,
    /// A `Connection` do driver é `Sync`, e quem a pega leva um `Arc` clonado
    /// sem segurar o cadeado: isso permite que a interrupção de uma consulta em
    /// andamento (`break_execution`) seja chamada de outra thread enquanto a
    /// consulta ainda corre — é o que faz o botão Parar funcionar
    /// (requisito 14).
    ///
    /// O `RwLock` existe só para a troca da reconexão, e a conexão nova é aberta
    /// **fora** dele: o cadeado de escrita é tomado apenas para trocar o
    /// ponteiro, o que leva microssegundos. Se a conexão nova fosse aberta com o
    /// cadeado na mão, o botão Parar ficaria esperando o tempo de um login.
    conexao: ConexaoDoBanco,
    /// Uma reconexão por vez, e quando foi a última que falhou.
    ///
    /// Quando a rede cai, tudo o que a sessão estava fazendo falha junto — a
    /// consulta, o autocomplete, a árvore —, e sem isto cada falha abriria a sua
    /// própria conexão nova. O instante guardado serve ao caso em que a rede
    /// **ainda não voltou**: o autocomplete pede sugestões a cada tecla, e sem a
    /// espera cada tecla enfileiraria uma tentativa de login que só termina no
    /// tempo do TCP.
    reconexao: Mutex<Option<Instant>>,
    /// Conexão à parte que responde quais nomes do editor o Ctrl+clique abre
    /// (requisito 22). Nasce com a primeira aba de consulta (ver
    /// [`Sessao::preparar_navegacao`]) e fecha junto com a sessão.
    ///
    /// Não é a conexão de metadados porque aquela atende um pedido por vez — a
    /// árvore e o autocomplete. A pergunta costuma sair em décimos de segundo,
    /// pelo `NAME_RESOLVE`, mas o caminho de reserva passa por `ALL_OBJECTS`,
    /// que neste banco leva segundos: feito ali, segurar o Ctrl travaria o
    /// autocomplete pelo mesmo tempo. Nem a da aba, onde o Ctrl+Enter executa e
    /// esperaria a pergunta terminar. E fica aberta, em vez de uma por
    /// pergunta, porque cada login custa perto de 350 ms e o Ctrl é apertado o
    /// tempo todo.
    navegacao: Mutex<Option<Arc<oracle::Connection>>>,
    /// Último objeto que um `CREATE` desta aba compilou. É o que o `SHOW ERRORS`
    /// sem nome mostra, como no SQL*Plus.
    ultimo_compilado: Mutex<Option<ObjetoPlsql>>,
}

/// A conexão desta sessão. Cada banco tem a sua, e não há forma comum entre
/// elas: a do Oracle é `Sync` e se compartilha por `Arc`; a do SQL Server é uma
/// thread com quem se conversa por canal (ver [`crate::sqlserver::Sessao`]).
///
/// Manter isto num `enum`, em vez de um objeto de trait, é o que faz o
/// compilador cobrar de cada caminho a declaração de que banco ele atende. Todo
/// o código de árvore, execução e metadados que existe hoje é do Oracle, e passa
/// por [`Sessao::oracle`]: com uma sessão de SQL Server ele para com a mensagem
/// certa em vez de fazer coisa errada em silêncio.
enum ConexaoDoBanco {
    Oracle(RwLock<Arc<oracle::Connection>>),
    SqlServer(crate::sqlserver::Sessao),
    MongoDb(crate::mongo::Sessao),
}

impl Sessao {
    pub fn nova(id: String, perfil: Perfil, conexao: oracle::Connection) -> Self {
        Sessao::com_conexao(
            id,
            perfil,
            ConexaoDoBanco::Oracle(RwLock::new(Arc::new(conexao))),
        )
    }

    pub fn nova_sqlserver(id: String, perfil: Perfil, conexao: crate::sqlserver::Sessao) -> Self {
        Sessao::com_conexao(id, perfil, ConexaoDoBanco::SqlServer(conexao))
    }

    pub fn nova_mongo(id: String, perfil: Perfil, conexao: crate::mongo::Sessao) -> Self {
        Sessao::com_conexao(id, perfil, ConexaoDoBanco::MongoDb(conexao))
    }

    fn com_conexao(id: String, perfil: Perfil, conexao: ConexaoDoBanco) -> Self {
        Sessao {
            id,
            auto_commit: AtomicBool::new(perfil.auto_commit),
            saida_dbms: AtomicBool::new(false),
            buffer_dbms: AtomicU32::new(0),
            perfil,
            conexao,
            reconexao: Mutex::new(None),
            navegacao: Mutex::new(None),
            ultimo_compilado: Mutex::new(None),
        }
    }

    pub fn lembrar_compilado(&self, objeto: ObjetoPlsql) {
        *self
            .ultimo_compilado
            .lock()
            .unwrap_or_else(|erro| erro.into_inner()) = Some(objeto);
    }

    pub fn ultimo_compilado(&self) -> Option<ObjetoPlsql> {
        self.ultimo_compilado
            .lock()
            .unwrap_or_else(|erro| erro.into_inner())
            .clone()
    }

    /// A conexão Oracle em uso agora.
    ///
    /// Devolve um `Arc` clonado, e não uma referência, justamente para o cadeado
    /// não ficar segurado enquanto a consulta corre.
    ///
    /// Erra quando a sessão é de SQL Server. Quem chama aqui é o que ainda só
    /// existe para o Oracle — árvore de objetos, fonte de PL/SQL, Saída DBMS —,
    /// e o erro é a resposta honesta enquanto o equivalente não existir do outro
    /// lado. Execução, transação, cursor e autocomplete já não passam por aqui:
    /// eles são atendidos pela [`crate::sqlserver::Sessao`].
    pub fn oracle(&self) -> Resultado<Arc<oracle::Connection>> {
        match &self.conexao {
            ConexaoDoBanco::Oracle(conexao) => Ok(conexao
                .read()
                .unwrap_or_else(|erro| erro.into_inner())
                .clone()),
            ConexaoDoBanco::SqlServer(_) => Err(Erro::novo(
                "esta operação ainda não existe para SQL Server; por enquanto ela só funciona em conexões Oracle",
            )),
            ConexaoDoBanco::MongoDb(_) => Err(Erro::novo("esta operação exige uma conexão Oracle")),
        }
    }

    /// A sessão de SQL Server desta conexão.
    pub fn sqlserver(&self) -> Resultado<&crate::sqlserver::Sessao> {
        match &self.conexao {
            ConexaoDoBanco::SqlServer(sessao) => Ok(sessao),
            ConexaoDoBanco::Oracle(_) => Err(Erro::novo("esta conexão é Oracle, não SQL Server")),
            ConexaoDoBanco::MongoDb(_) => Err(Erro::novo("esta conexão é MongoDB, não SQL Server")),
        }
    }

    pub fn mongo(&self) -> Resultado<&crate::mongo::Sessao> {
        match &self.conexao {
            ConexaoDoBanco::MongoDb(sessao) => Ok(sessao),
            _ => Err(Erro::novo("esta conexão não é MongoDB")),
        }
    }

    /// Desliga a conexão desta sessão.
    ///
    /// No Oracle o logoff limpo é o que libera os locks na hora. No SQL Server
    /// não há nada a chamar: a thread dona do cliente termina quando o canal
    /// fecha, e é ela quem desliga o socket no caminho.
    fn desligar(&self) {
        if let ConexaoDoBanco::Oracle(conexao) = &self.conexao {
            let _ = conexao
                .read()
                .unwrap_or_else(|erro| erro.into_inner())
                .close();
        }
        let navegacao = self
            .navegacao
            .lock()
            .unwrap_or_else(|erro| erro.into_inner())
            .take();
        if let Some(navegacao) = navegacao {
            let _ = navegacao.close();
        }
    }

    /// Roda a leitura na conexão de navegação, abrindo-a na primeira vez.
    ///
    /// Se ela tiver caído, é trocada por outra e a leitura se repete uma vez —
    /// como em [`com_reconexao`], é só leitura, e repetir não altera nada.
    pub fn com_conexao_de_navegacao<T>(
        &self,
        operacao: impl Fn(&oracle::Connection) -> Resultado<T>,
    ) -> Resultado<T> {
        let (conexao, _) = self.conexao_de_navegacao()?;
        let erro = match operacao(&conexao) {
            Ok(valor) => return Ok(valor),
            Err(erro) => erro,
        };
        if !e_falha_de_conexao(&erro, &conexao) {
            return Err(erro);
        }

        registro::aviso(
            "conexao",
            format!(
                "a conexão de navegação da sessão {} caiu ({erro}) — reabrindo",
                self.id
            ),
        );
        // Só descarta se ainda for a mesma: outra pergunta pode ter caído
        // junto e já ter aberto a nova.
        {
            let mut guardada = self.navegacao.lock().unwrap_or_else(|erro| erro.into_inner());
            if guardada.as_ref().is_some_and(|atual| Arc::ptr_eq(atual, &conexao)) {
                *guardada = None;
            }
        }
        let (nova, _) = self.conexao_de_navegacao()?;
        operacao(&nova)
    }

    /// Abre a conexão de navegação antes do primeiro Ctrl e, se ela acabou de
    /// abrir, roda nela o preparo.
    ///
    /// É o que tira do primeiro sublinhado o login e a primeira execução do
    /// bloco de resolução: no PRD, 326 ms e quase 700 ms, que se somavam à
    /// espera do Ctrl. Com a conexão já aberta, não faz nada.
    pub fn preparar_navegacao(&self, preparo: impl FnOnce(&oracle::Connection)) -> Resultado<()> {
        let (conexao, aberta_agora) = self.conexao_de_navegacao()?;
        if aberta_agora {
            preparo(&conexao);
        }
        Ok(())
    }

    /// A conexão de navegação, e se ela foi aberta nesta chamada.
    fn conexao_de_navegacao(&self) -> Resultado<(Arc<oracle::Connection>, bool)> {
        // Numa sessão de SQL Server a resposta é a mesma de todo recurso que
        // ainda só existe no Oracle, e não um erro de login do driver.
        self.oracle()?;
        // O cadeado fica na mão durante o login, ao contrário da reconexão da
        // conexão principal: aqui ninguém espera com pressa — não há botão
        // Parar em jogo —, e soltá-lo deixaria duas perguntas simultâneas
        // abrirem duas conexões para o mesmo fim.
        let mut guardada = self.navegacao.lock().unwrap_or_else(|erro| erro.into_inner());
        if let Some(conexao) = guardada.as_ref() {
            return Ok((conexao.clone(), false));
        }

        let inicio = Instant::now();
        let nova = Arc::new(abrir(&self.perfil)?);
        registro::informacao(
            "navegacao",
            format!(
                "sessão {}: conexão de navegação aberta em {} ms",
                self.id,
                inicio.elapsed().as_millis()
            ),
        );
        *guardada = Some(nova.clone());
        Ok((nova, true))
    }

    /// Abre outra conexão no lugar da que caiu.
    ///
    /// `anterior` é a conexão que falhou: se a sessão já estiver com outra, quem
    /// reconectou foi outra chamada que caiu junto, e não se abre uma segunda.
    pub fn reconectar(&self, anterior: &Arc<oracle::Connection>) -> Resultado<()> {
        let ConexaoDoBanco::Oracle(atual) = &self.conexao else {
            // A reconexão automática é do Oracle: os códigos de queda, o
            // `DBMS_OUTPUT` a religar e o `ORA-03113` são todos de lá.
            return Err(Erro::novo("reconexão automática ainda não existe para SQL Server"));
        };
        let mut ultima_falha = self
            .reconexao
            .lock()
            .unwrap_or_else(|erro| erro.into_inner());
        let em_uso = atual.read().unwrap_or_else(|erro| erro.into_inner()).clone();
        if !Arc::ptr_eq(&em_uso, anterior) {
            return Ok(());
        }
        // A rede ainda não voltou: não adianta tentar de novo agora, e tentar
        // custa o tempo de um login que vai falhar.
        if ultima_falha.is_some_and(|quando| quando.elapsed() < ESPERA_APOS_FALHA) {
            return Err(Erro::novo(
                "a conexão caiu e ainda não foi possível reabri-la",
            ));
        }

        let inicio = Instant::now();
        let nova = abrir(&self.perfil).inspect_err(|erro| {
            *ultima_falha = Some(Instant::now());
            registro::erro(
                "reconectar",
                format!("sessão {} não reabriu: {erro}", self.id),
            );
        })?;
        *ultima_falha = None;

        // Estado que a sessão nova não herda sozinha. O auto-commit é do Cururu
        // (requisito 7) e continua valendo sem fazer nada; o `DBMS_OUTPUT` é do
        // banco e precisa ser religado (requisito 8).
        if self.saida_dbms.load(Ordering::Relaxed) {
            let tamanho = self.buffer_dbms.load(Ordering::Relaxed) as i64;
            if let Err(erro) = nova.execute("begin dbms_output.enable(:tamanho); end;", &[&tamanho])
            {
                registro::aviso("reconectar", format!("saída DBMS não voltou: {erro}"));
            }
        }

        // A conexão velha não é fechada: ela está morta, e um `close` sem rede
        // fica esperando o tempo do TCP para confirmar o que já se sabe. Ela sai
        // de cena com o último `Arc` que a segurava, e o servidor limpa a sessão
        // órfã por conta própria.
        *atual.write().unwrap_or_else(|erro| erro.into_inner()) = Arc::new(nova);
        registro::aviso(
            "reconectar",
            format!(
                "sessão {} reaberta em {} ms",
                self.id,
                inicio.elapsed().as_millis()
            ),
        );
        Ok(())
    }
}

// --- Queda e reconexão -----------------------------------------------------

/// Quanto se espera antes de tentar reabrir outra vez uma sessão cuja reconexão
/// acabou de falhar.
///
/// É curto de propósito: quem religa a VPN e aperta F9 não pode esbarrar numa
/// espera — seria o mesmo defeito com outra roupa. Serve só para o autocomplete,
/// que pede sugestões a cada tecla, não empilhar uma tentativa de login por
/// tecla enquanto a rede está fora.
const ESPERA_APOS_FALHA: Duration = Duration::from_secs(2);

/// Códigos com que o Oracle avisa que a conexão morreu.
///
/// É o que se vê quando a VPN cai, quando o servidor derruba a sessão ou quando
/// a rede se perde no meio de um comando. Não entram aqui os erros que uma
/// conexão viva devolve — `ORA-00942`, `ORA-01017` e companhia —, porque reabrir
/// a sessão não mudaria nada neles.
const CODIGOS_DE_QUEDA: &[i32] = &[
    28,    // a sessão foi eliminada
    1012,  // não conectado
    1041,  // erro interno; a hostdef extension não existe
    1089,  // shutdown imediato em andamento
    3113,  // fim de arquivo no canal de comunicação
    3114,  // não conectado ao ORACLE
    3135,  // a conexão perdeu o contato
    12152, // TNS: não foi possível enviar a mensagem de interrupção
    12170, // TNS: tempo de conexão esgotado
    12537, // TNS: conexão fechada
    12541, // TNS: não há listener
    12543, // TNS: host de destino inacessível
    12545, // falha na conexão: host ou objeto de destino não existe
    12547, // TNS: contato perdido
    12560, // TNS: erro do adaptador de protocolo
    12571, // TNS: falha na escrita do pacote
    25401, // não é possível continuar as buscas
    25408, // não é possível repetir a chamada com segurança
    28511, // conexão RPC perdida
];

/// O ODPI não separa o código do texto: o número vem no começo da mensagem, e
/// não em `codigo` — por isso a procura aqui é pelo texto.
const AVISOS_DO_ODPI: &[&str] = &["DPI-1010", "DPI-1080"];

/// A operação falhou porque a conexão caiu, e não pelo comando que o usuário
/// mandou?
///
/// A resposta vem primeiro do código do erro, que é o caminho normal. Quando o
/// código não é conhecido, pergunta-se ao driver: `status` lê um atributo local
/// do OCI — sem ida à rede — e responde `NotConnected` quando o socket já se
/// foi. É a rede de segurança para o código de queda que não estiver na lista.
pub fn e_falha_de_conexao(erro: &Erro, conexao: &oracle::Connection) -> bool {
    e_codigo_de_queda(erro)
        || matches!(
            conexao.status(),
            Ok(oracle::ConnStatus::NotConnected | oracle::ConnStatus::Closed)
        )
}

/// O código do erro é de conexão perdida?
fn e_codigo_de_queda(erro: &Erro) -> bool {
    let numero = erro
        .codigo
        .as_deref()
        .and_then(|codigo| codigo.strip_prefix("ORA-"))
        .and_then(|numero| numero.parse::<i32>().ok());
    if numero.is_some_and(|numero| CODIGOS_DE_QUEDA.contains(&numero)) {
        return true;
    }
    AVISOS_DO_ODPI
        .iter()
        .any(|aviso| erro.mensagem.contains(aviso))
}

/// Roda a operação e, se a conexão tiver caído, reabre a sessão e tenta outra
/// vez.
///
/// **Só para leitura**: árvore de objetos, colunas, autocomplete, fonte. Repetir
/// sozinho é seguro porque nada disso altera o banco.
pub fn com_reconexao<T>(
    sessao: &Sessao,
    operacao: impl Fn(&oracle::Connection) -> Resultado<T>,
) -> Resultado<T> {
    let conexao = sessao.oracle()?;
    let erro = match operacao(&conexao) {
        Ok(valor) => return Ok(valor),
        Err(erro) => erro,
    };
    if !e_falha_de_conexao(&erro, &conexao) {
        return Err(erro);
    }

    registro::aviso(
        "conexao",
        format!("a sessão {} caiu ({erro}) — reabrindo", sessao.id),
    );
    sessao.reconectar(&conexao)?;
    let nova = sessao.oracle()?;
    operacao(&nova)
}

/// Roda a operação e, se a conexão tiver caído, reabre a sessão **sem repetir o
/// comando**: devolve o erro marcado, para a tela poder dizer que a conexão
/// voltou e que basta executar de novo.
///
/// É o caminho de tudo que altera dados. Repetir sozinho um `insert` exigiria
/// saber se ele chegou a valer no banco antes de a conexão morrer, e não há como
/// saber com certeza — quem decide reexecutar um DML é o usuário.
pub fn sem_repetir<T>(
    sessao: &Sessao,
    operacao: impl FnOnce(&oracle::Connection) -> Resultado<T>,
) -> Resultado<T> {
    let conexao = sessao.oracle()?;
    operacao(&conexao).map_err(|erro| {
        if !e_falha_de_conexao(&erro, &conexao) {
            return erro;
        }
        registro::aviso(
            "conexao",
            format!(
                "a sessão {} caiu ({erro}) — reabrindo sem repetir",
                sessao.id
            ),
        );
        match sessao.reconectar(&conexao) {
            Ok(()) => erro.reconectado(),
            // A reconexão também falhou: a rede ainda não voltou, e o erro dela
            // diz mais ao usuário do que o `ORA-03113` do comando.
            Err(falha) => falha,
        }
    })
}

/// Registro das sessões abertas, compartilhado por todos os comandos.
#[derive(Default)]
pub struct Sessoes {
    mapa: Mutex<HashMap<String, Arc<Sessao>>>,
}

impl Sessoes {
    pub fn registrar(&self, sessao: Sessao) -> String {
        let id = sessao.id.clone();
        let mut mapa = self.mapa.lock().unwrap_or_else(|erro| erro.into_inner());
        mapa.insert(id.clone(), Arc::new(sessao));
        id
    }

    pub fn obter(&self, id: &str) -> Resultado<Arc<Sessao>> {
        let mapa = self.mapa.lock().unwrap_or_else(|erro| erro.into_inner());
        mapa.get(id)
            .cloned()
            .ok_or_else(|| Erro::novo("a conexão não está mais aberta"))
    }

    pub fn encerrar(&self, id: &str) {
        let sessao = {
            let mut mapa = self.mapa.lock().unwrap_or_else(|erro| erro.into_inner());
            mapa.remove(id)
        };
        if let Some(sessao) = sessao {
            // Desconectar é um "melhor esforço": se o banco já derrubou a
            // sessão, não há o que relatar ao usuário.
            sessao.desligar();
        }
    }

    /// Encerra todas as sessões abertas — chamado na saída do aplicativo.
    ///
    /// O logoff limpo do OCI faz o servidor liberar os locks na hora. Sem isto,
    /// sair com um `UPDATE` sem commit deixaria os locks pendurados até o
    /// servidor perceber a conexão morta, o que pode demorar. Um fechamento
    /// forçado (crash, Gerenciador de Tarefas) não passa por aqui.
    pub fn encerrar_todas(&self) {
        let sessoes: Vec<Arc<Sessao>> = {
            let mut mapa = self.mapa.lock().unwrap_or_else(|erro| erro.into_inner());
            mapa.drain().map(|(_, sessao)| sessao).collect()
        };
        for sessao in sessoes {
            sessao.desligar();
        }
    }
}

/// Resultado do botão Testar da tela de conexões.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoTeste {
    pub sucesso: bool,
    pub mensagem: String,
    pub versao_banco: Option<String>,
    pub duracao_ms: u64,
}

/// Abre uma conexão a partir de um perfil já completo (com senha).
pub fn abrir(perfil: &Perfil) -> Resultado<oracle::Connection> {
    // Quem abre conexão aqui é só o OCI; o SQL Server tem o caminho dele, em
    // `sqlserver::Sessao::abrir`. Recusar aqui, com o motivo escrito, é o que
    // evita um perfil de SQL Server cair no driver do Oracle por engano de
    // roteamento e voltar com um `ORA-12154` que não explica nada.
    match perfil.tipo_banco {
        TipoBanco::Oracle => {}
        TipoBanco::SqlServer => {
            return Err(Erro::novo("conexão de SQL Server não se abre pelo driver do Oracle"));
        }
        TipoBanco::MongoDb => {
            return Err(Erro::novo("conexão de MongoDB não se abre pelo driver do Oracle"));
        }
    }

    let (usuario, senha) = credenciais(perfil)?;
    let cadeia = cadeia_de_conexao(perfil)?;

    let mut conector = oracle::Connector::new(usuario, senha, cadeia);
    match perfil.atribuicao {
        Atribuicao::Padrao => {}
        Atribuicao::Sysdba => {
            conector.privilege(oracle::Privilege::Sysdba);
        }
        Atribuicao::Sysoper => {
            conector.privilege(oracle::Privilege::Sysoper);
        }
    }

    // O auto-commit do driver fica sempre desligado; quem decide confirmar é a
    // camada de execução, olhando o interruptor da sessão (requisito 7).
    let conexao = conector.connect()?;
    Ok(conexao)
}

/// Testa a conexão e a encerra em seguida.
///
/// **Vale para os dois bancos.** O `abrir` daqui é o do Oracle; a sessão de SQL
/// Server nasce em [`crate::sqlserver::Sessao::abrir`], que é dona da thread e
/// da conexão dela.
pub fn testar(perfil: &Perfil) -> ResultadoTeste {
    let comeco = Instant::now();
    let resultado = match perfil.tipo_banco {
        TipoBanco::Oracle => testar_oracle(perfil),
        TipoBanco::SqlServer => crate::sqlserver::testar(perfil),
        TipoBanco::MongoDb => Err(Erro::novo("use o teste de conexão do MongoDB")),
    };
    let duracao_ms = comeco.elapsed().as_millis() as u64;

    match resultado {
        Ok(versao) => ResultadoTeste {
            sucesso: true,
            mensagem: "Com êxito".to_string(),
            versao_banco: versao,
            duracao_ms,
        },
        Err(erro) => ResultadoTeste {
            sucesso: false,
            mensagem: erro.mensagem,
            versao_banco: None,
            duracao_ms,
        },
    }
}

/// Abre, lê o banner da versão e desliga.
fn testar_oracle(perfil: &Perfil) -> Resultado<Option<String>> {
    let conexao = abrir(perfil)?;
    let versao = conexao.server_version().ok().map(|(_, banner)| banner);
    let _ = conexao.close();
    Ok(versao)
}

/// Usuário e senha efetivos, considerando a autenticação por usuário proxy.
///
/// No Oracle, um proxy conecta com o próprio usuário e senha, indicando o
/// usuário alvo entre colchetes: `proxy[alvo]`.
fn credenciais(perfil: &Perfil) -> Resultado<(String, String)> {
    if perfil.usar_proxy && !perfil.usuario_proxy.is_empty() {
        return Ok((
            format!("{}[{}]", perfil.usuario_proxy, perfil.usuario),
            perfil.senha_proxy.clone().unwrap_or_default(),
        ));
    }
    Ok((
        perfil.usuario.clone(),
        perfil.senha.clone().unwrap_or_default(),
    ))
}

/// Monta a cadeia de conexão conforme o Tipo de Conexão escolhido na tela.
fn cadeia_de_conexao(perfil: &Perfil) -> Resultado<String> {
    match perfil.tipo_conexao {
        TipoConexao::Tns => {
            if perfil.alias_tns.is_empty() {
                return Err(Erro::novo("informe o alias da rede"));
            }
            Ok(perfil.alias_tns.clone())
        }
        TipoConexao::Avancado => {
            if perfil.url_personalizada.is_empty() {
                return Err(Erro::novo("informe a URL personalizada"));
            }
            Ok(perfil.url_personalizada.clone())
        }
        TipoConexao::Basico => match perfil.identificacao {
            IdentificacaoBanco::Servico => {
                if perfil.servico.is_empty() {
                    return Err(Erro::novo("informe o nome do serviço"));
                }
                Ok(format!(
                    "//{}:{}/{}",
                    perfil.host, perfil.porta, perfil.servico
                ))
            }
            // O formato curto não aceita SID; é preciso o descritor completo.
            IdentificacaoBanco::Sid => {
                if perfil.sid.is_empty() {
                    return Err(Erro::novo("informe o SID"));
                }
                Ok(format!(
                    "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST={})(PORT={}))\
                     (CONNECT_DATA=(SID={})))",
                    perfil.host, perfil.porta, perfil.sid
                ))
            }
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn erro_do_banco(codigo: &str, mensagem: &str) -> Erro {
        Erro {
            mensagem: mensagem.to_string(),
            codigo: Some(codigo.to_string()),
            posicao: None,
            linha: None,
            reconectado: false,
            resultados: Vec::new(),
        }
    }

    #[test]
    fn queda_de_rede_e_reconhecida() {
        assert!(e_codigo_de_queda(&erro_do_banco(
            "ORA-03113",
            "fim de arquivo no canal de comunicação"
        )));
        assert!(e_codigo_de_queda(&erro_do_banco(
            "ORA-03114",
            "não conectado ao ORACLE"
        )));
        assert!(e_codigo_de_queda(&erro_do_banco(
            "ORA-12571",
            "TNS:falha na escrita do pacote"
        )));
    }

    #[test]
    fn erro_do_odpi_sem_codigo_e_reconhecido() {
        // O ODPI manda o número dentro do texto, e `codigo` vem vazio.
        assert!(e_codigo_de_queda(&Erro::novo(
            "DPI-1080: connection was closed by ORA-3113"
        )));
    }

    #[test]
    fn perfil_de_sql_server_nao_chega_ao_driver_do_oracle() {
        // Sem isto, um erro de roteamento mandaria um perfil de SQL Server ao
        // OCI, que voltaria com um `ORA-12154` sobre um alias que ninguém
        // escreveu.
        let perfil = Perfil {
            nome: "Vendas PRD".to_string(),
            tipo_banco: crate::perfis::TipoBanco::SqlServer,
            servidor: r"SRVSQL01\PROD,1433".to_string(),
            usuario: "sa".to_string(),
            ..Perfil::default()
        };
        let erro = abrir(&perfil).expect_err("SQL Server não deveria abrir pelo OCI");
        assert!(
            erro.mensagem.contains("SQL Server"),
            "a mensagem precisa dizer qual banco foi recusado: {}",
            erro.mensagem
        );
    }

    #[test]
    fn erro_do_comando_nao_e_queda() {
        assert!(!e_codigo_de_queda(&erro_do_banco(
            "ORA-00942",
            "tabela ou view não existe"
        )));
        assert!(!e_codigo_de_queda(&erro_do_banco(
            "ORA-01017",
            "nome de usuário/senha inválidos"
        )));
        assert!(!e_codigo_de_queda(&Erro::novo(
            "a conexão não está mais aberta"
        )));
    }
}
