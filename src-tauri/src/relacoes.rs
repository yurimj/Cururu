//! Tabelas, views e sinônimos do autocomplete Oracle.
//! O retrato local e a consulta por prefixo têm o mesmo escopo e os mesmos filtros.

use crate::{erro::Resultado, filtro::Filtro, metadados::Sugestao};
use serde::{Deserialize, Serialize};

pub const LIMITE: usize = 300;
const TIPOS: &str = "'TABLE', 'VIEW', 'MATERIALIZED VIEW'";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relacao {
    pub nome: String,
    pub tipo: String,
    pub dono: String,
    pub proprio: bool,
    pub situacao: Option<String>,
    pub gerado: Option<String>,
    #[serde(default)]
    pub comentario: Option<String>,
}

/// Acrescenta os comentários das views do usuário ao retrato que vai ao disco.
pub fn anexar_comentarios_de_views(
    conexao: &oracle::Connection,
    relacoes: &mut [Relacao],
) -> Resultado<()> {
    let comentarios: std::collections::HashMap<String, String> = conexao
        .query_as::<(String, String)>(
            "SELECT table_name, comments FROM user_tab_comments \
             WHERE table_type = 'VIEW' AND comments IS NOT NULL",
            &[],
        )?
        .collect::<Result<_, _>>()?;
    for relacao in relacoes.iter_mut().filter(|r| r.tipo == "VIEW") {
        relacao.comentario = comentarios.get(&relacao.nome).cloned();
    }
    Ok(())
}

impl Relacao {
    pub fn sugestao(&self) -> Sugestao {
        Sugestao {
            nome: self.nome.clone(),
            tipo: self.tipo.clone(),
            dono: if self.proprio {
                None
            } else {
                Some(self.dono.clone())
            },
        }
    }

    pub fn corresponde(
        &self,
        prefixo: &str,
        esquema: Option<&str>,
        filtro: Option<&Filtro>,
    ) -> bool {
        self.nome.starts_with(prefixo)
            && esquema.map_or(true, |e| self.dono == e)
            && (self.tipo != "TABLE"
                || filtro.map_or(true, |f| {
                    (!f.excluir_gerados || self.gerado.as_deref() == Some("N"))
                        && f.corresponde_oracle(&self.nome, self.situacao.as_deref())
                }))
    }
}

/// Ordem estável, preservando homônimos de donos distintos.
pub fn selecionar<'a>(
    relacoes: impl Iterator<Item = &'a Relacao>,
    prefixo: &str,
    esquema: Option<&str>,
    filtro: Option<&Filtro>,
    limite: usize,
) -> Vec<Sugestao> {
    let prefixo = prefixo.trim().to_uppercase();
    let esquema = esquema.map(str::to_uppercase);
    let mut achadas: Vec<_> = relacoes
        .filter(|r| r.corresponde(&prefixo, esquema.as_deref(), filtro))
        .collect();
    let prioridade = |r: &Relacao| {
        if r.proprio {
            0
        } else if r.dono == "PUBLIC" {
            1
        } else {
            2
        }
    };
    achadas.sort_by(|a, b| {
        (prioridade(a), &a.nome, &a.dono, &a.tipo).cmp(&(prioridade(b), &b.nome, &b.dono, &b.tipo))
    });
    achadas.dedup_by(|a, b| a.nome == b.nome && a.dono == b.dono);
    achadas
        .into_iter()
        .take(limite)
        .map(Relacao::sugestao)
        .collect()
}

