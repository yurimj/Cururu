//! Verificação de atualização e instalação da versão nova.
//!
//! O Cururu é distribuído por um repositório **público só de releases**
//! (`yurimj/Cururu-releases`), separado do repositório do código, que é
//! privado. A separação existe por um motivo concreto: release de repositório
//! privado também é privado, e a alternativa — embutir um token no executável
//! distribuído — daria a quem recebesse o aplicativo acesso de leitura ao
//! código-fonte. Assim a consulta é anônima e não há segredo algum no binário.
//!
//! O acesso à rede usa o **WinHTTP do próprio Windows**, e não uma biblioteca
//! HTTP em Rust. Dois motivos, ambos de máquina corporativa, que é onde o
//! Cururu roda: o WinHTTP respeita a configuração de proxy do Windows,
//! inclusive PAC e detecção automática, e valida o certificado pela loja de
//! certificados do sistema — então uma raiz interna instalada pela empresa é
//! aceita sem configuração nenhuma. Nenhuma dependência nova entrou no projeto
//! por causa disto.

use std::ffi::{c_void, OsStr};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_CONTENT_LENGTH, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_FLAG_NUMBER64,
    WINHTTP_QUERY_STATUS_CODE,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::erro::{Erro, Resultado};
use crate::registro;

/// Release mais recente do repositório público de distribuição.
const ENDERECO_DA_API: &str = "https://api.github.com/repos/yurimj/Cururu-releases/releases/latest";

/// Domínios de onde o instalador pode ser baixado.
///
/// O endereço do arquivo vem de fora — é um campo da resposta do GitHub —, e
/// daqui sai um executável que será rodado na máquina do usuário. A lista
/// impede que uma resposta adulterada aponte o download para qualquer servidor.
const SERVIDORES_ACEITOS: [&str; 2] = ["github.com", "githubusercontent.com"];

/// Tempos limite em milissegundos: resolução de nome, conexão, envio e
/// recepção. O de recepção é largo porque o instalador passa de 100 MB.
const LIMITES: (i32, i32, i32, i32) = (15_000, 15_000, 30_000, 120_000);

/// Valor a partir do qual `ShellExecuteW` considera a chamada bem-sucedida.
const SUCESSO_MINIMO: isize = 32;

/// Caminho do instalador já baixado nesta execução.
///
/// Guardado aqui, e não devolvido à interface e recebido de volta, para que o
/// comando que **executa** um programa não aceite um caminho vindo da tela.
static INSTALADOR_BAIXADO: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Versão publicada, quando é mais nova que a em execução.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Atualizacao {
    /// Número da versão publicada, sem o `v` da etiqueta: `0.1.11`.
    pub versao: String,
    /// Versão em execução, para a tela poder mostrar "0.1.10 → 0.1.11".
    pub versao_atual: String,
    /// Data de publicação no formato brasileiro.
    pub data: String,
    /// Corpo do release em markdown — a lista do que mudou.
    pub notas: String,
    /// Endereço do instalador.
    pub instalador: String,
    /// Tamanho do instalador em bytes, para a barra de progresso.
    pub tamanho: u64,
    /// Página do release no GitHub.
    pub pagina: String,
}

/// Andamento do download, enviado à tela enquanto o arquivo desce.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Andamento {
    pub baixados: u64,
    /// Total anunciado pelo servidor, ou zero quando ele não informa.
    pub total: u64,
}

