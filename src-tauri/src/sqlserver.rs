//! Conexão com SQL Server (requisito 1).
//!
//! Aqui moram o **Testar** da tela de conexões, a sessão aberta e tudo o que se
//! pergunta a ela: os bancos, o banco corrente da aba, o SQL Server Agent
//! (requisito 36) e a execução de SQL da janela de consulta. As consultas do
//! Agent ficam em [`crate::agente`]; as da execução, em
//! [`crate::execucao_sqlserver`]; as do autocomplete, em
//! [`crate::metadados_sqlserver`]. Este módulo é a **thread** que fala com
//! todas elas.
//!
//! O driver é o `tiberius`, TDS puro em Rust: nenhuma biblioteca nativa entra no
//! processo por causa dele, ao contrário do ODBC, e a runtime assíncrona que ele
//! exige já estava no binário por conta do Tauri (ver premissas.md).

use std::future::Future;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, SqlBrowser};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::agente::{
    self, DadosDeTrabalho, DetalheDoTrabalho, EstadoDoAgente, ExecucaoDoTrabalho, NovoTrabalho,
    Trabalho,
};
use crate::erro::{Erro, Resultado};
use crate::execucao::{BlocoDeLinhas, ResultadoDaExecucao};
use crate::execucao_sqlserver::{self, Cursor};
use crate::metadados::Sugestao;
use crate::metadados_sqlserver;
use crate::objetos::{Categoria, Coluna, DestinoDaReferencia, Pagina};
use crate::objetos_sqlserver;
use crate::perfis::{AutenticacaoSql, Perfil};
use crate::registro;
use crate::sql_texto::Comando;

/// O cliente do driver, já sobre o socket adaptado.
pub type Cliente = Client<Compat<TcpStream>>;

/// Quanto se espera por uma tentativa de conexão antes de desistir.
///
/// Sem teto, um host inalcançável deixaria o botão Testar girando pelo tempo do
/// TCP, que em rede corporativa passa de um minuto.
///
/// Eram os 15 s do SSMS, e caíram para 5: o servidor que responde, responde em
/// menos de um segundo, e os outros catorze só serviam para fazer esperar por
/// uma resposta que não vinha. Quem estiver atrás de um link lento e vir "não
/// respondeu em 5 segundos" num servidor que existe é o caso de aumentar de
/// novo — o número mora aqui, num lugar só.
const ESPERA_DA_CONEXAO: Duration = Duration::from_secs(5);

/// O "Nome do Servidor" da tela, já interpretado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    pub host: String,
    /// Instância nomeada, quando informada. Descobrir a porta dela é trabalho
    /// do SQL Browser, em UDP 1434.
    pub instancia: Option<String>,
    /// Porta explícita, quando o usuário a escreveu.
    ///
    /// Vazia de propósito quando ele não escreveu: quem escolhe a porta é o
    /// driver, e a escolha dele depende da instância — 1433 para a instância
    /// padrão, 1434 para perguntar ao SQL Browser onde a nomeada atende.
    pub porta: Option<u16>,
}

/// Interpreta o "Nome do Servidor" no formato que o SSMS aceita.
///
/// Um campo só carrega três informações — `HOST`, `HOST\INSTANCIA`,
/// `HOST,PORTA` e `HOST\INSTANCIA,PORTA` —, e é por isso que a tela não tem
/// campos separados: partir em host + porta perderia a instância nomeada.
pub fn interpretar_servidor(servidor: &str) -> Resultado<Destino> {
    let texto = servidor.trim();
    if texto.is_empty() {
        return Err(Erro::novo("informe o nome do servidor"));
    }

    // O SSMS aceita o protocolo na frente do nome. Só o TCP/IP é atendido aqui,
    // e recusar os outros com o motivo escrito evita que `np:SRV` vire uma
    // tentativa de conectar num host chamado "np".
    let texto = match texto.split_once(':') {
        Some((protocolo, resto)) if protocolo.eq_ignore_ascii_case("tcp") => resto.trim(),
        Some((protocolo, _))
            if protocolo.eq_ignore_ascii_case("np") || protocolo.eq_ignore_ascii_case("lpc") =>
        {
            return Err(Erro::novo(
                "só há conexão por TCP/IP; retire o prefixo do nome do servidor",
            ))
        }
        // Endereço IPv6 e nome comum caem aqui e seguem inteiros.
        _ => texto,
    };

    // A porta vem depois da vírgula, e é sempre a última parte.
    let (endereco, porta) = match texto.rsplit_once(',') {
        Some((endereco, porta)) => {
            let numero = porta.trim().parse::<u16>().ok().filter(|n| *n > 0);
            let Some(numero) = numero else {
                return Err(Erro::novo(format!(
                    "porta inválida no nome do servidor: {}",
                    porta.trim()
                )));
            };
            (endereco.trim(), Some(numero))
        }
        None => (texto, None),
    };

    // A instância vem depois da barra invertida.
    let (host, instancia) = match endereco.split_once('\\') {
        Some((host, instancia)) => (host.trim(), Some(instancia.trim())),
        None => (endereco, None),
    };

    if host.is_empty() {
        return Err(Erro::novo("informe o nome do servidor"));
    }
    if instancia.is_some_and(str::is_empty) {
        return Err(Erro::novo(
            "o nome da instância ficou vazio depois da barra invertida",
        ));
    }

    Ok(Destino {
        host: normalizar_host(host),
        instancia: instancia.map(str::to_string),
        porta,
    })
}

/// `.` e `(local)` são como o SSMS escreve "esta máquina".
fn normalizar_host(host: &str) -> String {
    if host == "." || host.eq_ignore_ascii_case("(local)") {
        return "localhost".to_string();
    }
    host.to_string()
}

/// Como o perfil se autentica.
///
/// Na autenticação do Windows **sem usuário informado** vale a identidade de
/// quem está logado — é o caminho normal do SSMS. Com usuário, é a mesma
/// autenticação feita com outra credencial, e o `DOMINIO\usuario` é separado
/// pelo próprio driver.
fn autenticacao(perfil: &Perfil) -> Resultado<AuthMethod> {
    let senha = perfil.senha.clone().unwrap_or_default();
    match perfil.autenticacao_sql {
        AutenticacaoSql::SqlServer => {
            if perfil.usuario.is_empty() {
                return Err(Erro::novo("informe o logon"));
            }
            Ok(AuthMethod::sql_server(&perfil.usuario, senha))
        }
        AutenticacaoSql::Windows if perfil.usuario.is_empty() => Ok(AuthMethod::Integrated),
        AutenticacaoSql::Windows => Ok(AuthMethod::windows(&perfil.usuario, senha)),
    }
}

// --- Sessão aberta ---------------------------------------------------------

/// Quanto se espera lendo o resto de um resultado abandonado antes de desistir
/// dele e reabrir a conexão.
///
/// Largar um grid pela metade e mandar outra consulta é gesto comum, e no TDS
/// ele tem um preço que o Oracle não cobra: a conexão atende **um** resultado de
/// cada vez, e o pendente só sai da frente sendo lido até o fim. Num `select`
/// sobre tabela enorme isso seria a rede inteira, e a tela pareceria travada.
/// Passado o prazo, a conexão é reaberta — custa um login — e o próximo
/// resultado avisa a tela, porque a sessão nova é outra sessão do banco.
const ESPERA_PARA_ESVAZIAR: Duration = Duration::from_secs(3);

/// Teto da pergunta "esta conexão ainda responde?", feita depois de uma falha.
const ESPERA_DA_CONFERENCIA: Duration = Duration::from_secs(5);

/// Uma conexão de SQL Server viva entre um comando e outro.
///
/// O cliente do `tiberius` é assíncrono e exige `&mut` em toda operação: não dá
/// para guardá-lo atrás de um `Arc` e chamá-lo de qualquer thread, como se faz
/// com a `Connection` do OCI. Então ele mora numa **thread própria**, dona
/// também da runtime, e quem precisa dele manda um pedido por canal.
///
/// Essa forma não é rodeio para contornar o driver: é o que faz o fetch parcial
/// (requisito 10) existir aqui. O resultado do `tiberius` empresta o cliente e
/// não sobrevive sozinho num mapa, como o `ResultSet` do Oracle; com o cliente
/// preso a uma thread, o cursor vira um laço dentro dela, e o grid pede mais
/// linhas pelo mesmo canal por onde pediu as primeiras.
///
/// Custo: uma thread do sistema por sessão. A pilha é **reservada**, não
/// residente — só o que ela toca é comprometido —, e a runtime `current_thread`
/// não tem pool de workers para manter.
pub struct Sessao {
    /// O cadeado serializa os pedidos: há um cliente só do outro lado, e cada
    /// pedido espera a própria resposta antes de o seguinte partir.
    pedidos: Mutex<Sender<Pedido>>,
    /// O canal do botão Parar (requisito 14).
    ///
    /// Fica **fora** do cadeado dos pedidos, e é a única forma de funcionar: a
    /// interrupção precisa chegar enquanto a consulta corre, e nesse momento o
    /// cadeado está na mão de quem espera por ela. Um "Parar" que entrasse na
    /// fila só seria lido depois do fim da consulta que ele deveria interromper.
    cancelamentos: UnboundedSender<()>,
    /// Banco em uso. A tela o mostra na barra da consulta, no lugar em que o
    /// Oracle mostra a Saída DBMS.
    ///
    /// Compartilhado com a thread porque o banco pode mudar por dentro: quem
    /// escreve `use Vendas` no editor troca o banco da aba, e quem descobre isso
    /// é quem executou.
    banco: Arc<Mutex<Option<String>>>,
}

