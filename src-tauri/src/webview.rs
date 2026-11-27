//! Versão do WebView2 em que a janela nasce.
//!
//! Todo Cururu aberto guarda o que a página guarda — preferências, atalhos,
//! botões rápidos, tudo no `localStorage` — na mesma pasta de dados do
//! WebView2, e uma pasta dessas serve a **um navegador por vez**: o segundo
//! Cururu não sobe navegador próprio, ele entra no do primeiro.
//!
//! O WebView2 se atualiza sozinho, pelo Windows, e o Cururu que já estava
//! aberto segue na versão com que abriu. O que abrisse depois carregaria a
//! versão nova, que não entra no navegador da velha — e a espera por ele não
//! acaba: o processo fica vivo, sem janela, sem erro e sem nada no log. Foi o
//! que se viu em 15/09/2026, com um Cururu aberto no 152.0.4191.66 desde a
//! véspera e o 153.0.4234.32 instalado de manhã: cada clique no atalho deixava
//! mais um processo invisível para trás.
//!
//! Então, havendo navegador aberto na pasta do Cururu numa versão que não é a
//! que este processo carregaria, este processo usa a dele. A versão velha
//! continua instalada enquanto houver quem a use, e o primeiro Cururu aberto
//! sem nenhum outro na tela já sobe na nova.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowThreadProcessId, HWND_MESSAGE,
};

use crate::registro;

/// Onde o carregador do WebView2 procura a pasta da versão a usar. É o jeito
/// documentado de escolher o runtime, e o mesmo que o Tauri usa para a versão
/// fixa.
const VARIAVEL: &str = "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER";

/// A janela só de mensagens que o navegador do WebView2 — um Chromium — cria
/// com a pasta de dados por título. É por ela que o próprio Chromium descobre
/// se a pasta já tem dono, e é por ela que o Cururu descobre quem serve a sua:
/// vale mesmo quando o Cururu que abriu aquele navegador já fechou e deixou
/// outro usando-o.
const CLASSE_DO_NAVEGADOR: &str = "Chrome_MessageWindow";

/// Ligado quando este processo escolheu a versão pela [`VARIAVEL`].
static FIXADA: AtomicBool = AtomicBool::new(false);

/// Ligado na primeira página carregada; as seguintes são recargas.
static CARREGADA: AtomicBool = AtomicBool::new(false);

/// Escolhe a versão do WebView2 antes de a janela nascer.
///
/// `identificador` é o do `tauri.conf.json`: o Tauri guarda os dados da página
/// em `%LOCALAPPDATA%\<identificador>`, e o WebView2 acrescenta `EBWebView`.
pub fn escolher_versao(identificador: &str) {
    preparar_portatil();
    let padrao = tauri::webview_version().ok();
    let instalada = padrao.as_deref().unwrap_or("não encontrado");

    // Quem abriu o Cururu já escolheu a pasta — um runtime fixo, um teste de
    // versão. A escolha é dele.
    if let Some(pasta) = std::env::var_os(VARIAVEL).filter(|_| !FIXADA.load(Ordering::SeqCst)) {
        registro::informacao(
            "webview",
            format!(
                "WebView2 {instalada}, de {VARIAVEL} = {}",
                Path::new(&pasta).display()
            ),
        );
        return;
    }

    let aberto = pasta_de_dados(identificador).and_then(|pasta| navegador_da_pasta(&pasta));
    let Some(executavel) = aberto else {
        registro::informacao("webview", format!("WebView2 {instalada}"));
        return;
    };
    match pasta_a_fixar(padrao.as_deref(), &executavel) {
        None => registro::informacao(
            "webview",
            format!("WebView2 {instalada}, o mesmo do Cururu já aberto"),
        ),
        Some(pasta) => {
            std::env::set_var(VARIAVEL, &pasta);
            FIXADA.store(true, Ordering::SeqCst);
            let aberta = versao_da_pasta(&pasta).unwrap_or("?");
            registro::aviso(
                "webview",
                format!(
                    "WebView2 {instalada} instalado, mas a pasta de dados já é do {aberta} de outro Cururu aberto: esta janela usa o {aberta}, de {}",
                    pasta.display()
                ),
            );
        }
    }
}

