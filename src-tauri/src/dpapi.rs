//! Proteção de senhas com o DPAPI do Windows (requisito 24).
//!
//! O DPAPI cifra com uma chave derivada da conta do usuário do Windows: o
//! arquivo de conexões copiado para outra máquina, ou lido por outro usuário,
//! não devolve a senha. Nenhuma senha é gravada em texto puro.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

use crate::erro::{Erro, Resultado};

/// Cifra um texto e devolve o resultado em hexadecimal, pronto para o JSON.
pub fn proteger(texto: &str) -> Resultado<String> {
    let bytes = texto.as_bytes();
    let entrada = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut saida = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    // SEGURANÇA: `entrada` aponta para um buffer vivo durante toda a chamada;
    // `saida` é alocado pelo Windows e liberado logo abaixo com LocalFree.
    let sucesso = unsafe {
        CryptProtectData(
            &entrada,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut saida,
        )
    };
    if sucesso == 0 {
        return Err(Erro::novo("não foi possível proteger a senha (DPAPI)"));
    }

    let cifrado = unsafe { std::slice::from_raw_parts(saida.pbData, saida.cbData as usize) };
    let hexadecimal = para_hexadecimal(cifrado);
    unsafe { LocalFree(saida.pbData as *mut c_void) };
    Ok(hexadecimal)
}

/// Desfaz o [`proteger`].
pub fn desproteger(hexadecimal: &str) -> Resultado<String> {
    let cifrado = de_hexadecimal(hexadecimal)?;
    let entrada = CRYPT_INTEGER_BLOB {
        cbData: cifrado.len() as u32,
        pbData: cifrado.as_ptr() as *mut u8,
    };
    let mut saida = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    // SEGURANÇA: mesmas garantias de `proteger`.
    let sucesso = unsafe {
        CryptUnprotectData(
            &entrada,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut saida,
        )
    };
    if sucesso == 0 {
        return Err(Erro::novo(
            "não foi possível ler a senha salva. Ela foi protegida por outro \
             usuário do Windows ou em outra máquina; informe a senha novamente.",
        ));
    }

    let bytes = unsafe { std::slice::from_raw_parts(saida.pbData, saida.cbData as usize) };
    let texto = String::from_utf8_lossy(bytes).into_owned();
    unsafe { LocalFree(saida.pbData as *mut c_void) };
    Ok(texto)
}

fn para_hexadecimal(bytes: &[u8]) -> String {
    let mut texto = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(texto, "{byte:02x}");
    }
    texto
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn protege_e_recupera_a_senha() {
        let original = "senha com acento: manutenção";
        let cifrada = proteger(original).expect("proteger deveria funcionar");

        // O texto cifrado não pode conter a senha original.
        assert!(!cifrada.contains("manuten"));
        assert_eq!(desproteger(&cifrada).expect("desproteger"), original);
    }

    #[test]
    fn recusa_conteudo_que_nao_e_hexadecimal() {
        assert!(desproteger("zz").is_err());
        assert!(desproteger("abc").is_err());
    }
}

fn de_hexadecimal(texto: &str) -> Resultado<Vec<u8>> {
    if texto.len() % 2 != 0 {
        return Err(Erro::novo("senha salva com formato inválido"));
    }
    (0..texto.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&texto[i..i + 2], 16)
                .map_err(|_| Erro::novo("senha salva com formato inválido"))
        })
        .collect()
}
