//! Planilha `.xlsx` da exportação do grid (requisito 16).
//!
//! Até a 0.1.91 a exportação para Excel gravava um XML do formato de 2003
//! (SpreadsheetML) com extensão `.xls`. O Excel abria, mas só depois do aviso
//! de que *o formato de arquivo e a extensão não correspondem*: desde 2007 ele
//! confere o conteúdo contra a extensão, e `.xls` promete o formato binário
//! antigo — que aquele XML não era. Agora sai um `.xlsx`, e o aviso some.
//!
//! Um `.xlsx` é um zip com alguns XMLs dentro, e os dois são montados aqui. O
//! zip não precisa de biblioteca — são três cabeçalhos e um índice —, e a
//! compressão sai do `flate2`, que já vinha na árvore de dependências pelo
//! `png` do Tauri: nada novo passou a ser compilado por causa disto.
//!
//! O pacote leva o mínimo que o Excel exige: os tipos de conteúdo, as duas
//! listas de relações, a pasta de trabalho, os estilos (que existem só para o
//! negrito do cabeçalho) e a planilha. Os textos vão embutidos na célula
//! (`inlineStr`), o que dispensa a tabela de textos repetidos que o Excel
//! costuma escrever — uma parte a menos para manter em dia.

use std::fmt::Write as _;
use std::io::Write as _;

use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};

/// Linhas de dados que cabem numa planilha: o teto do Excel é 1.048.576, e o
/// cabeçalho ocupa a primeira.
pub const MAXIMO_DE_LINHAS: usize = 1_048_575;

/// Colunas que cabem numa planilha — `XFD`, a última que o Excel tem.
pub const MAXIMO_DE_COLUNAS: usize = 16_384;

/// Caracteres que cabem numa célula. Passar disso faz o Excel recusar o
/// arquivo inteiro, então o texto é cortado.
const MAXIMO_DE_CARACTERES: usize = 32_767;

/// Dígitos que um número do Excel guarda sem arredondar. Além deles a conta é
/// feita em ponto flutuante e o valor muda sozinho — um `NUMBER(30)` do Oracle
/// viraria outro número —, então o valor vai como texto.
const DIGITOS_EXATOS: usize = 15;

/// O arquivo inteiro, pronto para gravar.
pub fn montar(colunas: &[String], linhas: &[Vec<Option<String>>]) -> Vec<u8> {
    let colunas = &colunas[..colunas.len().min(MAXIMO_DE_COLUNAS)];
    let linhas = &linhas[..linhas.len().min(MAXIMO_DE_LINHAS)];

    let mut zip = Zip::nova();
    zip.adicionar("[Content_Types].xml", TIPOS_DE_CONTEUDO.as_bytes());
    zip.adicionar("_rels/.rels", RELACOES.as_bytes());
    zip.adicionar("xl/workbook.xml", PASTA.as_bytes());
    zip.adicionar("xl/_rels/workbook.xml.rels", RELACOES_DA_PASTA.as_bytes());
    zip.adicionar("xl/styles.xml", ESTILOS.as_bytes());
    zip.adicionar(
        "xl/worksheets/sheet1.xml",
        planilha(colunas, linhas).as_bytes(),
    );
    zip.finalizar()
}

// --- As partes do pacote ----------------------------------------------------

const CABECALHO_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

const TIPOS_DE_CONTEUDO: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
    r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
    r#"<Default Extension="xml" ContentType="application/xml"/>"#,
    r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
    r#"<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#,
    r#"<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    "</Types>",
);

const RELACOES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>"#,
    "</Relationships>",
);

const PASTA: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main""#,
    r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
    r#"<sheets><sheet name="Consulta" sheetId="1" r:id="rId1"/></sheets>"#,
    "</workbook>",
);

const RELACOES_DA_PASTA: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>"#,
    r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
    "</Relationships>",
);