/// Usa o runtime distribuído ao lado do executável, sem instalar nada.
/// Uma escolha explícita no ambiente continua tendo prioridade.
fn preparar_portatil() {
    if std::env::var_os(VARIAVEL).is_some() {
        return;
    }
    let Some(pasta) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|pai| pai.join("webview2")))
        .filter(|pasta| pasta.join("msedgewebview2.exe").is_file())
    else {
        return;
    };

    // O Windows 10 exige leitura pelos contêineres do renderizador desde o
    // runtime 120. A permissão precisa ser aplicada no destino, depois da cópia
    // ou extração do ZIP; ACLs da máquina de build não viajam no arquivo.
    use std::os::windows::process::CommandExt;
    match std::process::Command::new("icacls.exe")
        .arg(&pasta)
        .args([
            "/grant",
            "*S-1-15-2-2:(OI)(CI)(RX)",
            "*S-1-15-2-1:(OI)(CI)(RX)",
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
    {
        Ok(saida) if saida.status.success() => {}
        _ => registro::aviso(
            "webview",
            "não foi possível preparar as permissões do runtime portátil",
        ),
    }
    std::env::set_var(VARIAVEL, &pasta);
    FIXADA.store(true, Ordering::SeqCst);
    registro::informacao("webview", format!("runtime portátil: {}", pasta.display()));
}

/// A primeira página terminou de carregar: a janela veio de fato.
///
/// Só aqui é certo que o WebView2 subiu. O `Ready` do Tauri chega antes de a
/// página existir — chegou até no processo que ficou esperando para sempre —,
/// e uma linha "janela criada" escrita ali mentiria justamente no caso em que
/// o log precisa dizer a verdade.
pub fn pagina_carregada(inicio: Instant) {
    if CARREGADA.swap(true, Ordering::SeqCst) {
        return;
    }
    liberar_versao();
    registro::informacao(
        "aplicativo",
        format!("página carregada em {} ms", inicio.elapsed().as_millis()),
    );
}

/// Tira a [`VARIAVEL`] do ambiente depois que a página carregou.
///
/// O carregador já a leu, e quem este processo abrir daqui em diante não pode
/// herdá-la: o instalador de uma atualização reabre o Cururu ao terminar, e a
/// versão velha sai do disco assim que ninguém mais a usa.
fn liberar_versao() {
    if FIXADA.swap(false, Ordering::SeqCst) {
        std::env::remove_var(VARIAVEL);
    }
}

fn pasta_de_dados(identificador: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join(identificador).join("EBWebView"))
}

/// O executável do navegador que serve a pasta de dados, se há um aberto.
fn navegador_da_pasta(pasta: &Path) -> Option<PathBuf> {
    let classe = texto_largo(OsStr::new(CLASSE_DO_NAVEGADOR));
    let titulo = texto_largo(pasta.as_os_str());
    let mut processo = 0u32;
    // SEGURANÇA: classe e título terminam em zero; a janela, quando vem, é a
    // da própria busca, e o id do processo é escrito na variável local.
    unsafe {
        let janela = FindWindowExW(
            HWND_MESSAGE,
            std::ptr::null_mut(),
            classe.as_ptr(),
            titulo.as_ptr(),
        );
        if janela.is_null() {
            return None;
        }
        GetWindowThreadProcessId(janela, &mut processo);
    }
    if processo == 0 {
        return None;
    }
    caminho_do_processo(processo)
}

fn caminho_do_processo(processo: u32) -> Option<PathBuf> {
    // 32 767 caracteres é o teto de um caminho no Windows, prefixo longo
    // incluído.
    let mut caminho = vec![0u16; 32_768];
    let mut tamanho = caminho.len() as u32;
    // SEGURANÇA: a API escreve no máximo `tamanho` caracteres no buffer e
    // devolve em `tamanho` quantos escreveu; o handle é fechado antes de sair.
    let lido = unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, processo);
        if handle.is_null() {
            return None;
        }
        let lido = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            caminho.as_mut_ptr(),
            &mut tamanho,
        );
        CloseHandle(handle);
        lido
    };
    (lido != 0).then(|| PathBuf::from(OsString::from_wide(&caminho[..tamanho as usize])))
}