/// O que se pede à thread dona do cliente.
enum Pedido {
    /// Bancos a que este login tem acesso; o booleano inclui os do sistema.
    Bancos(bool, Sender<Resultado<Vec<String>>>),
    /// Troca o banco corrente da sessão.
    Usar(String, Sender<Resultado<()>>),
    /// Executa os lotes de uma aba (requisitos 5, 10 e 15).
    Executar(Box<PedidoDeExecucao>),
    /// Mais um bloco de linhas do cursor que ficou aberto (requisito 10).
    BuscarMais(String, usize, Sender<Resultado<BlocoDeLinhas>>),
    /// A aba largou o grid: o cursor pode ir embora.
    FecharCursor(String, Sender<Resultado<()>>),
    /// Confirma (`true`) ou desfaz a transação da aba (requisito 7).
    Transacao(bool, Sender<Resultado<()>>),
    /// Liga ou desliga o auto-commit da sessão (requisito 7).
    AutoCommit(bool, Sender<Resultado<()>>),
    /// Nomes do catálogo que começam pelo prefixo (requisito 3): o banco, o
    /// esquema e o prefixo — os três pedaços do nome de três partes —, mais o
    /// filtro de Tabelas quando a opção do requisito 43 o pede.
    Sugerir(
        Option<String>,
        Option<String>,
        String,
        Option<crate::filtro::Filtro>,
        Sender<Resultado<Vec<Sugestao>>>,
    ),
    /// Colunas de um objeto, para o autocomplete (requisito 23).
    Colunas(Option<String>, String, Sender<Resultado<Vec<Coluna>>>),
    /// Uma página de objetos de uma categoria, para a árvore (requisito 2).
    ListarObjetos(Box<PedidoDeListagem>),
    /// Grava (ou apaga) o comentário de uma view (requisito 37).
    GravarComentario(
        Option<String>,
        String,
        String,
        Sender<Resultado<()>>,
    ),
    /// O fonte de uma view, procedure ou função; o DDL montado, nas tabelas.
    Fonte(
        Option<String>,
        Categoria,
        String,
        Sender<Resultado<String>>,
    ),
    /// O destino de um Ctrl+clique no editor (requisito 22): o banco da aba e
    /// o nome como ele está escrito na consulta.
    ResolverReferencia(
        Option<String>,
        String,
        Sender<Resultado<Option<DestinoDaReferencia>>>,
    ),
    /// Quais nomes do editor o Ctrl+clique abre (requisito 22).
    ReferenciasNavegaveis(Option<String>, Vec<String>, Sender<Resultado<Vec<String>>>),
    /// Situação do serviço SQL Server Agent (requisito 36).
    EstadoDoAgente(Sender<Resultado<EstadoDoAgente>>),
    /// Trabalhos do Agent que este login enxerga, com o filtro da conexão
    /// (requisito 4).
    Trabalhos(Option<crate::filtro::Filtro>, Sender<Resultado<Vec<Trabalho>>>),
    /// Categorias, logins e nome do servidor, para a tela de Novo Trabalho.
    DadosDeTrabalho(Sender<Resultado<DadosDeTrabalho>>),
    /// Cria um trabalho novo.
    CriarTrabalho(Box<NovoTrabalho>, Sender<Resultado<()>>),
    /// Um trabalho por inteiro: geral, etapas e agendas.
    Detalhe(String, Sender<Resultado<DetalheDoTrabalho>>),
    /// Grava o que a tela de propriedades mudou.
    AtualizarTrabalho(String, Box<NovoTrabalho>, Sender<Resultado<()>>),
    /// Liga ou desliga o trabalho.
    HabilitarTrabalho(String, bool, Sender<Resultado<()>>),
    /// Troca o nome do trabalho.
    RenomearTrabalho(String, String, Sender<Resultado<()>>),
    /// Exclui o trabalho e as agendas que ficarem sem dono.
    ExcluirTrabalho(String, Sender<Resultado<()>>),
    /// Manda o Agent rodar o trabalho, opcionalmente a partir de uma etapa.
    IniciarTrabalho(String, Option<String>, Sender<Resultado<()>>),
    /// Manda o Agent interromper o trabalho.
    PararTrabalho(String, Sender<Resultado<()>>),
    /// Em que pé está a execução do trabalho, para a tela de acompanhamento.
    ExecucaoDoTrabalho(String, Sender<Resultado<ExecucaoDoTrabalho>>),
}

/// O que a árvore leva para a thread ao expandir uma categoria.
///
/// Vai numa caixa pelo mesmo motivo da execução: são cinco campos, e o
/// `clippy` recusa uma variante muito maior que as outras.
struct PedidoDeListagem {
    banco: Option<String>,
    categoria: Categoria,
    /// O filtro da categoria (requisito 4), quando há um guardado.
    filtro: Option<crate::filtro::Filtro>,
    deslocamento: u32,
    limite: u32,
    resposta: Sender<Resultado<Pagina>>,
}

/// Tudo o que a execução de uma aba leva para a thread.
///
/// Vai numa caixa porque são muitos campos para uma variante de `enum`, e a
/// variante inteira viaja pelo canal a cada `Ctrl+Enter`.
struct PedidoDeExecucao {
    /// Identificador da sessão: o cursor nasce com um nome derivado dele, e é
    /// esse nome que a aba devolve ao pedir mais linhas.
    sessao_id: String,
    lotes: Vec<Comando>,
    binds: Vec<(String, String)>,
    tamanho_do_fetch: usize,
    /// Falso no modo script: ali a saída é textual e ninguém pede mais linhas.
    manter_cursor: bool,
    resposta: Sender<Resultado<Vec<ResultadoDaExecucao>>>,
}

impl Sessao {
    /// Abre a conexão e a deixa apontada para um banco.
    ///
    /// `banco` preenchido evita uma ida ao servidor: é o caminho das abas
    /// irmãs, que já sabem em que banco a conexão está. Vazio, a sessão escolhe
    /// **o primeiro banco da lista** — o mesmo que a árvore mostra em cima.
    ///
    /// `auto_commit` não vem do perfil por um motivo: a sessão da **conexão** —
    /// a que atende árvore e autocomplete — abre sempre com ele ligado, e só a
    /// sessão de uma aba respeita o interruptor. Com o auto-commit desligado, o
    /// SQL Server abre transação implícita em **todo** comando, `select`
    /// inclusive; a sessão da árvore ficaria com uma transação aberta a vida
    /// inteira, segurando lock e log por consulta nenhuma do usuário.
    pub fn abrir(perfil: &Perfil, banco: Option<String>, auto_commit: bool) -> Resultado<Sessao> {
        let (config, instancia) = preparar(perfil)?;
        let (envio_pedidos, recepcao_pedidos) = mpsc::channel::<Pedido>();
        let (envio_cancelamentos, recepcao_cancelamentos) = unbounded_channel::<()>();
        let (envio_abertura, recepcao_abertura) = mpsc::channel::<Resultado<Option<String>>>();
        let banco_em_uso: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let banco_da_thread = Arc::clone(&banco_em_uso);

        std::thread::Builder::new()
            .name("cururu-sqlserver".to_string())
            .spawn(move || {
                atender(
                    Abertura {
                        config,
                        instancia,
                        banco,
                        auto_commit,
                    },
                    banco_da_thread,
                    envio_abertura,
                    recepcao_pedidos,
                    recepcao_cancelamentos,
                )
            })
            .map_err(|erro| Erro::novo(format!("não foi possível abrir a conexão: {erro}")))?;

        // A thread responde com o banco escolhido ou com o motivo de a conexão
        // não ter subido; nos dois casos ela já cuidou de si e saiu.
        recepcao_abertura
            .recv()
            .map_err(|_| Erro::novo("a conexão terminou antes de responder"))??;

        Ok(Sessao {
            pedidos: Mutex::new(envio_pedidos),
            cancelamentos: envio_cancelamentos,
            banco: banco_em_uso,
        })
    }

    /// Banco em uso agora.
    pub fn banco(&self) -> Option<String> {
        self.banco
            .lock()
            .unwrap_or_else(|erro| erro.into_inner())
            .clone()
    }

