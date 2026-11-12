//! Metadados do schema para o autocomplete (requisitos 3 e 23): os **nomes**
//! que começam por um prefixo e as **colunas** de um objeto, ambos guardados
//! por sessão.
//!
//! O alcance aqui é **maior que o da árvore de propósito**. A árvore mostra o
//! schema do usuário conectado, mas uma consulta pode ler qualquer objeto a
//! que o usuário tenha acesso — inclusive de outro dono, alcançado por
//! sinônimo público. O requisito 3 pede "tudo que comece com", e é isso.
//!
//! Toda busca é **por prefixo, com teto de linhas**, e só a partir de dois
//! caracteres digitados — vale tanto para os objetos do próprio usuário quanto
//! para os de outros donos.
//!
//! Não se lê mais o schema inteiro de uma vez. A versão anterior lia
//! `user_objects` sem teto na primeira tecla digitada e guardava tudo em
//! memória; numa base com dezenas de milhares de objetos essa leitura não
//! voltava, e o autocomplete ficava travado pelo resto da sessão sem deixar
//! rastro no log.
//!
//! Para ainda assim não ir ao banco a cada tecla, o resultado de cada prefixo
//! fica em cache por sessão. Quando um prefixo mais curto já veio **completo**
//! — sem bater no teto —, os mais longos saem dele filtrando em memória.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::erro::Resultado;
use crate::metadados_sqlserver::{self, Catalogo};
use crate::objetos::{self, Coluna, Subprograma};
use crate::perfis::Perfil;
use crate::registro;
use crate::sqlserver;

/// Quantas sugestões, no máximo, sobem para a interface de uma vez.
const LIMITE_DE_SUGESTOES: usize = 300;

/// Abaixo disso não se consulta o banco: o prefixo curto casaria com meio
/// dicionário e a espera não compensaria.
const MINIMO_PARA_BUSCAR_NO_BANCO: usize = 2;

// Os retratos em disco não usam teto; apenas as sugestões de cada prefixo.

/// Acima disso a busca é registrada no log, para não passar despercebida.
const DEMORA_QUE_MERECE_REGISTRO_MS: u128 = 500;

/// Tipos de objeto que interessam numa consulta.
const TIPOS: &str =
    "'TABLE', 'VIEW', 'PACKAGE', 'PROCEDURE', 'FUNCTION', 'SEQUENCE', 'TYPE', 'MATERIALIZED VIEW'";

/// Um nome que pode ser sugerido no editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sugestao {
    pub nome: String,
    /// `TABLE`, `VIEW`, `SYNONYM`, `PACKAGE`…
    pub tipo: String,
    /// Dono, quando não é o próprio usuário conectado.
    pub dono: Option<String>,
}

/// Uma busca já feita, guardada para as teclas seguintes.
struct Busca {
    achadas: Vec<Sugestao>,
    /// Falso quando o teto de linhas cortou o resultado. Nesse caso não se pode
    /// derivar prefixos mais longos filtrando em memória: o que ficou de fora
    /// pode ser justamente o que o prefixo mais longo pede.
    completa: bool,
}

#[derive(Default)]
struct CatalogoSqlServer {
    salvo: OnceLock<Catalogo>,
    atualizado: OnceLock<Catalogo>,
}

impl CatalogoSqlServer {
    fn atual(&self) -> Option<&Catalogo> {
        self.atualizado.get().or_else(|| self.salvo.get())
    }
}

/// Cache das buscas já feitas, por sessão e por prefixo.
#[derive(Default)]
pub struct CacheDeMetadados {
    por_sessao: Mutex<HashMap<String, HashMap<String, Busca>>>,
    /// Colunas já lidas, por sessão e por nome de objeto em maiúsculas.
    colunas_por_sessao: Mutex<HashMap<String, HashMap<String, Vec<Coluna>>>>,
    /// Subprogramas já lidos, por sessão e por nome de pacote em maiúsculas.
    subprogramas_por_sessao: Mutex<HashMap<String, HashMap<String, Vec<Subprograma>>>>,
    catalogos_sqlserver: Mutex<HashMap<String, Arc<CatalogoSqlServer>>>,
}

impl CacheDeMetadados {
    pub fn preparar_catalogo_sqlserver(
        &self,
        aplicativo: &AppHandle,
        sessao_id: &str,
        perfil: &Perfil,
        banco: &str,
    ) {
        let mut catalogos = self
            .catalogos_sqlserver
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let std::collections::hash_map::Entry::Vacant(entrada) =
            catalogos.entry(chave(sessao_id, Some(banco)))
        else {
            return;
        };
        let destino = Arc::new(CatalogoSqlServer::default());
        entrada.insert(Arc::clone(&destino));
        drop(catalogos);

        let caminho = metadados_sqlserver::caminho_do_catalogo(perfil, banco)
            .inspect_err(|erro| registro::aviso("catalogo_sqlserver", format!("{banco}: {erro}")))
            .ok();
        if let Some(caminho) = &caminho {
            let inicio = Instant::now();
            match metadados_sqlserver::ler_do_disco(caminho) {
                Ok(Some(catalogo)) => {
                    let _ = destino.salvo.set(catalogo);
                    registro::informacao(
                        "catalogo_sqlserver",
                        format!(
                            "{banco}: lido do disco em {} ms",
                            inicio.elapsed().as_millis()
                        ),
                    );
                }
                Ok(None) => {}
                Err(erro) => registro::aviso("catalogo_sqlserver", format!("{banco}: {erro}")),
            }
        }

        let perfil = perfil.clone();
        let banco = banco.to_string();
        let sessao_id = sessao_id.to_string();
        let aplicativo = aplicativo.clone();
        tauri::async_runtime::spawn(async move {
            let inicio = Instant::now();
            match sqlserver::carregar_catalogo(&perfil, &banco).await {
                Ok(Some(catalogo)) => {
                    let catalogo = destino.atualizado.get_or_init(|| catalogo);
                    let _ = aplicativo.emit("catalogo-sqlserver-atualizado", &sessao_id);
                    registro::informacao(
                        "catalogo_sqlserver",
                        format!("{banco}: atualizado em {} ms", inicio.elapsed().as_millis()),
                    );
                    if let Some(caminho) = caminho {
                        if let Err(erro) = metadados_sqlserver::guardar_no_disco(&caminho, catalogo)
                        {
                            registro::aviso("catalogo_sqlserver", format!("{banco}: {erro}"));
                        }
                    }
                }
                Ok(None) => {
                    registro::informacao(
                        "catalogo_sqlserver",
                        format!("{banco}: catálogo acima do limite"),
                    );
                }
                Err(erro) => {
                    registro::aviso("catalogo_sqlserver", format!("{banco}: {erro}"));
                }
            }
        });
    }

