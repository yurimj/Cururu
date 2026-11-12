//! Arquivos que o Windows manda abrir (requisito 12).
//!
//! Duplo clique num `.sql`, **Abrir com** e arquivo arrastado até o
//! `cururu.exe` chegam do mesmo jeito: o Windows inicia **um processo novo**
//! com o caminho na linha de comando. Com o Cururu já aberto, isso seria uma
//! segunda janela — outra árvore, outras conexões, a senha pedida de novo —
//! para mostrar um texto que cabia numa aba da primeira.
//!
//! Então o processo novo **entrega** os arquivos à janela que já está aberta e
//! sai sem mostrar nada. Só abre a própria quando não há outra, ou quando
//! nenhuma aceita. Abrir o Cururu **sem** arquivo continua abrindo outra
//! janela, como sempre: a entrega é do arquivo, e não uma trava de instância
//! única (ver premissas.md).
//!
//! Tudo com o que o próprio Windows oferece, sem dependência nova:
//!
//! - um **mutex nomeado** diz que há um Cururu vivo — inclusive o que ainda
//!   está subindo e não tem onde receber;
//! - cada Cururu tem uma **janela de mensagens**, invisível e fora da barra de
//!   tarefas, que recebe os caminhos por `WM_COPYDATA`;
//! - o que chega vai para uma **fila**, que a interface esvazia quando está
//!   pronta. A janela de mensagens nasce antes de a página carregar, e um
//!   arquivo pode chegar antes de haver quem o abra.

use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM,
};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, FindWindowExW,
    GetWindowThreadProcessId, RegisterClassW, SendMessageTimeoutW, HWND_MESSAGE,
    SMTO_ABORTIFHUNG, WM_COPYDATA, WNDCLASSW,
};

use crate::registro;

/// Diz que há um Cururu vivo. `Local\` é o espaço da sessão do Windows: outro
/// usuário logado na mesma máquina tem os seus.
///
/// O build de depuração usa outros nomes, e os dois não se enxergam: o `.sql`
/// aberto pelo Windows vai para o Cururu de verdade, e não para o que está
/// sendo desenvolvido.
const PRESENCA: &str = if cfg!(debug_assertions) {
    r"Local\br.dev.cururu.presenca.depuracao"
} else {
    r"Local\br.dev.cururu.presenca"
};

/// Classe da janela de mensagens, pela qual o processo novo a encontra.
const CLASSE: &str = if cfg!(debug_assertions) {
    "br.dev.cururu.arquivos.depuracao"
} else {
    "br.dev.cururu.arquivos"
};

/// Marca do pacote: um `WM_COPYDATA` que não a traga não é tomado por arquivo.
const MARCA: usize = 0x4355_5255; // "CURU"

/// Resposta de quem aceitou os arquivos. Zero é recusa.
const ACEITO: LRESULT = 1;

/// Quanto o processo novo espera a janela de mensagens de um Cururu que ainda
/// está subindo. Na prática ela nasce em menos de um segundo; o prazo é para a
/// máquina lenta, e só corre quando há um Cururu vivo que ainda não tem janela.
const ESPERA_PELA_JANELA: Duration = Duration::from_secs(10);

/// Quanto espera a resposta de uma janela encontrada. Passado isso ela é tida
/// por travada, e o arquivo abre numa janela nova.
const ESPERA_PELA_RESPOSTA_MS: u32 = 5_000;

/// Arquivos recebidos que a interface ainda não abriu.
static FILA: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Ligado quando o fechamento foi pedido: aceitar arquivo a partir daí e fechar
/// em seguida seria perdê-lo. Recusado, quem mandou abre a própria janela.
static RECUSANDO: AtomicBool = AtomicBool::new(false);

/// Para trazer a janela para a frente e avisar a interface.
static APLICATIVO: OnceLock<AppHandle> = OnceLock::new();

/// O que a linha de comando pediu para abrir.
pub struct Pedido {
    /// Arquivos que existem, com o caminho inteiro.
    pub arquivos: Vec<String>,
    /// O resto — pasta, caminho que não existe —, que só vai para o log.
    pub ignorados: Vec<String>,
}

/// Lê os argumentos com que o processo foi iniciado.
pub fn pedido_da_linha_de_comando() -> Pedido {
    let pasta = std::env::current_dir().unwrap_or_default();
    separar(std::env::args_os().skip(1), &pasta)
}