/// Sem prefixo, lê o retrato inteiro. Com prefixo, limita a resposta só depois
/// dos filtros, para nomes fora da primeira página não desaparecerem.
pub fn ler(
    conexao: &oracle::Connection,
    outros: bool,
    prefixo: Option<&str>,
    esquema: Option<&str>,
    filtro: Option<&Filtro>,
) -> Resultado<Vec<Relacao>> {
    let sql = sql_da_leitura(outros, prefixo.is_some(), esquema.is_some(), filtro);
    let padrao = prefixo.map(|p| {
        format!(
            "{}%",
            p.trim()
                .to_uppercase()
                .replace('\\', "\\\\")
                .replace('_', "\\_")
                .replace('%', "\\%")
        )
    });
    let esquema = esquema.map(str::to_uppercase);
    let condicao = filtro.map(|f| f.condicao("object_name", "status"));
    let mut binds: Vec<(&str, &dyn oracle::sql_type::ToSql)> = Vec::new();
    if let Some(p) = &padrao {
        binds.push(("prefixo", p));
    }
    if let Some(e) = &esquema {
        binds.push(("esquema", e));
    }
    if let Some(c) = &condicao {
        for (nome, valor) in &c.valores {
            binds.push((nome, valor));
        }
    }
    let mut consulta = conexao.statement(&sql);
    consulta.fetch_array_size(1000);
    let mut consulta = consulta.build()?;
    let mut relacoes = Vec::new();
    for linha in consulta.query_named(&binds)? {
        let linha = linha?;
        relacoes.push(Relacao {
            nome: linha.get(0)?,
            tipo: linha.get(1)?,
            dono: linha.get(2)?,
            proprio: !outros,
            situacao: linha.get(3)?,
            gerado: linha.get(4)?,
            comentario: None,
        });
    }
    Ok(relacoes)
}