    pub fn colunas_no_catalogo(
        &self,
        sessao_id: &str,
        banco: &str,
        objeto: &str,
    ) -> Option<Vec<Coluna>> {
        self.catalogo_sqlserver(sessao_id, banco)?
            .atual()?
            .colunas(objeto)
    }

    fn catalogo_sqlserver(&self, sessao_id: &str, banco: &str) -> Option<Arc<CatalogoSqlServer>> {
        self.catalogos_sqlserver
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&chave(sessao_id, Some(banco)))
            .cloned()
    }

    /// A busca remota complementa a fonte local; nunca recebe um resultado
    /// parcial do índice como se fosse uma resposta completa do Oracle.
    pub fn sugerir_relacoes(
        &self,
        sessao_id: &str,
        conexao: &oracle::Connection,
        prefixo: &str,
        esquema: Option<&str>,
        filtro: Option<&crate::filtro::Filtro>,
    ) -> Resultado<Vec<Sugestao>> {
        if prefixo.trim().chars().count() < MINIMO_PARA_BUSCAR_NO_BANCO {
            return Ok(Vec::new());
        }
        let chave = format!(
            "{sessao_id}|relacoes:{}",
            serde_json::to_string(&(esquema, filtro))?
        );
        if let Some(achadas) = self.reaproveitar(&chave, prefixo) {
            return Ok(achadas);
        }
        let inicio = Instant::now();
        let proprias = crate::relacoes::ler(conexao, false, Some(prefixo), esquema, filtro)?;
        let outras = crate::relacoes::ler(conexao, true, Some(prefixo), esquema, filtro)?;
        let limite = crate::relacoes::LIMITE;
        let completa = proprias.len() <= limite && outras.len() <= limite;
        let achadas = crate::relacoes::selecionar(
            proprias.iter().chain(outras.iter()),
            prefixo,
            esquema,
            filtro,
            limite + 1,
        );
        let completa = completa && achadas.len() <= limite;
        let achadas: Vec<_> = achadas.into_iter().take(limite).collect();
        registro::informacao(
            "sugerir_relacoes_banco",
            format!(
                "{prefixo:?}: {} nome(s) em {} ms",
                achadas.len(),
                inicio.elapsed().as_millis()
            ),
        );
        self.guardar(&chave, prefixo, &achadas, completa);
        Ok(achadas)
    }

    /// Descarta o cache da sessão, para reler após criar ou remover objetos.
    ///
    /// No SQL Server a chave leva o banco atrás da sessão (ver [`chave`]), então
    /// não basta remover a entrada de nome igual: saem todas as que começam pelo
    /// identificador da sessão. Criar uma tabela num banco não pode deixar o
    /// autocomplete cego para ela porque o cache dela mora sob outro nome.
    pub fn invalidar(&self, sessao_id: &str) {
        let prefixo = format!("{sessao_id}|");
        let derivada = |chave: &String| chave == sessao_id || chave.starts_with(&prefixo);
        self.catalogos_sqlserver
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|chave, _| !derivada(chave));
        let mut mapa = self.por_sessao.lock().unwrap_or_else(|e| e.into_inner());
        mapa.retain(|chave, _| !derivada(chave));
        let mut colunas = self
            .colunas_por_sessao
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        colunas.retain(|chave, _| !derivada(chave));
        // Recompilar um pacote muda os subprogramas dele: manter os antigos
        // faria o autocomplete oferecer o que acabou de sair.
        let mut subprogramas = self
            .subprogramas_por_sessao
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        subprogramas.retain(|chave, _| !derivada(chave));
    }

    /// Colunas de um objeto do SQL Server, guardadas como as do Oracle.
    ///
    /// A leitura muda de catálogo; o cache, não. É de propósito: o que a aba faz
    /// com o resultado é idêntico, e duplicar o cache seria duplicar também a
    /// invalidação e o escopo — os dois lugares em que um cache erra.
    pub fn colunas_sqlserver(
        &self,
        sessao_id: &str,
        sessao: &sqlserver::Sessao,
        banco: Option<&str>,
        objeto: &str,
    ) -> Resultado<Vec<Coluna>> {
        let chave_da_sessao = chave(sessao_id, banco);
        let chave_do_objeto = objeto.trim().to_uppercase();
        if let Some(guardadas) = self.colunas_guardadas(&chave_da_sessao, &chave_do_objeto) {
            return Ok(guardadas);
        }

        let inicio = Instant::now();
        let lidas = sessao.colunas(banco, objeto).inspect_err(|erro| {
            registro::aviso("listar_colunas", format!("{objeto}: {erro}"));
        })?;
        registro::informacao(
            "listar_colunas",
            format!(
                "{objeto}: {} coluna(s) em {} ms",
                lidas.len(),
                inicio.elapsed().as_millis()
            ),
        );

        self.guardar_colunas(&chave_da_sessao, &chave_do_objeto, &lidas);
        Ok(lidas)
    }

    /// Sugestões do catálogo do SQL Server para o prefixo digitado.
    ///
    /// `esquema` é a segunda parte do nome de três partes (`Vendas.dbo.cli`), e
    /// entra na chave do cache pelo mesmo motivo que o banco: a lista de `dbo.`
    /// não é a lista de `rh.`, e servir uma no lugar da outra é a forma mais
    /// rápida de o autocomplete deixar de ser confiável.
    /// `filtro_de_tabelas` é a opção **Tabelas** do requisito 43, e chega
    /// preenchida só quando ela pede o recorte. Ela entra na chave do cache
    /// junto com o escopo: o resultado guardado de "todas" não pode responder a
    /// "apenas as filtradas", nem o contrário.
    pub fn sugerir_sqlserver(
        &self,
        sessao_id: &str,
        sessao: &sqlserver::Sessao,
        banco: Option<&str>,
        esquema: Option<&str>,
        prefixo: &str,
        filtro_de_tabelas: Option<&crate::filtro::Filtro>,
        apenas_tabelas_filtradas: bool,
    ) -> Resultado<Vec<Sugestao>> {
        let prefixo = prefixo.trim();
        if prefixo.chars().count() < MINIMO_PARA_BUSCAR_NO_BANCO {
            return Ok(Vec::new());
        }
        let chave_da_sessao = format!(
            "{}|tabelas:{}",
            chave_do_escopo(sessao_id, banco, esquema),
            if apenas_tabelas_filtradas {
                "filtradas"
            } else {
                "todas"
            }
        );
        if let Some(reaproveitadas) = self.reaproveitar(&chave_da_sessao, prefixo) {
            return Ok(reaproveitadas);
        }
        if filtro_de_tabelas.is_none() {
            if let Some(catalogo) =
                banco.and_then(|banco| self.catalogo_sqlserver(sessao_id, banco))
            {
                if let Some(catalogo) = catalogo.atual() {
                    if let Some(achadas) = catalogo.sugerir(prefixo, esquema) {
                        return Ok(achadas);
                    }
                }
            }
        }

        let inicio = Instant::now();
        let achadas = sessao.sugerir(banco, esquema, prefixo, filtro_de_tabelas)?;
        let demora = inicio.elapsed().as_millis();
        if demora > DEMORA_QUE_MERECE_REGISTRO_MS || achadas.is_empty() {
            registro::informacao(
                "sugerir_objetos",
                format!(
                    "\"{}\": {} objeto(s) em {demora} ms",
                    nome_pedido(banco, esquema, prefixo),
                    achadas.len()
                ),
            );
        }

        // Bater no teto significa que há nomes fora do resultado; aí este prefixo
        // não serve de base para os maiores.
        let completa = achadas.len() < LIMITE_DE_SUGESTOES;
        self.guardar(&chave_da_sessao, prefixo, &achadas, completa);
        Ok(achadas)
    }

    /// Colunas de uma tabela ou view, lidas uma vez por sessão.
    ///
    /// Antes a memória era **de cada aba**: abrir a segunda aba na mesma
    /// conexão pagava a leitura de novo, e a árvore de objetos pagava a sua à
    /// parte, para as mesmas colunas. Aqui o escopo é a sessão, então quem
    /// chegar depois não espera.
    ///
    /// O resultado vazio de um nome que não existe **não** fica guardado: quem
    /// insiste num nome errado paga pouco — um parse que falha —, e guardar o
    /// erro faria a coluna de uma tabela criada agora não aparecer nunca.
    pub fn colunas(
        &self,
        sessao_id: &str,
        conexao: &oracle::Connection,
        objeto: &str,
    ) -> Resultado<Vec<Coluna>> {
        let chave = objeto.trim().to_uppercase();
        if let Some(guardadas) = self.colunas_guardadas(sessao_id, &chave) {
            return Ok(guardadas);
        }

        let inicio = Instant::now();
        let lidas = objetos::colunas(conexao, objeto).inspect_err(|erro| {
            // Nome que não é tabela é rotina: o editor pede as colunas de tudo
            // que se pareça com uma. Fica como aviso, e não como erro, mas fica
            // — é a resposta à pergunta "por que não sugeriu nada aqui?".
            registro::aviso("listar_colunas", format!("{objeto}: {erro}"));
        })?;

        // Uma linha por tabela e por sessão: é o que o cache permite registrar
        // sem encher o log, e é onde se vê quanto custou e quantas colunas
        // vieram — sem isso não dá para separar "demorou" de "não achou".
        registro::informacao(
            "listar_colunas",
            format!(
                "{objeto}: {} coluna(s) em {} ms",
                lidas.len(),
                inicio.elapsed().as_millis()
            ),
        );

        self.guardar_colunas(sessao_id, &chave, &lidas);
        Ok(lidas)
    }

    /// Subprogramas de um pacote, guardados como as colunas de uma tabela.
    ///
    /// O cache aqui pesa mais que nos outros casos: a leitura passa por
    /// `USER_ARGUMENTS`, que numa base de ERP é a consulta mais cara que o
    /// autocomplete faz. Guardar por sessão, e não por aba, é o que faz o
    /// `PKG.` de uma aba nova abrir na hora depois de o pacote ter sido pedido
    /// uma vez em qualquer outra.
    ///
    /// O erro **não** fica guardado, pelo mesmo motivo das colunas: o editor
    /// pede os subprogramas de todo nome com ponto atrás, e um nome que não é
    /// pacote responde lista vazia — que fica, porque é resposta legítima e
    /// evita repetir a leitura cara a cada tecla.
    pub fn subprogramas(
        &self,
        sessao_id: &str,
        conexao: &oracle::Connection,
        pacote: &str,
    ) -> Resultado<Vec<Subprograma>> {
        let chave = pacote.trim().to_uppercase();
        if let Some(guardados) = self.subprogramas_guardados(sessao_id, &chave) {
            return Ok(guardados);
        }

        let inicio = Instant::now();
        let lidos = objetos::subprogramas(conexao, pacote).inspect_err(|erro| {
            registro::aviso("listar_subprogramas", format!("{pacote}: {erro}"));
        })?;
        registro::informacao(
            "listar_subprogramas",
            format!(
                "{pacote}: {} subprograma(s) em {} ms",
                lidos.len(),
                inicio.elapsed().as_millis()
            ),
        );

        let mut mapa = self
            .subprogramas_por_sessao
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        mapa.entry(sessao_id.to_string())
            .or_default()
            .insert(chave, lidos.clone());
        Ok(lidos)
    }

    fn subprogramas_guardados(&self, sessao_id: &str, chave: &str) -> Option<Vec<Subprograma>> {
        let mapa = self
            .subprogramas_por_sessao
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Some(mapa.get(sessao_id)?.get(chave)?.clone())
    }

    fn colunas_guardadas(&self, sessao_id: &str, chave: &str) -> Option<Vec<Coluna>> {
        let mapa = self
            .colunas_por_sessao
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Some(mapa.get(sessao_id)?.get(chave)?.clone())
    }

    fn guardar_colunas(&self, sessao_id: &str, chave: &str, colunas: &[Coluna]) {
        let mut mapa = self
            .colunas_por_sessao
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        mapa.entry(sessao_id.to_string())
            .or_default()
            .insert(chave.to_string(), colunas.to_vec());
    }

    /// Aproveita uma busca anterior, quando dá: a do mesmo prefixo, ou a do
    /// prefixo mais longo que já veio completo e contém este.
    fn reaproveitar(&self, sessao_id: &str, prefixo: &str) -> Option<Vec<Sugestao>> {
        let mapa = self.por_sessao.lock().unwrap_or_else(|e| e.into_inner());
        let buscas = mapa.get(sessao_id)?;
        let alvo = prefixo.to_uppercase();

        // Quanto mais longo o prefixo já buscado, menos sobra para filtrar.
        let mut escolhida: Option<(usize, &Busca)> = None;
        for (feito, busca) in buscas {
            if !alvo.starts_with(feito.as_str()) || !(busca.completa || *feito == alvo) {
                continue;
            }
            if escolhida.map_or(true, |(tamanho, _)| feito.len() > tamanho) {
                escolhida = Some((feito.len(), busca));
            }
        }

        let (_, busca) = escolhida?;
        Some(filtrar(&busca.achadas, &alvo))
    }

    fn guardar(&self, sessao_id: &str, prefixo: &str, achadas: &[Sugestao], completa: bool) {
        let mut mapa = self.por_sessao.lock().unwrap_or_else(|e| e.into_inner());
        mapa.entry(sessao_id.to_string()).or_default().insert(
            prefixo.to_uppercase(),
            Busca {
                achadas: achadas.to_vec(),
                completa,
            },
        );
    }

    /// Sugestões para o prefixo digitado.
    ///
    /// Os objetos do próprio usuário vêm primeiro; depois os de outros donos e
    /// os sinônimos públicos.
    ///
    /// `do_indice` são as rotinas do usuário guardadas em disco (requisito 40),
    /// e só chegam fora de lugar de tabela. Com elas a resposta sai inteira da
    /// memória, sem ir ao banco.
    ///
    /// `filtro_de_tabelas` chega preenchido na opção "apenas as filtradas"
    /// (requisito 43), e vale só para as linhas de tabela: pacote, procedure,
    /// função, view e sinônimo não são da categoria Tabelas e passam intactos.
    ///
    /// A chave do cache vem de [`chave_de_sugestoes`]: a resposta de uma opção
    /// não pode atender a outra, nem a lista de rotinas o lugar de tabela.
    pub fn sugerir(
        &self,
        chave_da_sessao: &str,
        conexao: &oracle::Connection,
        prefixo: &str,
        do_indice: Option<&[Sugestao]>,
        filtro_de_tabelas: Option<&crate::filtro::Filtro>,
    ) -> Resultado<Vec<Sugestao>> {
        let prefixo = prefixo.trim();
        let curto = prefixo.chars().count() < MINIMO_PARA_BUSCAR_NO_BANCO;
        // Prefixo curto não vai ao banco — casaria com meio dicionário. Mas o
        // índice responde de memória, e é ele que faz `EXECUTE ` listar as
        // rotinas com prefixo nenhum.
        if curto && do_indice.is_none() {
            return Ok(Vec::new());
        }
        if let Some(reaproveitadas) = self.reaproveitar(chave_da_sessao, prefixo) {
            return Ok(reaproveitadas);
        }

        let inicio = Instant::now();
        let proprios = match do_indice {
            Some(objetos) => filtrar(objetos, &prefixo.to_uppercase()),
            None => buscar_do_usuario(conexao, prefixo, filtro_de_tabelas)?,
        };
        // Com índice, a resposta precisa vir só da memória. Esperar aqui a
        // busca em outros donos devolveria os mesmos segundos que ele evita.
        let alheios = if curto || do_indice.is_some() {
            Vec::new()
        } else {
            buscar_de_outros_donos(conexao, prefixo, filtro_de_tabelas)?
        };
        let demora = inicio.elapsed().as_millis();
        // Registra quando demora ou quando não acha nada: são justamente os
        // dois casos em que o usuário estranha o autocomplete.
        if demora > DEMORA_QUE_MERECE_REGISTRO_MS || (proprios.is_empty() && alheios.is_empty()) {
            registro::informacao(
                "sugerir_objetos",
                format!(
                    "\"{prefixo}\": {} do usuário + {} de outros donos em {demora} ms",
                    proprios.len(),
                    alheios.len()
                ),
            );
        }

        // Bater no teto em qualquer das duas consultas significa que há nomes
        // fora do resultado; aí este prefixo não serve de base para os maiores.
        let cortou = proprios.len() >= LIMITE_DE_SUGESTOES || alheios.len() >= LIMITE_DE_SUGESTOES;

        let mut achadas = proprios;
        achadas.extend(alheios);
        // Um mesmo nome pode vir como tabela e como sinônimo: fica o primeiro,
        // que é o do próprio usuário.
        achadas.dedup_by(|a, b| a.nome.eq_ignore_ascii_case(&b.nome));
        achadas.truncate(LIMITE_DE_SUGESTOES);

        self.guardar(
            chave_da_sessao,
            prefixo,
            &achadas,
            !cortou && achadas.len() < LIMITE_DE_SUGESTOES,
        );
        Ok(achadas)
    }
}

