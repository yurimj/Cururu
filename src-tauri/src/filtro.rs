//! Filtro de objetos da árvore (requisito 4).
//!
//! Espelha o filtro do Oracle SQL Developer: vários critérios, cada um com
//! campo, operador, valor e distinção de maiúsculas, combinados por "qualquer
//! um" (OU) ou "todos" (E).
//!
//! Os valores **sempre** viram bind; nunca entram concatenados no SQL. Campo e
//! operador vêm de enumerações fechadas, então também não há como injetar por
//! ali.

use serde::{Deserialize, Serialize};

/// Campo do objeto sobre o qual o critério age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Campo {
    Nome,
    Situacao,
}

/// Comparação aplicada ao campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Operador {
    Like,
    NaoLike,
    Igual,
    Diferente,
    Em,
    NaoEm,
    Comeca,
    Termina,
    NaoComeca,
    NaoTermina,
}

impl Operador {
    /// O operador aceita vários valores separados por vírgula?
    fn aceita_lista(self) -> bool {
        matches!(self, Operador::Em | Operador::NaoEm)
    }

    /// Critérios de exclusão são sempre restrições. Ligá-los por `OR` faria um
    /// par de `NOT LIKE` aceitar praticamente todos os objetos.
    fn exclui_correspondencia(self) -> bool {
        matches!(
            self,
            Operador::NaoLike
                | Operador::Diferente
                | Operador::NaoEm
                | Operador::NaoComeca
                | Operador::NaoTermina
        )
    }
}

/// Um critério isolado.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Criterio {
    pub campo: Campo,
    pub operador: Operador,
    pub valor: String,
    /// Marcado, compara respeitando a caixa das letras.
    #[serde(default)]
    pub distingue_maiusculas: bool,
}

/// O filtro completo, como a tela monta.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Filtro {
    #[serde(default)]
    pub criterios: Vec<Criterio>,
    /// Campo legado mantido para desserializar filtros da interface anterior.
    #[serde(default)]
    pub corresponder_todos: bool,
    /// Esconde objetos criados pelo próprio banco (tabelas de apoio, filas).
    #[serde(default)]
    pub excluir_gerados: bool,
    /// Lista também os sinônimos do usuário junto com tabelas e views.
    #[serde(default)]
    pub incluir_sinonimos: bool,
}

/// Trecho de SQL com os binds que ele espera.
pub struct Condicao {
    /// Vazio quando não há critério nenhum.
    pub sql: String,
    pub valores: Vec<(String, String)>,
}

impl Filtro {
    /// Replica o filtro Oracle em memória: inclusões por OU e exclusões por E.
    pub fn corresponde_oracle(&self, nome: &str, situacao: Option<&str>) -> bool {
        let mut tem_inclusao = false;
        let mut incluida = false;
        for criterio in self.criterios_validos() {
            let alvo = match criterio.campo {
                Campo::Nome => Some(nome),
                Campo::Situacao => situacao,
            };
            let corresponde = alvo.is_some_and(|alvo| {
                let encontrado = separar(&criterio.valor, criterio.operador)
                    .iter()
                    .any(|termo| {
                        let valor = if criterio.distingue_maiusculas {
                            termo.clone()
                        } else {
                            termo.to_uppercase()
                        };
                        match criterio.operador {
                            Operador::Like | Operador::NaoLike => corresponde_like(alvo, &valor),
                            Operador::Comeca | Operador::NaoComeca => alvo.starts_with(&valor),
                            Operador::Termina | Operador::NaoTermina => alvo.ends_with(&valor),
                            _ => alvo == valor,
                        }
                    });
                if criterio.operador.exclui_correspondencia() {
                    !encontrado
                } else {
                    encontrado
                }
            });
            if criterio.operador.exclui_correspondencia() {
                if !corresponde {
                    return false;
                }
            } else {
                tem_inclusao = true;
                incluida |= corresponde;
            }
        }
        !tem_inclusao || incluida
    }

