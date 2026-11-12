//! Envio de alertas ao Power Automate pelo WinHTTP, com o proxy e os
//! certificados do Windows. A URL assinada nunca é incluída em erros ou logs.

use std::{ffi::c_void, ptr::null};

use serde_json::json;
use tauri::Url;
use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts,
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

use crate::erro::{Erro, Resultado};

pub fn separar_webhook(endereco: &str) -> Resultado<Url> {
    let invalido = || Erro::novo("alerta webhook invalido");
    let url = Url::parse(endereco).map_err(|_| invalido())?;
    let servidor = url.host_str().ok_or_else(invalido)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || !(servidor.ends_with(".api.powerplatform.com") || servidor.ends_with(".logic.azure.com"))
        || !url
            .query_pairs()
            .any(|(chave, valor)| chave == "sig" && !valor.is_empty())
    {
        return Err(invalido());
    }
    Ok(url)
}

pub fn notificar(endereco: &str, mensagem: &str) -> Resultado<()> {
    let url = separar_webhook(endereco)?;
    let corpo = serde_json::to_vec(&json!({
        "type": "message",
        "attachments": [{
            "contentType": "application/vnd.microsoft.card.adaptive",
            "contentUrl": null,
            "content": {
                "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                "type": "AdaptiveCard",
                "version": "1.2",
                "body": [{ "type": "TextBlock", "text": mensagem, "wrap": true }]
            }
        }]
    }))?;
    postar(&url, &corpo)
}

struct Handle(*mut c_void);

impl Handle {
    fn novo(valor: *mut c_void) -> Resultado<Self> {
        verificar(!valor.is_null())?;
        Ok(Self(valor))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

fn postar(url: &Url, corpo: &[u8]) -> Resultado<()> {
    let agente = utf16("Cururu");
    let servidor = utf16(url.host_str().unwrap_or_default());
    let recurso = utf16(&format!(
        "{}?{}",
        url.path(),
        url.query().unwrap_or_default()
    ));
    let verbo = utf16("POST");
    let cabecalhos = utf16("Content-Type: application/json\r\n");
    // Os buffers e os handles vivem até depois da última chamada que os usa.
    unsafe {
        let sessao = Handle::novo(WinHttpOpen(
            agente.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            null(),
            null(),
            0,
        ))?;
        let conexao = Handle::novo(WinHttpConnect(
            sessao.0,
            servidor.as_ptr(),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ))?;
        let pedido = Handle::novo(WinHttpOpenRequest(
            conexao.0,
            verbo.as_ptr(),
            recurso.as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        ))?;
        let politica = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        verificar(
            WinHttpSetOption(
                pedido.0,
                WINHTTP_OPTION_REDIRECT_POLICY,
                (&politica as *const u32).cast(),
                size_of::<u32>() as u32,
            ) != 0,
        )?;
        verificar(WinHttpSetTimeouts(pedido.0, 5_000, 5_000, 10_000, 15_000) != 0)?;
        verificar(
            WinHttpSendRequest(
                pedido.0,
                cabecalhos.as_ptr(),
                u32::MAX,
                corpo.as_ptr().cast(),
                corpo.len() as u32,
                corpo.len() as u32,
                0,
            ) != 0,
        )?;
        verificar(WinHttpReceiveResponse(pedido.0, std::ptr::null_mut()) != 0)?;
        let mut codigo = 0u32;
        let mut tamanho = size_of::<u32>() as u32;
        verificar(
            WinHttpQueryHeaders(
                pedido.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut codigo as *mut u32).cast(),
                &mut tamanho,
                std::ptr::null_mut(),
            ) != 0,
        )?;
        if !(200..300).contains(&codigo) {
            return Err(Erro::novo(format!(
                "o webhook do Teams respondeu HTTP {codigo}"
            )));
        }
    }
    Ok(())
}

fn verificar(ok: bool) -> Resultado<()> {
    if ok {
        Ok(())
    } else {
        let codigo = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default();
        Err(Erro::novo(format!(
            "falha de rede ao chamar o Teams (WinHTTP {codigo})"
        )))
    }
}

fn utf16(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain([0]).collect()
}