/// Consulta o repositório de releases e devolve a versão publicada quando ela
/// for mais nova que a em execução. `None` significa "já está atualizado".
pub fn verificar() -> Resultado<Option<Atualizacao>> {
    let mut corpo = Vec::new();
    obter(ENDERECO_DA_API, |pedaco, _| {
        corpo.extend_from_slice(pedaco);
        Ok(())
    })?;

    let release: serde_json::Value = serde_json::from_slice(&corpo)
        .map_err(|erro| Erro::novo(format!("resposta do GitHub ilegível: {erro}")))?;

    let etiqueta = texto(&release, "tag_name");
    let versao = etiqueta.trim_start_matches(['v', 'V']).trim().to_string();
    if versao.is_empty() {
        return Err(Erro::novo("o release publicado não tem número de versão"));
    }

    let atual = env!("CARGO_PKG_VERSION").to_string();
    if !mais_nova(&versao, &atual) {
        registro::informacao(
            "atualizacao",
            format!("versão publicada {versao}; em execução {atual}; nada a fazer"),
        );
        return Ok(None);
    }

    let Some((instalador, tamanho)) = instalador_do_release(&release) else {
        return Err(Erro::novo(format!(
            "a versão {versao} foi publicada sem instalador anexado"
        )));
    };

    registro::informacao(
        "atualizacao",
        format!("versão {versao} disponível (em execução {atual})"),
    );

    Ok(Some(Atualizacao {
        versao,
        versao_atual: atual,
        data: data_brasileira(&texto(&release, "published_at")),
        notas: texto(&release, "body"),
        instalador,
        tamanho,
        pagina: texto(&release, "html_url"),
    }))
}

/// Baixa o instalador para a pasta temporária, informando o andamento.
///
/// O `andamento` recebe (bytes baixados, total) e é chamado a cada bloco lido.
pub fn baixar(
    endereco: &str,
    mut andamento: impl FnMut(u64, u64),
) -> Resultado<PathBuf> {
    let destino = caminho_de_destino(endereco)?;
    if let Some(pasta) = destino.parent() {
        std::fs::create_dir_all(pasta)?;
    }

    // Grava com outro nome e só renomeia no fim: um download interrompido não
    // pode deixar para trás um arquivo com cara de instalador completo.
    let parcial = destino.with_extension("parcial");
    let mut arquivo = std::fs::File::create(&parcial)?;
    let mut baixados: u64 = 0;

    let resultado = obter(endereco, |pedaco, total| {
        arquivo.write_all(pedaco)?;
        baixados += pedaco.len() as u64;
        andamento(baixados, total);
        Ok(())
    });

    drop(arquivo);
    if let Err(erro) = resultado {
        let _ = std::fs::remove_file(&parcial);
        return Err(erro);
    }

    let _ = std::fs::remove_file(&destino);
    std::fs::rename(&parcial, &destino)?;

    guardar_instalador(destino.clone());
    registro::informacao(
        "atualizacao",
        format!("instalador baixado em {} ({baixados} bytes)", destino.display()),
    );
    Ok(destino)
}

/// Executa o instalador baixado nesta execução.
///
/// Não recebe caminho da interface de propósito: o que roda aqui é um
/// executável, e o único caminho aceito é o que este módulo mesmo gravou.
pub fn instalar() -> Resultado<()> {
    let caminho = INSTALADOR_BAIXADO
        .get()
        .and_then(|guarda| guarda.lock().ok().and_then(|conteudo| conteudo.clone()))
        .ok_or_else(|| Erro::novo("nenhum instalador foi baixado nesta execução"))?;

    if !caminho.is_file() {
        return Err(Erro::novo(format!(
            "o instalador não está mais em {}",
            caminho.display()
        )));
    }

    let operacao = para_utf16("open");
    let alvo = para_utf16(&caminho.display().to_string());

    // SEGURANÇA: os dois vetores vivem até o fim da chamada e terminam em zero,
    // como a API exige. A janela-pai nula abre o instalador sem dono, para ele
    // sobreviver ao fechamento do Cururu, que vem logo em seguida.
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
            "não foi possível iniciar o instalador (código {})",
            retorno as isize
        )));
    }

    registro::informacao("atualizacao", format!("instalador iniciado: {}", caminho.display()));
    Ok(())
}

// --- HTTP pelo WinHTTP -----------------------------------------------------