    /// Bancos do servidor a que este login tem acesso.
    ///
    /// `incluir_sistema` separa os dois usos: a árvore esconde `master`,
    /// `tempdb`, `model` e `msdb`, que não são onde se trabalha; a lista da aba
    /// de consulta os oferece, porque consultar o catálogo do `master` é
    /// trabalho legítimo de quem administra.
    pub fn bancos(&self, incluir_sistema: bool) -> Resultado<Vec<String>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::Bancos(incluir_sistema, envio), recepcao)
    }

    /// Troca o banco corrente desta sessão.
    ///
    /// Vale só para esta sessão — que é a de **uma aba** (requisito 7). Trocar o
    /// banco numa aba não mexe nas outras nem na conexão da árvore, do mesmo
    /// jeito que o `USE` do SSMS vale só para a janela em que foi dado.
    pub fn usar_banco(&self, banco: &str) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::Usar(banco.to_string(), envio), recepcao)
    }

    /// Executa os lotes de uma aba de consulta (requisito 5).
    pub fn executar(
        &self,
        sessao_id: &str,
        lotes: Vec<Comando>,
        binds: Vec<(String, String)>,
        tamanho_do_fetch: usize,
        manter_cursor: bool,
    ) -> Resultado<Vec<ResultadoDaExecucao>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::Executar(Box::new(PedidoDeExecucao {
                sessao_id: sessao_id.to_string(),
                lotes,
                binds,
                tamanho_do_fetch,
                manter_cursor,
                resposta: envio,
            })),
            recepcao,
        )
    }

    /// Mais um bloco de linhas de uma consulta já executada (requisito 10).
    pub fn buscar_mais(&self, cursor_id: &str, quantidade: usize) -> Resultado<BlocoDeLinhas> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::BuscarMais(cursor_id.to_string(), quantidade, envio),
            recepcao,
        )
    }

    /// Larga o cursor de um grid que saiu da tela.
    pub fn fechar_cursor(&self, cursor_id: &str) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::FecharCursor(cursor_id.to_string(), envio), recepcao)
    }

    /// Interrompe o que estiver correndo nesta sessão (requisito 14).
    ///
    /// Não passa pelo canal dos pedidos: ele está ocupado justamente com a
    /// consulta que se quer parar.
    pub fn cancelar(&self) -> Resultado<()> {
        self.cancelamentos
            .send(())
            .map_err(|_| Erro::novo("a conexão não está mais aberta"))
    }

    /// Commit (`true`) ou rollback da transação da aba (requisito 7).
    pub fn transacao(&self, confirmar: bool) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::Transacao(confirmar, envio), recepcao)
    }

    /// Liga ou desliga o auto-commit desta sessão (requisito 7).
    pub fn definir_auto_commit(&self, ligado: bool) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::AutoCommit(ligado, envio), recepcao)
    }

    /// Nomes do catálogo que começam pelo prefixo (requisito 3).
    pub fn sugerir(
        &self,
        banco: Option<&str>,
        esquema: Option<&str>,
        prefixo: &str,
        filtro_de_tabelas: Option<&crate::filtro::Filtro>,
    ) -> Resultado<Vec<Sugestao>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::Sugerir(
                banco.map(str::to_string),
                esquema.map(str::to_string),
                prefixo.to_string(),
                filtro_de_tabelas.cloned(),
                envio,
            ),
            recepcao,
        )
    }

    /// Colunas de um objeto, para o autocomplete (requisito 23).
    pub fn colunas(&self, banco: Option<&str>, objeto: &str) -> Resultado<Vec<Coluna>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::Colunas(banco.map(str::to_string), objeto.to_string(), envio),
            recepcao,
        )
    }

    /// Uma página de objetos de uma categoria, dentro de um banco (requisito 2).
    pub fn listar_objetos(
        &self,
        banco: Option<&str>,
        categoria: Categoria,
        filtro: Option<&crate::filtro::Filtro>,
        deslocamento: u32,
        limite: u32,
    ) -> Resultado<Pagina> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::ListarObjetos(Box::new(PedidoDeListagem {
                banco: banco.map(str::to_string),
                categoria,
                filtro: filtro.cloned(),
                deslocamento,
                limite,
                resposta: envio,
            })),
            recepcao,
        )
    }

    /// O fonte do objeto: o texto guardado, ou o `CREATE TABLE` montado.
    ///
    /// `parte` não chega aqui: ela existe por causa do pacote do PL/SQL, que
    /// tem especificação e corpo separados. No SQL Server o módulo é um só.
    pub fn fonte_do_objeto(
        &self,
        banco: Option<&str>,
        categoria: Categoria,
        nome: &str,
    ) -> Resultado<String> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::Fonte(
                banco.map(str::to_string),
                categoria,
                nome.to_string(),
                envio,
            ),
            recepcao,
        )
    }

    /// Grava (ou apaga) o comentário de uma view (requisito 37).
    pub fn gravar_comentario(
        &self,
        banco: Option<&str>,
        nome: &str,
        comentario: &str,
    ) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::GravarComentario(
                banco.map(str::to_string),
                nome.to_string(),
                comentario.to_string(),
                envio,
            ),
            recepcao,
        )
    }

    /// Situação do serviço SQL Server Agent (requisito 36).
    pub fn estado_do_agente(&self) -> Resultado<EstadoDoAgente> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::EstadoDoAgente(envio), recepcao)
    }

    /// O destino de um Ctrl+clique no editor (requisito 22).
    pub fn resolver_referencia(
        &self,
        banco: Option<&str>,
        referencia: &str,
    ) -> Resultado<Option<DestinoDaReferencia>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::ResolverReferencia(
                banco.map(str::to_string),
                referencia.to_string(),
                envio,
            ),
            recepcao,
        )
    }

    /// Quais nomes do editor o Ctrl+clique abre (requisito 22).
    pub fn referencias_navegaveis(
        &self,
        banco: Option<&str>,
        referencias: &[String],
    ) -> Resultado<Vec<String>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::ReferenciasNavegaveis(banco.map(str::to_string), referencias.to_vec(), envio),
            recepcao,
        )
    }

    /// Trabalhos do Agent, na ordem em que a árvore os pendura, recortados pelo
    /// filtro da conexão (requisito 4).
    pub fn trabalhos(&self, filtro: Option<&crate::filtro::Filtro>) -> Resultado<Vec<Trabalho>> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::Trabalhos(filtro.cloned(), envio), recepcao)
    }

    /// O que a tela de Novo Trabalho precisa saber antes de abrir.
    pub fn dados_de_trabalho(&self) -> Resultado<DadosDeTrabalho> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::DadosDeTrabalho(envio), recepcao)
    }

    /// Cria um trabalho com as etapas e agendas informadas.
    pub fn criar_trabalho(&self, trabalho: NovoTrabalho) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::CriarTrabalho(Box::new(trabalho), envio), recepcao)
    }

    /// Um trabalho por inteiro, para a tela de propriedades e para o DDL.
    pub fn detalhe_do_trabalho(&self, id: &str) -> Resultado<DetalheDoTrabalho> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::Detalhe(id.to_string(), envio), recepcao)
    }

    /// Grava o que a tela de propriedades mudou.
    pub fn atualizar_trabalho(&self, id: &str, trabalho: NovoTrabalho) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::AtualizarTrabalho(id.to_string(), Box::new(trabalho), envio),
            recepcao,
        )
    }

    pub fn habilitar_trabalho(&self, id: &str, ligado: bool) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::HabilitarTrabalho(id.to_string(), ligado, envio),
            recepcao,
        )
    }

    pub fn renomear_trabalho(&self, id: &str, nome: &str) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::RenomearTrabalho(id.to_string(), nome.to_string(), envio),
            recepcao,
        )
    }

    pub fn excluir_trabalho(&self, id: &str) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::ExcluirTrabalho(id.to_string(), envio), recepcao)
    }

    pub fn iniciar_trabalho(&self, id: &str, etapa: Option<String>) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(
            Pedido::IniciarTrabalho(id.to_string(), etapa, envio),
            recepcao,
        )
    }

    pub fn parar_trabalho(&self, id: &str) -> Resultado<()> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::PararTrabalho(id.to_string(), envio), recepcao)
    }

    pub fn execucao_do_trabalho(&self, id: &str) -> Resultado<ExecucaoDoTrabalho> {
        let (envio, recepcao) = mpsc::channel();
        self.pedir(Pedido::ExecucaoDoTrabalho(id.to_string(), envio), recepcao)
    }

    fn pedir<T>(&self, pedido: Pedido, recepcao: Receiver<Resultado<T>>) -> Resultado<T> {
        let pedidos = self.pedidos.lock().unwrap_or_else(|erro| erro.into_inner());
        pedidos
            .send(pedido)
            .map_err(|_| Erro::novo("a conexão não está mais aberta"))?;
        recepcao
            .recv()
            .map_err(|_| Erro::novo("a conexão caiu no meio da operação"))?
    }
}

// A thread termina sozinha quando o `Sender` sai de cena: o `recv` dela falha, o
// laço acaba e o cliente é desligado no caminho. Não há o que fechar à mão.

/// O que a thread precisa saber para abrir — e para **reabrir** — a conexão.
struct Abertura {
    config: Config,
    instancia: Option<String>,
    banco: Option<String>,
    auto_commit: bool,
}

/// O conserto que uma operação deixou pendente para a conexão.
enum Reparo {
    /// Nada a fazer: a conexão está limpa.
    Nada,
    /// Depois de uma falha, é preciso descobrir se a conexão morreu.
    Conferir,
    /// A conexão ficou inutilizável e precisa ser reaberta agora.
    Reabrir(&'static str),
}

/// A conexão viva da thread, com o que é preciso para levantá-la de novo.
struct Conexao {
    cliente: Cliente,
    abertura: Abertura,
    /// Compartilhado com a [`Sessao`]: a barra da aba lê daqui.
    banco: Arc<Mutex<Option<String>>>,
    /// A conexão foi reaberta e o próximo resultado precisa avisar a tela.
    ///
    /// A sessão nova é **outra** sessão do banco: o que estava sem commit ficou
    /// para trás, e quem está na tela precisa saber disso antes de continuar
    /// (requisito 35).
    reaberta: bool,
}

impl Conexao {
    /// Abre outra conexão no lugar desta e devolve a sessão ao estado em que
    /// estava: banco corrente e auto-commit.
    fn reabrir(&mut self, runtime: &tokio::runtime::Runtime, motivo: &str) -> Resultado<()> {
        let inicio = Instant::now();
        let config = self.abertura.config.clone();
        let instancia = self.abertura.instancia.clone();
        let nova = runtime.block_on(abrir_com_espera(config, instancia))?;

        // A conexão velha não é fechada: ou ela está morta, ou está com um
        // resultado pendente que só sairia da frente sendo lido inteiro — que é
        // justamente o que se quis evitar. Ela sai de cena aqui, e o servidor
        // limpa a sessão órfã por conta própria.
        self.cliente = nova;
        self.reaberta = true;

        let banco = self.banco();
        runtime.block_on(preparar_sessao(
            &mut self.cliente,
            banco.as_deref(),
            self.abertura.auto_commit,
        ))?;
        registro::aviso(
            "sqlserver",
            format!(
                "conexão reaberta em {} ms — {motivo}",
                inicio.elapsed().as_millis()
            ),
        );
        Ok(())
    }

    /// Aplica o conserto que a operação anterior deixou pendente.
    fn consertar(&mut self, runtime: &tokio::runtime::Runtime, reparo: Reparo) {
        let motivo = match reparo {
            Reparo::Nada => return,
            Reparo::Conferir if self.viva(runtime) => return,
            Reparo::Conferir => "a conexão não responde mais",
            Reparo::Reabrir(motivo) => motivo,
        };
        if let Err(erro) = self.reabrir(runtime, motivo) {
            registro::erro("sqlserver", format!("não foi possível reabrir: {erro}"));
        }
    }

    fn banco(&self) -> Option<String> {
        self.banco
            .lock()
            .unwrap_or_else(|erro| erro.into_inner())
            .clone()
    }