    /// Há algo que efetivamente restrinja a listagem?
    pub fn ativo(&self) -> bool {
        self.excluir_gerados || self.incluir_sinonimos || !self.criterios_validos().is_empty()
    }

    fn criterios_validos(&self) -> Vec<&Criterio> {
        self.criterios
            .iter()
            .filter(|criterio| !criterio.valor.trim().is_empty())
            .collect()
    }

    /// Monta a cláusula que vai no `WHERE`.
    ///
    /// As colunas são recebidas de fora porque a consulta usa os nomes reais do
    /// dicionário (`object_name`, `job_name`…) e não um apelido: predicado
    /// sobre a coluna original é o que permite ao Oracle combiná-lo com o resto
    /// do filtro em vez de materializar tudo antes.
    pub fn condicao(&self, coluna_nome: &str, coluna_situacao: &str) -> Condicao {
        let criterios = self.criterios_validos();
        if criterios.is_empty() {
            return Condicao {
                sql: String::new(),
                valores: Vec::new(),
            };
        }

        let mut inclusoes = Vec::new();
        let mut exclusoes = Vec::new();
        let mut valores = Vec::new();

        for (indice, criterio) in criterios.iter().enumerate() {
            let coluna = match criterio.campo {
                Campo::Nome => coluna_nome,
                Campo::Situacao => coluna_situacao,
            };
            // Sem distinção de caixa, só o valor procurado sobe para
            // maiúsculas — a coluna fica intocada de propósito. `UPPER(coluna)`
            // impede o Oracle de usar o índice do dicionário e foi o que fez
            // uma filtragem levar 25 s numa base grande. Nomes de objeto no
            // Oracle são maiúsculos, salvo se criados entre aspas — caso raro,
            // registrado em requisitos.md como limitação conhecida.
            let alvo = coluna.to_string();

            let termos = separar(&criterio.valor, criterio.operador);
            let mut binds = Vec::new();
            for (posicao, termo) in termos.iter().enumerate() {
                let nome_bind = format!("f{indice}_{posicao}");
                let valor = ajustar(termo, criterio.operador);
                binds.push(if criterio.distingue_maiusculas {
                    format!(":{nome_bind}")
                } else {
                    format!("UPPER(:{nome_bind})")
                });
                valores.push((nome_bind, valor));
            }

            let parte = montar(&alvo, criterio.operador, &binds);
            if criterio.operador.exclui_correspondencia() {
                exclusoes.push(parte);
            } else {
                inclusoes.push(parte);
            }
        }

        // A tela apresenta dois grupos: basta corresponder a uma inclusão, mas
        // nenhuma exclusão pode corresponder. Em SQL, `NOT (A OR B)` equivale a
        // `NOT A AND NOT B`, que é exatamente a junção abaixo.
        let sql = match (inclusoes.is_empty(), exclusoes.is_empty()) {
            (false, false) => format!(
                "(({}) AND {})",
                inclusoes.join(" OR "),
                exclusoes.join(" AND ")
            ),
            (false, true) => format!("({})", inclusoes.join(" OR ")),
            (true, false) => format!("({})", exclusoes.join(" AND ")),
            (true, true) => String::new(),
        };
        Condicao { sql, valores }
    }

