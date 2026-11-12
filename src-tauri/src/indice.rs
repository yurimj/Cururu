//! Índice local dos metadados, guardado em disco (requisito 40).
//!
//! O cache de [`crate::metadados`] morre com a sessão, e neste banco **toda
//! primeira leitura do dia é cara**: a lista de subprogramas de um pacote
//! custou de 14 a 23 segundos, medida no log. Abrir o aplicativo e pagar isso
//! de novo é o que este índice evita — o segundo `DEM_INT.` já era instantâneo
//! dentro da mesma sessão; agora o primeiro também é, porque a resposta ficou
//! gravada da vez passada.
//!
//! Guardado em `%APPDATA%\Cururu\indice\<conexão>.json`, um arquivo por
//! conexão. A chave leva o banco atrás do perfil no SQL Server, porque os
//! objetos de um banco não são os do outro.
//!
//! **O índice é sugestão, e não verdade.** Ele responde o que existia da última
//! vez; o que vale para executar, para abrir fonte e para o grid continua sendo
//! o que o banco disser agora. Uma conexão dedicada atualiza o arquivo em
//! background a cada abertura, sem prender a conexão usada pelo autocomplete.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::erro::Resultado;
use crate::filtro::{Campo, Filtro};
use crate::metadados::Sugestao;
use crate::objetos::{Objeto, Pagina, Subprograma};
use crate::perfis;
use crate::registro;

/// Formato do arquivo. Muda quando o conteúdo deixa de ser compatível, e um
/// arquivo de versão diferente é descartado em vez de ser lido errado.
const VERSAO: u32 = 3;

/// O que fica gravado para uma conexão.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Conteudo {
    #[serde(default)]
    pub versao: u32,
    /// Quando os objetos foram lidos, em texto legível — serve ao diagnóstico
    /// pelo próprio arquivo, sem precisar do aplicativo.
    #[serde(default)]
    pub objetos_lidos_em: String,
    /// Pacotes, procedures e funções do usuário — o que se **chama**.
    ///
    /// Vão inteiros, sem filtro: são poucos, e são o que `EXECUTE ` precisa
    /// listar sem prefixo nenhum.
    ///
    /// Tabelas, views e sinônimos usam um arquivo separado. Os campos antigos
    /// `tabelas` e `tabelas_indexadas` eram um recorte e continuam ignorados.
    #[serde(default)]
    pub rotinas: Vec<Sugestao>,
    /// Ausente nos arquivos antigos; lista vazia significa leitura sem pacotes.
    #[serde(default)]
    pub pacotes: Option<Vec<PacoteIndexado>>,
    /// Procedures com os campos necessários ao filtro da árvore.
    #[serde(default)]
    pub procedures: Option<Vec<PacoteIndexado>>,
    /// Subprogramas por pacote, com o nome do pacote em maiúsculas.
    #[serde(default)]
    pub subprogramas: HashMap<String, Vec<Subprograma>>,
}

/// O acervo de tabelas é muito maior que o de rotinas. Tem arquivo e tranca
/// próprios, para não atrasar packages nem regravar tudo ao abrir um subprograma.
#[derive(Default, Serialize, Deserialize)]
struct RelacoesEmDisco {
    versao: u32,
    usuario: Vec<crate::relacoes::Relacao>,
    outros: Vec<crate::relacoes::Relacao>,
}

#[derive(Clone, Default)]
struct RelacoesGuardadas {
    usuario: Option<Arc<Vec<crate::relacoes::Relacao>>>,
    outros: Option<Arc<Vec<crate::relacoes::Relacao>>>,
}

/// Campos usados pela árvore de pacotes Oracle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacoteIndexado {
    pub nome: String,
    pub situacao: Option<String>,
    pub gerado: Option<String>,
}

/// Índice por conexão, compartilhado entre árvore e autocomplete.
#[derive(Default)]
pub struct RepositorioDeIndice {
    em_memoria: Mutex<HashMap<String, Conteudo>>,
    gravacao: Mutex<()>,
    relacoes_em_memoria: Mutex<HashMap<String, RelacoesGuardadas>>,
    gravacao_relacoes: Mutex<()>,
    /// Resultado completo dos filtros ativos. A árvore só recorta a página.
    listagens: Mutex<HashMap<String, Arc<Vec<Objeto>>>>,
}

impl RepositorioDeIndice {
    /// Preaquece ambos os arquivos sem usar a conexão do autocomplete.
    pub fn preparar(&self, chave: &str) {
        Self::carregar(&mut self.travar(), chave);
        self.relacoes_guardadas(chave);
    }