    fn anotar_banco(&self, banco: Option<String>) {
        *self.banco.lock().unwrap_or_else(|erro| erro.into_inner()) = banco;
    }

    /// A conexão ainda responde?
    ///
    /// Perguntar custa uma ida e volta, e só se pergunta **depois de uma
    /// falha**: é ali que interessa separar "o comando estava errado" de "a
    /// conexão morreu", e é essa distinção que decide entre mostrar o erro do
    /// servidor e reabrir a sessão (requisito 35).
    fn viva(&mut self, runtime: &tokio::runtime::Runtime) -> bool {
        let cliente = &mut self.cliente;
        runtime.block_on(async {
            let pergunta = async {
                match cliente.simple_query("select 1").await {
                    Ok(resposta) => resposta.into_results().await.is_ok(),
                    Err(_) => false,
                }
            };
            tokio::time::timeout(ESPERA_DA_CONFERENCIA, pergunta)
                .await
                .unwrap_or(false)
        })
    }

    /// Marca o primeiro resultado do lote quando a sessão foi reaberta.
    fn consumir_reabertura(&mut self) -> bool {
        std::mem::take(&mut self.reaberta)
    }
}

/// O laço da thread dona do cliente.
///
/// Entre um pedido e outro ela fica parada no `recv`, sem runtime rodando: o TDS
/// é pergunta e resposta, e não há nada a escutar no intervalo.
fn atender(
    abertura: Abertura,
    banco_compartilhado: Arc<Mutex<Option<String>>>,
    aviso_de_abertura: Sender<Resultado<Option<String>>>,
    pedidos: Receiver<Pedido>,
    mut cancelamentos: UnboundedReceiver<()>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(erro) => {
            let _ = aviso_de_abertura.send(Err(Erro::novo(format!(
                "não foi possível preparar a rede: {erro}"
            ))));
            return;
        }
    };

    let mut cliente = match runtime.block_on(abrir_com_espera(
        abertura.config.clone(),
        abertura.instancia.clone(),
    )) {
        Ok(cliente) => cliente,
        Err(erro) => {
            let _ = aviso_de_abertura.send(Err(erro));
            return;
        }
    };

    // O banco corrente é escolhido antes de a sessão ser anunciada: quem receber
    // o identificador já recebe a conexão apontada para o lugar certo. O
    // auto-commit vem logo atrás, porque no SQL Server ele é estado do servidor
    // (`SET IMPLICIT_TRANSACTIONS`), e não uma decisão só do cliente como no
    // Oracle.
    let escolhido = match runtime.block_on(async {
        let escolhido = escolher_banco(&mut cliente, abertura.banco.clone()).await?;
        definir_transacoes_implicitas(&mut cliente, !abertura.auto_commit).await?;
        Ok::<_, Erro>(escolhido)
    }) {
        Ok(escolhido) => escolhido,
        Err(erro) => {
            let _ = aviso_de_abertura.send(Err(erro));
            let _ = runtime.block_on(cliente.close());
            return;
        }
    };
    *banco_compartilhado
        .lock()
        .unwrap_or_else(|erro| erro.into_inner()) = escolhido.clone();
    if aviso_de_abertura.send(Ok(escolhido)).is_err() {
        // Ninguém mais espera por esta conexão.
        let _ = runtime.block_on(cliente.close());
        return;
    }

    let mut conexao = Conexao {
        cliente,
        abertura,
        banco: banco_compartilhado,
        reaberta: false,
    };
    let mut contador_de_cursores: u64 = 0;
    let mut pendente: Option<Pedido> = None;

    loop {
        let pedido = match pendente.take() {
            Some(pedido) => pedido,
            None => match pedidos.recv() {
                Ok(pedido) => pedido,
                Err(_) => break,
            },
        };
        pendente = atender_pedido(
            &runtime,
            &mut conexao,
            &pedidos,
            &mut cancelamentos,
            &mut contador_de_cursores,
            pedido,
        );
    }

    // Desligar é melhor esforço: se a conexão já caiu, não há a quem relatar.
    let _ = runtime.block_on(conexao.cliente.close());
    registro::informacao("sqlserver", "sessão encerrada");
}

/// Atende um pedido. Devolve o pedido que sobrou quando o laço do cursor teve de
/// largar o grid para atender outra coisa.
fn atender_pedido(
    runtime: &tokio::runtime::Runtime,
    conexao: &mut Conexao,
    pedidos: &Receiver<Pedido>,
    cancelamentos: &mut UnboundedReceiver<()>,
    contador: &mut u64,
    pedido: Pedido,
) -> Option<Pedido> {
    match pedido {
        Pedido::Executar(execucao) => {
            return executar_lotes(runtime, conexao, pedidos, cancelamentos, contador, *execucao);
        }
        // Sem cursor aberto não há de onde tirar mais linhas: a aba pediu de um
        // resultado que já foi embora.
        Pedido::BuscarMais(_, _, resposta) => {
            let _ = resposta.send(Err(Erro::novo(
                "o resultado desta consulta não está mais disponível",
            )));
        }
        Pedido::FecharCursor(_, resposta) => {
            let _ = resposta.send(Ok(()));
        }
        Pedido::Bancos(incluir_sistema, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, |cliente| {
                Box::pin(listar_bancos(cliente, incluir_sistema))
            }));
        }
        Pedido::Usar(banco, resposta) => {
            let escolhido = banco.clone();
            let feito = responder(runtime, conexao, move |cliente| {
                let banco = escolhido.clone();
                Box::pin(trocar_banco_da_aba(cliente, banco))
            });
            if feito.is_ok() {
                conexao.anotar_banco(Some(banco));
            }
            let _ = resposta.send(feito);
        }
        Pedido::Transacao(confirmar, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, |cliente| {
                Box::pin(encerrar_transacao(cliente, confirmar))
            }));
        }
        Pedido::AutoCommit(ligado, resposta) => {
            let feito = responder(runtime, conexao, |cliente| {
                Box::pin(trocar_auto_commit(cliente, ligado))
            });
            if feito.is_ok() {
                conexao.abertura.auto_commit = ligado;
            }
            let _ = resposta.send(feito);
        }
        Pedido::Sugerir(banco, esquema, prefixo, filtro, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, esquema, prefixo, filtro) = (
                    banco.clone(),
                    esquema.clone(),
                    prefixo.clone(),
                    filtro.clone(),
                );
                Box::pin(async move {
                    metadados_sqlserver::sugerir(
                        cliente,
                        banco.as_deref(),
                        esquema.as_deref(),
                        &prefixo,
                        filtro.as_ref(),
                    )
                    .await
                })
            }));
        }
        Pedido::Colunas(banco, objeto, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, objeto) = (banco.clone(), objeto.clone());
                Box::pin(async move {
                    metadados_sqlserver::colunas(cliente, banco.as_deref(), &objeto).await
                })
            }));
        }
        Pedido::ListarObjetos(pedido) => {
            let PedidoDeListagem {
                banco,
                categoria,
                filtro,
                deslocamento,
                limite,
                resposta,
            } = *pedido;
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, filtro) = (banco.clone(), filtro.clone());
                Box::pin(async move {
                    objetos_sqlserver::listar(
                        cliente,
                        banco.as_deref(),
                        categoria,
                        filtro.as_ref(),
                        deslocamento,
                        limite,
                    )
                    .await
                })
            }));
        }
        Pedido::Fonte(banco, categoria, nome, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, nome) = (banco.clone(), nome.clone());
                Box::pin(async move {
                    objetos_sqlserver::fonte(cliente, banco.as_deref(), categoria, &nome).await
                })
            }));
        }
        Pedido::GravarComentario(banco, nome, comentario, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, nome, comentario) =
                    (banco.clone(), nome.clone(), comentario.clone());
                Box::pin(async move {
                    objetos_sqlserver::gravar_comentario(
                        cliente,
                        banco.as_deref(),
                        &nome,
                        &comentario,
                    )
                    .await
                })
            }));
        }
        Pedido::EstadoDoAgente(resposta) => {
            let _ = resposta.send(Ok(runtime.block_on(agente::estado(&mut conexao.cliente))));
        }
        Pedido::ResolverReferencia(banco, referencia, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, referencia) = (banco.clone(), referencia.clone());
                Box::pin(async move {
                    objetos_sqlserver::resolver_referencia(
                        cliente,
                        banco.as_deref(),
                        &referencia,
                    )
                    .await
                })
            }));
        }
        Pedido::ReferenciasNavegaveis(banco, referencias, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let (banco, referencias) = (banco.clone(), referencias.clone());
                Box::pin(async move {
                    objetos_sqlserver::referencias_navegaveis(
                        cliente,
                        banco.as_deref(),
                        &referencias,
                    )
                    .await
                })
            }));
        }
        Pedido::Trabalhos(filtro, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let filtro = filtro.clone();
                Box::pin(async move { agente::listar(cliente, filtro.as_ref()).await })
            }));
        }
        Pedido::DadosDeTrabalho(resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, |cliente| {
                Box::pin(agente::dados(cliente))
            }));
        }
        Pedido::CriarTrabalho(trabalho, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let trabalho = trabalho.clone();
                Box::pin(async move { agente::criar(cliente, &trabalho).await })
            }));
        }
        Pedido::Detalhe(id, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let id = id.clone();
                Box::pin(async move { agente::detalhe(cliente, &id).await })
            }));
        }
        Pedido::AtualizarTrabalho(id, trabalho, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let (id, trabalho) = (id.clone(), trabalho.clone());
                Box::pin(async move { agente::atualizar(cliente, &id, &trabalho).await })
            }));
        }
        Pedido::HabilitarTrabalho(id, ligado, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let id = id.clone();
                Box::pin(async move { agente::definir_habilitado(cliente, &id, ligado).await })
            }));
        }
        Pedido::RenomearTrabalho(id, nome, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let (id, nome) = (id.clone(), nome.clone());
                Box::pin(async move { agente::renomear(cliente, &id, &nome).await })
            }));
        }
        Pedido::ExcluirTrabalho(id, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let id = id.clone();
                Box::pin(async move { agente::excluir(cliente, &id).await })
            }));
        }
        Pedido::IniciarTrabalho(id, etapa, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let (id, etapa) = (id.clone(), etapa.clone());
                Box::pin(async move { agente::iniciar(cliente, &id, etapa.as_deref()).await })
            }));
        }
        Pedido::PararTrabalho(id, resposta) => {
            let _ = resposta.send(responder(runtime, conexao, move |cliente| {
                let id = id.clone();
                Box::pin(async move { agente::parar(cliente, &id).await })
            }));
        }
        Pedido::ExecucaoDoTrabalho(id, resposta) => {
            let _ = resposta.send(tentar(runtime, conexao, move |cliente| {
                let id = id.clone();
                Box::pin(async move { agente::execucao(cliente, &id).await })
            }));
        }
    }
    None
}

