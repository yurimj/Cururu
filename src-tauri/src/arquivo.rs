//! Leitura de arquivo de texto do disco (requisito 12).
//!
//! Gravar é uma linha e fica na fronteira, em `comandos.rs`. Ler não é: um
//! `.sql` numa máquina Windows aparece em UTF-8 (com ou sem BOM), em UTF-16
//! quando saiu do "Unicode" do Bloco de Notas, e em ANSI/Windows-1252, que é o
//! que o SQL Developer e as ferramentas antigas gravavam. Ler tudo como UTF-8
//! estragaria os acentos nos dois últimos casos, e comentário de SQL escrito em
//! português é justamente onde os acentos vivem.

use crate::erro::Resultado;

/// Windows-1252 só difere do Latin-1 na faixa `0x80`–`0x9F`, que traz aspas
/// tipográficas, travessões e reticências — o que o Word e o Outlook colam
/// dentro de um comentário. Fora dessa faixa, o byte já é o próprio código
/// Unicode.
const CP1252_ALTO: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// Lê o arquivo e devolve o texto, descobrindo a codificação pelo conteúdo.
pub fn ler(caminho: &str) -> Resultado<String> {
    let bytes = std::fs::read(caminho)?;
    Ok(decodificar(&bytes))
}

fn decodificar(bytes: &[u8]) -> String {
    // 1. BOM de UTF-16 LE: é o "Unicode" do Bloco de Notas. O BE não aparece
    //    no Windows e por isso não é tratado.
    if let Some(resto) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let unidades: Vec<u16> = resto
            .chunks_exact(2)
            .map(|par| u16::from_le_bytes([par[0], par[1]]))
            .collect();
        return String::from_utf16_lossy(&unidades);
    }

    // 2. UTF-8, com o BOM descartado quando houver.
    let sem_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(sem_bom) {
        Ok(texto) => texto.to_string(),
        // 3. Não sendo UTF-8 válido, só sobra o ANSI da máquina.
        Err(_) => de_windows1252(sem_bom),
    }
}

fn de_windows1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80..=0x9F => CP1252_ALTO[(byte - 0x80) as usize],
            _ => byte as char,
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::decodificar;

    #[test]
    fn le_utf8_com_e_sem_bom() {
        assert_eq!(decodificar("-- não".as_bytes()), "-- não");
        let com_bom = [&[0xEF, 0xBB, 0xBF][..], "-- não".as_bytes()].concat();
        assert_eq!(decodificar(&com_bom), "-- não");
    }

    #[test]
    fn recua_para_windows1252_quando_nao_e_utf8() {
        // "-- ação" em ANSI: 0xE7 é ç e 0xE3 é ã, inválidos em UTF-8.
        let ansi = [b'-', b'-', b' ', b'a', 0xE7, 0xE3, b'o'];
        assert_eq!(decodificar(&ansi), "-- ação");
    }

    #[test]
    fn le_utf16_do_bloco_de_notas() {
        let mut bytes = vec![0xFF, 0xFE];
        for unidade in "-- não".encode_utf16() {
            bytes.extend_from_slice(&unidade.to_le_bytes());
        }
        assert_eq!(decodificar(&bytes), "-- não");
    }
}