/// A pasta a fixar: a do navegador aberto, quando ele é de outra versão que a
/// que este processo carregaria.
///
/// Cada versão do runtime mora numa pasta com o número dela
/// (`...\EdgeWebView\Application\152.0.4191.66\msedgewebview2.exe`), e o número
/// que o carregador informa pode trazer o canal atrás (`… beta`). Sem saber qual
/// este processo carregaria, fica a do navegador aberto: é a única que com
/// certeza entra nele.
fn pasta_a_fixar(padrao: Option<&str>, executavel: &Path) -> Option<PathBuf> {
    let pasta = executavel.parent()?;
    let aberta = versao_da_pasta(pasta)?;
    let padrao = padrao.and_then(|texto| texto.split_whitespace().next());
    (padrao != Some(aberta)).then(|| pasta.to_path_buf())
}

fn versao_da_pasta(pasta: &Path) -> Option<&str> {
    pasta.file_name()?.to_str()
}

fn texto_largo(texto: &OsStr) -> Vec<u16> {
    texto.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WNDCLASSW,
    };

    const RUNTIME: &str = r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application";

    fn executavel(versao: &str) -> PathBuf {
        Path::new(RUNTIME).join(versao).join("msedgewebview2.exe")
    }

    #[test]
    fn a_mesma_versao_nao_fixa_nada() {
        assert_eq!(
            pasta_a_fixar(Some("152.0.4191.66"), &executavel("152.0.4191.66")),
            None
        );
    }

    /// O caso de 15/09/2026: o 153 instalado de manhã, o Cururu da véspera no
    /// 152.
    #[test]
    fn outra_versao_fixa_a_pasta_do_navegador_aberto() {
        assert_eq!(
            pasta_a_fixar(Some("153.0.4234.32"), &executavel("152.0.4191.66")),
            Some(Path::new(RUNTIME).join("152.0.4191.66"))
        );
    }

    #[test]
    fn o_canal_depois_do_numero_nao_conta() {
        assert_eq!(
            pasta_a_fixar(Some("152.0.4191.66 beta"), &executavel("152.0.4191.66")),
            None
        );
    }

    #[test]
    fn sem_saber_a_instalada_fica_a_do_navegador_aberto() {
        assert_eq!(
            pasta_a_fixar(None, &executavel("152.0.4191.66")),
            Some(Path::new(RUNTIME).join("152.0.4191.66"))
        );
    }

    /// O caminho do Windows inteiro, com uma janela igual à do Chromium criada
    /// aqui mesmo: a busca acha o processo dono pelo título, que é a pasta de
    /// dados, e devolve o executável dele — o deste teste.
    #[test]
    fn acha_o_navegador_pela_janela_da_pasta() {
        let pasta = std::env::temp_dir()
            .join(format!("cururu-teste-{}", std::process::id()))
            .join("EBWebView");
        let classe = texto_largo(OsStr::new(CLASSE_DO_NAVEGADOR));
        let titulo = texto_largo(pasta.as_os_str());
        // SEGURANÇA: registro e criação com textos terminados em zero; a
        // janela é destruída pela mesma thread que a criou.
        let janela = unsafe {
            let modulo = GetModuleHandleW(std::ptr::null());
            let mut definicao: WNDCLASSW = std::mem::zeroed();
            definicao.lpfnWndProc = Some(DefWindowProcW);
            definicao.hInstance = modulo;
            definicao.lpszClassName = classe.as_ptr();
            RegisterClassW(&definicao);
            CreateWindowExW(
                0,
                classe.as_ptr(),
                titulo.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                modulo,
                std::ptr::null(),
            )
        };
        assert!(!janela.is_null(), "janela de teste");

        let achado = navegador_da_pasta(&pasta);
        let outra = navegador_da_pasta(&pasta.join("outra"));
        // SEGURANÇA: a janela é desta thread.
        unsafe { DestroyWindow(janela) };

        let proprio = std::env::current_exe().unwrap();
        assert_eq!(
            achado.map(|caminho| caminho.to_string_lossy().to_lowercase()),
            Some(proprio.to_string_lossy().to_lowercase())
        );
        assert_eq!(outra, None);
    }
}