/// Roda a operação e, se ela falhar com a conexão morta, reabre a sessão e
/// **tenta outra vez**.
///
/// Só para leitura: lista de bancos, autocomplete, árvore do Agent. Repetir
/// sozinho é seguro porque nada disso altera o banco.
fn tentar<T>(
    runtime: &tokio::runtime::Runtime,
    conexao: &mut Conexao,
    mut operacao: impl for<'a> FnMut(&'a mut Cliente) -> BoxFuture<'a, Resultado<T>>,
) -> Resultado<T> {
    let erro = match runtime.block_on(operacao(&mut conexao.cliente)) {
        Ok(valor) => return Ok(valor),
        Err(erro) => erro,
    };
    if conexao.viva(runtime) {
        return Err(erro);
    }
    registro::aviso("sqlserver", format!("a conexão caiu ({erro}) — reabrindo"));
    conexao.reabrir(runtime, "leitura falhou")?;
    runtime.block_on(operacao(&mut conexao.cliente))
}

/// Roda a operação **sem repetir**, mas reabre a conexão que tiver morrido, para
/// o pedido seguinte já encontrar a sessão de pé.
///
/// É o caminho de tudo que escreve. Repetir sozinho um `insert` exigiria saber
/// se ele chegou a valer no banco antes de a conexão morrer, e não há como saber
/// com certeza — quem decide reexecutar é o usuário.
fn responder<T>(
    runtime: &tokio::runtime::Runtime,
    conexao: &mut Conexao,
    mut operacao: impl for<'a> FnMut(&'a mut Cliente) -> BoxFuture<'a, Resultado<T>>,
) -> Resultado<T> {
    let erro = match runtime.block_on(operacao(&mut conexao.cliente)) {
        Ok(valor) => return Ok(valor),
        Err(erro) => erro,
    };
    if conexao.viva(runtime) {
        return Err(erro);
    }
    registro::aviso(
        "sqlserver",
        format!("a conexão caiu ({erro}) — reabrindo sem repetir"),
    );
    match conexao.reabrir(runtime, "comando falhou") {
        Ok(()) => Err(erro.reconectado()),
        // A reconexão também falhou: o erro dela diz mais ao usuário do que o do
        // comando.
        Err(falha) => Err(falha),
    }
}

// --- Execução (requisitos 5, 10, 14 e 15) -----------------------------------

/// Executa os lotes de uma aba e, se o grid ficar pela metade, entra no laço do
/// cursor.
fn executar_lotes(
    runtime: &tokio::runtime::Runtime,
    conexao: &mut Conexao,
    pedidos: &Receiver<Pedido>,
    cancelamentos: &mut UnboundedReceiver<()>,
    contador: &mut u64,
    pedido: PedidoDeExecucao,
) -> Option<Pedido> {
    // Um "Parar" de uma execução que já terminou não pode matar a próxima.
    while cancelamentos.try_recv().is_ok() {}

    let PedidoDeExecucao {
        sessao_id,
        lotes,
        binds,
        tamanho_do_fetch,
        manter_cursor,
        resposta,
    } = pedido;
    let auto_commit = conexao.abertura.auto_commit;
    let reaberta = conexao.consumir_reabertura();
    // O banco da aba pode mudar por dentro, se o texto tiver um `use`: a lista
    // da barra precisa parar de mostrar o banco de antes (requisito 3).
    let pode_trocar_banco = lotes
        .iter()
        .any(|lote| execucao_sqlserver::parece_trocar_banco(&lote.sql));
    let mut resultados: Vec<ResultadoDaExecucao> = Vec::new();

    let Some((ultimo, anteriores)) = lotes.split_last() else {
        let _ = resposta.send(Err(Erro::novo("não há comando para executar")));
        return None;
    };

    // Os lotes anteriores ao último são lidos até o fim: é o que libera a
    // conexão para o lote seguinte. Só o último pode deixar cursor aberto.
    for lote in anteriores {
        let mut reparo = Reparo::Nada;
        let saida = rodar_ate_o_fim(
            runtime,
            &mut conexao.cliente,
            cancelamentos,
            lote,
            &binds,
            tamanho_do_fetch,
            auto_commit,
            &mut reparo,
        );
        conexao.consertar(runtime, reparo);
        match saida {
            Ok(deste) => resultados.extend(deste),
            Err(erro) => {
                // Os lotes que já passaram vão junto com o erro: quem executou
                // um script de dez lotes precisa ver os nove que rodaram.
                let _ = resposta.send(Err(erro.apos(std::mem::take(&mut resultados))));
                return None;
            }
        }
    }

    // Um lote anterior pode ter derrubado e reaberto a conexão; o aviso é deste
    // resultado, e não do próximo.
    let reaberta = reaberta || conexao.consumir_reabertura();

    *contador += 1;
    let cursor_id = format!("{sessao_id}:c{contador}");
    let mut reparo = Reparo::Nada;
    let sobra = ultimo_lote(
        runtime,
        &mut conexao.cliente,
        pedidos,
        cancelamentos,
        Lote {
            comando: ultimo,
            binds: &binds,
            tamanho_do_fetch,
            manter_cursor,
            auto_commit,
            cursor_id: &cursor_id,
        },
        resultados,
        reaberta,
        resposta,
        &mut reparo,
    );
    conexao.consertar(runtime, reparo);
    if pode_trocar_banco {
        if let Some(agora) = runtime.block_on(execucao_sqlserver::banco_corrente(&mut conexao.cliente)) {
            conexao.anotar_banco(Some(agora));
        }
    }
    sobra
}

/// Os parâmetros de um lote, juntos para a assinatura não virar uma lista.
struct Lote<'p> {
    comando: &'p Comando,
    binds: &'p [(String, String)],
    tamanho_do_fetch: usize,
    manter_cursor: bool,
    auto_commit: bool,
    cursor_id: &'p str,
}

/// Roda um lote lendo o fluxo até o fim, sem deixar cursor.
#[allow(clippy::too_many_arguments)]
fn rodar_ate_o_fim(
    runtime: &tokio::runtime::Runtime,
    cliente: &mut Cliente,
    cancelamentos: &mut UnboundedReceiver<()>,
    comando: &Comando,
    binds: &[(String, String)],
    tamanho_do_fetch: usize,
    auto_commit: bool,
    reparo: &mut Reparo,
) -> Resultado<Vec<ResultadoDaExecucao>> {
    let (mut resultados, precisa_contagem) = {
        let saida = executar_interrompivel(
            runtime,
            &mut *cliente,
            cancelamentos,
            comando,
            binds,
            tamanho_do_fetch,
            false,
            reparo,
        )?;
        let execucao_sqlserver::Execucao {
            resultados,
            cursor,
            precisa_contagem,
        } = saida;
        // Este lote não é o último: o que sobrou do resultado precisa sair da
        // frente antes do próximo, e sair da frente é lê-lo — com prazo.
        if let Some(mut resto) = cursor {
            *reparo = descartar(runtime, &mut resto);
        }
        (resultados, precisa_contagem)
    };

    if precisa_contagem {
        let depois = runtime.block_on(execucao_sqlserver::ler_depois_do_comando(cliente));
        if let Some(primeiro) = resultados.first_mut() {
            execucao_sqlserver::concluir_comando(primeiro, depois, auto_commit);
        }
    }
    Ok(resultados)
}

/// Roda o último lote e, se ele deixar grid pela metade, atende os pedidos do
/// cursor até o grid acabar, ser fechado ou dar lugar a outra coisa.
///
/// A resposta ao pedido de execução sai **daqui**, e não de quem chamou: com o
/// cursor aberto a thread não volta ao laço principal, e a aba não pode ficar
/// esperando um resultado que já existe.
#[allow(clippy::too_many_arguments)]
fn ultimo_lote(
    runtime: &tokio::runtime::Runtime,
    cliente: &mut Cliente,
    pedidos: &Receiver<Pedido>,
    cancelamentos: &mut UnboundedReceiver<()>,
    lote: Lote<'_>,
    anteriores: Vec<ResultadoDaExecucao>,
    reaberta: bool,
    resposta: Sender<Resultado<Vec<ResultadoDaExecucao>>>,
    reparo: &mut Reparo,
) -> Option<Pedido> {
    let mut resultados = anteriores;

    // O `Result` fica dentro do `match`, sem nome próprio: um `Result<Execucao>`
    // guardado numa variável tem destrutor, e o empréstimo do cliente duraria
    // até o fim da função — inclusive no caminho em que não sobra cursor nenhum.
    let execucao_sqlserver::Execucao {
        resultados: mut resultados_do_lote,
        cursor,
        precisa_contagem,
    } = match executar_interrompivel(
        runtime,
        &mut *cliente,
        cancelamentos,
        lote.comando,
        lote.binds,
        lote.tamanho_do_fetch,
        lote.manter_cursor,
        reparo,
    ) {
        Ok(saida) => saida,
        Err(erro) => {
            let _ = resposta.send(Err(erro.apos(std::mem::take(&mut resultados))));
            return None;
        }
    };

    let mut cursor = cursor;
    // No modo script ninguém vai pedir mais linhas: o que sobrou do resultado é
    // descartado aqui mesmo, com prazo, para a conexão ficar limpa.
    if !lote.manter_cursor {
        if let Some(resto) = cursor.as_mut() {
            *reparo = descartar(runtime, resto);
        }
        cursor = None;
    }

    if cursor.is_none() {
        // Sem cursor o empréstimo do cliente acaba aqui — e é o `drop` que o
        // encerra, para dar para perguntar quantas linhas o comando afetou.
        drop(cursor);
        if precisa_contagem {
            let depois = runtime.block_on(execucao_sqlserver::ler_depois_do_comando(cliente));
            if let Some(primeiro) = resultados_do_lote.first_mut() {
                execucao_sqlserver::concluir_comando(primeiro, depois, lote.auto_commit);
            }
        }
        resultados.extend(resultados_do_lote);
        avisar_reabertura(&mut resultados, reaberta);
        let _ = resposta.send(Ok(resultados));
        return None;
    }
    let mut cursor = cursor.expect("o cursor acabou de ser conferido");

    for resultado in resultados_do_lote.iter_mut() {
        if let Some(consulta) = resultado.consulta.as_mut() {
            if consulta.tem_mais {
                consulta.cursor_id = lote.cursor_id.to_string();
            }
        }
    }
    resultados.extend(resultados_do_lote);
    avisar_reabertura(&mut resultados, reaberta);
    let _ = resposta.send(Ok(resultados));

    laco_do_cursor(
        runtime,
        pedidos,
        cancelamentos,
        &mut cursor,
        lote.cursor_id,
        reparo,
    )
}