    /// Prepara durante a conexão as listas que a árvore abrirá depois.
    pub fn preparar_listagens(
        &self,
        chave: &str,
        filtros: &crate::filtros_salvos::FiltrosDaConexao,
    ) {
        for categoria in [
            crate::objetos::Categoria::Tabelas,
            crate::objetos::Categoria::Views,
            crate::objetos::Categoria::Procedures,
        ] {
            if let Some(filtro) = filtros.get(&categoria).filter(|filtro| filtro.ativo()) {
                self.preparar_listagem(chave, categoria, Some(filtro));
            }
        }
    }

    /// Recalcula uma lista quando o usuário grava outro filtro.
    pub fn preparar_listagem(
        &self,
        chave: &str,
        categoria: crate::objetos::Categoria,
        filtro: Option<&Filtro>,
    ) {
        match categoria {
            crate::objetos::Categoria::Tabelas | crate::objetos::Categoria::Views => {
                let _ = self.listar_relacoes(chave, categoria, filtro, 0, 0);
            }
            crate::objetos::Categoria::Procedures => {
                let _ = self.listar_procedures(chave, filtro, 0, 0);
            }
            _ => {}
        }
    }

    fn relacoes_guardadas(&self, chave: &str) -> RelacoesGuardadas {
        self.relacoes_em_memoria
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(chave.to_string())
            .or_insert_with(|| ler_relacoes(chave))
            .clone()
    }

    pub fn sugerir_relacoes(
        &self,
        chave: &str,
        prefixo: &str,
        esquema: Option<&str>,
        filtro: Option<&Filtro>,
    ) -> Vec<Sugestao> {
        let acervo = self.relacoes_guardadas(chave);
        crate::relacoes::selecionar(
            acervo
                .usuario
                .iter()
                .flat_map(|r| r.iter())
                .chain(acervo.outros.iter().flat_map(|r| r.iter())),
            prefixo,
            esquema,
            filtro,
            crate::relacoes::LIMITE,
        )
    }