    /// A mesma cláusula, escrita em T-SQL (requisito 4 no SQL Server).
    ///
    /// É um método à parte, e não um parâmetro do outro, porque as três diferenças
    /// não são de dialeto — são de comportamento:
    ///
    /// * **Os parâmetros do `tiberius` são posicionais** (`@P1`, `@P2`), e não
    ///   nomeados. Daí o `primeiro_parametro`: a consulta da árvore já gastou dois
    ///   com o `OFFSET`/`FETCH`, e o filtro continua a numeração de onde ela parou.
    /// * **O `LIKE` do T-SQL tem um curinga a mais**: `[` abre classe de
    ///   caracteres. Um `[` digitado no filtro precisa ser escapado, ou `TB[01]`
    ///   casaria com `TB0` e `TB1` em vez de com o nome literal.
    /// * **A caixa depende do collation do banco**, não do valor. No Oracle basta
    ///   subir o valor para maiúsculas porque o nome guardado já é maiúsculo; aqui
    ///   `Cliente` e `cliente` são dois nomes possíveis, e a comparação precisa
    ///   dizer explicitamente o que quer. `UPPER` dos dois lados dá o mesmo
    ///   resultado em banco sensível ou não; a distinção de caixa, quando pedida,
    ///   sai por collation binário — sem ele, um banco `CI` ignoraria o pedido.
    ///
    /// A situação não entra: no SQL Server não existe objeto inválido esperando
    /// recompilação, e a tela não oferece esse campo. Critério que peça situação é
    /// descartado em vez de virar SQL que não faz sentido.
    pub fn condicao_tsql(&self, coluna_nome: &str, primeiro_parametro: usize) -> CondicaoTsql {
        let criterios: Vec<&Criterio> = self
            .criterios_validos()
            .into_iter()
            .filter(|criterio| criterio.campo == Campo::Nome)
            .collect();
        if criterios.is_empty() {
            return CondicaoTsql {
                sql: String::new(),
                valores: Vec::new(),
            };
        }

        let mut inclusoes = Vec::new();
        let mut exclusoes = Vec::new();
        let mut valores: Vec<String> = Vec::new();

        for criterio in criterios {
            let termos = separar(&criterio.valor, criterio.operador);
            let mut binds = Vec::new();
            for termo in &termos {
                let posicao = primeiro_parametro + valores.len();
                valores.push(ajustar_tsql(termo, criterio.operador));
                binds.push(if criterio.distingue_maiusculas {
                    format!("@P{posicao}")
                } else {
                    format!("UPPER(@P{posicao})")
                });
            }
            let alvo = if criterio.distingue_maiusculas {
                format!("{coluna_nome} COLLATE Latin1_General_BIN2")
            } else {
                format!("UPPER({coluna_nome})")
            };

            let parte = montar(&alvo, criterio.operador, &binds);
            if criterio.operador.exclui_correspondencia() {
                exclusoes.push(parte);
            } else {
                inclusoes.push(parte);
            }
        }

        // Mesma composição do Oracle: basta corresponder a uma inclusão, e nenhuma
        // exclusão pode corresponder.
        let sql = match (inclusoes.is_empty(), exclusoes.is_empty()) {
            (false, false) => format!(
                "(({}) AND {})",
                inclusoes.join(" OR "),
                exclusoes.join(" AND ")
            ),
            (false, true) => format!("({})", inclusoes.join(" OR ")),
            (true, false) => format!("({})", exclusoes.join(" AND ")),
            (true, true) => String::new(),
        };
        CondicaoTsql { sql, valores }
    }
}

/// Trecho de T-SQL com os valores dos parâmetros, na ordem em que entram.
#[derive(Debug, Default)]
pub struct CondicaoTsql {
    /// Vazio quando não sobrou critério nenhum.
    pub sql: String,
    pub valores: Vec<String>,
}

/// Quebra o valor em termos quando o operador aceita lista.
fn separar(valor: &str, operador: Operador) -> Vec<String> {
    if !operador.aceita_lista() {
        return vec![valor.trim().to_string()];
    }
    valor
        .split(',')
        .map(str::trim)
        .filter(|termo| !termo.is_empty())
        .map(str::to_string)
        .collect()
}

/// Só `%` é curinga; sublinhados e barras são literais, como no SQL do filtro.
fn corresponde_like(alvo: &str, padrao: &str) -> bool {
    let letras: Vec<char> = alvo.chars().collect();
    let mut anterior = vec![false; letras.len() + 1];
    anterior[0] = true;
    for letra in padrao.chars() {
        let mut atual = vec![false; letras.len() + 1];
        if letra == '%' {
            atual[0] = anterior[0];
        }
        for i in 1..=letras.len() {
            atual[i] = if letra == '%' {
                anterior[i] || atual[i - 1]
            } else {
                anterior[i - 1] && letras[i - 1] == letra
            };
        }
        anterior = atual;
    }
    anterior[letras.len()]
}