/// Atende os pedidos enquanto o grid está pela metade.
fn laco_do_cursor(
    runtime: &tokio::runtime::Runtime,
    pedidos: &Receiver<Pedido>,
    cancelamentos: &mut UnboundedReceiver<()>,
    cursor: &mut Cursor<'_>,
    cursor_id: &str,
    reparo: &mut Reparo,
) -> Option<Pedido> {
    loop {
        let Ok(pedido) = pedidos.recv() else {
            // A sessão está sendo encerrada; não há o que preservar.
            return None;
        };
        match pedido {
            Pedido::BuscarMais(id, quantidade, resposta) if id == cursor_id => {
                let mut interrompido = false;
                let bloco = runtime.block_on(async {
                    tokio::select! {
                        biased;
                        _ = cancelamentos.recv() => {
                            interrompido = true;
                            Err(Erro::novo("busca interrompida a pedido"))
                        }
                        lido = cursor.buscar_mais(quantidade) => lido,
                    }
                });
                match bloco {
                    Ok((linhas, tem_mais)) => {
                        let _ = resposta.send(Ok(BlocoDeLinhas { linhas, tem_mais }));
                        if !tem_mais {
                            return None;
                        }
                    }
                    Err(erro) => {
                        let _ = resposta.send(Err(erro));
                        *reparo = if interrompido {
                            Reparo::Reabrir("busca interrompida no meio")
                        } else {
                            Reparo::Conferir
                        };
                        return None;
                    }
                }
            }
            Pedido::FecharCursor(id, resposta) if id == cursor_id => {
                let _ = resposta.send(Ok(()));
                *reparo = descartar(runtime, cursor);
                return None;
            }
            // Cursor de outro grid, que já não existe: responder é o suficiente.
            Pedido::FecharCursor(_, resposta) => {
                let _ = resposta.send(Ok(()));
            }
            Pedido::BuscarMais(_, _, resposta) => {
                let _ = resposta.send(Err(Erro::novo(
                    "o resultado desta consulta não está mais disponível",
                )));
            }
            // Qualquer outra coisa toma a conexão: o grid pela metade sai de
            // cena, e quem chamou atende o pedido no laço principal.
            outro => {
                *reparo = descartar(runtime, cursor);
                return Some(outro);
            }
        }
    }
}

/// Lê o que sobrou de um resultado abandonado, dentro do prazo.
///
/// Passado o prazo não adianta insistir: a conexão fica presa até a última linha
/// chegar, e quem está na tela já pediu outra coisa. Reabrir custa um login.
fn descartar(runtime: &tokio::runtime::Runtime, cursor: &mut Cursor<'_>) -> Reparo {
    let inicio = Instant::now();
    let esvaziado = runtime.block_on(async {
        tokio::time::timeout(ESPERA_PARA_ESVAZIAR, cursor.esvaziar()).await
    });
    match esvaziado {
        Ok(Ok(())) => Reparo::Nada,
        Ok(Err(erro)) => {
            registro::aviso("sqlserver", format!("resultado abandonado falhou: {erro}"));
            Reparo::Conferir
        }
        Err(_) => {
            registro::aviso(
                "sqlserver",
                format!(
                    "resultado grande abandonado depois de {} ms — a conexão será reaberta",
                    inicio.elapsed().as_millis()
                ),
            );
            Reparo::Reabrir("resultado grande abandonado")
        }
    }
}

/// Executa um lote podendo ser interrompido pelo botão Parar (requisito 14).
///
/// Interromper aqui é largar a conexão: o TDS não tem, no `tiberius`, como
/// mandar o sinal de atenção que o SSMS manda. Largar o socket faz o servidor
/// abortar a consulta do mesmo jeito — é o que aconteceria com um `KILL` —, e a
/// conexão é reaberta em seguida, com o banco e o auto-commit de volta.
#[allow(clippy::too_many_arguments)]
fn executar_interrompivel<'a>(
    runtime: &tokio::runtime::Runtime,
    cliente: &'a mut Cliente,
    cancelamentos: &mut UnboundedReceiver<()>,
    comando: &Comando,
    binds: &[(String, String)],
    tamanho_do_fetch: usize,
    manter_cursor: bool,
    reparo: &mut Reparo,
) -> Resultado<execucao_sqlserver::Execucao<'a>> {
    let mut interrompido = false;
    let saida = runtime.block_on(async {
        tokio::select! {
            biased;
            _ = cancelamentos.recv() => {
                interrompido = true;
                Err(Erro::novo("execução interrompida a pedido"))
            }
            resultado = execucao_sqlserver::executar(
                cliente,
                comando,
                binds,
                tamanho_do_fetch,
                manter_cursor,
            ) => resultado,
        }
    });

    match saida {
        Ok(saida) => Ok(saida),
        Err(erro) => {
            *reparo = if interrompido {
                registro::aviso("sqlserver", "execução interrompida a pedido do usuário");
                Reparo::Reabrir("execução interrompida")
            } else {
                Reparo::Conferir
            };
            Err(execucao_sqlserver::com_linha_do_editor(erro, comando))
        }
    }
}

/// Marca o primeiro resultado quando a sessão foi reaberta antes deste lote.
fn avisar_reabertura(resultados: &mut [ResultadoDaExecucao], reaberta: bool) {
    if !reaberta {
        return;
    }
    if let Some(primeiro) = resultados.first_mut() {
        primeiro.reconectado = true;
    }
}

// --- Transação e banco corrente (requisitos 3 e 7) --------------------------

/// Deixa a sessão nova no mesmo estado da que ela substitui.
async fn preparar_sessao(
    cliente: &mut Cliente,
    banco: Option<&str>,
    auto_commit: bool,
) -> Resultado<()> {
    if let Some(banco) = banco {
        usar_banco(cliente, banco).await?;
    }
    definir_transacoes_implicitas(cliente, !auto_commit).await
}

/// O auto-commit do SQL Server é `SET IMPLICIT_TRANSACTIONS`, e ao contrário do
/// Oracle ele é **estado do servidor**.
///
/// Com transações implícitas ligadas, o primeiro comando abre uma transação que
/// só fecha no commit ou no rollback — que é exatamente o auto-commit desligado
/// do requisito 7. É também o que os drivers padrão (JDBC, ODBC) fazem quando se
/// pede `setAutoCommit(false)`.
async fn definir_transacoes_implicitas(cliente: &mut Cliente, ligadas: bool) -> Resultado<()> {
    let valor = if ligadas { "on" } else { "off" };
    cliente
        .simple_query(format!("set implicit_transactions {valor}"))
        .await?
        .into_results()
        .await?;
    Ok(())
}

/// Commit ou rollback do que a aba deixou pendente.
///
/// O `if @@trancount > 0` não é zelo: um `COMMIT` sem transação aberta é **erro**
/// no SQL Server, e no Oracle é uma não-operação. O botão é o mesmo nos dois, e
/// apertá-lo sem nada pendente não pode devolver erro num banco e silêncio no
/// outro. O `while` do commit fecha também os níveis aninhados, que um `begin
/// tran` escrito à mão pode ter deixado.
async fn encerrar_transacao(cliente: &mut Cliente, confirmar: bool) -> Resultado<()> {
    let comando = if confirmar {
        "while @@trancount > 0 commit transaction"
    } else {
        "if @@trancount > 0 rollback transaction"
    };
    cliente.simple_query(comando).await?.into_results().await?;
    Ok(())
}

/// Liga ou desliga o auto-commit, confirmando o que estiver pendente ao ligar.
///
/// Ligar o auto-commit com transação aberta e deixá-la aberta faria o
/// interruptor mentir: a tela diria "auto-commit" e nada seria confirmado, nem o
/// que veio antes nem o que viesse depois — porque a transação aberta engole os
/// comandos seguintes. Confirmar é o que os drivers padrão fazem, e é a leitura
/// honesta de quem ligou o interruptor.
async fn trocar_auto_commit(cliente: &mut Cliente, ligado: bool) -> Resultado<()> {
    if !ligado {
        return definir_transacoes_implicitas(cliente, true).await;
    }
    cliente
        .simple_query("while @@trancount > 0 commit transaction; set implicit_transactions off")
        .await?
        .into_results()
        .await?;
    Ok(())
}

