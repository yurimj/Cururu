//! Abertura de endereços no navegador padrão do Windows.
//!
//! O único link do aplicativo é o do autor, na tela Sobre. Ele não pode abrir
//! dentro da janela do Cururu: a webview não tem barra de endereço nem botão de
//! voltar, então a tela ficaria presa no site. Vai para o navegador do sistema.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::erro::{Erro, Resultado};

/// Valor a partir do qual `ShellExecuteW` considera a chamada bem-sucedida.
/// Abaixo disso o retorno é um código de erro herdado do Windows de 16 bits.
const SUCESSO_MINIMO: isize = 32;

/// Abre o endereço no navegador padrão.
///
/// Aceita **apenas `https://`**. Este caminho fica exposto à interface, e
/// `ShellExecuteW` abriria com a mesma disposição um executável, um caminho de
/// rede ou um `file://` — a restrição é o que impede que abrir um link vire
/// executar um programa.
pub fn abrir(endereco: &str) -> Resultado<()> {
    if !endereco.starts_with("https://") {
        return Err(Erro::novo("só é possível abrir endereços https"));
    }

    let operacao = para_utf16("open");
    let alvo = para_utf16(endereco);

    // SEGURANÇA: os dois vetores vivem até o fim da chamada e terminam em zero,
    // como a API exige. A janela-pai nula abre o navegador sem dono.
    let retorno = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operacao.as_ptr(),
            alvo.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if (retorno as isize) <= SUCESSO_MINIMO {
        return Err(Erro::novo(format!(
            "não foi possível abrir o navegador (código {})",
            retorno as isize
        )));
    }
    Ok(())
}

/// Texto no formato que as funções W do Windows esperam: UTF-16 terminado em zero.
fn para_utf16(texto: &str) -> Vec<u16> {
    OsStr::new(texto).encode_wide().chain(Some(0)).collect()
}