/// Tipos que se **chamam**: é o que o autocomplete oferece fora de lugar de
/// tabela, e o que `EXECUTE ` precisa listar inteiro.
const TIPOS_DE_ROTINA: &str = "'PACKAGE', 'PROCEDURE', 'FUNCTION'";

/// O acervo do schema do usuário, para o índice local (requisito 40).
///
/// São dois grupos, e os dois são do que se **chama**:
///
/// - As **rotinas** vão inteiras, sem filtro. São poucas e são o que se digita
///   depois de `EXECUTE`, onde a lista precisa aparecer sem prefixo nenhum.
/// - Os membros das packages são lidos em lote para `PACOTE.` também responder
///   imediatamente.
///
/// Tabela não entra: o lugar de tabela precisa também de view, de sinônimo e
/// do que é de outro dono, e é respondido pela consulta por prefixo.
pub struct AcervoDoUsuario {
    pub pacotes: Vec<crate::indice::PacoteIndexado>,
    pub procedures: Vec<crate::indice::PacoteIndexado>,
    pub rotinas: Vec<Sugestao>,
    pub subprogramas: HashMap<String, Vec<Subprograma>>,
}

/// Lê o acervo do usuário.
pub fn acervo_do_usuario(conexao: &oracle::Connection) -> Resultado<AcervoDoUsuario> {
    let sql = format!("SELECT DISTINCT object_name, object_type, status, generated FROM user_objects WHERE object_type IN ({TIPOS_DE_ROTINA}) ORDER BY object_name");
    let mut rotinas = Vec::new();
    let mut pacotes = Vec::new();
    let mut procedures = Vec::new();
    for linha in conexao.query(&sql, &[])? {
        let linha = linha?;
        let nome: String = linha.get(0)?;
        let tipo: String = linha.get(1)?;
        if tipo == "PACKAGE" {
            pacotes.push(crate::indice::PacoteIndexado {
                nome: nome.clone(),
                situacao: linha.get(2)?,
                gerado: linha.get(3)?,
            });
        } else if tipo == "PROCEDURE" {
            procedures.push(crate::indice::PacoteIndexado {
                nome: nome.clone(),
                situacao: linha.get(2)?,
                gerado: linha.get(3)?,
            });
        }
        rotinas.push(Sugestao {
            nome,
            tipo,
            dono: None,
        });
    }
    let subprogramas = subprogramas_de_todas_as_packages(conexao)?;

    Ok(AcervoDoUsuario {
        pacotes,
        procedures,
        rotinas,
        subprogramas,
    })
}