    /// As listas são imutáveis e compartilhadas: não se clona o acervo inteiro
    /// nem se segura a tranca das sugestões durante a serialização ou o I/O.
    pub fn guardar_relacoes(
        &self,
        chave: &str,
        outros: bool,
        relacoes: Vec<crate::relacoes::Relacao>,
    ) {
        self.invalidar_listagens(chave);
        let _gravacao = self
            .gravacao_relacoes
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut acervo = self.relacoes_guardadas(chave);
        if outros {
            acervo.outros = Some(Arc::new(relacoes));
        } else {
            acervo.usuario = Some(Arc::new(relacoes));
        }
        self.relacoes_em_memoria
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(chave.to_string(), acervo.clone());
        #[derive(Serialize)]
        struct Arquivo<'a> {
            versao: u32,
            usuario: &'a [crate::relacoes::Relacao],
            outros: &'a [crate::relacoes::Relacao],
        }
        let gravar = || -> Resultado<()> {
            let caminho = caminho_relacoes(chave)?;
            if let Some(pasta) = caminho.parent() {
                std::fs::create_dir_all(pasta)?;
            }
            let arquivo = Arquivo {
                versao: 1,
                usuario: acervo.usuario.as_deref().map_or(&[], Vec::as_slice),
                outros: acervo.outros.as_deref().map_or(&[], Vec::as_slice),
            };
            let mut destino = std::io::BufWriter::new(std::fs::File::create(&caminho)?);
            serde_json::to_writer(&mut destino, &arquivo)?;
            std::io::Write::flush(&mut destino)?;
            Ok(())
        };
        if let Err(erro) = gravar() {
            registro::aviso("indice_relacoes", format!("{chave}: não gravou: {erro}"));
        }
    }

    /// Página da árvore para tabelas e views do próprio usuário.
    pub fn listar_relacoes(
        &self,
        chave: &str,
        categoria: crate::objetos::Categoria,
        filtro: Option<&Filtro>,
        deslocamento: u32,
        limite: u32,
    ) -> Option<Pagina> {
        let chave_listagem = chave_da_listagem(chave, categoria, filtro);
        if let Some(objetos) = self
            .listagens
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&chave_listagem)
            .cloned()
        {
            return Some(paginar(&objetos, deslocamento, limite));
        }
        let acervo = self.relacoes_guardadas(chave);
        let usuario = acervo.usuario?;
        let incluir_sinonimos = filtro.is_some_and(|f| f.incluir_sinonimos);
        let tipo = categoria.tipo_oracle();
        let mut selecionados: Vec<_> = usuario
            .iter()
            .filter(|r| r.tipo == tipo || (incluir_sinonimos && r.tipo == "SYNONYM"))
            .filter(|r| {
                filtro.map_or(true, |f| {
                    (r.tipo == "SYNONYM"
                        || !f.excluir_gerados
                        || r.gerado.as_deref() == Some("N"))
                        && f.corresponde_oracle(&r.nome, r.situacao.as_deref())
                })
            })
            .collect();
        selecionados.sort_by(|a, b| a.nome.cmp(&b.nome).then(a.tipo.cmp(&b.tipo)));
        let objetos: Vec<_> = selecionados
            .into_iter()
            .map(|r| Objeto {
                nome: r.nome.clone(),
                situacao: r.situacao.clone(),
                comentario: r.comentario.clone(),
            })
            .collect();
        let objetos = Arc::new(objetos);
        self.listagens
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(chave_listagem, Arc::clone(&objetos));
        Some(paginar(&objetos, deslocamento, limite))
    }

    /// Aproveita nomes dos arquivos antigos, sem inventar a situação do objeto.
    /// Filtros que precisam dos campos novos exigem uma leitura completa.
    pub fn listar_pacotes(
        &self,
        chave: &str,
        filtro: Option<&Filtro>,
        deslocamento: u32,
        limite: u32,
    ) -> Option<Pagina> {
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        let legados;
        let pacotes = if let Some(pacotes) = &conteudo.pacotes {
            pacotes
        } else {
            if conteudo.objetos_lidos_em.is_empty()
                || filtro.is_some_and(|f| {
                    f.excluir_gerados
                        || f.criterios
                            .iter()
                            .any(|c| c.campo == Campo::Situacao && !c.valor.trim().is_empty())
                })
            {
                return None;
            }
            legados = conteudo
                .rotinas
                .iter()
                .filter(|r| r.tipo == "PACKAGE" && r.dono.is_none())
                .map(|r| PacoteIndexado {
                    nome: r.nome.clone(),
                    situacao: None,
                    gerado: None,
                })
                .collect::<Vec<_>>();
            &legados
        };
        let mut selecionados: Vec<_> = pacotes
            .iter()
            .filter(|p| {
                filtro.map_or(true, |f| {
                    (!f.excluir_gerados || p.gerado.as_deref() == Some("N"))
                        && f.corresponde_oracle(&p.nome, p.situacao.as_deref())
                })
            })
            .collect();
        selecionados.sort_by(|a, b| a.nome.cmp(&b.nome));
        let tem_mais = (deslocamento as usize).saturating_add(limite as usize) < selecionados.len();
        let objetos = selecionados
            .into_iter()
            .skip(deslocamento as usize)
            .take(limite as usize)
            .map(|p| Objeto {
                nome: p.nome.clone(),
                situacao: p.situacao.clone(),
                comentario: None,
            })
            .collect();
        Some(Pagina { objetos, tem_mais })
    }

    pub fn listar_procedures(
        &self,
        chave: &str,
        filtro: Option<&Filtro>,
        deslocamento: u32,
        limite: u32,
    ) -> Option<Pagina> {
        let categoria = crate::objetos::Categoria::Procedures;
        let chave_listagem = chave_da_listagem(chave, categoria, filtro);
        if let Some(objetos) = self
            .listagens
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&chave_listagem)
            .cloned()
        {
            return Some(paginar(&objetos, deslocamento, limite));
        }
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        let procedures = conteudo.procedures.as_ref()?;
        let objetos = Arc::new(objetos_de_codigo(procedures, filtro));
        drop(memoria);
        self.listagens
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(chave_listagem, Arc::clone(&objetos));
        Some(paginar(&objetos, deslocamento, limite))
    }

    pub fn guardar_procedures(&self, chave: &str, procedures: Vec<PacoteIndexado>) {
        self.invalidar_listagens(chave);
        let _gravacao = self.gravacao.lock().unwrap_or_else(|e| e.into_inner());
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        conteudo.procedures = Some(procedures);
        let copia = conteudo.clone();
        drop(memoria);
        Self::gravar(chave, &copia);
    }

    pub fn guardar_pacotes(&self, chave: &str, pacotes: Vec<PacoteIndexado>) {
        let _gravacao = self.gravacao.lock().unwrap_or_else(|e| e.into_inner());
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        conteudo.pacotes = Some(pacotes);
        let copia = conteudo.clone();
        drop(memoria);
        Self::gravar(chave, &copia);
    }

    /// Os subprogramas guardados de um pacote, se houver.
    pub fn subprogramas(&self, chave: &str, pacote: &str) -> Option<Vec<Subprograma>> {
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        conteudo.subprogramas.get(&pacote.to_uppercase()).cloned()
    }

    /// Guarda os subprogramas de um pacote e grava o arquivo.
    pub fn guardar_subprogramas(&self, chave: &str, pacote: &str, lidos: &[Subprograma]) {
        let _gravacao = self.gravacao.lock().unwrap_or_else(|e| e.into_inner());
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        conteudo
            .subprogramas
            .insert(pacote.to_uppercase(), lidos.to_vec());
        let copia = conteudo.clone();
        drop(memoria);
        Self::gravar(chave, &copia);
    }

    /// As rotinas do usuário que começam pelo prefixo, ou `None` enquanto o
    /// índice não foi lido.
    ///
    /// Serve a quem está **fora de lugar de tabela**: ali só cabe o que se
    /// chama, e as rotinas estão aqui inteiras. É o que faz `EXECUTE `
    /// responder de disco, com prefixo vazio.
    ///
    /// A filtragem acontece aqui dentro, com a tranca na mão, e não fora com
    /// uma cópia da lista: são dezenas de milhares de nomes, e clonar tudo a
    /// cada tecla digitada custaria mais do que a consulta que isto evita.
    pub fn rotinas_que_comecam_com(&self, chave: &str, prefixo: &str) -> Option<Vec<Sugestao>> {
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        if conteudo.objetos_lidos_em.is_empty() {
            return None;
        }
        Some(crate::metadados::filtrar(&conteudo.rotinas, prefixo))
    }

    /// Guarda o acervo lido e grava o arquivo.
    ///
    /// Substitui o retrato anterior pelo que a conferência em background acabou
    /// de ler, incluindo os membros novos das packages.
    pub fn guardar_acervo(&self, chave: &str, acervo: &crate::metadados::AcervoDoUsuario) {
        self.invalidar_listagens(chave);
        let _gravacao = self.gravacao.lock().unwrap_or_else(|e| e.into_inner());
        let mut memoria = self.travar();
        let conteudo = Self::carregar(&mut memoria, chave);
        conteudo.rotinas = acervo.rotinas.clone();
        conteudo.pacotes = Some(acervo.pacotes.clone());
        conteudo.procedures = Some(acervo.procedures.clone());
        conteudo.subprogramas = acervo.subprogramas.clone();
        conteudo.objetos_lidos_em = registro::formatar(std::time::SystemTime::now());
        let copia = conteudo.clone();
        drop(memoria);
        Self::gravar(chave, &copia);
    }

    /// Esquece o índice de uma conexão, na memória e no disco.
    ///
    /// É o que "atualizar metadados" precisa: depois de criar ou recompilar um
    /// objeto, a lista guardada passa a mentir.
    pub fn esquecer(&self, chave: &str) {
        self.invalidar_listagens(chave);
        let _gravacao = self.gravacao.lock().unwrap_or_else(|e| e.into_inner());
        let _relacoes = self
            .gravacao_relacoes
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.relacoes_em_memoria
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(chave);
        if let Ok(caminho) = caminho_relacoes(chave) {
            if let Err(erro) = std::fs::remove_file(caminho) {
                if erro.kind() != std::io::ErrorKind::NotFound {
                    registro::aviso("indice_relacoes", erro);
                }
            }
        }
        self.travar().remove(chave);
        if let Ok(caminho) = caminho(chave) {
            if let Err(erro) = std::fs::remove_file(&caminho) {
                if erro.kind() != std::io::ErrorKind::NotFound {
                    registro::aviso("indice", format!("{chave}: não apagou o arquivo: {erro}"));
                }
            }
        }
    }

    fn travar(&self) -> std::sync::MutexGuard<'_, HashMap<String, Conteudo>> {
        self.em_memoria
            .lock()
            .unwrap_or_else(|erro| erro.into_inner())
    }

    fn invalidar_listagens(&self, chave: &str) {
        let prefixo = format!("{chave}|");
        self.listagens
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|chave, _| !chave.starts_with(&prefixo));
    }

    /// Traz o índice da conexão para a memória, lendo o disco na primeira vez.
    fn carregar<'a>(memoria: &'a mut HashMap<String, Conteudo>, chave: &str) -> &'a mut Conteudo {
        memoria
            .entry(chave.to_string())
            .or_insert_with(|| ler_do_disco(chave))
    }

    fn gravar(chave: &str, conteudo: &Conteudo) {
        if let Err(erro) = gravar_no_disco(chave, conteudo) {
            // Índice é conveniência: falhar em gravar não pode derrubar nada.
            // Mas precisa aparecer no log, senão vira "às vezes está lento".
            registro::aviso("indice", format!("{chave}: não gravou: {erro}"));
        }
    }
}