/// Prepara o valor para o operador, escapando curingas e acrescentando os que
/// o operador implica.
fn ajustar(termo: &str, operador: Operador) -> String {
    match operador {
        // No LIKE, só `%` continua curinga. O sublinhado é literal: quem digita
        // `%_REL_%` quer o caractere sublinhado antes e depois de REL, e não
        // "um caractere qualquer" — que traria SREL020 sem sublinhado nenhum.
        Operador::Like | Operador::NaoLike => escapar(termo, false),
        // "começa com" e "termina com" tratam o texto todo como literal.
        Operador::Comeca | Operador::NaoComeca => format!("{}%", escapar(termo, true)),
        Operador::Termina | Operador::NaoTermina => format!("%{}", escapar(termo, true)),
        _ => termo.to_string(),
    }
}

/// O mesmo preparo do valor, com os curingas do T-SQL.
fn ajustar_tsql(termo: &str, operador: Operador) -> String {
    match operador {
        Operador::Like | Operador::NaoLike => escapar_tsql(termo, false),
        Operador::Comeca | Operador::NaoComeca => format!("{}%", escapar_tsql(termo, true)),
        Operador::Termina | Operador::NaoTermina => format!("%{}", escapar_tsql(termo, true)),
        _ => termo.to_string(),
    }
}

/// Neutraliza os curingas do `LIKE` do T-SQL, que são um a mais que os do
/// Oracle: além de `%` e `_`, o colchete abre classe de caracteres.
fn escapar_tsql(termo: &str, inclusive_porcento: bool) -> String {
    let mut escapado = termo
        .replace('\\', "\\\\")
        .replace('_', "\\_")
        .replace('[', "\\[");
    if inclusive_porcento {
        escapado = escapado.replace('%', "\\%");
    }
    escapado
}

/// Neutraliza os curingas do `LIKE` do Oracle.
///
/// A barra invertida vem primeiro, senão as escapadas seguintes seriam
/// escapadas de novo.
fn escapar(termo: &str, inclusive_porcento: bool) -> String {
    let mut escapado = termo.replace('\\', "\\\\").replace('_', "\\_");
    if inclusive_porcento {
        escapado = escapado.replace('%', "\\%");
    }
    escapado
}

