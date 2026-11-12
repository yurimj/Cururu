//! Registro de erros e operações (requisito 28).
//!
//! Texto puro, uma ocorrência por linha, em `%APPDATA%\Cururu\logs\`. Precisa
//! ser legível direto no Bloco de Notas, sem ferramenta nenhuma.
//!
//! Regra que não se quebra: **senha nunca vai para o log**.

use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::System::SystemInformation::GetLocalTime;

/// Arquivo aberto para acréscimo, protegido para escrita de várias threads.
static ARQUIVO: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static CAMINHO: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub enum Nivel {
    Informacao,
    Aviso,
    Erro,
}

impl Nivel {
    fn etiqueta(self) -> &'static str {
        match self {
            Nivel::Informacao => "INFO ",
            Nivel::Aviso => "AVISO",
            Nivel::Erro => "ERRO ",
        }
    }
}

/// Abre o log e instala o gancho de *panic*.
///
/// Chamar uma vez, no início do processo. Falhar aqui não pode impedir o
/// aplicativo de subir: sem log, ele ainda funciona.
pub fn iniciar() {
    let caminho = match preparar_arquivo() {
        Ok(caminho) => caminho,
        Err(erro) => {
            eprintln!("Cururu: não foi possível abrir o log: {erro}");
            return;
        }
    };
    let _ = CAMINHO.set(caminho);

    // Sem isto, um panic mata o processo em silêncio — foi exatamente o que
    // deixou o "fechou sozinho" sem explicação.
    std::panic::set_hook(Box::new(|informacao| {
        let local = informacao
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "local desconhecido".to_string());
        escrever(Nivel::Erro, "panic", format!("{informacao} — em {local}"));
    }));

    informacao("aplicativo", "Cururu iniciado");
}

/// Caminho do arquivo de log em uso, quando há um.
pub fn caminho() -> Option<&'static PathBuf> {
    CAMINHO.get()
}

pub fn informacao(origem: &str, mensagem: impl Display) {
    escrever(Nivel::Informacao, origem, mensagem);
}

pub fn aviso(origem: &str, mensagem: impl Display) {
    escrever(Nivel::Aviso, origem, mensagem);
}

pub fn erro(origem: &str, mensagem: impl Display) {
    escrever(Nivel::Erro, origem, mensagem);
}

fn escrever(nivel: Nivel, origem: &str, mensagem: impl Display) {
    let linha = format!("{} {} [{origem}] {mensagem}\n", agora(), nivel.etiqueta());

    // Em modo de depuração o mesmo texto vai para o console, que é onde ele
    // aparece durante o desenvolvimento.
    if cfg!(debug_assertions) {
        eprint!("{linha}");
    }

    let trava = ARQUIVO.get_or_init(|| Mutex::new(None));
    let mut guarda = trava.lock().unwrap_or_else(|erro| erro.into_inner());
    if let Some(arquivo) = guarda.as_mut() {
        let _ = arquivo.write_all(linha.as_bytes());
        let _ = arquivo.flush();
    }
}

/// Abre (ou cria) o arquivo do dia e o guarda para as próximas escritas.
fn preparar_arquivo() -> std::io::Result<PathBuf> {
    let pasta = crate::perfis::pasta_de_dados()
        .map_err(|erro| std::io::Error::other(erro.mensagem))?
        .join("logs");
    std::fs::create_dir_all(&pasta)?;

    // Um arquivo por dia: mantém cada um pequeno e fácil de achar, sem precisar
    // de rotação por tamanho.
    let hoje = data_de_hoje();
    let caminho = pasta.join(format!("cururu-{hoje}.log"));
    let arquivo = OpenOptions::new().create(true).append(true).open(&caminho)?;

    let trava = ARQUIVO.get_or_init(|| Mutex::new(None));
    let mut guarda = trava.lock().unwrap_or_else(|erro| erro.into_inner());
    *guarda = Some(arquivo);

    Ok(caminho)
}

/// Formata um instante do sistema como `29/07/2026 16:52`, hora local.
pub fn formatar(momento: std::time::SystemTime) -> String {
    let segundos_utc = momento
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    formatar_local(segundos_utc + deslocamento_local())
}

/// Quanto o relógio local está adiantado em relação ao UTC, em segundos.
///
/// A diferença vem da própria API do Windows, comparando as duas leituras
/// feitas no mesmo instante — e a comparação é entre os **instantes inteiros**,
/// não entre as horas do dia.
///
/// Comparar só a hora do dia parecia bastar e custou uma data errada: das 21h
/// à meia-noite em Brasília o UTC já virou o dia, e `21:12 − 00:12` dá +21 h
/// onde o fuso é −3 h. Os dois números diferem por um dia inteiro, e a tela
/// **Versões** passava a noite anunciando um executável compilado no dia
/// seguinte — o mesmo valia para as datas da tela de recuperação.
///
/// O deslocamento é o de **agora**, aplicado a um instante que pode ser de
/// outro dia: onde há horário de verão, uma data de fora da estação corrente
/// sai com uma hora de diferença. No Brasil não há desde 2019, e carregar as
/// regras de fuso não se paga para mostrar a data de compilação.
fn deslocamento_local() -> i64 {
    // SEGURANÇA: GetLocalTime só escreve na estrutura passada.
    let t = unsafe {
        let mut tempo = std::mem::zeroed();
        GetLocalTime(&mut tempo);
        tempo
    };
    let agora_utc = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let agora_local = segundos_civis(
        i64::from(t.wYear),
        i64::from(t.wMonth),
        i64::from(t.wDay),
        i64::from(t.wHour),
        i64::from(t.wMinute),
        i64::from(t.wSecond),
    );
    agora_local - agora_utc
}