/// Só o necessário para existir um formato em negrito, que é o `s="1"` do
/// cabeçalho. As listas de preenchimento e de borda estão aí porque o Excel
/// reclama do arquivo quando faltam, não porque alguma célula as use.
const ESTILOS: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    r#"<fonts count="2">"#,
    r#"<font><sz val="11"/><name val="Calibri"/></font>"#,
    r#"<font><b/><sz val="11"/><name val="Calibri"/></font>"#,
    "</fonts>",
    r#"<fills count="2">"#,
    r#"<fill><patternFill patternType="none"/></fill>"#,
    r#"<fill><patternFill patternType="gray125"/></fill>"#,
    "</fills>",
    r#"<borders count="1"><border/></borders>"#,
    r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
    r#"<cellXfs count="2">"#,
    r#"<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>"#,
    r#"<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>"#,
    "</cellXfs>",
    "</styleSheet>",
);

/// A planilha: o cabeçalho em negrito na linha 1 e os dados a partir da 2.
fn planilha(colunas: &[String], linhas: &[Vec<Option<String>>]) -> String {
    let letras: Vec<String> = (0..colunas.len()).map(letras_da_coluna).collect();

    let mut xml = String::with_capacity(512 + (linhas.len() + 1) * (colunas.len() * 48 + 16));
    xml.push_str(CABECALHO_XML);
    xml.push_str(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );

    xml.push_str(r#"<row r="1">"#);
    for (indice, nome) in colunas.iter().enumerate() {
        celula_de_texto(&mut xml, &letras[indice], 1, nome, true);
    }
    xml.push_str("</row>");

    for (indice_da_linha, linha) in linhas.iter().enumerate() {
        let numero = indice_da_linha + 2;
        let _ = write!(xml, r#"<row r="{numero}">"#);
        for (indice, valor) in linha.iter().take(letras.len()).enumerate() {
            // Nulo não vira célula nenhuma: é assim que uma célula fica vazia.
            let Some(valor) = valor else { continue };
            match numero_do_excel(valor) {
                Some(numero_da_celula) => {
                    let _ = write!(
                        xml,
                        r#"<c r="{}{numero}"><v>{numero_da_celula}</v></c>"#,
                        letras[indice]
                    );
                }
                None => celula_de_texto(&mut xml, &letras[indice], numero, valor, false),
            }
        }
        xml.push_str("</row>");
    }

    xml.push_str("</sheetData></worksheet>");
    xml
}

/// Uma célula de texto. O `xml:space` só entra quando o valor tem espaço nas
/// pontas ou quebra de linha, que é quando o Excel aparar o texto mudaria o
/// que a consulta devolveu.
fn celula_de_texto(xml: &mut String, coluna: &str, linha: usize, valor: &str, cabecalho: bool) {
    let estilo = if cabecalho { r#" s="1""# } else { "" };
    let texto = limpar(valor);
    let espaco = if precisa_preservar_espaco(&texto) {
        r#" xml:space="preserve""#
    } else {
        ""
    };
    let _ = write!(
        xml,
        r#"<c r="{coluna}{linha}"{estilo} t="inlineStr"><is><t{espaco}>{}</t></is></c>"#,
        escapar(&texto)
    );
}

/// Corta no teto da célula e tira os caracteres de controle, que o XML não
/// aceita e fariam o Excel recusar o arquivo. Tabulação, quebra de linha e
/// retorno de carro ficam: são válidos e aparecem em texto de banco.
fn limpar(valor: &str) -> String {
    valor
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
        .take(MAXIMO_DE_CARACTERES)
        .collect()
}

fn precisa_preservar_espaco(texto: &str) -> bool {
    texto.starts_with(char::is_whitespace) || texto.ends_with(char::is_whitespace)
}

fn escapar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            _ => saida.push(c),
        }
    }
    saida
}

/// O nome da coluna de índice `indice`: `A`, …, `Z`, `AA`, `AB`, …
fn letras_da_coluna(indice: usize) -> String {
    let mut letras = Vec::new();
    let mut resta = indice + 1;
    while resta > 0 {
        let resto = (resta - 1) % 26;
        letras.push(b'A' + resto as u8);
        resta = (resta - 1) / 26;
    }
    letras.reverse();
    String::from_utf8(letras).unwrap_or_default()
}