fn sql_da_leitura(outros: bool, prefixo: bool, esquema: bool, filtro: Option<&Filtro>) -> String {
    let (objetos, sinonimos, dono, restricao) = if outros {
        ("all_objects", "all_synonyms", "owner", " AND owner <> USER")
    } else {
        ("user_objects", "user_synonyms", "USER", "")
    };
    let escopo = if esquema {
        format!(" AND {dono} = :esquema")
    } else {
        String::new()
    };
    let nome = if prefixo {
        " AND object_name LIKE :prefixo ESCAPE '\\'"
    } else {
        ""
    };
    let sinonimo = if prefixo {
        " AND synonym_name LIKE :prefixo ESCAPE '\\'"
    } else {
        ""
    };
    let recorte = crate::metadados::recorte_de_tabelas(filtro);
    let teto = if prefixo {
        format!(" ORDER BY CASE WHEN dono = 'PUBLIC' THEN 0 ELSE 1 END, nome, dono, tipo FETCH FIRST {} ROWS ONLY", LIMITE + 1)
    } else {
        String::new()
    };
    format!("SELECT nome, tipo, dono, situacao, gerado FROM ( \
        SELECT object_name AS nome, object_type AS tipo, {dono} AS dono, status AS situacao, generated AS gerado \
        FROM {objetos} WHERE object_type IN ({TIPOS}){restricao}{escopo}{nome}{} \
        UNION ALL SELECT synonym_name, 'SYNONYM', {dono}, CAST(NULL AS VARCHAR2(7)), CAST(NULL AS VARCHAR2(1)) \
        FROM {sinonimos} WHERE 1 = 1{restricao}{escopo}{sinonimo} \
        ){teto}", recorte.sql)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn relacao(nome: &str, tipo: &str, dono: &str) -> Relacao {
        Relacao {
            nome: nome.into(),
            tipo: tipo.into(),
            dono: dono.into(),
            proprio: dono == "MEU",
            situacao: Some("VALID".into()),
            gerado: Some("N".into()),
            comentario: None,
        }
    }

    #[test]
    fn filtra_o_acervo_antes_do_teto_e_preserva_donos_e_views() {
        let mut acervo: Vec<_> = (0..400)
            .map(|i| relacao(&format!("DEM_A{i:03}"), "TABLE", "MEU"))
            .collect();
        acervo.extend([
            relacao("DEM_CAD_CLIENTE", "TABLE", "MEU"),
            relacao("DEM_CAD_CLIENTE", "SYNONYM", "MEU"),
            relacao("DEM_CAD_CLIENTE", "TABLE", "OUTRO"),
            relacao("DEM_CAD_VIEW", "VIEW", "OUTRO"),
            relacao("DEM_CAD_SIN", "SYNONYM", "PUBLIC"),
        ]);
        assert_eq!(
            selecionar(acervo.iter(), "DE", None, None, LIMITE).len(),
            LIMITE
        );
        let achadas = selecionar(acervo.iter(), "dem_cad_", None, None, LIMITE);
        assert_eq!(achadas.len(), 4);
        assert!(achadas
            .iter()
            .any(|r| r.nome == "DEM_CAD_CLIENTE" && r.dono.is_none()));
        assert!(achadas
            .iter()
            .any(|r| r.nome == "DEM_CAD_CLIENTE" && r.dono.as_deref() == Some("OUTRO")));
        assert_eq!(
            selecionar(acervo.iter(), "DEM_CAD", Some("outro"), None, LIMITE).len(),
            2
        );
        let filtro: Filtro = serde_json::from_str(
            r#"{"criterios":[{"campo":"nome","operador":"igual","valor":"NAO_EXISTE"}]}"#,
        )
        .unwrap();
        let achadas = selecionar(acervo.iter(), "DEM_CAD", None, Some(&filtro), LIMITE);
        assert!(achadas.iter().all(|r| r.tipo != "TABLE"));
        assert!(achadas.iter().any(|r| r.tipo == "VIEW"));
    }

    #[test]
    fn situacao_e_gerados_so_restringem_tabelas() {
        let mut tabela = relacao("DEM_INVALIDA", "TABLE", "MEU");
        tabela.situacao = Some("INVALID".into());
        let mut gerada = relacao("DEM_GERADA", "TABLE", "MEU");
        gerada.gerado = Some("Y".into());
        let acervo = [
            tabela,
            gerada,
            relacao("DEM_OK", "TABLE", "MEU"),
            relacao("DEM_VIEW", "VIEW", "MEU"),
        ];
        let filtro: Filtro = serde_json::from_str(r#"{"excluirGerados":true,"criterios":[{"campo":"situacao","operador":"igual","valor":"VALID"}]}"#).unwrap();
        let nomes: Vec<_> = selecionar(acervo.iter(), "DEM_", None, Some(&filtro), LIMITE)
            .into_iter()
            .map(|r| r.nome)
            .collect();
        assert_eq!(nomes, ["DEM_OK", "DEM_VIEW"]);
        let sql = sql_da_leitura(true, true, true, Some(&filtro));
        assert!(sql.contains("owner = :esquema"));
        assert!(sql.contains("generated = 'N'"));
        assert!(sql.contains("object_type <> 'TABLE'"));
        assert!(sql.contains("FETCH FIRST 301 ROWS ONLY"));
        assert!(!sql_da_leitura(false, false, false, None).contains("FETCH FIRST"));
    }

    /// Executado à parte, com um perfil salvo indicado explicitamente no ambiente.
    /// CURURU_OBJETO_TESTE indica uma tabela própria, não gerada, para comparação.
    /// Apenas SELECTs; não cria objetos nem muda os dados do banco.
    #[test]
    #[ignore]
    fn conferir_consultas_no_oracle() {
        crate::registro::iniciar();
        assert!(crate::instantclient::preparar().utilizavel());
        let id = std::env::var("CURURU_PERFIL_TESTE").expect("informe CURURU_PERFIL_TESTE");
        let objeto = std::env::var("CURURU_OBJETO_TESTE").expect("informe CURURU_OBJETO_TESTE");
        assert!(!objeto.trim().is_empty(), "informe uma tabela de teste");
        let prefixo: String = objeto.chars().take(3).collect();
        let repositorio = crate::perfis::Repositorio::default();
        let perfil = repositorio
            .listar()
            .unwrap()
            .into_iter()
            .find(|p| p.id == id)
            .expect("perfil salvo");
        let perfil = repositorio.com_senhas(&perfil).unwrap();
        let conexao = crate::sessao::abrir(&perfil).unwrap();
        let mut acervo = Vec::new();
        for outros in [false, true] {
            let inicio = std::time::Instant::now();
            let lidas = ler(&conexao, outros, None, None, None).unwrap();
            crate::registro::informacao(
                "teste_relacoes",
                format!(
                    "outros={outros}: {} nome(s), {} ms",
                    lidas.len(),
                    inicio.elapsed().as_millis()
                ),
            );
            acervo.extend(lidas);
        }
        let filtro: Filtro = serde_json::from_value(serde_json::json!({
            "excluirGerados": true,
            "criterios": [{"campo": "nome", "operador": "comeca", "valor": &prefixo}]
        })).unwrap();
        let esquema: String = conexao.query_row_as("SELECT USER FROM dual", &[]).unwrap();
        for escopo in [None, Some(esquema.as_str())] {
            for filtro in [None, Some(&filtro)] {
                let mut remotas = ler(&conexao, false, Some(&prefixo), escopo, filtro).unwrap();
                remotas.extend(ler(&conexao, true, Some(&prefixo), escopo, filtro).unwrap());
                let do_banco = selecionar(remotas.iter(), &prefixo, escopo, filtro, LIMITE);
                let inicio = std::time::Instant::now();
                let locais = selecionar(acervo.iter(), &prefixo, escopo, filtro, LIMITE);
                crate::registro::informacao(
                    "teste_relacoes",
                    format!(
                        "busca local: {} nome(s), {} microssegundos",
                        locais.len(),
                        inicio.elapsed().as_micros()
                    ),
                );
                assert_eq!(
                    serde_json::to_value(&locais).unwrap(),
                    serde_json::to_value(&do_banco).unwrap()
                );
                assert!(locais.iter().any(|r| r.nome == objeto));
            }
        }
        // Exercita também serialização, leitura fria e independência das rotinas.
        // Por padrão usa uma chave temporária. A opção explícita prepara o índice
        // da conexão para o teste manual do executável recém-gerado.
        let preparar = std::env::var("CURURU_PREPARAR_INDICE").as_deref() == Ok("1");
        let chave = if preparar {
            id.clone()
        } else {
            format!("verificacao-relacoes-{}", std::process::id())
        };
        let indice = crate::indice::RepositorioDeIndice::default();
        let mut do_usuario: Vec<_> = acervo.iter().filter(|r| r.proprio).cloned().collect();
        anexar_comentarios_de_views(&conexao, &mut do_usuario).unwrap();
        indice.guardar_relacoes(&chave, false, do_usuario);
        indice.guardar_relacoes(
            &chave,
            true,
            acervo.iter().filter(|r| !r.proprio).cloned().collect(),
        );
        if preparar {
            let procedures = conexao
                .query_as::<(String, Option<String>, Option<String>)>(
                    "SELECT object_name, status, generated FROM user_objects \
                     WHERE object_type = 'PROCEDURE' ORDER BY object_name",
                    &[],
                )
                .unwrap()
                .map(|linha| {
                    let (nome, situacao, gerado) = linha.unwrap();
                    crate::indice::PacoteIndexado { nome, situacao, gerado }
                })
                .collect();
            indice.guardar_procedures(&chave, procedures);
        }
        let indice = crate::indice::RepositorioDeIndice::default();
        let do_arquivo = indice.sugerir_relacoes(&chave, &prefixo, None, None);
        assert_eq!(
            serde_json::to_value(do_arquivo).unwrap(),
            serde_json::to_value(selecionar(acervo.iter(), &prefixo, None, None, LIMITE)).unwrap()
        );
        if !preparar {
            indice.esquecer(&chave);
        }
        crate::registro::informacao(
            "teste_relacoes",
            "comparações entre índice e Oracle aprovadas",
        );
    }
}