/// Formata segundos desde 1970-01-01 **já em hora local**.
fn formatar_local(segundos: i64) -> String {
    let (ano, mes, dia) = data_civil(segundos.div_euclid(86_400));
    let hora_do_dia = segundos.rem_euclid(86_400);

    format!(
        "{dia:02}/{mes:02}/{ano:04} {:02}:{:02}",
        hora_do_dia / 3600,
        (hora_do_dia % 3600) / 60
    )
}

/// Converte dias desde 1970-01-01 em (ano, mês, dia).
///
/// Algoritmo "days from civil" de Howard Hinnant, na forma inversa.
fn data_civil(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let ano = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    (if mes <= 2 { ano + 1 } else { ano }, mes, dia)
}

/// Converte uma data e hora civis em segundos desde 1970-01-01.
///
/// É a ida do mesmo algoritmo de Hinnant que [`data_civil`] desfaz. Existe para
/// o deslocamento do fuso sair de uma subtração entre instantes inteiros, que é
/// a única que atravessa a virada do dia sem perder um dia no caminho.
fn segundos_civis(ano: i64, mes: i64, dia: i64, hora: i64, minuto: i64, segundo: i64) -> i64 {
    let ano = if mes <= 2 { ano - 1 } else { ano };
    let era = ano.div_euclid(400);
    let ano_da_era = ano - era * 400;
    let mp = if mes > 2 { mes - 3 } else { mes + 9 };
    let dia_do_ano = (153 * mp + 2) / 5 + dia - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    let dias = era * 146_097 + dia_da_era - 719_468;

    dias * 86_400 + hora * 3600 + minuto * 60 + segundo
}

/// Data e hora local, no formato `2026-07-29 14:32:10.123`.
fn agora() -> String {
    // SEGURANÇA: GetLocalTime só escreve na estrutura passada.
    let t = unsafe {
        let mut tempo = std::mem::zeroed();
        GetLocalTime(&mut tempo);
        tempo
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

fn data_de_hoje() -> String {
    // SEGURANÇA: idem.
    let t = unsafe {
        let mut tempo = std::mem::zeroed();
        GetLocalTime(&mut tempo);
        tempo
    };
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn data_e_hora_saem_no_formato_esperado() {
        let momento = agora();
        assert_eq!(momento.len(), 23, "formato inesperado: {momento}");
        assert_eq!(&momento[4..5], "-");
        assert_eq!(&momento[10..11], " ");
        assert_eq!(&momento[19..20], ".");

        let dia = data_de_hoje();
        assert_eq!(dia.len(), 10);
        assert_eq!(dia, &momento[..10]);
    }

    #[test]
    fn a_ida_e_a_volta_do_calendario_batem() {
        for (ano, mes, dia) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2026, 8, 29),
            (2026, 12, 31),
            (2100, 3, 1),
        ] {
            let segundos = segundos_civis(ano, mes, dia, 0, 0, 0);
            assert_eq!(data_civil(segundos / 86_400), (ano, mes, dia));
        }
    }

    #[test]
    fn formata_a_hora_local_com_dia_e_minuto() {
        assert_eq!(
            formatar_local(segundos_civis(2026, 8, 29, 8, 37, 54)),
            "29/08/2026 08:37"
        );
        assert_eq!(
            formatar_local(segundos_civis(2026, 1, 1, 0, 0, 0)),
            "01/01/2026 00:00"
        );
        assert_eq!(
            formatar_local(segundos_civis(2024, 2, 29, 23, 59, 59)),
            "29/02/2024 23:59"
        );
    }

    /// Das 21h à meia-noite o UTC já virou o dia. Comparando só a hora do dia,
    /// `21:12 − 00:12` dava +21 h onde o fuso é −3 h, e a data de compilação
    /// aparecia um dia à frente na tela **Versões**.
    #[test]
    fn o_deslocamento_atravessa_a_virada_do_dia() {
        let local = segundos_civis(2026, 8, 29, 21, 12, 0);
        let utc = segundos_civis(2026, 8, 30, 0, 12, 0);
        let deslocamento = local - utc;

        assert_eq!(deslocamento, -3 * 3600);

        let compilado = segundos_civis(2026, 8, 29, 11, 37, 54); // 08:37 em UTC−3
        assert_eq!(
            formatar_local(compilado + deslocamento),
            "29/08/2026 08:37"
        );
    }
}