fn objetos_de_codigo(
    objetos: &[PacoteIndexado],
    filtro: Option<&Filtro>,
) -> Vec<Objeto> {
    let mut selecionados: Vec<_> = objetos
        .iter()
        .filter(|objeto| {
            filtro.map_or(true, |f| {
                (!f.excluir_gerados || objeto.gerado.as_deref() == Some("N"))
                    && f.corresponde_oracle(&objeto.nome, objeto.situacao.as_deref())
            })
        })
        .collect();
    selecionados.sort_by(|a, b| a.nome.cmp(&b.nome));
    selecionados
        .into_iter()
        .map(|objeto| Objeto {
            nome: objeto.nome.clone(),
            situacao: objeto.situacao.clone(),
            comentario: None,
        })
        .collect()
}

fn chave_da_listagem(
    chave: &str,
    categoria: crate::objetos::Categoria,
    filtro: Option<&Filtro>,
) -> String {
    let filtro = serde_json::to_string(&filtro).unwrap_or_default();
    format!("{chave}|{categoria:?}|{filtro}")
}

fn paginar(objetos: &[Objeto], deslocamento: u32, limite: u32) -> Pagina {
    let inicio = (deslocamento as usize).min(objetos.len());
    let fim = inicio.saturating_add(limite as usize).min(objetos.len());
    Pagina {
        objetos: objetos[inicio..fim].to_vec(),
        tem_mais: fim < objetos.len(),
    }
}

