//! O idioma escolhido no instalador (requisito 31).
//!
//! O instalador NSIS mostra a lista de idiomas já pré-selecionada com o da
//! máquina, e grava a escolha em `HKCU\Software\Cururu\Idioma` (ver
//! `instalador.nsh`). Este módulo é o outro lado dessa conversa: o Cururu lê a
//! chave **na primeira execução**, antes de montar a tela, para abrir no idioma
//! que a pessoa escolheu enquanto instalava.
//!
//! Só na primeira: a partir do momento em que existe escolha guardada na
//! máquina — a do menu **Language** —, é ela que manda. O instalador diz por
//! onde começar, e não o que valer para sempre.
//!
//! É lido pelo `RegGetValueW` do próprio Windows, sem dependência nova: o
//! projeto já usa `windows-sys` para o DPAPI e o WinHTTP.

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use windows_sys::Win32::System::Registry::{
    RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ,
};

/// Onde o instalador deixa a escolha.
const CAMINHO: &str = r"Software\Cururu";
const VALOR: &str = "Idioma";

/// Os três códigos que a interface conhece; qualquer outra coisa é ignorada.
const ACEITOS: [&str; 3] = ["pt-BR", "es", "en"];

/// O idioma gravado pelo instalador, se houver um utilizável.
///
/// Devolve `None` quando a chave não existe — instalação por cópia da pasta
/// portátil, ou versão anterior a esta —, quando a leitura falha e quando o
/// valor não é um dos três idiomas. Em todos esses casos a tela decide sozinha,
/// pelo idioma do navegador embutido, como sempre decidiu.
pub fn escolhido() -> Option<String> {
    let caminho: Vec<u16> = texto_largo(CAMINHO);
    let valor: Vec<u16> = texto_largo(VALOR);
    // Duas chamadas: a primeira pergunta o tamanho, a segunda lê. É o contrato
    // do `RegGetValueW`, e pedir o tamanho antes evita chutar um buffer.
    let mut tamanho: u32 = 0;
    let situacao = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            caminho.as_ptr(),
            valor.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut tamanho,
        )
    };
    if situacao != 0 || tamanho == 0 {
        return None;
    }

    // `tamanho` vem em bytes e inclui o terminador zero.
    let mut dados = vec![0u16; (tamanho as usize).div_ceil(2)];
    let situacao = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            caminho.as_ptr(),
            valor.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            dados.as_mut_ptr().cast(),
            &mut tamanho,
        )
    };
    if situacao != 0 {
        return None;
    }

    let fim = dados.iter().position(|&c| c == 0).unwrap_or(dados.len());
    let lido = OsString::from_wide(&dados[..fim]).to_string_lossy().to_string();
    ACEITOS
        .iter()
        .find(|aceito| aceito.eq_ignore_ascii_case(lido.trim()))
        .map(|aceito| (*aceito).to_string())
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

    #[test]
    fn le_sem_estourar_mesmo_sem_a_chave() {
        // A máquina de teste não tem a chave do instalador: o contrato é
        // devolver `None`, e não entrar em pânico.
        let _ = escolhido();
    }

    #[test]
    fn o_texto_largo_termina_em_zero() {
        let largo = texto_largo("ab");
        assert_eq!(largo, vec![b'a' as u16, b'b' as u16, 0]);
    }
}