fn separar(argumentos: impl Iterator<Item = OsString>, pasta: &Path) -> Pedido {
    let mut pedido = Pedido {
        arquivos: Vec::new(),
        ignorados: Vec::new(),
    };
    for argumento in argumentos {
        // O Windows manda o caminho inteiro; relativo só aparece digitando no
        // terminal. Ele é resolvido aqui porque a janela que vai recebê-lo pode
        // estar noutra pasta.
        let caminho = pasta.join(&argumento);
        match caminho.to_str() {
            Some(texto) if caminho.is_file() => pedido.arquivos.push(texto.to_string()),
            _ => pedido
                .ignorados
                .push(argumento.to_string_lossy().into_owned()),
        }
    }
    pedido
}

/// Registra que este Cururu está vivo, e diz se já havia outro.
///
/// O mutex fica aberto até o processo acabar — ninguém o trava, o que conta é
/// ele existir. Se a criação falhar, este Cururu segue como se estivesse
/// sozinho: o pior que acontece é abrir uma janela a mais.
pub fn marcar_presenca() -> bool {
    marcar_presenca_em(PRESENCA)
}

fn marcar_presenca_em(nome: &str) -> bool {
    let nome = texto_largo(nome);
    // SEGURANÇA: o nome termina em zero e atributos nulos são os padrão. O
    // `GetLastError` vem logo em seguida, antes de qualquer outra chamada que
    // pudesse trocá-lo. O handle não é fechado de propósito: ele é a presença.
    let (mutex, erro) = unsafe {
        let mutex = CreateMutexW(std::ptr::null(), 0, nome.as_ptr());
        (mutex, GetLastError())
    };
    !mutex.is_null() && erro == ERROR_ALREADY_EXISTS
}

/// Entrega os arquivos a um Cururu já aberto. `true` quando algum aceitou.
pub fn entregar(arquivos: &[String]) -> bool {
    entregar_em(CLASSE, arquivos, ESPERA_PELA_JANELA)
}