fn montar(alvo: &str, operador: Operador, binds: &[String]) -> String {
    match operador {
        // O ESCAPE acompanha todo LIKE, porque `escapar` marca com barra
        // invertida o que precisa valer como caractere comum.
        Operador::Like | Operador::Comeca | Operador::Termina => {
            format!("{alvo} LIKE {} ESCAPE '\\'", binds[0])
        }
        Operador::NaoLike | Operador::NaoComeca | Operador::NaoTermina => {
            format!("{alvo} NOT LIKE {} ESCAPE '\\'", binds[0])
        }
        Operador::Igual => format!("{alvo} = {}", binds[0]),
        Operador::Diferente => format!("{alvo} <> {}", binds[0]),
        Operador::Em => format!("{alvo} IN ({})", binds.join(", ")),
        Operador::NaoEm => format!("{alvo} NOT IN ({})", binds.join(", ")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn filtro_local_preserva_curingas_literais_caixa_e_exclusoes() {
        assert!(corresponde_like("PKG_A\\B", "%_A\\%"));
        assert!(!corresponde_like("PKGXA", "%_A"));
        assert!(corresponde_like("AÇÃO", "%ÇÃO"));
        let filtro = Filtro {
            criterios: vec![
                criterio(Operador::Comeca, "pkg_", false),
                criterio(Operador::Igual, "OUTRO", true),
                criterio(Operador::NaoLike, "%TMP%", false),
                criterio(Operador::NaoEm, "PKG_X, PKG_Y", true),
            ],
            ..Default::default()
        };
        assert!(filtro.corresponde_oracle("PKG_A", None));
        assert!(filtro.corresponde_oracle("OUTRO", None));
        for nome in ["PKG_TMP", "PKG_X", "PKG_Y", "pkg_a", "ABC"] {
            assert!(!filtro.corresponde_oracle(nome, None), "{nome}");
        }
        for (operador, valor, esperado) in [
            (Operador::Like, "PKG%", true),
            (Operador::NaoLike, "PKG%", false),
            (Operador::Igual, "PKG_A", true),
            (Operador::Diferente, "PKG_A", false),
            (Operador::Em, "X, PKG_A", true),
            (Operador::NaoEm, "X, PKG_A", false),
            (Operador::Comeca, "PKG_", true),
            (Operador::NaoComeca, "PKG_", false),
            (Operador::Termina, "_A", true),
            (Operador::NaoTermina, "_A", false),
        ] {
            let filtro = Filtro {
                criterios: vec![criterio(operador, valor, true)],
                ..Default::default()
            };
            assert_eq!(
                filtro.corresponde_oracle("PKG_A", None),
                esperado,
                "{operador:?}"
            );
        }
    }

    fn criterio(operador: Operador, valor: &str, distingue: bool) -> Criterio {
        Criterio {
            campo: Campo::Nome,
            operador,
            valor: valor.to_string(),
            distingue_maiusculas: distingue,
        }
    }

    // --- T-SQL (requisito 4 no SQL Server) --------------------------------

    #[test]
    fn o_tsql_numera_os_parametros_a_partir_do_que_a_consulta_ja_gastou() {
        let filtro = Filtro {
            criterios: vec![
                criterio(Operador::Comeca, "TB", false),
                criterio(Operador::NaoLike, "%TMP%", false),
            ],
            ..Default::default()
        };
        let condicao = filtro.condicao_tsql("o.name", 3);

        // Os dois primeiros são do OFFSET/FETCH da listagem.
        assert!(condicao.sql.contains("@P3"), "{}", condicao.sql);
        assert!(condicao.sql.contains("@P4"), "{}", condicao.sql);
        assert!(!condicao.sql.contains("@P1"), "{}", condicao.sql);
        assert_eq!(condicao.valores, vec!["TB%".to_string(), "%TMP%".to_string()]);
        // Inclusão e exclusão compõem como no Oracle.
        assert!(condicao.sql.contains(" AND "), "{}", condicao.sql);
    }

    #[test]
    fn o_tsql_compara_sem_caixa_dos_dois_lados() {
        // No SQL Server o nome guardado é como foi escrito, e o collation do
        // banco decide a comparação: subir só o valor, como no Oracle, deixaria
        // "cliente" sem achar "Cliente" num banco sensível a caixa.
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "%cli%", false)],
            ..Default::default()
        };
        let condicao = filtro.condicao_tsql("o.name", 3);
        assert!(condicao.sql.contains("UPPER(o.name)"), "{}", condicao.sql);
        assert!(condicao.sql.contains("UPPER(@P3)"), "{}", condicao.sql);
    }

    #[test]
    fn distinguir_caixa_no_tsql_usa_collation_binario() {
        // Sem o collation explícito, um banco CI ignoraria o pedido em silêncio.
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "%Cli%", true)],
            ..Default::default()
        };
        let condicao = filtro.condicao_tsql("o.name", 3);
        assert!(condicao.sql.contains("COLLATE Latin1_General_BIN2"), "{}", condicao.sql);
        assert!(!condicao.sql.contains("UPPER("), "{}", condicao.sql);
    }

    #[test]
    fn o_colchete_e_escapado_no_tsql_e_nao_no_oracle() {
        // `[` abre classe de caracteres só no T-SQL: sem escapar, `TB[01]`
        // casaria com TB0 e TB1 em vez de com o nome literal.
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Comeca, "TB[01]", false)],
            ..Default::default()
        };
        assert_eq!(
            filtro.condicao_tsql("o.name", 3).valores,
            vec![r"TB\[01]%".to_string()]
        );
        assert_eq!(
            filtro.condicao("object_name", "status").valores[0].1,
            "TB[01]%".to_string()
        );
    }

    #[test]
    fn situacao_nao_vira_condicao_no_tsql() {
        // No SQL Server não existe objeto inválido esperando recompilação: o
        // critério é descartado em vez de virar uma coluna que não existe.
        let filtro = Filtro {
            criterios: vec![Criterio {
                campo: Campo::Situacao,
                operador: Operador::Igual,
                valor: "INVALID".to_string(),
                distingue_maiusculas: false,
            }],
            ..Default::default()
        };
        assert!(filtro.condicao_tsql("o.name", 3).sql.is_empty());
        // No Oracle, o mesmo critério continua valendo.
        assert!(!filtro.condicao("object_name", "status").sql.is_empty());
    }

    #[test]
    fn o_valor_nunca_entra_concatenado_no_tsql() {
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Igual, "CLIENTE'; drop table x --", false)],
            ..Default::default()
        };
        let condicao = filtro.condicao_tsql("o.name", 3);
        assert!(!condicao.sql.contains("drop table"), "{}", condicao.sql);
        assert_eq!(condicao.valores.len(), 1);
    }

    #[test]
    fn filtro_vazio_nao_gera_condicao() {
        let filtro = Filtro::default();
        assert!(!filtro.ativo());
        assert!(filtro.condicao("nome", "situacao").sql.is_empty());
    }

    #[test]
    fn criterio_em_branco_e_descartado() {
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "   ", false)],
            ..Default::default()
        };
        assert!(!filtro.ativo());
        assert!(filtro.condicao("nome", "situacao").sql.is_empty());
    }

    #[test]
    fn sem_distincao_de_caixa_sobe_apenas_o_valor_procurado() {
        // A coluna precisa ficar sem função em volta para o Oracle poder usar
        // o índice do dicionário.
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "tbl%", false)],
            ..Default::default()
        };
        let condicao = filtro.condicao("nome", "situacao");
        assert_eq!(condicao.sql, "(nome LIKE UPPER(:f0_0) ESCAPE '\\')");
        assert_eq!(condicao.valores, [("f0_0".to_string(), "tbl%".to_string())]);
    }

    #[test]
    fn a_coluna_do_criterio_vem_de_fora() {
        let filtro = Filtro {
            criterios: vec![Criterio {
                campo: Campo::Situacao,
                operador: Operador::Igual,
                valor: "INVALID".to_string(),
                distingue_maiusculas: false,
            }],
            ..Default::default()
        };
        assert_eq!(
            filtro.condicao("object_name", "status").sql,
            "(status = UPPER(:f0_0))"
        );
    }

    #[test]
    fn com_distincao_de_caixa_compara_direto() {
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "Tbl%", true)],
            ..Default::default()
        };
        assert_eq!(
            filtro.condicao("nome", "situacao").sql,
            "(nome LIKE :f0_0 ESCAPE '\\')"
        );
    }

    #[test]
    fn sublinhado_e_literal_no_like() {
        // `%_REL_%` precisa exigir o sublinhado antes e depois de REL. Sem
        // escapar, o `_` do Oracle casaria com qualquer caractere e traria
        // nomes como SREL020.
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "%_REL_%", false)],
            ..Default::default()
        };
        let condicao = filtro.condicao("nome", "situacao");
        assert_eq!(condicao.valores[0].1, "%\\_REL\\_%");
        assert!(condicao.sql.contains("ESCAPE '\\'"));
    }

    #[test]
    fn porcento_continua_curinga_no_like() {
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Like, "DEM_%", false)],
            ..Default::default()
        };
        assert_eq!(filtro.condicao("nome", "situacao").valores[0].1, "DEM\\_%");
    }

    #[test]
    fn comeca_e_termina_tratam_o_texto_como_literal() {
        let comeca = Filtro {
            criterios: vec![criterio(Operador::Comeca, "TBL_", false)],
            ..Default::default()
        };
        assert_eq!(comeca.condicao("nome", "situacao").valores[0].1, "TBL\\_%");

        let termina = Filtro {
            criterios: vec![criterio(Operador::Termina, "_LOG", false)],
            ..Default::default()
        };
        assert_eq!(termina.condicao("nome", "situacao").valores[0].1, "%\\_LOG");

        // Aqui o `%` digitado também é literal.
        let literal = Filtro {
            criterios: vec![criterio(Operador::Comeca, "50%", false)],
            ..Default::default()
        };
        assert_eq!(literal.condicao("nome", "situacao").valores[0].1, "50\\%%");

        let nao_comeca = Filtro {
            criterios: vec![criterio(Operador::NaoComeca, "TMP_", false)],
            ..Default::default()
        };
        assert_eq!(
            nao_comeca.condicao("nome", "situacao").valores[0].1,
            "TMP\\_%"
        );
    }

    #[test]
    fn inclusoes_usam_ou() {
        let filtro = Filtro {
            criterios: vec![
                criterio(Operador::Like, "DEM_%", false),
                criterio(Operador::Like, "INT_%", false),
            ],
            ..Default::default()
        };
        assert_eq!(
            filtro.condicao("nome", "situacao").sql,
            "(nome LIKE UPPER(:f0_0) ESCAPE '\\' OR nome LIKE UPPER(:f1_0) ESCAPE '\\')"
        );
    }

    #[test]
    fn exclusoes_restringem_as_inclusoes() {
        let filtro = Filtro {
            criterios: vec![
                criterio(Operador::Like, "%DEM%", false),
                criterio(Operador::Like, "%EXEMPLO%", false),
                criterio(Operador::NaoLike, "%DEML%", false),
                criterio(Operador::NaoLike, "%DEMR%", false),
            ],
            ..Default::default()
        };

        assert_eq!(
            filtro.condicao("nome", "situacao").sql,
            "((nome LIKE UPPER(:f0_0) ESCAPE '\\' OR nome LIKE UPPER(:f1_0) ESCAPE '\\') AND nome NOT LIKE UPPER(:f2_0) ESCAPE '\\' AND nome NOT LIKE UPPER(:f3_0) ESCAPE '\\')"
        );
    }

    #[test]
    fn apenas_exclusoes_usam_e() {
        let filtro = Filtro {
            criterios: vec![
                criterio(Operador::NaoLike, "%DEML%", false),
                criterio(Operador::NaoLike, "%DEMR%", false),
            ],
            ..Default::default()
        };
        assert!(filtro
            .condicao("nome", "situacao")
            .sql
            .contains(" NOT LIKE UPPER(:f0_0) ESCAPE '\\' AND nome NOT LIKE "));
    }

    #[test]
    fn campo_legado_corresponder_todos_nao_muda_os_grupos() {
        let filtro = Filtro {
            criterios: vec![
                criterio(Operador::Like, "DEM_%", false),
                criterio(Operador::Like, "INT_%", false),
            ],
            corresponder_todos: true,
            ..Default::default()
        };
        assert!(filtro.condicao("nome", "situacao").sql.contains(" OR "));
    }

    #[test]
    fn operador_em_quebra_a_lista_em_binds() {
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Em, "VALID, INVALID", false)],
            ..Default::default()
        };
        let condicao = filtro.condicao("nome", "situacao");
        assert_eq!(condicao.sql, "(nome IN (UPPER(:f0_0), UPPER(:f0_1)))");
        assert_eq!(condicao.valores.len(), 2);
        assert_eq!(condicao.valores[1].1, "INVALID");
    }

    #[test]
    fn valor_nunca_entra_concatenado_no_sql() {
        let malicioso = "X' OR '1'='1";
        let filtro = Filtro {
            criterios: vec![criterio(Operador::Igual, malicioso, false)],
            ..Default::default()
        };
        let condicao = filtro.condicao("nome", "situacao");
        assert!(!condicao.sql.contains(malicioso));
        assert_eq!(condicao.valores[0].1, malicioso);
    }

    #[test]
    fn excluir_gerados_e_sinonimos_tornam_o_filtro_ativo() {
        let gerados = Filtro {
            excluir_gerados: true,
            ..Default::default()
        };
        assert!(gerados.ativo());

        let sinonimos = Filtro {
            incluir_sinonimos: true,
            ..Default::default()
        };
        assert!(sinonimos.ativo());
    }
}