/// A chave de uma conexão: o perfil, com o banco atrás quando existe.
///
/// O banco entra porque no SQL Server a mesma conexão troca de banco, e os
/// objetos de um não são os do outro.
pub fn chave(perfil_id: &str, banco: Option<&str>) -> String {
    match banco.filter(|nome| !nome.is_empty()) {
        Some(nome) => format!("{perfil_id}@{nome}"),
        None => perfil_id.to_string(),
    }
}

fn caminho_relacoes(chave: &str) -> Resultado<PathBuf> {
    Ok(caminho(chave)?.with_extension("relacoes.json"))
}

fn ler_relacoes(chave: &str) -> RelacoesGuardadas {
    let inicio = std::time::Instant::now();
    let ler = || -> Resultado<RelacoesGuardadas> {
        let caminho = caminho_relacoes(chave)?;
        if !caminho.exists() {
            return Ok(RelacoesGuardadas::default());
        }
        let texto = std::fs::read(caminho)?;
        let arquivo: RelacoesEmDisco = serde_json::from_slice(&texto)?;
        if arquivo.versao != 1 {
            return Ok(RelacoesGuardadas::default());
        }
        registro::informacao(
            "indice_relacoes",
            format!(
                "{chave}: {} nome(s) do disco em {} ms",
                arquivo.usuario.len() + arquivo.outros.len(),
                inicio.elapsed().as_millis()
            ),
        );
        Ok(RelacoesGuardadas {
            usuario: Some(Arc::new(arquivo.usuario)),
            outros: Some(Arc::new(arquivo.outros)),
        })
    };
    ler().unwrap_or_else(|erro| {
        registro::aviso("indice_relacoes", format!("{chave}: não leu: {erro}"));
        RelacoesGuardadas::default()
    })
}

fn ler_do_disco(chave: &str) -> Conteudo {
    let Ok(caminho) = caminho(chave) else {
        return Conteudo::default();
    };
    let Ok(texto) = std::fs::read_to_string(&caminho) else {
        return Conteudo::default();
    };
    match serde_json::from_str::<Conteudo>(&texto) {
        // Arquivo de outra versão é descartado inteiro: ler pela metade seria
        // pior que não ler.
        Ok(conteudo) if conteudo.versao == VERSAO => {
            registro::informacao(
                "indice",
                format!(
                    "{chave}: {} rotina(s) e {} pacote(s) do disco",
                    conteudo.rotinas.len(),
                    conteudo.subprogramas.len()
                ),
            );
            conteudo
        }
        Ok(_) => Conteudo::default(),
        Err(erro) => {
            registro::aviso("indice", format!("{chave}: arquivo ilegível ({erro})"));
            Conteudo::default()
        }
    }
}