/// Lê os membros de todas as packages em duas consultas, não uma por package.
fn subprogramas_de_todas_as_packages(
    conexao: &oracle::Connection,
) -> Resultado<HashMap<String, Vec<Subprograma>>> {
    let funcoes: std::collections::HashSet<(String, String)> = conexao
        .query_as::<(String, String)>(
            "SELECT DISTINCT package_name, object_name FROM user_arguments \
              WHERE package_name IS NOT NULL AND position = 0",
            &[],
        )?
        .collect::<Result<_, _>>()?;
    let linhas = conexao.query_as::<(String, String)>(
        "SELECT DISTINCT object_name, procedure_name FROM user_procedures \
          WHERE object_type = 'PACKAGE' AND procedure_name IS NOT NULL \
          ORDER BY object_name, procedure_name",
        &[],
    )?;
    let mut por_pacote: HashMap<String, Vec<Subprograma>> = HashMap::new();
    for linha in linhas {
        let (pacote, nome) = linha?;
        let especie = if funcoes.contains(&(pacote.clone(), nome.clone())) {
            "FUNCTION"
        } else {
            "PROCEDURE"
        };
        por_pacote.entry(pacote).or_default().push(Subprograma {
            nome,
            especie: especie.to_string(),
        });
    }
    Ok(por_pacote)
}

