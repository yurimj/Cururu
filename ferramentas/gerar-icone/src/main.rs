//! Gera o ícone do Cururu a partir de `ferramentas/cururu.svg`.
//!
//! O SVG é a fonte do desenho; este utilitário apenas rasteriza. Para mudar o
//! cururu, edite o SVG e rode `cargo run --release` aqui dentro.

use std::path::{Path, PathBuf};

use resvg::tiny_skia;
use resvg::usvg;

/// Resoluções embutidas no .ico. O Windows escolhe a mais adequada conforme o
/// contexto (16 px na barra de título, 256 px em ícones grandes do Explorer).
const TAMANHOS_ICO: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// PNGs adicionais usados pelo bundle do Tauri.
const PNGS: [(u32, &str); 4] = [
    (32, "32x32.png"),
    (128, "128x128.png"),
    (256, "128x128@2x.png"),
    (512, "icon.png"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raiz = raiz_do_projeto();
    let svg = raiz.join("ferramentas").join("cururu.svg");
    let destino = raiz.join("src-tauri").join("icons");
    std::fs::create_dir_all(&destino)?;

    let dados = std::fs::read(&svg)?;
    let arvore = usvg::Tree::from_data(&dados, &usvg::Options::default())?;

    let mut diretorio = ico::IconDir::new(ico::ResourceType::Icon);
    for tamanho in TAMANHOS_ICO {
        let pixmap = renderizar(&arvore, tamanho);
        let imagem = ico::IconImage::from_rgba_data(tamanho, tamanho, demultiplicar(&pixmap));
        diretorio.add_entry(ico::IconDirEntry::encode(&imagem)?);
    }
    let caminho_ico = destino.join("icon.ico");
    diretorio.write(std::fs::File::create(&caminho_ico)?)?;
    println!("gerado: {}", caminho_ico.display());

    for (tamanho, nome) in PNGS {
        let caminho = destino.join(nome);
        renderizar(&arvore, tamanho).save_png(&caminho)?;
        println!("gerado: {}", caminho.display());
    }

    Ok(())
}

fn renderizar(arvore: &usvg::Tree, tamanho: u32) -> tiny_skia::Pixmap {
    let mut pixmap = tiny_skia::Pixmap::new(tamanho, tamanho).expect("tamanho de pixmap válido");
    let escala = tamanho as f32 / arvore.size().width();
    resvg::render(
        arvore,
        tiny_skia::Transform::from_scale(escala, escala),
        &mut pixmap.as_mut(),
    );
    pixmap
}

/// O tiny-skia guarda os pixels com alfa pré-multiplicado; o .ico espera RGBA
/// comum.
fn demultiplicar(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let cor = pixel.demultiply();
            [cor.red(), cor.green(), cor.blue(), cor.alpha()]
        })
        .collect()
}

/// Sobe de `ferramentas/gerar-icone` até a raiz do repositório.
fn raiz_do_projeto() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("estrutura de diretórios esperada")
        .to_path_buf()
}