fn entregar_em(classe: &str, arquivos: &[String], espera: Duration) -> bool {
    let classe = texto_largo(classe);
    let pacote = empacotar(arquivos);
    let limite = Instant::now() + espera;
    loop {
        // Pode haver mais de um Cururu aberto; qualquer um serve, e o que
        // estiver fechando recusa e passa a vez ao seguinte.
        let mut janela: HWND = std::ptr::null_mut();
        let mut achou = false;
        loop {
            // SEGURANÇA: a classe termina em zero; `janela` é nula ou uma
            // janela devolvida pela própria busca.
            janela = unsafe {
                FindWindowExW(HWND_MESSAGE, janela, classe.as_ptr(), std::ptr::null())
            };
            if janela.is_null() {
                break;
            }
            achou = true;
            if enviar(janela, &pacote) {
                return true;
            }
        }
        // Havia janela e nenhuma aceitou: estão fechando, ou travadas. Esperar
        // não muda isso.
        if achou || Instant::now() >= limite {
            return false;
        }
        // Nenhuma janela ainda: o Cururu vivo está subindo, e a dele nasce em
        // instantes.
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn enviar(janela: HWND, pacote: &[u8]) -> bool {
    let dados = COPYDATASTRUCT {
        dwData: MARCA,
        cbData: pacote.len() as u32,
        lpData: pacote.as_ptr() as *mut _,
    };
    let mut resposta: usize = 0;
    // SEGURANÇA: `dados` e o pacote vivem até o fim da chamada, que é síncrona:
    // o Windows copia os bytes para o outro processo antes de voltar.
    let enviado = unsafe {
        // O Windows só deixa uma janela vir para a frente se quem está na frente
        // ceder a vez — e quem está é este processo, recém-aberto pelo clique.
        let mut processo = 0u32;
        GetWindowThreadProcessId(janela, &mut processo);
        AllowSetForegroundWindow(processo);

        SendMessageTimeoutW(
            janela,
            WM_COPYDATA,
            0,
            &dados as *const COPYDATASTRUCT as LPARAM,
            SMTO_ABORTIFHUNG,
            ESPERA_PELA_RESPOSTA_MS,
            &mut resposta,
        )
    };
    enviado != 0 && resposta == ACEITO as usize
}

/// O pacote do `WM_COPYDATA`: os caminhos em UTF-8, um por linha — quebra de
/// linha não existe em caminho do Windows.
fn empacotar(arquivos: &[String]) -> Vec<u8> {
    arquivos.join("\n").into_bytes()
}

fn desempacotar(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .filter(|caminho| !caminho.is_empty())
        .map(str::to_string)
        .collect()
}

/// Cria a janela que recebe os arquivos mandados pelos outros processos.
///
/// Chamar na thread principal, depois de o laço de eventos existir: é ele que
/// entrega as mensagens a ela. Falhar aqui não impede nada — o duplo clique só
/// passa a abrir outra janela.
pub fn escutar(aplicativo: AppHandle) {
    let _ = APLICATIVO.set(aplicativo);
    if let Err(codigo) = criar_janela(CLASSE) {
        registro::erro(
            "instancia",
            format!(
                "janela de recepção não criada (erro {codigo}): cada arquivo aberto pelo Windows abrirá outra janela"
            ),
        );
    }
}

fn criar_janela(classe: &str) -> Result<(), u32> {
    let classe = texto_largo(classe);
    // SEGURANÇA: a estrutura zerada é o registro vazio que a API aceita; o nome
    // da classe termina em zero, e o Windows guarda a própria cópia dele.
    unsafe {
        let modulo = GetModuleHandleW(std::ptr::null());
        let mut definicao: WNDCLASSW = std::mem::zeroed();
        definicao.lpfnWndProc = Some(procedimento);
        definicao.hInstance = modulo;
        definicao.lpszClassName = classe.as_ptr();
        if RegisterClassW(&definicao) == 0 {
            return Err(GetLastError());
        }
        // `HWND_MESSAGE` a faz janela só de mensagens: não aparece, não tem
        // lugar na barra de tarefas e não recebe as transmissões gerais.
        let janela = CreateWindowExW(
            0,
            classe.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            modulo,
            std::ptr::null(),
        );
        if janela.is_null() {
            return Err(GetLastError());
        }
    }
    Ok(())
}

/// O que o Windows chama a cada mensagem da janela de recepção.
unsafe extern "system" fn procedimento(
    janela: HWND,
    mensagem: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if mensagem == WM_COPYDATA && lparam != 0 {
        // SEGURANÇA: no `WM_COPYDATA` o `lparam` aponta para um
        // `COPYDATASTRUCT` válido durante a chamada, e os bytes dele também.
        let dados = unsafe { &*(lparam as *const COPYDATASTRUCT) };
        if dados.dwData == MARCA {
            let bytes: &[u8] = if dados.lpData.is_null() {
                &[]
            } else {
                unsafe {
                    std::slice::from_raw_parts(dados.lpData as *const u8, dados.cbData as usize)
                }
            };
            return receber(desempacotar(bytes));
        }
    }
    // SEGURANÇA: o resto segue o caminho padrão, com os mesmos argumentos.
    unsafe { DefWindowProcW(janela, mensagem, wparam, lparam) }
}

/// Guarda o que chegou de outro processo e avisa a interface.
fn receber(arquivos: Vec<String>) -> LRESULT {
    if RECUSANDO.load(Ordering::SeqCst) || arquivos.is_empty() {
        return 0;
    }
    registro::informacao(
        "instancia",
        format!("recebido de outro processo: {}", arquivos.join("; ")),
    );
    guardar(arquivos);
    if let Some(aplicativo) = APLICATIVO.get() {
        let aplicativo = aplicativo.clone();
        // Fora desta chamada: ela roda dentro do laço de eventos, e mexer na
        // janela daqui seria entrar nele de novo. De outra thread o pedido vai
        // para a fila do laço, como qualquer outro.
        std::thread::spawn(move || avisar(&aplicativo));
    }
    ACEITO
}

/// Traz a janela para a frente — minimizada, ela não teria como — e avisa a
/// interface de que há arquivo na fila.
fn avisar(aplicativo: &AppHandle) {
    if let Some(janela) = aplicativo.get_webview_window("main") {
        if janela.is_minimized().unwrap_or(false) {
            let _ = janela.unminimize();
        }
        let _ = janela.set_focus();
    }
    let _ = aplicativo.emit("arquivos-recebidos", ());
}

/// Põe na fila os arquivos da linha de comando deste processo.
pub fn aceitar_pedido(pedido: Pedido, havia_outro: bool) {
    for ignorado in &pedido.ignorados {
        registro::aviso(
            "instancia",
            format!("argumento ignorado, não é arquivo: {ignorado}"),
        );
    }
    if pedido.arquivos.is_empty() {
        return;
    }
    if havia_outro {
        registro::aviso(
            "instancia",
            "o Cururu aberto não recebeu os arquivos; abrindo nesta janela",
        );
    }
    registro::informacao(
        "instancia",
        format!("linha de comando: {}", pedido.arquivos.join("; ")),
    );
    guardar(pedido.arquivos);
}

fn guardar(arquivos: Vec<String>) {
    let mut fila = FILA.lock().unwrap_or_else(|erro| erro.into_inner());
    for arquivo in arquivos {
        // Dois duplos cliques no mesmo arquivo antes de a interface buscar a
        // fila são um arquivo só.
        if !fila.contains(&arquivo) {
            fila.push(arquivo);
        }
    }
}

/// Leva a fila inteira: quem pergunta abre, e o mesmo arquivo não abre duas
/// vezes por ter sido perguntado duas vezes.
pub fn retirar() -> Vec<String> {
    std::mem::take(&mut *FILA.lock().unwrap_or_else(|erro| erro.into_inner()))
}

/// Daqui em diante, recusa arquivo novo (ver [`RECUSANDO`]).
pub fn recusar_arquivos() {
    RECUSANDO.store(true, Ordering::SeqCst);
}

fn texto_largo(texto: &str) -> Vec<u16> {
    std::ffi::OsStr::new(texto)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG};

    /// Nome que nenhum outro teste — nem um Cururu aberto na máquina — usa.
    fn nome_unico(base: &str) -> String {
        let instante = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("br.dev.cururu.teste.{base}.{}.{instante}", std::process::id())
    }

    #[test]
    fn separa_arquivo_do_que_nao_e() {
        let pasta = std::env::temp_dir();
        let nome = format!("{}.sql", nome_unico("separar"));
        let arquivo = pasta.join(&nome);
        std::fs::write(&arquivo, "select 1 from dual;").unwrap();

        let pedido = separar(
            [
                OsString::from(&nome),
                OsString::from("nao-existe.sql"),
                pasta.clone().into_os_string(),
            ]
            .into_iter(),
            &pasta,
        );
        let _ = std::fs::remove_file(&arquivo);

        // O relativo volta com o caminho inteiro: é ele que viaja até a outra
        // janela, que pode estar noutra pasta.
        assert_eq!(pedido.arquivos, vec![arquivo.to_str().unwrap().to_string()]);
        assert_eq!(pedido.ignorados.len(), 2, "{:?}", pedido.ignorados);
    }

    #[test]
    fn o_pacote_vai_e_volta_com_acento_e_espaco() {
        let arquivos = vec![
            r"C:\Relatórios\ação.sql".to_string(),
            r"D:\pasta com espaço\b.sql".to_string(),
        ];
        assert_eq!(desempacotar(&empacotar(&arquivos)), arquivos);
    }

    #[test]
    fn a_presenca_aparece_para_o_segundo() {
        let nome = format!(r"Local\{}", nome_unico("presenca"));
        assert!(!marcar_presenca_em(&nome), "o primeiro está sozinho");
        assert!(marcar_presenca_em(&nome), "o segundo encontra o primeiro");
    }

    #[test]
    fn sem_janela_nenhuma_desiste_no_prazo() {
        let inicio = Instant::now();
        let arquivos = ["a.sql".to_string()];
        assert!(!entregar_em(&nome_unico("ninguem"), &arquivos, Duration::from_millis(300)));
        assert!(inicio.elapsed() >= Duration::from_millis(300));
    }

    /// O caminho inteiro do Windows: a janela nasce noutra thread, com o próprio
    /// laço de mensagens, e o arquivo chega por `WM_COPYDATA` — a mesma
    /// conversa que há entre dois processos. Recusando, o remetente desiste na
    /// hora, sem esperar o prazo.
    #[test]
    fn a_janela_recebe_e_depois_recusa() {
        let classe = nome_unico("janela");
        let classe_da_thread = classe.clone();
        std::thread::spawn(move || {
            criar_janela(&classe_da_thread).expect("janela de teste");
            let mut mensagem = MSG::default();
            // SEGURANÇA: laço de mensagens padrão, com a estrutura desta thread.
            unsafe {
                while GetMessageW(&mut mensagem, std::ptr::null_mut(), 0, 0) > 0 {
                    DispatchMessageW(&mensagem);
                }
            }
        });

        let arquivos = vec![r"C:\consultas\relatório mensal.sql".to_string()];
        assert!(entregar_em(&classe, &arquivos, Duration::from_secs(5)));
        assert_eq!(retirar(), arquivos);

        recusar_arquivos();
        let inicio = Instant::now();
        assert!(!entregar_em(&classe, &arquivos, Duration::from_secs(5)));
        assert!(inicio.elapsed() < Duration::from_secs(2), "a recusa não espera o prazo");
        RECUSANDO.store(false, Ordering::SeqCst);
        assert!(retirar().is_empty());
    }
}