/// Objetos do schema do usuário que começam pelo prefixo, com os sinônimos
/// privados.
///
/// Mesma forma da busca em outros donos, e pelo mesmo motivo: predicado por
/// prefixo direto na coluna, sem função em volta, para o Oracle poder usar o
/// índice do dicionário, e teto de linhas para o custo não depender do tamanho
/// do schema.
fn buscar_do_usuario(
    conexao: &oracle::Connection,
    prefixo: &str,
    filtro_de_tabelas: Option<&crate::filtro::Filtro>,
) -> Resultado<Vec<Sugestao>> {
    let recorte = recorte_de_tabelas(filtro_de_tabelas);
    let sql = sql_do_usuario(&recorte);

    let padrao = padrao_de_prefixo(prefixo);
    let valores = com_o_prefixo(&padrao, &recorte);
    let linhas = conexao.query_as_named::<(String, String)>(&sql, &valores)?;

    linhas
        .map(|linha| {
            let (nome, tipo) = linha?;
            Ok(Sugestao {
                nome,
                tipo,
                dono: None,
            })
        })
        .collect()
}

/// A consulta dos objetos do próprio usuário, com o recorte já embutido.
///
/// Separada da execução porque é a parte que dá para conferir sem banco: o
/// recorte entra no meio de um `UNION ALL`, e um pedaço fora de lugar só
/// apareceria como `ORA-00933` na cara de quem está digitando.
fn sql_do_usuario(recorte: &crate::filtro::Condicao) -> String {
    format!(
        "SELECT * FROM ( \
           SELECT object_name AS nome, object_type AS tipo \
             FROM user_objects \
            WHERE object_type IN ({TIPOS}) \
              AND object_name LIKE :prefixo ESCAPE '\\'{} \
           UNION ALL \
           SELECT synonym_name, 'SYNONYM' \
             FROM user_synonyms \
            WHERE synonym_name LIKE :prefixo ESCAPE '\\' \
         ) ORDER BY nome \
         FETCH FIRST {LIMITE_DE_SUGESTOES} ROWS ONLY",
        recorte.sql
    )
}