/// Handle do WinHTTP que se fecha sozinho ao sair de escopo.
///
/// Sem isto, cada `return` de erro no meio da requisição vazaria um handle.
struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        // SEGURANÇA: o handle é sempre não nulo aqui — `Handle` só é construído
        // depois da verificação.
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// GET em `endereco`, entregando o corpo em blocos.
///
/// O segundo parâmetro de `receber` é o tamanho total anunciado pelo servidor,
/// ou zero quando ele não informa — é o que alimenta a barra de progresso.
fn obter(endereco: &str, mut receber: impl FnMut(&[u8], u64) -> Resultado<()>) -> Resultado<()> {
    let (servidor, recurso) = separar_endereco(endereco)?;

    let agente = para_utf16(&format!("Cururu/{}", env!("CARGO_PKG_VERSION")));
    let nome_do_servidor = para_utf16(&servidor);
    let caminho = para_utf16(&recurso);
    let verbo = para_utf16("GET");
    let cabecalhos = para_utf16("Accept: application/vnd.github+json\r\n");

    // SEGURANÇA: todos os vetores UTF-16 acima vivem até o fim desta função,
    // que é depois da última chamada que os usa.
    let sessao = unsafe {
        WinHttpOpen(
            agente.as_ptr(),
            // Detecção automática de proxy: é o que faz o Cururu enxergar a
            // internet atrás do proxy da empresa sem ninguém configurar nada.
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if sessao.is_null() {
        return Err(falha_do_windows("não foi possível iniciar a conexão"));
    }
    let sessao = Handle(sessao);

    let conexao = unsafe {
        WinHttpConnect(
            sessao.0,
            nome_do_servidor.as_ptr(),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        )
    };
    if conexao.is_null() {
        return Err(falha_do_windows(&format!("não foi possível alcançar {servidor}")));
    }
    let conexao = Handle(conexao);

    let pedido = unsafe {
        WinHttpOpenRequest(
            conexao.0,
            verbo.as_ptr(),
            caminho.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    };
    if pedido.is_null() {
        return Err(falha_do_windows("não foi possível preparar a requisição"));
    }
    let pedido = Handle(pedido);

    let (resolucao, conectar, enviar, receber_limite) = LIMITES;
    unsafe { WinHttpSetTimeouts(pedido.0, resolucao, conectar, enviar, receber_limite) };

    // `u32::MAX` no tamanho diz ao WinHTTP que os cabeçalhos terminam em zero.
    let enviado = unsafe {
        WinHttpSendRequest(
            pedido.0,
            cabecalhos.as_ptr(),
            u32::MAX,
            std::ptr::null(),
            0,
            0,
            0,
        )
    };
    if enviado == 0 {
        return Err(falha_do_windows("não foi possível falar com o GitHub"));
    }

    if unsafe { WinHttpReceiveResponse(pedido.0, std::ptr::null_mut()) } == 0 {
        return Err(falha_do_windows("o GitHub não respondeu"));
    }

    let codigo = codigo_da_resposta(&pedido)?;
    if codigo == 404 {
        return Err(Erro::novo(
            "nenhuma versão publicada foi encontrada no repositório de releases",
        ));
    }
    if !(200..300).contains(&codigo) {
        return Err(Erro::novo(format!(
            "o GitHub respondeu {codigo} ao procurar a versão mais recente"
        )));
    }
    let total = tamanho_da_resposta(&pedido);

    let mut bloco = vec![0u8; 64 * 1024];
    loop {
        let mut lidos: u32 = 0;
        // SEGURANÇA: o buffer tem `bloco.len()` bytes e é esse o limite passado.
        let ok = unsafe {
            WinHttpReadData(
                pedido.0,
                bloco.as_mut_ptr() as *mut c_void,
                bloco.len() as u32,
                &mut lidos,
            )
        };
        if ok == 0 {
            return Err(falha_do_windows("a transferência foi interrompida"));
        }
        if lidos == 0 {
            break;
        }
        receber(&bloco[..lidos as usize], total)?;
    }

    Ok(())
}

fn codigo_da_resposta(pedido: &Handle) -> Resultado<u32> {
    let mut codigo: u32 = 0;
    let mut tamanho = std::mem::size_of::<u32>() as u32;
    // SEGURANÇA: o buffer é um u32 e é esse o tamanho informado.
    let ok = unsafe {
        WinHttpQueryHeaders(
            pedido.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut codigo as *mut u32 as *mut c_void,
            &mut tamanho,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(falha_do_windows("resposta do GitHub sem código de retorno"));
    }
    Ok(codigo)
}

/// Tamanho anunciado no `Content-Length`, ou zero quando o servidor não informa.
fn tamanho_da_resposta(pedido: &Handle) -> u64 {
    let mut total: u64 = 0;
    let mut tamanho = std::mem::size_of::<u64>() as u32;
    // SEGURANÇA: o buffer é um u64 e é esse o tamanho informado.
    let ok = unsafe {
        WinHttpQueryHeaders(
            pedido.0,
            WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER64,
            std::ptr::null(),
            &mut total as *mut u64 as *mut c_void,
            &mut tamanho,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        0
    } else {
        total
    }
}

// --- Apoio -----------------------------------------------------------------

/// Separa `https://servidor/caminho` nas duas partes que o WinHTTP pede.
///
/// Só aceita `https` e só os servidores do GitHub: o endereço do instalador vem
/// da resposta da rede, e dele sai um programa que vai ser executado.
fn separar_endereco(endereco: &str) -> Resultado<(String, String)> {
    let resto = endereco
        .strip_prefix("https://")
        .ok_or_else(|| Erro::novo("só é possível baixar de endereços https"))?;

    let corte = resto.find('/').unwrap_or(resto.len());
    let (servidor, caminho) = resto.split_at(corte);

    if servidor.is_empty() || servidor.contains(':') || servidor.contains('@') {
        return Err(Erro::novo(format!("endereço de download inválido: {endereco}")));
    }
    if !servidor_aceito(servidor) {
        return Err(Erro::novo(format!(
            "o download foi apontado para {servidor}, que não é um servidor do GitHub"
        )));
    }

    let caminho = if caminho.is_empty() { "/" } else { caminho };
    Ok((servidor.to_string(), caminho.to_string()))
}

/// Verdadeiro para o próprio domínio ou um subdomínio dele — `github.com` e
/// `objects.githubusercontent.com` passam, `github.com.outro.br` não.
fn servidor_aceito(servidor: &str) -> bool {
    let servidor = servidor.to_ascii_lowercase();
    SERVIDORES_ACEITOS.iter().any(|aceito| {
        servidor == *aceito || servidor.ends_with(&format!(".{aceito}"))
    })
}

/// Escolhe onde gravar o instalador: pasta temporária, subpasta do Cururu.
fn caminho_de_destino(endereco: &str) -> Resultado<PathBuf> {
    let nome = endereco
        .rsplit('/')
        .next()
        .map(|nome| nome.split(['?', '#']).next().unwrap_or(nome))
        .unwrap_or_default();

    // O nome vem da rede: só se aproveita dele o que for nome de arquivo mesmo.
    let seguro = nome
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect::<String>();
    let nome = if seguro.to_ascii_lowercase().ends_with(".exe") && seguro.len() > 4 {
        seguro
    } else {
        "cururu-instalador.exe".to_string()
    };

    Ok(std::env::temp_dir().join("Cururu").join(nome))
}

fn guardar_instalador(caminho: PathBuf) {
    let trava = INSTALADOR_BAIXADO.get_or_init(|| Mutex::new(None));
    let mut guarda = trava.lock().unwrap_or_else(|erro| erro.into_inner());
    *guarda = Some(caminho);
}

/// Instalador anexado ao release: o primeiro `.exe` da lista, com o tamanho.
fn instalador_do_release(release: &serde_json::Value) -> Option<(String, u64)> {
    release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|anexo| {
            texto(anexo, "name")
                .to_ascii_lowercase()
                .ends_with(".exe")
        })
        .map(|anexo| {
            (
                texto(anexo, "browser_download_url"),
                anexo.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
            )
        })
        .filter(|(endereco, _)| !endereco.is_empty())
}

fn texto(valor: &serde_json::Value, campo: &str) -> String {
    valor
        .get(campo)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// `2026-07-31T12:34:56Z` vira `31/07/2026`. Fora desse formato, devolve vazio.
fn data_brasileira(publicado_em: &str) -> String {
    let data = publicado_em.split('T').next().unwrap_or_default();
    let partes: Vec<&str> = data.split('-').collect();
    match partes.as_slice() {
        [ano, mes, dia] if ano.len() == 4 => format!("{dia}/{mes}/{ano}"),
        _ => String::new(),
    }
}

/// Compara duas versões `x.y.z` **número a número**.
///
/// Comparar como texto erraria justamente onde importa: `"0.1.9"` é maior que
/// `"0.1.10"` na ordem alfabética, e a atualização nunca seria oferecida.
fn mais_nova(publicada: &str, em_execucao: &str) -> bool {
    numeros(publicada) > numeros(em_execucao)
}

fn numeros(versao: &str) -> Vec<u64> {
    versao
        .split('.')
        .map(|parte| {
            let digitos: String = parte.chars().take_while(|c| c.is_ascii_digit()).collect();
            digitos.parse().unwrap_or(0)
        })
        .collect()
}

fn falha_do_windows(contexto: &str) -> Erro {
    Erro::novo(format!("{contexto}: {}", std::io::Error::last_os_error()))
}

/// Texto no formato que as funções W do Windows esperam: UTF-16 terminado em zero.
fn para_utf16(texto: &str) -> Vec<u16> {
    OsStr::new(texto).encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Resposta real da API do GitHub para o release 0.1.11, com os campos que
    /// interessam e alguns dos que não interessam — estes últimos de propósito,
    /// para o teste provar que o resto do payload é ignorado sem estorvo.
    const RELEASE_DO_GITHUB: &str = r#"{
      "url": "https://api.github.com/repos/yurimj/Cururu-releases/releases/123",
      "html_url": "https://github.com/yurimj/Cururu-releases/releases/tag/v0.1.11",
      "id": 123,
      "author": { "login": "yurimj", "id": 1 },
      "tag_name": "v0.1.11",
      "name": "Cururu 0.1.11",
      "draft": false,
      "prerelease": false,
      "created_at": "2026-07-31T17:10:00Z",
      "published_at": "2026-07-31T17:11:40Z",
      "assets": [
        {
          "id": 456,
          "name": "Cururu_0.1.11_x64-setup.exe",
          "label": null,
          "uploader": { "login": "yurimj" },
          "content_type": "application/octet-stream",
          "state": "uploaded",
          "size": 55066193,
          "download_count": 0,
          "browser_download_url": "https://github.com/yurimj/Cururu-releases/releases/download/v0.1.11/Cururu_0.1.11_x64-setup.exe"
        }
      ],
      "tarball_url": "https://api.github.com/repos/yurimj/Cururu-releases/tarball/v0.1.11",
      "body": "- **O Cururu passa a ter instalador**: um `setup.exe` com o Instant Client embutido.\n- Nova opção **Procurar Atualizações…** no menu.\n"
    }"#;

    /// O caminho que só acontece quando existe versão nova de verdade — o que a
    /// execução contra o GitHub não exercita enquanto a publicada e a em uso
    /// forem a mesma.
    #[test]
    fn le_o_release_do_github_e_monta_a_atualizacao() {
        let release: serde_json::Value = serde_json::from_str(RELEASE_DO_GITHUB).unwrap();

        assert_eq!(texto(&release, "tag_name").trim_start_matches('v'), "0.1.11");
        assert_eq!(data_brasileira(&texto(&release, "published_at")), "31/07/2026");
        assert_eq!(
            texto(&release, "html_url"),
            "https://github.com/yurimj/Cururu-releases/releases/tag/v0.1.11"
        );

        let (instalador, tamanho) = instalador_do_release(&release).expect("o release tem .exe");
        assert_eq!(
            instalador,
            "https://github.com/yurimj/Cururu-releases/releases/download/v0.1.11/Cururu_0.1.11_x64-setup.exe"
        );
        assert_eq!(tamanho, 55_066_193);

        // O endereço do anexo precisa passar pela lista de servidores aceitos,
        // senão o download seria recusado logo depois de ser oferecido.
        assert!(separar_endereco(&instalador).is_ok());

        // E o arquivo cai com o nome certo, não com o genérico.
        let destino = caminho_de_destino(&instalador).unwrap();
        assert_eq!(
            destino.file_name().unwrap().to_str().unwrap(),
            "Cururu_0.1.11_x64-setup.exe"
        );

        // A tela mostra o corpo como lista; ele precisa ter itens de lista.
        let notas = texto(&release, "body");
        assert_eq!(notas.lines().filter(|l| l.starts_with("- ")).count(), 2);
    }

    #[test]
    fn release_sem_executavel_nao_vira_atualizacao() {
        // Um release publicado só com notas, sem instalador anexado: oferecer
        // atualização aqui levaria a um download impossível.
        let release: serde_json::Value = serde_json::from_str(
            r#"{"tag_name":"v0.2.0","assets":[{"name":"notas.txt","size":10,
                "browser_download_url":"https://github.com/x/y/notas.txt"}]}"#,
        )
        .unwrap();
        assert!(instalador_do_release(&release).is_none());

        // Nem um release sem anexo nenhum.
        let vazio: serde_json::Value = serde_json::from_str(r#"{"tag_name":"v0.2.0"}"#).unwrap();
        assert!(instalador_do_release(&vazio).is_none());
    }

    #[test]
    fn versao_e_comparada_por_numero_e_nao_por_texto() {
        // O caso que a comparação por texto erra.
        assert!(mais_nova("0.1.10", "0.1.9"));
        assert!(!mais_nova("0.1.9", "0.1.10"));

        assert!(mais_nova("0.2.0", "0.1.11"));
        assert!(mais_nova("1.0.0", "0.9.9"));
        assert!(!mais_nova("0.1.10", "0.1.10"));
        assert!(!mais_nova("0.1.9", "0.1.9"));
    }

    #[test]
    fn so_baixa_de_servidor_do_github() {
        assert!(separar_endereco("https://github.com/yurimj/x/releases/download/v1/a.exe").is_ok());
        assert!(separar_endereco("https://objects.githubusercontent.com/pacote").is_ok());

        // Domínio que só termina parecido não passa.
        assert!(separar_endereco("https://github.com.invasor.br/a.exe").is_err());
        assert!(separar_endereco("https://exemplo.com/a.exe").is_err());
        // Nem endereço sem TLS.
        assert!(separar_endereco("http://github.com/a.exe").is_err());
    }

    #[test]
    fn endereco_sem_caminho_vira_barra() {
        let (servidor, caminho) = separar_endereco("https://github.com").unwrap();
        assert_eq!(servidor, "github.com");
        assert_eq!(caminho, "/");
    }

    #[test]
    fn nome_do_instalador_sai_do_endereco_sem_surpresa() {
        let destino = caminho_de_destino(
            "https://github.com/yurimj/Cururu-releases/releases/download/v0.1.11/Cururu_0.1.11_x64-setup.exe",
        )
        .unwrap();
        assert_eq!(
            destino.file_name().unwrap().to_str().unwrap(),
            "Cururu_0.1.11_x64-setup.exe"
        );

        // Tentativa de subir de pasta não sobrevive à filtragem.
        let destino = caminho_de_destino("https://github.com/x/..%2F..%2Fevil.exe").unwrap();
        let nome = destino.file_name().unwrap().to_str().unwrap();
        assert!(!nome.contains('/') && !nome.contains('\\'));

        // Sem extensão de executável, o nome é descartado.
        let destino = caminho_de_destino("https://github.com/x/notas.txt").unwrap();
        assert_eq!(
            destino.file_name().unwrap().to_str().unwrap(),
            "cururu-instalador.exe"
        );
    }

    #[test]
    fn data_do_github_vira_formato_brasileiro() {
        assert_eq!(data_brasileira("2026-07-31T12:34:56Z"), "31/07/2026");
        assert_eq!(data_brasileira(""), "");
    }
}