/// Troca o banco da aba, recusando enquanto houver transação aberta.
///
/// O `USE` não é aceito dentro de transação, e com o auto-commit desligado
/// (transações implícitas) sempre há uma aberta depois do primeiro comando.
/// Confirmar ou desfazer por conta própria para poder trocar seria decidir pelo
/// usuário o destino do trabalho dele; dizer o que falta é a resposta certa.
async fn trocar_banco_da_aba(cliente: &mut Cliente, banco: String) -> Resultado<()> {
    let abertas = cliente
        .simple_query("select @@trancount")
        .await?
        .into_row()
        .await?
        .and_then(|linha| linha.get::<i32, _>(0))
        .unwrap_or(0);
    if abertas > 0 {
        return Err(Erro::novo(
            "há transação aberta nesta aba; confirme (commit) ou desfaça (rollback) antes de trocar de banco",
        ));
    }
    usar_banco(cliente, &banco).await
}


/// Usa o banco pedido, ou o primeiro da lista quando nenhum foi indicado.
async fn escolher_banco(cliente: &mut Cliente, banco: Option<String>) -> Resultado<Option<String>> {
    if let Some(banco) = banco {
        usar_banco(cliente, &banco).await?;
        return Ok(Some(banco));
    }

    // Sem os do sistema: a sessão nasce no primeiro banco de **trabalho**, que é
    // o mesmo primeiro nome que a árvore mostra sob "Banco de Dados".
    let Some(primeiro) = listar_bancos(cliente, false).await?.into_iter().next() else {
        // Login sem acesso a banco nenhum: a conexão está de pé, mas não há onde
        // trabalhar. Dizer isso vale mais que abrir uma sessão inútil.
        return Err(Erro::novo(
            "este login não tem acesso a nenhum banco de dados de trabalho neste servidor",
        ));
    };
    usar_banco(cliente, &primeiro).await?;
    Ok(Some(primeiro))
}

/// Bancos que este login enxerga e pode abrir.
///
/// `state = 0` deixa de fora os que estão offline ou restaurando, e
/// `has_dbaccess` deixa de fora os que a conta não pode abrir — listar o que vai
/// falhar ao ser clicado só atrapalha.
async fn listar_bancos(cliente: &mut Cliente, incluir_sistema: bool) -> Resultado<Vec<String>> {
    // `database_id > 4` tira `master` (1), `tempdb` (2), `model` (3) e `msdb`
    // (4). Os quatro têm identificador fixo desde sempre, o que torna o corte
    // por número mais seguro que por nome: um banco de trabalho pode se chamar
    // qualquer coisa, mas não pode ocupar esses identificadores.
    let sem_sistema = if incluir_sistema { "" } else { "and database_id > 4 " };
    let linhas = cliente
        .simple_query(format!(
            "select name from sys.databases \
             where state = 0 and has_dbaccess(name) = 1 {sem_sistema}order by name"
        ))
        .await?
        .into_first_result()
        .await?;

    Ok(linhas
        .iter()
        .filter_map(|linha| linha.get::<&str, _>("name"))
        .map(str::to_string)
        .collect())
}

async fn usar_banco(cliente: &mut Cliente, banco: &str) -> Resultado<()> {
    // Nome de banco não vai em bind: `USE` não aceita parâmetro, e o nome entra
    // no texto do comando. O colchete duplicado é o que impede o nome de virar
    // comando.
    cliente
        .simple_query(format!("use {}", entre_colchetes(banco)))
        .await?
        .into_results()
        .await?;
    Ok(())
}

/// `meu]banco` vira `[meu]]banco]`, como o SQL Server exige.
pub fn entre_colchetes(nome: &str) -> String {
    format!("[{}]", nome.replace(']', "]]"))
}

/// Abre a conexão, com o mesmo teto de espera do teste.
async fn abrir_com_espera(config: Config, instancia: Option<String>) -> Resultado<Cliente> {
    match tokio::time::timeout(ESPERA_DA_CONEXAO, abrir_cliente(config, instancia)).await {
        Ok(resultado) => resultado,
        Err(_) => Err(Erro::novo(format!(
            "o servidor não respondeu em {} segundos",
            ESPERA_DA_CONEXAO.as_secs()
        ))),
    }
}

/// Monta a configuração do driver e diz qual instância precisa do SQL Browser.
fn preparar(perfil: &Perfil) -> Resultado<(Config, Option<String>)> {
    let destino = interpretar_servidor(&perfil.servidor)?;

    let mut config = Config::new();
    config.host(&destino.host);
    // A porta só é informada quando o usuário a escreveu, e isso **não** é
    // detalhe: uma porta fixada aqui vence a que o `tiberius` escolheria
    // sozinho, e com instância nomeada a escolha dele é a **UDP 1434**, do SQL
    // Browser. Fixando 1433 para todo mundo, a pergunta pela porta da instância
    // ia para a porta errada, ninguém respondia e toda instância nomeada
    // (`SERVIDOR\INSTANCIA`, `192.0.2.10\INSTANCIA_EXEMPLO`, `.\SQLEXPRESS`) falhava com
    // um tempo esgotado que parecia firewall. Sem porta e sem instância o
    // driver usa 1433, que é o que se queria dizer.
    if let Some(porta) = destino.porta {
        config.port(porta);
    }
    if let Some(instancia) = &destino.instancia {
        config.instance_name(instancia);
    }
    config.authentication(autenticacao(perfil)?);
    // Aparece na coluna `program_name` do `sys.dm_exec_sessions`: o DBA que
    // olhar as sessões do servidor precisa saber quem chegou.
    config.application_name("Cururu");

    // Mesmo comportamento do SSMS com "Encrypt connection" desmarcado: o login
    // vai cifrado — a senha nunca atravessa a rede em claro —, o resto da sessão
    // não, e o certificado do servidor não é validado. Validar exigiria uma
    // autoridade certificadora que instalação interna de SQL Server quase nunca
    // tem, e sem isto não se conectaria em base nenhuma da casa.
    config.encryption(EncryptionLevel::Off);
    config.trust_cert();

    // Com porta explícita não se pergunta ao SQL Browser: o usuário já disse
    // onde a instância atende, e a consulta UDP só somaria espera.
    let instancia = destino.porta.is_none().then_some(destino.instancia).flatten();
    Ok((config, instancia))
}

/// Testa a conexão e devolve o banner de versão do servidor.
pub fn testar(perfil: &Perfil) -> Resultado<Option<String>> {
    let (config, instancia) = preparar(perfil)?;
    bloqueando(async move {
        let mut cliente = abrir_com_espera(config, instancia).await?;
        let versao = ler_versao(&mut cliente).await;
        // Desligar é melhor esforço: o teste já respondeu o que interessava.
        let _ = cliente.close().await;
        versao
    })
}

/// Consulta periódica numa sessão exclusiva, com prazo para conexão e leitura.
pub fn consultar_alerta(
    perfil: &Perfil, banco: &str, consulta: &str, legada: bool,
) -> Resultado<Option<String>> {
    let (mut config, instancia) = preparar(perfil)?;
    config.database(banco);
    let sql = if legada {
        format!("SELECT TOP (2) valor FROM (\n{consulta}\n) AS alerta(valor)")
    } else {
        format!("SELECT TOP (1) CONVERT(nvarchar(max), valor) FROM (\n{consulta}\n) AS alerta(valor)")
    };
    bloqueando(async move {
        let mut cliente = match abrir_com_espera(config.clone(), instancia.clone()).await {
            Ok(cliente) => cliente,
            Err(_) => abrir_com_espera(config.clone(), instancia.clone()).await?,
        };
        let resultado = ler_resultado_do_alerta(&mut cliente, &sql).await;
        let linhas = match resultado {
            Ok(Err(tiberius::error::Error::Io { .. })) => {
                let mut nova = abrir_com_espera(config, instancia).await?;
                ler_resultado_do_alerta(&mut nova, &sql).await
            }
            resultado => resultado,
        }
        .map_err(|_| Erro::novo("tempo esgotado na consulta do alerta"))??;
        let linha = match linhas.as_slice() {
            [] => return Ok(None),
            [linha] => linha,
            _ => return Err(Erro::novo("a consulta do alerta precisa retornar uma linha")),
        };
        if !legada {
            return Ok(linha.try_get::<&str, _>(0)?.map(str::to_owned));
        }
        use tiberius::ColumnType;
        let valor = match linha.columns()[0].column_type() {
            ColumnType::Int1 => linha.try_get::<u8, _>(0)?.map(i64::from),
            ColumnType::Int2 => linha.try_get::<i16, _>(0)?.map(i64::from),
            ColumnType::Int4 => linha.try_get::<i32, _>(0)?.map(i64::from),
            ColumnType::Int8 => linha.try_get::<i64, _>(0)?,
            _ => None,
        };
        valor.map(|numero| Some(numero.to_string()))
            .ok_or_else(|| Erro::novo("a consulta do alerta precisa retornar um inteiro"))
    })
}

async fn ler_resultado_do_alerta(
    cliente: &mut Cliente,
    sql: &str,
) -> std::result::Result<tiberius::Result<Vec<tiberius::Row>>, tokio::time::error::Elapsed> {
    tokio::time::timeout(Duration::from_secs(15), async {
        cliente.simple_query(sql).await?.into_first_result().await
    })
    .await
}

/// Lê o catálogo numa conexão que não disputa a fila da árvore ou das abas.
pub async fn carregar_catalogo(
    perfil: &Perfil,
    banco: &str,
) -> Resultado<Option<metadados_sqlserver::Catalogo>> {
    let (config, instancia) = preparar(perfil)?;
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut cliente = abrir_com_espera(config, instancia).await?;
        usar_banco(&mut cliente, banco).await?;
        metadados_sqlserver::catalogo(&mut cliente).await
    })
    .await
    .map_err(|_| Erro::novo("tempo esgotado na leitura do catálogo"))?
}

async fn ler_versao(cliente: &mut Cliente) -> Resultado<Option<String>> {
    let linha = cliente
        .query("select @@version", &[])
        .await?
        .into_row()
        .await?;
    Ok(linha
        .as_ref()
        .and_then(|linha| linha.get::<&str, _>(0))
        .map(colapsar_espacos))
}