/// A consulta dos objetos de outros donos, com o mesmo recorte.
fn sql_de_outros_donos(recorte: &crate::filtro::Condicao) -> String {
    format!(
        "SELECT * FROM ( \
           SELECT object_name AS nome, object_type AS tipo, owner AS dono \
             FROM all_objects \
            WHERE owner <> USER AND object_type IN ({TIPOS}) \
              AND object_name LIKE :prefixo ESCAPE '\\'{} \
           UNION ALL \
           SELECT synonym_name, 'SYNONYM', owner \
             FROM all_synonyms \
            WHERE owner <> USER AND synonym_name LIKE :prefixo ESCAPE '\\' \
         ) ORDER BY CASE WHEN dono = 'PUBLIC' THEN 0 ELSE 1 END, nome \
         FETCH FIRST {LIMITE_DE_SUGESTOES} ROWS ONLY",
        recorte.sql
    )
}

/// O predicado que restringe **só as linhas de tabela** ao filtro da conexão.
///
/// Sai vazio quando não há filtro, e aí o SQL fica idêntico ao de sempre: a
/// opção **Todas** (requisito 43) não paga nada por a outra existir.
///
/// A forma é `AND (object_type <> 'TABLE' OR (…))` porque a consulta traz oito
/// tipos de objeto numa lista só. O filtro é o da **categoria Tabelas** da
/// árvore (requisito 4); aplicá-lo a um pacote, a uma view ou a um sinônimo
/// seria usar um recorte para descrever o que ele não descreve.
pub(crate) fn recorte_de_tabelas(
    filtro: Option<&crate::filtro::Filtro>,
) -> crate::filtro::Condicao {
    let vazio = crate::filtro::Condicao {
        sql: String::new(),
        valores: Vec::new(),
    };
    let Some(filtro) = filtro else {
        return vazio;
    };
    let condicao = filtro.condicao("object_name", "status");
    let mut partes: Vec<String> = Vec::new();
    if filtro.excluir_gerados {
        partes.push("generated = 'N'".to_string());
    }
    if !condicao.sql.is_empty() {
        partes.push(format!("({})", condicao.sql));
    }
    if partes.is_empty() {
        return vazio;
    }
    crate::filtro::Condicao {
        sql: format!(
            " AND (object_type <> 'TABLE' OR ({}))",
            partes.join(" AND ")
        ),
        valores: condicao.valores,
    }
}

/// Os binds da consulta: o prefixo digitado mais os do recorte, se houver.
fn com_o_prefixo<'a>(
    padrao: &'a String,
    recorte: &'a crate::filtro::Condicao,
) -> Vec<(&'a str, &'a dyn oracle::sql_type::ToSql)> {
    let mut valores: Vec<(&str, &dyn oracle::sql_type::ToSql)> = vec![("prefixo", padrao)];
    valores.extend(
        recorte
            .valores
            .iter()
            .map(|(nome, valor)| (nome.as_str(), valor as &dyn oracle::sql_type::ToSql)),
    );
    valores
}

/// `EXEMPLO_` vira `EXEMPLO\_%`.
///
/// O sublinhado é literal: quem digita `EXEMPLO_` quer o caractere, não o curinga
/// de uma posição do `LIKE`.
fn padrao_de_prefixo(prefixo: &str) -> String {
    format!(
        "{}%",
        prefixo
            .to_uppercase()
            .replace('\\', "\\\\")
            .replace('_', "\\_")
    )
}