/// O valor como número para o Excel, quando o grid o mostra no padrão
/// brasileiro — ponto de milhar e vírgula decimal. `None` em todo o resto, e
/// aí a célula vai como texto.
///
/// Fica de fora, de propósito, o que *parece* número mas não é: código com
/// zero à esquerda (um CEP `01310` viraria 1310) e número com mais dígitos do
/// que o Excel guarda sem arredondar.
fn numero_do_excel(valor: &str) -> Option<String> {
    let (sinal, resto) = match valor.strip_prefix('-') {
        Some(resto) => ("-", resto),
        None => ("", valor),
    };

    let mut partes = resto.split(',');
    let inteira = partes.next()?;
    let decimal = partes.next();
    if partes.next().is_some() {
        return None;
    }
    if decimal.is_some_and(|d| d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }

    let inteira = sem_milhares(inteira)?;
    // Zero à esquerda é marca de código, não de número: `007` fica texto.
    if inteira.len() > 1 && inteira.starts_with('0') {
        return None;
    }
    let significativos = inteira.trim_start_matches('0').len() + decimal.unwrap_or("").len();
    if significativos > DIGITOS_EXATOS {
        return None;
    }

    Some(match decimal {
        Some(decimal) => format!("{sinal}{inteira}.{decimal}"),
        None => format!("{sinal}{inteira}"),
    })
}

/// A parte inteira sem os pontos de milhar, ou `None` quando os pontos não
/// estão onde o padrão brasileiro os põe — `1.234` passa, `12.34` não.
fn sem_milhares(inteira: &str) -> Option<String> {
    if inteira.is_empty() {
        return None;
    }
    if !inteira.contains('.') {
        return inteira
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| inteira.to_string());
    }

    let mut grupos = inteira.split('.');
    let primeiro = grupos.next()?;
    if primeiro.is_empty() || primeiro.len() > 3 || !primeiro.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut digitos = primeiro.to_string();
    for grupo in grupos {
        if grupo.len() != 3 || !grupo.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digitos.push_str(grupo);
    }
    Some(digitos)
}

// --- O zip ------------------------------------------------------------------

/// Data e hora que cada parte leva, no formato do MS-DOS que o zip guarda:
/// 1º de janeiro de 1980, o começo da contagem. É valor fixo porque ninguém o
/// vê — o Excel não mostra a data das partes internas — e porque assim a mesma
/// consulta gera sempre o mesmo arquivo, o que um teste consegue conferir.
const DATA_DOS: u16 = (1 << 5) | 1;
const HORA_DOS: u16 = 0;

/// Versão mínima para extrair: 2.0, que é a que tem deflate.
const VERSAO: u16 = 20;
const DEFLATE: u16 = 8;

struct Zip {
    bytes: Vec<u8>,
    indice: Vec<Entrada>,
}

struct Entrada {
    nome: &'static str,
    crc: u32,
    comprimido: u32,
    original: u32,
    deslocamento: u32,
}

impl Zip {
    fn nova() -> Self {
        Zip {
            bytes: Vec::new(),
            indice: Vec::new(),
        }
    }