/// Abre o socket e faz o login.
async fn abrir_cliente(config: Config, instancia: Option<String>) -> Resultado<Cliente> {
    let tcp = if let Some(instancia) = instancia {
        // Instância nomeada sem porta explícita: quem diz em que porta ela
        // atende é o SQL Browser, perguntado em UDP 1434. Falhar aqui devolve o
        // erro cru do socket, que não diz o que fazer — serviço parado e porta
        // UDP bloqueada no firewall são as duas causas comuns, e quem lê a
        // mensagem precisa saber onde olhar.
        TcpStream::connect_named(&config).await.map_err(|erro| {
            let motivo = Erro::from(erro).mensagem;
            Erro::novo(format!(
                "não foi possível localizar a instância {instancia}: {motivo} — confira se o \
                 serviço SQL Server Browser está no ar e se a porta UDP 1434 está liberada, ou \
                 informe a porta direto no nome do servidor (servidor,porta)"
            ))
        })?
    } else {
        TcpStream::connect(config.get_addr()).await?
    };
    // Sem isto o TDS paga o atraso do algoritmo de Nagle em cada ida e volta.
    tcp.set_nodelay(true)?;

    Ok(Client::connect(config, tcp.compat_write()).await?)
}

/// O `@@version` vem em várias linhas, com tabulações. A barra de status quer
/// uma linha só.
fn colapsar_espacos(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Roda o futuro numa thread própria, com uma runtime de uma linha só.
///
/// Não dá para chamar `block_on` na thread de quem chamou: todo comando marcado
/// `#[tauri::command(async)]` já roda **dentro** de um worker do tokio do Tauri,
/// e abrir uma runtime dentro de outra é pânico imediato. A thread nova não tem
/// runtime no contexto, e ali o `block_on` é legítimo.
///
/// Ela nasce e morre com o teste. Nada fica de pé entre um clique e outro: uma
/// runtime `current_thread` não tem pool de workers para manter, e o custo em
/// memória volta a zero assim que a thread termina.
fn bloqueando<T, F>(futuro: F) -> Resultado<T>
where
    F: Future<Output = Resultado<T>> + Send,
    T: Send,
{
    std::thread::scope(|escopo| {
        escopo
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|erro| {
                        Erro::novo(format!("não foi possível preparar a rede: {erro}"))
                    })?;
                runtime.block_on(futuro)
            })
            .join()
            .unwrap_or_else(|_| {
                Err(Erro::novo(
                    "a tentativa de conexão terminou de forma inesperada",
                ))
            })
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn destino(servidor: &str) -> Destino {
        interpretar_servidor(servidor).expect("deveria interpretar")
    }

    #[test]
    fn host_sozinho_usa_a_porta_padrao() {
        let alvo = destino("SRVSQL01");
        assert_eq!(alvo.host, "SRVSQL01");
        assert_eq!(alvo.instancia, None);
        assert_eq!(alvo.porta, None);
    }

    #[test]
    fn instancia_nomeada_e_separada_do_host() {
        let alvo = destino(r"SRVSQL01\PROD");
        assert_eq!(alvo.host, "SRVSQL01");
        assert_eq!(alvo.instancia.as_deref(), Some("PROD"));
        assert_eq!(alvo.porta, None);
    }

    #[test]
    fn porta_explicita_e_lida() {
        let alvo = destino("SRVSQL01,1433");
        assert_eq!(alvo.host, "SRVSQL01");
        assert_eq!(alvo.porta, Some(1433));

        let com_instancia = destino(r"SRVSQL01\PROD,49812");
        assert_eq!(com_instancia.host, "SRVSQL01");
        assert_eq!(com_instancia.instancia.as_deref(), Some("PROD"));
        assert_eq!(com_instancia.porta, Some(49812));
    }

    #[test]
    fn prefixo_tcp_do_ssms_e_aceito() {
        assert_eq!(destino("tcp:SRVSQL01,1433"), destino("SRVSQL01,1433"));
    }

    #[test]
    fn ponto_e_local_querem_dizer_esta_maquina() {
        assert_eq!(destino(".").host, "localhost");
        assert_eq!(destino("(local)").host, "localhost");
        assert_eq!(destino(r".\SQLEXPRESS").instancia.as_deref(), Some("SQLEXPRESS"));
    }

    #[test]
    fn endereco_ipv6_nao_e_confundido_com_prefixo_de_protocolo() {
        // Os dois-pontos do IPv6 não podem ser lidos como "protocolo:resto".
        assert_eq!(destino("[::1],1433").host, "[::1]");
    }

    #[test]
    fn servidor_invalido_e_recusado_com_o_motivo() {
        for servidor in ["", "   ", r"SRVSQL01\", "SRVSQL01,0", "SRVSQL01,porta"] {
            assert!(
                interpretar_servidor(servidor).is_err(),
                "deveria recusar: {servidor:?}"
            );
        }
        assert!(interpretar_servidor("np:SRVSQL01")
            .expect_err("named pipe não é atendido")
            .mensagem
            .contains("TCP/IP"));
    }

    /// O endereço para onde o driver vai perguntar — e o defeito que ele pega.
    ///
    /// Instância nomeada sem porta precisa sair em **1434**: é a porta do SQL
    /// Browser, a quem se pergunta onde a instância atende. Enquanto a porta
    /// era fixada em 1433 aqui, a pergunta ia para a porta errada e nenhuma
    /// instância nomeada conectava.
    #[test]
    fn instancia_nomeada_pergunta_ao_sql_browser_na_1434() {
        let endereco = |servidor: &str| {
            let perfil = Perfil {
                servidor: servidor.to_string(),
                autenticacao_sql: AutenticacaoSql::Windows,
                ..Perfil::default()
            };
            preparar(&perfil).expect("deveria preparar").0.get_addr()
        };

        assert_eq!(endereco(r"192.0.2.10\INSTANCIA_EXEMPLO"), "192.0.2.10:1434");
        assert_eq!(endereco(r"SRVSQL01\PROD"), "SRVSQL01:1434");
        assert_eq!(endereco(r".\SQLEXPRESS"), "localhost:1434");
        // Sem instância, a porta padrão do SQL Server.
        assert_eq!(endereco("SRVSQL01"), "SRVSQL01:1433");
        // Com porta escrita, é ela — e aí não se pergunta a ninguém.
        assert_eq!(endereco(r"SRVSQL01\PROD,49812"), "SRVSQL01:49812");
        assert_eq!(endereco("192.0.2.10,1433"), "192.0.2.10:1433");
    }

    /// Só se pergunta ao SQL Browser quando há instância nomeada **e** nenhuma
    /// porta escrita: com a porta na mão, a consulta UDP só somaria espera.
    #[test]
    fn o_sql_browser_so_e_consultado_quando_falta_a_porta() {
        let instancia = |servidor: &str| {
            let perfil = Perfil {
                servidor: servidor.to_string(),
                autenticacao_sql: AutenticacaoSql::Windows,
                ..Perfil::default()
            };
            preparar(&perfil).expect("deveria preparar").1
        };

        assert_eq!(instancia(r"192.0.2.10\INSTANCIA_EXEMPLO").as_deref(), Some("INSTANCIA_EXEMPLO"));
        assert_eq!(instancia(r"SRVSQL01\PROD,49812"), None);
        assert_eq!(instancia("SRVSQL01"), None);
    }

    #[test]
    fn autenticacao_do_windows_sem_usuario_e_integrada() {
        let perfil = Perfil {
            autenticacao_sql: AutenticacaoSql::Windows,
            ..Perfil::default()
        };
        assert!(matches!(
            autenticacao(&perfil).unwrap(),
            AuthMethod::Integrated
        ));
    }

    #[test]
    fn autenticacao_do_sql_server_exige_logon() {
        let perfil = Perfil {
            autenticacao_sql: AutenticacaoSql::SqlServer,
            ..Perfil::default()
        };
        assert!(autenticacao(&perfil).is_err());
    }

    /// O Testar precisa sobreviver a ser chamado **de dentro** de uma runtime.
    ///
    /// É exatamente onde ele roda de verdade: `#[tauri::command(async)]` sobre
    /// função síncrona executa num worker do tokio do Tauri. Se `bloqueando`
    /// deixasse de abrir a thread própria, isto viraria pânico
    /// ("Cannot start a runtime from within a runtime") no primeiro clique — e
    /// sem este teste a falha só apareceria na mão de quem usa.
    #[test]
    fn testar_funciona_chamado_de_dentro_de_uma_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("runtime de teste");

        let perfil = Perfil {
            tipo_banco: crate::perfis::TipoBanco::SqlServer,
            // Porta 1 em loopback: ninguém atende, e a recusa volta na hora.
            servidor: "127.0.0.1,1".to_string(),
            autenticacao_sql: AutenticacaoSql::SqlServer,
            usuario: "sa".to_string(),
            senha: Some("qualquer".to_string()),
            ..Perfil::default()
        };

        let erro = runtime
            .block_on(async move {
                tokio::task::spawn_blocking(move || testar(&perfil))
                    .await
                    .expect("a tarefa não pode entrar em pânico")
            })
            .expect_err("ninguém atende na porta 1");
        assert!(
            !erro.mensagem.is_empty(),
            "a falha de conexão precisa chegar com motivo"
        );
        // O Windows já entrega o texto no idioma do sistema; o embrulho em
        // inglês do driver não pode voltar a vazar para a barra de status.
        assert!(
            !erro.mensagem.contains("An error occured"),
            "a mensagem saiu com o prefixo em inglês do driver: {}",
            erro.mensagem
        );
    }

    #[test]
    fn banner_de_versao_vira_uma_linha_so() {
        let banner = "Microsoft SQL Server 2019\n\t(RTM) - 15.0.2000.5\n\tStandard Edition";
        assert_eq!(
            colapsar_espacos(banner),
            "Microsoft SQL Server 2019 (RTM) - 15.0.2000.5 Standard Edition"
        );
    }
}