/// Objetos de outros donos que o usuário enxerga, e sinônimos públicos.
///
/// O predicado é por prefixo direto na coluna, sem função em volta, para o
/// Oracle poder usar o índice do dicionário.
fn buscar_de_outros_donos(
    conexao: &oracle::Connection,
    prefixo: &str,
    filtro_de_tabelas: Option<&crate::filtro::Filtro>,
) -> Resultado<Vec<Sugestao>> {
    // O mesmo recorte das tabelas do próprio usuário: quem escolheu "apenas as
    // filtradas" não espera que a tabela de outro schema escape da regra só por
    // ser de outro dono.
    let recorte = recorte_de_tabelas(filtro_de_tabelas);
    let sql = sql_de_outros_donos(&recorte);

    let padrao = padrao_de_prefixo(prefixo);
    let valores = com_o_prefixo(&padrao, &recorte);
    let linhas = conexao.query_as_named::<(String, String, String)>(&sql, &valores)?;

    linhas
        .map(|linha| {
            let (nome, tipo, dono) = linha?;
            Ok(Sugestao {
                nome,
                tipo,
                dono: Some(dono),
            })
        })
        .collect()
}

/// A chave do cache: a sessão e, no SQL Server, o banco que a aba está usando.
///
/// Sem o banco, trocar de banco na barra da aba continuaria oferecendo as
/// tabelas do banco anterior — que é a forma mais rápida de o autocomplete
/// deixar de ser confiável. No Oracle não há banco, e a chave é só a sessão.
pub fn chave(sessao_id: &str, banco: Option<&str>) -> String {
    match banco {
        Some(banco) => format!("{sessao_id}|{banco}"),
        None => sessao_id.to_string(),
    }
}

/// O nome como o editor o escreveu, para o log dizer de que banco e de que
/// esquema se falava: `Vendas.dbo.tbl`.
///
/// Sem isto, a linha de uma busca que não achou nada registra só o prefixo, e a
/// pergunta seguinte — "em que banco ele procurou?" — fica sem resposta no log,
/// que é onde este projeto responde as perguntas de defeito.
fn nome_pedido(banco: Option<&str>, esquema: Option<&str>, prefixo: &str) -> String {
    match (banco, esquema) {
        (None, None) => prefixo.to_string(),
        (Some(banco), None) => format!("{banco}..{prefixo}"),
        (None, Some(esquema)) => format!("{esquema}.{prefixo}"),
        (Some(banco), Some(esquema)) => format!("{banco}.{esquema}.{prefixo}"),
    }
}

/// A chave do cache quando o nome vem qualificado por esquema.
///
/// O ponto na frente do esquema é o que impede a colisão com um banco de nome
/// parecido: a chave de um banco nunca tem `|.` dentro. Continua começando pelo
/// identificador da sessão, que é por onde [`CacheDeMetadados::invalidar`] varre.
fn chave_do_escopo(sessao_id: &str, banco: Option<&str>, esquema: Option<&str>) -> String {
    match esquema {
        Some(esquema) => format!("{}|.{esquema}", chave(sessao_id, banco)),
        None => chave(sessao_id, banco),
    }
}

/// A chave do cache de sugestões do Oracle.
///
/// A opção **Tabelas** (requisito 43) entra na chave: o resultado guardado de
/// "todas" não pode responder a "apenas as filtradas", nem o contrário.
///
/// A resposta do índice local tem chave própria porque é **um pedaço**: só
/// rotinas, sem tabela e sem view. Na mesma chave, e marcada completa como vem,
/// ela respondia pelo lugar de tabela — depois de um `SELECT V`, o
/// `FROM VW_REL_` saía dela sem view nenhuma. Todas começam pela sessão e pelo
/// `|`, que é por onde [`CacheDeMetadados::invalidar`] varre.
pub fn chave_de_sugestoes(
    sessao_id: &str,
    resposta_do_indice: bool,
    apenas_tabelas_filtradas: bool,
) -> String {
    if resposta_do_indice {
        format!("{sessao_id}|rotinas")
    } else if apenas_tabelas_filtradas {
        format!("{sessao_id}|tabelas:filtradas")
    } else {
        format!("{sessao_id}|tabelas:todas")
    }
}