fn gravar_no_disco(chave: &str, conteudo: &Conteudo) -> Resultado<()> {
    let caminho = caminho(chave)?;
    if let Some(pasta) = caminho.parent() {
        std::fs::create_dir_all(pasta)?;
    }
    let mut gravado = conteudo.clone();
    gravado.versao = VERSAO;
    std::fs::write(&caminho, serde_json::to_string_pretty(&gravado)?)?;
    Ok(())
}

fn caminho(chave: &str) -> Resultado<PathBuf> {
    Ok(perfis::pasta_de_dados()?
        .join("indice")
        .join(format!("{}.json", nome_de_arquivo(chave))))
}

/// A chave como nome de arquivo.
///
/// O identificador do perfil é hexadecimal, mas o nome do banco vem do usuário
/// e pode ter qualquer coisa dentro — inclusive `\` e `..`, que sairiam da
/// pasta do índice. Só letra, dígito, `-`, `_` e `@` passam; o resto vira `_`.
fn nome_de_arquivo(chave: &str) -> String {
    chave
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '@') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn relacoes_nao_usam_o_recorte_legado_e_separam_conexoes() {
        let repositorio = RepositorioDeIndice::default();
        let antigo: Conteudo = serde_json::from_str(
            r#"{
            "versao":3,"objetos_lidos_em":"antes",
            "tabelas":[{"nome":"DEM_ANTIGA","tipo":"TABLE","dono":null}],
            "tabelas_indexadas":true
        }"#,
        )
        .unwrap();
        repositorio.travar().insert("legado".into(), antigo);
        repositorio
            .relacoes_em_memoria
            .lock()
            .unwrap()
            .insert("legado".into(), RelacoesGuardadas::default());
        assert!(repositorio
            .sugerir_relacoes("legado", "DEM", None, None)
            .is_empty());
        let relacao = crate::relacoes::Relacao {
            nome: "DEM_CAD_CLIENTE".into(),
            tipo: "TABLE".into(),
            dono: "MEU".into(),
            proprio: true,
            situacao: Some("VALID".into()),
            gerado: Some("N".into()),
            comentario: None,
        };
        repositorio.relacoes_em_memoria.lock().unwrap().insert(
            "novo".into(),
            RelacoesGuardadas {
                usuario: Some(Arc::new(vec![relacao])),
                outros: Some(Arc::new(vec![])),
            },
        );
        assert_eq!(
            repositorio
                .sugerir_relacoes("novo", "DEM_CAD_", Some("MEU"), None)
                .len(),
            1
        );
        assert!(repositorio
            .sugerir_relacoes("novo", "DEM_CAD_", Some("OUTRO"), None)
            .is_empty());
        assert!(repositorio
            .sugerir_relacoes("legado", "DEM_CAD_", None, None)
            .is_empty());
    }

    #[test]
    fn arvore_de_tabelas_e_views_filtra_paginação_sinonimos_e_comentarios() {
        let repositorio = RepositorioDeIndice::default();
        let relacao = |nome: &str, tipo: &str, situacao: Option<&str>, gerado: Option<&str>, comentario: Option<&str>| {
            crate::relacoes::Relacao {
                nome: nome.into(), tipo: tipo.into(), dono: "MEU".into(), proprio: true,
                situacao: situacao.map(str::to_string), gerado: gerado.map(str::to_string),
                comentario: comentario.map(str::to_string),
            }
        };
        repositorio.relacoes_em_memoria.lock().unwrap().insert(
            "arvore".into(),
            RelacoesGuardadas {
                usuario: Some(Arc::new(vec![
                    relacao("TAB_A", "TABLE", Some("VALID"), Some("N"), None),
                    relacao("TAB_B", "TABLE", Some("INVALID"), Some("N"), None),
                    relacao("TAB_G", "TABLE", Some("VALID"), Some("Y"), None),
                    relacao("TAB_S", "SYNONYM", None, None, None),
                    relacao("VW_A", "VIEW", Some("VALID"), Some("N"), Some("descrição")),
                ])),
                outros: Some(Arc::new(vec![])),
            },
        );
        let filtro: Filtro = serde_json::from_str(r#"{
            "excluirGerados":true,"incluirSinonimos":true,
            "criterios":[{"campo":"nome","operador":"comeca","valor":"TAB_"}]
        }"#).unwrap();
        let primeira = repositorio.listar_relacoes("arvore", crate::objetos::Categoria::Tabelas, Some(&filtro), 0, 2).unwrap();
        assert_eq!(primeira.objetos.iter().map(|o| o.nome.as_str()).collect::<Vec<_>>(), ["TAB_A", "TAB_B"]);
        assert!(primeira.tem_mais);
        let segunda = repositorio.listar_relacoes("arvore", crate::objetos::Categoria::Tabelas, Some(&filtro), 2, 2).unwrap();
        assert_eq!(segunda.objetos[0].nome, "TAB_S");
        assert!(!segunda.tem_mais);
        let views = repositorio.listar_relacoes("arvore", crate::objetos::Categoria::Views, None, 0, 200).unwrap();
        assert_eq!(views.objetos[0].comentario.as_deref(), Some("descrição"));
        assert!(repositorio.listar_relacoes("sem_indice", crate::objetos::Categoria::Tabelas, None, 0, 200).is_none());
    }

    #[test]
    fn procedures_filtradas_saem_do_indice() {
        let repositorio = RepositorioDeIndice::default();
        repositorio.travar().insert("procedures".into(), Conteudo {
            procedures: Some(vec![
                PacoteIndexado { nome: "PRC_A".into(), situacao: Some("VALID".into()), gerado: Some("N".into()) },
                PacoteIndexado { nome: "PRC_B".into(), situacao: Some("INVALID".into()), gerado: Some("N".into()) },
            ]), ..Conteudo::default()
        });
        let filtro: Filtro = serde_json::from_str(r#"{
            "criterios":[{"campo":"situacao","operador":"igual","valor":"VALID"}]
        }"#).unwrap();
        let pagina = repositorio.listar_procedures("procedures", Some(&filtro), 0, 200).unwrap();
        assert_eq!(pagina.objetos.len(), 1);
        assert_eq!(pagina.objetos[0].nome, "PRC_A");
    }

    #[test]
    fn pacotes_legados_nao_inventam_situacao_e_nao_incluem_procedures() {
        let repositorio = RepositorioDeIndice::default();
        let conteudo: Conteudo = serde_json::from_str(
            r#"{
            "versao":3,"objetos_lidos_em":"agora",
            "rotinas":[{"nome":"PKG_B","tipo":"PACKAGE","dono":null},
                       {"nome":"PROC_A","tipo":"PROCEDURE","dono":null},
                       {"nome":"PKG_A","tipo":"PACKAGE","dono":null}]
        }"#,
        )
        .unwrap();
        repositorio.travar().insert("teste".into(), conteudo);
        let pagina = repositorio.listar_pacotes("teste", None, 0, 1).unwrap();
        assert_eq!(pagina.objetos[0].nome, "PKG_A");
        assert!(pagina.objetos[0].situacao.is_none());
        assert!(pagina.tem_mais);
        let pagina = repositorio.listar_pacotes("teste", None, 1, 1).unwrap();
        assert_eq!(pagina.objetos[0].nome, "PKG_B");
        assert!(!pagina.tem_mais);
        let filtro: Filtro = serde_json::from_str(
            r#"{"criterios":[{"campo":"situacao","operador":"igual","valor":"VALID"}]}"#,
        )
        .unwrap();
        assert!(repositorio
            .listar_pacotes("teste", Some(&filtro), 0, 200)
            .is_none());
    }

    #[test]
    fn pacotes_completos_filtram_antes_de_paginar_e_preservam_lista_vazia() {
        let repositorio = RepositorioDeIndice::default();
        let conteudo: Conteudo = serde_json::from_str(
            r#"{
            "pacotes":[{"nome":"PKG_C","situacao":"VALID","gerado":"N"},
                       {"nome":"PKG_A","situacao":"INVALID","gerado":"N"},
                       {"nome":"PKG_B","situacao":"VALID","gerado":"Y"}]
        }"#,
        )
        .unwrap();
        repositorio.travar().insert("teste".into(), conteudo);
        let filtro: Filtro = serde_json::from_str(r#"{"excluirGerados":true,"criterios":[{"campo":"situacao","operador":"igual","valor":"VALID"}]}"#).unwrap();
        let pagina = repositorio
            .listar_pacotes("teste", Some(&filtro), 0, 1)
            .unwrap();
        assert_eq!(pagina.objetos[0].nome, "PKG_C");
        assert!(!pagina.tem_mais);
        repositorio.travar().get_mut("teste").unwrap().pacotes = Some(vec![]);
        assert!(repositorio
            .listar_pacotes("teste", None, 0, 200)
            .unwrap()
            .objetos
            .is_empty());
        repositorio
            .travar()
            .insert("sem_leitura".into(), Conteudo::default());
        assert!(repositorio
            .listar_pacotes("sem_leitura", None, 0, 200)
            .is_none());
    }

    #[test]
    fn chave_leva_o_banco_atras_do_perfil() {
        assert_eq!(chave("abc123", None), "abc123");
        assert_eq!(chave("abc123", Some("")), "abc123");
        assert_eq!(chave("abc123", Some("Vendas")), "abc123@Vendas");
    }

    #[test]
    fn nome_de_arquivo_nao_sai_da_pasta() {
        // O nome do banco vem do usuário: sem isto, `..\..\conexoes` seria um
        // caminho válido e o índice gravaria por cima de outro arquivo. O que
        // importa é a propriedade, e não a contagem de traços.
        for perigoso in ["abc@..\\..\\conexoes", "abc@a/b", "abc@..", "abc@c:\\x"] {
            let seguro = nome_de_arquivo(perigoso);
            assert!(!seguro.contains('\\'), "{seguro}");
            assert!(!seguro.contains('/'), "{seguro}");
            assert!(!seguro.contains(".."), "{seguro}");
            assert!(!seguro.contains(':'), "{seguro}");
        }
        // O que já é seguro passa inteiro: a chave normal não vira sopa de traço.
        assert_eq!(nome_de_arquivo("abc123@Vendas"), "abc123@Vendas");
        assert_eq!(nome_de_arquivo("1a2b3c"), "1a2b3c");
    }

    #[test]
    fn conteudo_vazio_e_o_padrao_de_arquivo_que_nao_existe() {
        let conteudo = ler_do_disco("conexão que não existe em disco nenhum");
        assert!(conteudo.rotinas.is_empty());
        assert!(conteudo.subprogramas.is_empty());
    }

    #[test]
    fn indice_sem_leitura_nao_responde_nada() {
        // Sem `objetos_lidos_em`, nem a lista de rotinas vale: ela estaria
        // vazia por não ter sido lida, e o autocomplete tomaria isso por "não
        // existe rotina nenhuma".
        let repositorio = RepositorioDeIndice::default();
        let chave = "conexão sem índice";
        assert!(repositorio.rotinas_que_comecam_com(chave, "").is_none());
        assert!(repositorio.rotinas_que_comecam_com(chave, "DEM").is_none());
    }

    #[test]
    fn rotinas_saem_por_prefixo_sem_distinguir_caixa() {
        let repositorio = RepositorioDeIndice::default();
        repositorio.em_memoria.lock().unwrap().insert(
            "perfil".to_string(),
            Conteudo {
                objetos_lidos_em: "agora".to_string(),
                rotinas: vec![Sugestao {
                    nome: "DEM_INT".to_string(),
                    tipo: "PACKAGE".to_string(),
                    dono: None,
                }],
                ..Conteudo::default()
            },
        );

        let achadas = |prefixo| {
            repositorio
                .rotinas_que_comecam_com("perfil", prefixo)
                .unwrap()
                .len()
        };
        assert_eq!(achadas("dem"), 1);
        // Prefixo vazio é o `EXECUTE `: a lista inteira.
        assert_eq!(achadas(""), 1);
        assert_eq!(achadas("VW_REL_"), 0);
    }

    #[test]
    fn arquivo_com_tabelas_das_versoes_anteriores_continua_valendo() {
        // Até a 0.1.72 o arquivo levava as tabelas do filtro. A versão do
        // formato não mudou por isso: descartar o arquivo faria o primeiro
        // `DEM_INT.` depois da atualização pagar de novo os vinte segundos que
        // o índice existe para evitar.
        let texto = r#"{
            "versao": 3,
            "objetos_lidos_em": "11/09/2026 18:25",
            "rotinas": [{ "nome": "DEM_INT", "tipo": "PACKAGE", "dono": null }],
            "tabelas": [{ "nome": "EXEMPLO_CLIENTE", "tipo": "TABLE", "dono": null }],
            "tabelas_indexadas": true,
            "subprogramas": {}
        }"#;
        let conteudo: Conteudo = serde_json::from_str(texto).unwrap();
        assert_eq!(conteudo.versao, VERSAO);
        assert_eq!(conteudo.rotinas.len(), 1);
    }
}