    /// Uma parte do pacote: cabeçalho local, dados comprimidos e uma linha no
    /// índice, que só é escrito no fim.
    fn adicionar(&mut self, nome: &'static str, conteudo: &[u8]) {
        let mut crc = Crc::new();
        crc.update(conteudo);
        let comprimido = comprimir(conteudo);
        let entrada = Entrada {
            nome,
            crc: crc.sum(),
            comprimido: comprimido.len() as u32,
            original: conteudo.len() as u32,
            deslocamento: self.bytes.len() as u32,
        };

        self.bytes.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        self.bytes.extend_from_slice(&VERSAO.to_le_bytes());
        self.bytes.extend_from_slice(&0u16.to_le_bytes()); // sem marcas
        self.bytes.extend_from_slice(&DEFLATE.to_le_bytes());
        self.bytes.extend_from_slice(&HORA_DOS.to_le_bytes());
        self.bytes.extend_from_slice(&DATA_DOS.to_le_bytes());
        self.bytes.extend_from_slice(&entrada.crc.to_le_bytes());
        self.bytes
            .extend_from_slice(&entrada.comprimido.to_le_bytes());
        self.bytes
            .extend_from_slice(&entrada.original.to_le_bytes());
        self.bytes
            .extend_from_slice(&(nome.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(&0u16.to_le_bytes()); // sem campo extra
        self.bytes.extend_from_slice(nome.as_bytes());
        self.bytes.extend_from_slice(&comprimido);

        self.indice.push(entrada);
    }

    /// O índice central e o fecho, que é por onde todo leitor de zip começa.
    fn finalizar(mut self) -> Vec<u8> {
        let inicio_do_indice = self.bytes.len() as u32;
        let indice = std::mem::take(&mut self.indice);

        for entrada in &indice {
            self.bytes.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            self.bytes.extend_from_slice(&VERSAO.to_le_bytes()); // versão de quem escreveu
            self.bytes.extend_from_slice(&VERSAO.to_le_bytes());
            self.bytes.extend_from_slice(&0u16.to_le_bytes());
            self.bytes.extend_from_slice(&DEFLATE.to_le_bytes());
            self.bytes.extend_from_slice(&HORA_DOS.to_le_bytes());
            self.bytes.extend_from_slice(&DATA_DOS.to_le_bytes());
            self.bytes.extend_from_slice(&entrada.crc.to_le_bytes());
            self.bytes
                .extend_from_slice(&entrada.comprimido.to_le_bytes());
            self.bytes
                .extend_from_slice(&entrada.original.to_le_bytes());
            self.bytes
                .extend_from_slice(&(entrada.nome.len() as u16).to_le_bytes());
            self.bytes.extend_from_slice(&0u16.to_le_bytes()); // sem campo extra
            self.bytes.extend_from_slice(&0u16.to_le_bytes()); // sem comentário
            self.bytes.extend_from_slice(&0u16.to_le_bytes()); // disco 0
            self.bytes.extend_from_slice(&0u16.to_le_bytes()); // sem atributos internos
            self.bytes.extend_from_slice(&0u32.to_le_bytes()); // sem atributos externos
            self.bytes
                .extend_from_slice(&entrada.deslocamento.to_le_bytes());
            self.bytes.extend_from_slice(entrada.nome.as_bytes());
        }

        let tamanho_do_indice = self.bytes.len() as u32 - inicio_do_indice;
        let quantas = indice.len() as u16;
        self.bytes.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.bytes.extend_from_slice(&0u16.to_le_bytes()); // disco deste fecho
        self.bytes.extend_from_slice(&0u16.to_le_bytes()); // disco do índice
        self.bytes.extend_from_slice(&quantas.to_le_bytes());
        self.bytes.extend_from_slice(&quantas.to_le_bytes());
        self.bytes
            .extend_from_slice(&tamanho_do_indice.to_le_bytes());
        self.bytes
            .extend_from_slice(&inicio_do_indice.to_le_bytes());
        self.bytes.extend_from_slice(&0u16.to_le_bytes()); // sem comentário

        self.bytes
    }
}

fn comprimir(conteudo: &[u8]) -> Vec<u8> {
    let mut deflate = DeflateEncoder::new(Vec::new(), Compression::default());
    // Escrita em memória: não há erro possível a tratar.
    let _ = deflate.write_all(conteudo);
    deflate.finish().unwrap_or_default()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn colunas_passam_de_z_para_aa() {
        for (indice, esperado) in [(0, "A"), (25, "Z"), (26, "AA"), (701, "ZZ"), (702, "AAA")] {
            assert_eq!(letras_da_coluna(indice), esperado, "{indice}");
        }
    }

    #[test]
    fn numero_so_quando_e_numero_de_verdade() {
        for (valor, esperado) in [
            ("1.234,56", "1234.56"),
            ("-1.234,56", "-1234.56"),
            ("0,5", "0.5"),
            ("42", "42"),
            ("0", "0"),
            ("999.999.999", "999999999"),
        ] {
            assert_eq!(numero_do_excel(valor).as_deref(), Some(esperado), "{valor}");
        }

        for valor in [
            "01310",              // CEP: o zero à esquerda se perderia
            "12.34",              // ponto fora do lugar do milhar
            "1.2345",             // idem
            "123456789012345678", // mais dígitos do que o Excel guarda
            "1,",
            "1,2,3",
            "",
            "-",
            "1 234",
            "R$ 10,00",
            "2026-10-07",
            "07/10/2026",
        ] {
            assert_eq!(numero_do_excel(valor), None, "{valor}");
        }
    }

    #[test]
    fn celula_leva_tipo_escape_e_corte() {
        let longo = "x".repeat(MAXIMO_DE_CARACTERES + 10);
        let xml = planilha(
            &["Nome & Cia".to_string(), "Valor".to_string()],
            &[
                vec![Some("<a>".to_string()), Some("1.234,50".to_string())],
                vec![None, Some(" espaço ".to_string())],
                vec![Some(format!("ruim\u{1}aqui{longo}")), None],
            ],
        );

        assert!(xml.contains(r#"<c r="A1" s="1" t="inlineStr"><is><t>Nome &amp; Cia</t></is></c>"#));
        assert!(xml.contains(r#"<c r="A2" t="inlineStr"><is><t>&lt;a&gt;</t></is></c>"#));
        assert!(xml.contains(r#"<c r="B2"><v>1234.50</v></c>"#));
        // Linha 3: a coluna A é nula e não vira célula nenhuma.
        assert!(xml.contains(r#"<row r="3"><c r="B3" t="inlineStr"#));
        assert!(xml.contains(r#"<t xml:space="preserve"> espaço </t>"#));
        // O caractere de controle sai e o texto é cortado no teto da célula.
        assert!(!xml.contains('\u{1}'));
        assert!(xml.contains(&format!("ruimaqui{}", "x".repeat(MAXIMO_DE_CARACTERES - 8))));
        assert!(!xml.contains(&format!("ruimaqui{}", "x".repeat(MAXIMO_DE_CARACTERES - 7))));
    }

    #[test]
    fn pacote_tem_as_partes_e_o_indice_fecha() {
        let bytes = montar(&["A".to_string()], &[vec![Some("1".to_string())]]);

        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let fecho = bytes.len() - 22;
        assert_eq!(&bytes[fecho..fecho + 4], b"PK\x05\x06");
        assert_eq!(
            u16::from_le_bytes([bytes[fecho + 10], bytes[fecho + 11]]),
            6
        );

        let tamanho =
            u32::from_le_bytes(bytes[fecho + 12..fecho + 16].try_into().unwrap()) as usize;
        let inicio = u32::from_le_bytes(bytes[fecho + 16..fecho + 20].try_into().unwrap()) as usize;
        assert_eq!(inicio + tamanho, fecho);
        assert_eq!(&bytes[inicio..inicio + 4], b"PK\x01\x02");

        for parte in [
            "[Content_Types].xml",
            "_rels/.rels",
            "xl/workbook.xml",
            "xl/_rels/workbook.xml.rels",
            "xl/styles.xml",
            "xl/worksheets/sheet1.xml",
        ] {
            let procurado = parte.as_bytes();
            assert!(
                bytes.windows(procurado.len()).any(|j| j == procurado),
                "{parte}"
            );
        }
    }

    /// Grava uma planilha de exemplo para conferir fora daqui — `cargo test
    /// exemplo -- --ignored --nocapture` diz o caminho. Fica de fora da suíte
    /// porque escreve em disco e só existe para olho humano (ou para o Excel).
    #[test]
    #[ignore]
    fn exemplo_para_abrir_no_excel() {
        let colunas = vec![
            "ID".to_string(),
            "CLIENTE".to_string(),
            "CEP".to_string(),
            "VALOR".to_string(),
            "OBSERVACAO".to_string(),
        ];
        let linhas = vec![
            vec![
                Some("1".to_string()),
                Some("Açúcar & Cia <Ltda>".to_string()),
                Some("01310".to_string()),
                Some("1.234,56".to_string()),
                Some("linha 1\nlinha 2".to_string()),
            ],
            vec![
                Some("2".to_string()),
                Some("José da Silva".to_string()),
                Some("20040-002".to_string()),
                Some("-98,70".to_string()),
                None,
            ],
            vec![
                Some("3".to_string()),
                Some(" com espaço ".to_string()),
                None,
                Some("0".to_string()),
                Some("texto \"com aspas\"".to_string()),
            ],
        ];

        let caminho = std::env::temp_dir().join("cururu-exemplo.xlsx");
        std::fs::write(&caminho, montar(&colunas, &linhas)).unwrap();
        println!("exemplo em {}", caminho.display());
    }
}