/// Filtra por prefixo, sem diferenciar maiúsculas de minúsculas.
///
/// É o requisito 3: digitar `TBL_` ou `tbl_` traz o mesmo conjunto.
pub fn filtrar(nomes: &[Sugestao], prefixo: &str) -> Vec<Sugestao> {
    let prefixo = prefixo.trim().to_uppercase();
    nomes
        .iter()
        .filter(|sugestao| prefixo.is_empty() || sugestao.nome.to_uppercase().starts_with(&prefixo))
        .take(LIMITE_DE_SUGESTOES)
        .cloned()
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_lista_de_rotinas_do_indice_nao_responde_pelo_lugar_de_tabela() {
        // Fora de lugar de tabela o índice devolve só rotinas, e a lista vem
        // marcada completa. Guardada na chave do `FROM`, o "V" de um `SELECT V`
        // respondia pelo `FROM VW_REL_` seguinte, e a view não aparecia.
        let cache = CacheDeMetadados::default();
        let rotinas = [Sugestao {
            nome: "VALIDA_CPF".to_string(),
            tipo: "FUNCTION".to_string(),
            dono: None,
        }];
        let das_rotinas = chave_de_sugestoes("s1", true, false);
        cache.guardar(&das_rotinas, "V", &rotinas, true);

        for apenas_filtradas in [false, true] {
            let do_from = chave_de_sugestoes("s1", false, apenas_filtradas);
            assert!(cache.reaproveitar(&do_from, "VW_REL_").is_none());
        }
        // A opção Tabelas continua separando as duas respostas do `FROM`.
        assert_ne!(
            chave_de_sugestoes("s1", false, false),
            chave_de_sugestoes("s1", false, true)
        );
        // Onde a lista de rotinas vale, continua valendo.
        let reaproveitada = cache.reaproveitar(&das_rotinas, "VA");
        assert_eq!(reaproveitada.map(|lista| lista.len()), Some(1));

        // E continua sendo da sessão: `invalidar` a leva junto.
        cache.invalidar("s1");
        assert!(cache.reaproveitar(&das_rotinas, "VA").is_none());
    }

    fn exemplo() -> Vec<Sugestao> {
        [
            "TBL_CLIENTE",
            "TBL_PEDIDO",
            "tbl_minusculo",
            "VW_CLIENTE",
            "PKG_FATURA",
        ]
        .into_iter()
        .map(|nome| Sugestao {
            nome: nome.to_string(),
            tipo: "TABLE".to_string(),
            dono: None,
        })
        .collect()
    }

    /// Um filtro de Tabelas como o que a árvore grava.
    fn filtro_de_tabelas(excluir_gerados: bool, valor: &str) -> crate::filtro::Filtro {
        crate::filtro::Filtro {
            criterios: if valor.is_empty() {
                Vec::new()
            } else {
                vec![crate::filtro::Criterio {
                    campo: crate::filtro::Campo::Nome,
                    operador: crate::filtro::Operador::Comeca,
                    valor: valor.to_string(),
                    distingue_maiusculas: false,
                }]
            },
            corresponder_todos: false,
            excluir_gerados,
            incluir_sinonimos: false,
        }
    }

    #[test]
    fn sem_filtro_o_recorte_nao_muda_a_consulta() {
        // A opção "todas" não pode custar um caractere de SQL a mais: é o
        // caminho de sempre, e ele é o padrão.
        let recorte = recorte_de_tabelas(None);
        assert!(recorte.sql.is_empty());
        assert!(recorte.valores.is_empty());
    }

    #[test]
    fn o_recorte_alcanca_a_tabela_e_poupa_o_resto() {
        let filtro = filtro_de_tabelas(false, "EXEMPLO");
        let recorte = recorte_de_tabelas(Some(&filtro));

        // A guarda `object_type <> 'TABLE'` é o que impede o filtro da
        // categoria Tabelas de recortar pacote, view e sequência junto.
        assert!(recorte.sql.starts_with(" AND (object_type <> 'TABLE' OR ("));
        assert!(recorte.sql.ends_with("))"));
        // O valor procurado vira bind, nunca texto concatenado no SQL.
        assert_eq!(recorte.valores.len(), 1);
        assert!(!recorte.sql.contains("EXEMPLO"));
    }

    #[test]
    fn excluir_gerados_entra_no_recorte_mesmo_sem_criterio() {
        // "Sem objetos gerados" sozinho já é um filtro ativo, e precisa valer
        // para o autocomplete como vale para a árvore.
        let filtro = filtro_de_tabelas(true, "");
        let recorte = recorte_de_tabelas(Some(&filtro));

        assert!(recorte.sql.contains("generated = 'N'"));
        assert!(recorte.valores.is_empty());
    }

    /// Parênteses equilibrados: o teste que pega o recorte no lugar errado.
    fn equilibrada(sql: &str) -> bool {
        let mut abertos = 0i32;
        for caractere in sql.chars() {
            match caractere {
                '(' => abertos += 1,
                ')' => abertos -= 1,
                _ => {}
            }
            if abertos < 0 {
                return false;
            }
        }
        abertos == 0
    }

    #[test]
    fn a_consulta_de_sugestoes_fica_valida_com_e_sem_recorte() {
        let filtro = filtro_de_tabelas(true, "EXEMPLO");
        let com = recorte_de_tabelas(Some(&filtro));
        let sem = recorte_de_tabelas(None);

        for sql in [
            sql_do_usuario(&com),
            sql_do_usuario(&sem),
            sql_de_outros_donos(&com),
            sql_de_outros_donos(&sem),
        ] {
            assert!(equilibrada(&sql), "parênteses desequilibrados: {sql}");
            // O recorte é do primeiro `SELECT`, e precisa ficar antes do
            // `UNION ALL`: depois dele estaria recortando os sinônimos.
            let uniao = sql.find("UNION ALL").expect("sem UNION ALL");
            if let Some(recorte) = sql.find("object_type <> 'TABLE'") {
                assert!(recorte < uniao, "recorte caiu depois do UNION ALL: {sql}");
            }
            // O `FETCH FIRST` fecha a consulta: o recorte não pode empurrá-lo
            // para dentro da subconsulta.
            assert!(sql.trim_end().ends_with("ROWS ONLY"), "{sql}");
        }

        // Sem filtro, a consulta é exatamente a de sempre.
        assert!(!sql_do_usuario(&sem).contains("object_type <> 'TABLE'"));
        assert!(sql_do_usuario(&com).contains("generated = 'N'"));
    }

    #[test]
    fn filtra_por_prefixo_ignorando_caixa() {
        let nomes = exemplo();

        let maiusculo: Vec<_> = filtrar(&nomes, "TBL_")
            .into_iter()
            .map(|s| s.nome)
            .collect();
        let minusculo: Vec<_> = filtrar(&nomes, "tbl_")
            .into_iter()
            .map(|s| s.nome)
            .collect();

        assert_eq!(maiusculo, minusculo);
        assert_eq!(maiusculo, ["TBL_CLIENTE", "TBL_PEDIDO", "tbl_minusculo"]);
    }

    #[test]
    fn prefixo_vazio_traz_tudo() {
        assert_eq!(filtrar(&exemplo(), "").len(), 5);
        assert_eq!(filtrar(&exemplo(), "   ").len(), 5);
    }

    #[test]
    fn prefixo_sem_correspondencia_nao_traz_nada() {
        assert!(filtrar(&exemplo(), "ZZZ").is_empty());
    }
}
