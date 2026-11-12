//! Árvore de objetos do SQL Server (requisito 2).
//!
//! O que [`crate::objetos`] pergunta ao dicionário do Oracle, este módulo
//! pergunta ao catálogo do SQL Server. Duas diferenças de fundo:
//!
//! * **O escopo é o banco, não o schema.** No Oracle a árvore mostra o schema do
//!   usuário conectado e nada mais (requisito 2); aqui ela pendura as categorias
//!   de cada banco, e um banco tem vários esquemas. Daí o nome exibido ser
//!   `esquema.objeto`, como no SSMS — que é também o nome que se escreve numa
//!   consulta, e o que `OBJECT_ID` resolve sem ambiguidade.
//! * **O DDL de tabela é montado aqui.** O Oracle tem `DBMS_METADATA.GET_DDL`; o
//!   SQL Server não tem equivalente em T-SQL — o *Script Table as* do SSMS é
//!   código do próprio SSMS, não do servidor. View, procedure e função, essas
//!   sim, têm o fonte guardado em `sys.sql_modules` e saem de lá exatamente como
//!   foram criadas.
//!
//! Como em [`crate::metadados_sqlserver`], o banco vai **no nome de três
//! partes**, e não num `use`: quem responde é a sessão da conexão, e trocar o
//! banco dela estragaria a árvore que está sendo montada.

use tiberius::ToSql;

use crate::erro::{Erro, Resultado};
use crate::metadados_sqlserver::{descrever_tipo, nome_completo, qualificar};
use crate::objetos::{Categoria, DestinoDaReferencia, Objeto, Pagina, ParteDoFonte};
use crate::sqlserver::{entre_colchetes, Cliente};

/// Uma página de objetos de uma categoria, dentro de um banco.
///
/// Paginada pela mesma razão do Oracle: há bancos com milhares de tabelas, e
/// trazer tudo custa tempo no servidor e memória na tela.
pub async fn listar(
    cliente: &mut Cliente,
    banco: Option<&str>,
    categoria: Categoria,
    filtro: Option<&crate::filtro::Filtro>,
    deslocamento: u32,
    limite: u32,
) -> Resultado<Pagina> {
    let tipos = categoria.tipos_sqlserver().ok_or_else(|| {
        Erro::novo(format!("o SQL Server não tem a categoria {categoria:?}"))
    })?;
    let catalogo = qualificar(banco, "sys");

    // O comentário da view (requisito 37) sai na mesma consulta, e aqui isso é
    // seguro: no Oracle a junção teria de ficar de fora por causa do plano da
    // listagem, mas `sys.extended_properties` é uma tabela do catálogo com
    // índice pela chave que se busca — `class`, `major_id`, `minor_id`, `name` —,
    // e o `left join` não muda o acesso a `sys.objects`.
    //
    // Só nas views: nas outras categorias nem a coluna nem a junção existem.
    let comentado = categoria == Categoria::Views;
    let (coluna_comentario, juncao_comentario) = if comentado {
        (
            ", cast(p.value as nvarchar(4000)) as comentario",
            format!(
                " left join {catalogo}.extended_properties p \
                         on p.class = 1 and p.major_id = o.object_id \
                        and p.minor_id = 0 and p.name = N'{NOME_DA_PROPRIEDADE}'"
            ),
        )
    } else {
        ("", String::new())
    };

    // O filtro (requisito 4) age sobre `o.name`, que é o nome sem o esquema —
    // o mesmo que o usuário digita e o mesmo que a árvore mostra. Os dois
    // primeiros parâmetros já são do `OFFSET`/`FETCH`, então ele começa no
    // terceiro.
    let recorte = match filtro {
        Some(filtro) => filtro.condicao_tsql("o.name", 3),
        None => crate::filtro::CondicaoTsql::default(),
    };
    let condicao_do_filtro = if recorte.sql.is_empty() {
        String::new()
    } else {
        format!(" and {}", recorte.sql)
    };

    // `is_ms_shipped = 0` é o que deixa de fora as centenas de objetos do
    // próprio servidor — e é também o que faz a opção "excluir objetos gerados"
    // não existir aqui: ela já vale sempre. A ordenação é a do texto que
    // aparece na tela — esquema e depois nome —, para a lista sair alfabética
    // como se lê.
    let sql = format!(
        "select e.name as esquema, o.name as nome{coluna_comentario} \
           from {catalogo}.objects o \
           join {catalogo}.schemas e on e.schema_id = o.schema_id{juncao_comentario} \
          where o.is_ms_shipped = 0 and o.type in ({tipos}){condicao_do_filtro} \
          order by e.name, o.name \
         offset @P1 rows fetch next @P2 rows only"
    );

    // Um a mais que o pedido: se ele vier, é porque há próxima página.
    let inicio = i32::try_from(deslocamento).unwrap_or(i32::MAX);
    let quantidade = i32::try_from(limite).unwrap_or(i32::MAX - 1) + 1;
    let mut parametros: Vec<&dyn ToSql> = vec![&inicio, &quantidade];
    parametros.extend(recorte.valores.iter().map(|valor| valor as &dyn ToSql));
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    let mut objetos: Vec<Objeto> = linhas
        .iter()
        .filter_map(|linha| {
            let nome = linha.get::<&str, _>("nome")?;
            let esquema = linha.get::<&str, _>("esquema").unwrap_or("dbo");
            Some(Objeto {
                nome: format!("{esquema}.{nome}"),
                // Situação é do Oracle: no SQL Server não existe objeto
                // inválido esperando recompilação.
                situacao: None,
                // A coluna só está no `select` das views; pedi-la nas outras
                // categorias seria pedir uma coluna que não veio.
                comentario: comentado
                    .then(|| linha.get::<&str, _>("comentario"))
                    .flatten()
                    .map(str::to_string),
            })
        })
        .collect();

    let tem_mais = objetos.len() > limite as usize;
    objetos.truncate(limite as usize);
    Ok(Pagina { objetos, tem_mais })
}

/// O nome da *extended property* que guarda a descrição de um objeto.
///
/// É **convenção**, não regra do servidor: nada impede outro nome, e um objeto
/// pode ter várias propriedades ao mesmo tempo. `MS_Description` é o nome que o
/// SSMS lê e escreve na caixa *Description* das propriedades, e é por isso que
/// é o usado aqui — comentário posto pelo Cururu aparece no SSMS, e o posto
/// pelo SSMS aparece no Cururu. Qualquer outro nome daria uma caixa que só o
/// Cururu enxerga.
const NOME_DA_PROPRIEDADE: &str = "MS_Description";

/// Grava o comentário de uma view, ou o apaga quando o texto vem vazio
/// (requisito 37).
///
/// O SQL Server não tem o `COMMENT ON` do Oracle, que grava, altera e apaga com
/// um comando só: são três procedures, e chamar a errada é erro, não
/// no-operação — `sp_addextendedproperty` recusa a propriedade que já existe.
/// Por isso quem escolhe é um `EXISTS`, e ele vai **no mesmo lote** da chamada:
/// perguntar numa ida e responder noutra deixaria uma janela entre a pergunta e
/// a gravação.
///
/// As procedures agem sobre o banco em que rodam, e a sessão não troca de banco
/// (é a regra do módulo). Daí serem chamadas qualificadas — `[Banco].sys.sp_…` —,
/// que é a forma documentada de rodar uma procedure de sistema em outro banco.
pub async fn gravar_comentario(
    cliente: &mut Cliente,
    banco: Option<&str>,
    nome: &str,
    comentario: &str,
) -> Resultado<()> {
    let catalogo = qualificar(banco, "sys");
    let alvo = nome_completo(banco, nome);
    let (esquema, objeto) = esquema_e_objeto(nome);
    let comentario = comentario.trim().to_string();

    // Os `begin`/`end` não são enfeite: `if <cmd>; else` é erro de sintaxe no
    // T-SQL, e sem eles o lote não compila.
    let sql = format!(
        "declare @existe bit = case when exists ( \
             select 1 from {catalogo}.extended_properties \
              where class = 1 and major_id = object_id(@P1) \
                and minor_id = 0 and name = N'{NOME_DA_PROPRIEDADE}') \
           then 1 else 0 end; \
         if @existe = 1 and @P2 = N'' begin \
             exec {catalogo}.sp_dropextendedproperty \
                  @name = N'{NOME_DA_PROPRIEDADE}', \
                  @level0type = N'SCHEMA', @level0name = @P3, \
                  @level1type = N'VIEW', @level1name = @P4; \
         end else if @existe = 1 begin \
             exec {catalogo}.sp_updateextendedproperty \
                  @name = N'{NOME_DA_PROPRIEDADE}', @value = @P2, \
                  @level0type = N'SCHEMA', @level0name = @P3, \
                  @level1type = N'VIEW', @level1name = @P4; \
         end else if @P2 <> N'' begin \
             exec {catalogo}.sp_addextendedproperty \
                  @name = N'{NOME_DA_PROPRIEDADE}', @value = @P2, \
                  @level0type = N'SCHEMA', @level0name = @P3, \
                  @level1type = N'VIEW', @level1name = @P4; \
         end"
    );

    let parametros: Vec<&dyn ToSql> = vec![&alvo, &comentario, &esquema, &objeto];
    cliente.execute(sql, &parametros).await?;
    Ok(())
}

/// Parte o `esquema.objeto` da árvore nos dois pedaços que as procedures de
/// propriedade estendida exigem em `@level0name` e `@level1name`.
///
/// Ao contrário do Oracle, aqui o esquema não é opcional: `@level0name` é
/// obrigatório. Sem ponto no nome — que a árvore não produz, mas um caminho
/// novo pode —, vale o `dbo`, que é o esquema padrão de quem não escolheu outro.
fn esquema_e_objeto(nome: &str) -> (String, String) {
    match nome.split_once('.') {
        Some((esquema, objeto)) => (esquema.trim().to_string(), objeto.trim().to_string()),
        None => ("dbo".to_string(), nome.trim().to_string()),
    }
}

/// O fonte do objeto: o texto guardado, ou o `CREATE TABLE` montado.
pub async fn fonte(
    cliente: &mut Cliente,
    banco: Option<&str>,
    categoria: Categoria,
    nome: &str,
) -> Resultado<String> {
    match categoria {
        Categoria::Tabelas => ddl_de_tabela(cliente, banco, nome).await,
        _ => fonte_do_modulo(cliente, banco, nome).await,
    }
}

/// View, procedure e função: o `CREATE` como ele foi executado.
///
/// Vem de `sys.sql_modules`, e não de `OBJECT_DEFINITION`, porque a coluna
/// existe também quando o módulo foi criado com `WITH ENCRYPTION` — nesse caso
/// vazia, e é isso que permite dizer *criptografado* em vez de *não encontrado*.
async fn fonte_do_modulo(
    cliente: &mut Cliente,
    banco: Option<&str>,
    nome: &str,
) -> Resultado<String> {
    let catalogo = qualificar(banco, "sys");
    let sql = format!(
        "select m.definition as fonte \
           from {catalogo}.sql_modules m \
          where m.object_id = object_id(@P1)"
    );

    let alvo = nome_completo(banco, nome);
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    let Some(linha) = linhas.first() else {
        return Err(Erro::novo(format!(
            "{nome} não foi encontrado neste banco"
        )));
    };
    match linha.get::<&str, _>("fonte") {
        Some(fonte) => Ok(fonte.trim_end().to_string()),
        None => Err(Erro::novo(format!(
            "o fonte de {nome} está criptografado (WITH ENCRYPTION) e o servidor não o devolve"
        ))),
    }
}

/// O `CREATE TABLE` montado a partir do catálogo.
///
/// Sai no formato do *Script Table as CREATE* do SSMS, na mesma ordem: colunas,
/// chave primária e restrições de unicidade dentro do `CREATE`; padrões, chaves
/// estrangeiras, `CHECK` e índices depois, cada um no seu `ALTER`/`CREATE`. É
/// script para ler, guardar e rodar em outro servidor — por isso os `GO`.
async fn ddl_de_tabela(
    cliente: &mut Cliente,
    banco: Option<&str>,
    nome: &str,
) -> Resultado<String> {
    let catalogo = qualificar(banco, "sys");
    let alvo = nome_completo(banco, nome);
    let tabela = identificar(cliente, &catalogo, &alvo, nome).await?;

    let mut script = String::new();
    if let Some(banco) = banco {
        script.push_str(&format!("USE {}\nGO\n\n", entre_colchetes(banco)));
    }
    script.push_str(&format!("CREATE TABLE {tabela} (\n"));

    let colunas = colunas_da_tabela(cliente, &catalogo, &alvo).await?;
    if colunas.is_empty() {
        return Err(Erro::novo(format!("{nome} não tem colunas legíveis")));
    }
    let chaves = chaves_da_tabela(cliente, &catalogo, &alvo).await?;

    let mut linhas: Vec<String> = colunas.iter().map(|coluna| coluna.declaracao()).collect();
    linhas.extend(chaves.iter().map(Chave::declaracao));
    script.push_str(&linhas.join(",\n"));
    script.push_str("\n)\nGO\n");

    // O padrão sai como restrição própria, e não colado na coluna: é assim que
    // ele existe no catálogo — com nome — e é o que permite removê-lo depois.
    for coluna in colunas.iter().filter(|c| c.padrao.is_some()) {
        let (nome_padrao, expressao) = coluna.padrao.as_ref().expect("filtrado acima");
        script.push_str(&format!(
            "\nALTER TABLE {tabela} ADD CONSTRAINT {} DEFAULT {expressao} FOR {}\nGO\n",
            entre_colchetes(nome_padrao),
            entre_colchetes(&coluna.nome),
        ));
    }

    for restricao in verificacoes(cliente, &catalogo, &alvo).await? {
        script.push_str(&format!(
            "\nALTER TABLE {tabela} WITH CHECK ADD CONSTRAINT {} CHECK {}\nGO\n",
            entre_colchetes(&restricao.0),
            restricao.1,
        ));
    }

    for chave in estrangeiras(cliente, &catalogo, &alvo).await? {
        script.push_str(&format!("\n{}\nGO\n", chave.declaracao(&tabela)));
    }

    for indice in indices(cliente, &catalogo, &alvo).await? {
        script.push_str(&format!("\n{}\nGO\n", indice.declaracao(&tabela)));
    }

    Ok(script)
}

/// `[esquema].[tabela]` como o catálogo o escreve — e a prova de que ele existe.
async fn identificar(
    cliente: &mut Cliente,
    catalogo: &str,
    alvo: &str,
    nome: &str,
) -> Resultado<String> {
    let sql = format!(
        "select e.name as esquema, o.name as nome \
           from {catalogo}.objects o \
           join {catalogo}.schemas e on e.schema_id = o.schema_id \
          where o.object_id = object_id(@P1)"
    );
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    let achado = linhas.first().and_then(|linha| {
        let esquema = linha.get::<&str, _>("esquema")?;
        let tabela = linha.get::<&str, _>("nome")?;
        Some(format!(
            "{}.{}",
            entre_colchetes(esquema),
            entre_colchetes(tabela)
        ))
    });
    achado.ok_or_else(|| Erro::novo(format!("{nome} não foi encontrado neste banco")))
}

/// Uma coluna, com o que a declaração dela precisa.
struct ColunaDdl {
    nome: String,
    tipo: String,
    aceita_nulo: bool,
    /// `IDENTITY(semente, incremento)`, quando é.
    identidade: Option<(i64, i64)>,
    /// Expressão da coluna calculada; ela não declara tipo nem nulidade.
    calculada: Option<String>,
    /// Nome e expressão do `DEFAULT`, que sai como restrição à parte.
    padrao: Option<(String, String)>,
}

impl ColunaDdl {
    fn declaracao(&self) -> String {
        let nome = entre_colchetes(&self.nome);
        if let Some(expressao) = &self.calculada {
            return format!("    {nome} AS {expressao}");
        }
        let identidade = match self.identidade {
            Some((semente, incremento)) => format!(" IDENTITY({semente},{incremento})"),
            None => String::new(),
        };
        let nulo = if self.aceita_nulo { "NULL" } else { "NOT NULL" };
        format!("    {nome} {}{identidade} {nulo}", self.tipo)
    }
}

async fn colunas_da_tabela(
    cliente: &mut Cliente,
    catalogo: &str,
    alvo: &str,
) -> Resultado<Vec<ColunaDdl>> {
    // `seed_value` e `increment_value` são `sql_variant`: sem o CAST o driver
    // não sabe que tipo está lendo.
    let sql = format!(
        "select c.name as nome, type_name(c.user_type_id) as tipo, \
                c.max_length as tamanho, c.precision as precisao, c.scale as escala, \
                c.is_nullable as aceita_nulo, c.is_identity as identidade, \
                cast(i.seed_value as bigint) as semente, \
                cast(i.increment_value as bigint) as incremento, \
                cc.definition as calculada, \
                d.name as nome_padrao, d.definition as padrao \
           from {catalogo}.columns c \
           left join {catalogo}.identity_columns i \
                  on i.object_id = c.object_id and i.column_id = c.column_id \
           left join {catalogo}.computed_columns cc \
                  on cc.object_id = c.object_id and cc.column_id = c.column_id \
           left join {catalogo}.default_constraints d on d.object_id = c.default_object_id \
          where c.object_id = object_id(@P1) \
          order by c.column_id"
    );
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    Ok(linhas
        .iter()
        .filter_map(|linha| {
            let nome = linha.get::<&str, _>("nome")?.to_string();
            let identidade = linha.get::<bool, _>("identidade").unwrap_or(false).then(|| {
                (
                    linha.get::<i64, _>("semente").unwrap_or(1),
                    linha.get::<i64, _>("incremento").unwrap_or(1),
                )
            });
            let padrao = linha
                .get::<&str, _>("nome_padrao")
                .zip(linha.get::<&str, _>("padrao"))
                .map(|(nome, expressao)| (nome.to_string(), expressao.to_string()));
            Some(ColunaDdl {
                nome,
                tipo: descrever_tipo(
                    linha.get::<&str, _>("tipo").unwrap_or(""),
                    linha.get::<i16, _>("tamanho").unwrap_or(0),
                    linha.get::<u8, _>("precisao").unwrap_or(0),
                    linha.get::<u8, _>("escala").unwrap_or(0),
                ),
                aceita_nulo: linha.get::<bool, _>("aceita_nulo").unwrap_or(true),
                identidade,
                calculada: linha.get::<&str, _>("calculada").map(str::to_string),
                padrao,
            })
        })
        .collect())
}

/// Chave primária ou restrição de unicidade, com as colunas dela.
struct Chave {
    nome: String,
    primaria: bool,
    /// `CLUSTERED` ou `NONCLUSTERED`.
    forma: String,
    colunas: Vec<String>,
}

impl Chave {
    fn declaracao(&self) -> String {
        let especie = if self.primaria {
            "PRIMARY KEY"
        } else {
            "UNIQUE"
        };
        format!(
            "    CONSTRAINT {} {especie} {} ({})",
            entre_colchetes(&self.nome),
            self.forma,
            self.colunas.join(", ")
        )
    }
}

async fn chaves_da_tabela(
    cliente: &mut Cliente,
    catalogo: &str,
    alvo: &str,
) -> Resultado<Vec<Chave>> {
    let sql = format!(
        "select k.name as nome, k.type as especie, i.type_desc as forma, \
                c.name as coluna, ic.is_descending_key as descendente \
           from {catalogo}.key_constraints k \
           join {catalogo}.indexes i \
                on i.object_id = k.parent_object_id and i.index_id = k.unique_index_id \
           join {catalogo}.index_columns ic \
                on ic.object_id = i.object_id and ic.index_id = i.index_id \
           join {catalogo}.columns c \
                on c.object_id = ic.object_id and c.column_id = ic.column_id \
          where k.parent_object_id = object_id(@P1) \
          order by k.type, k.name, ic.key_ordinal"
    );
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    let mut chaves: Vec<Chave> = Vec::new();
    for linha in linhas.iter() {
        let Some(nome) = linha.get::<&str, _>("nome") else {
            continue;
        };
        let coluna = coluna_ordenada(
            linha.get::<&str, _>("coluna").unwrap_or(""),
            linha.get::<bool, _>("descendente").unwrap_or(false),
        );
        match chaves.last_mut() {
            Some(atual) if atual.nome == nome => atual.colunas.push(coluna),
            _ => chaves.push(Chave {
                nome: nome.to_string(),
                primaria: linha.get::<&str, _>("especie").unwrap_or("").trim() == "PK",
                forma: linha
                    .get::<&str, _>("forma")
                    .unwrap_or("NONCLUSTERED")
                    .to_string(),
                colunas: vec![coluna],
            }),
        }
    }
    Ok(chaves)
}

/// Chave estrangeira, com as colunas dos dois lados.
struct Estrangeira {
    nome: String,
    colunas: Vec<String>,
    referencia: String,
    colunas_referidas: Vec<String>,
    ao_excluir: String,
    ao_atualizar: String,
}

impl Estrangeira {
    fn declaracao(&self, tabela: &str) -> String {
        let mut sql = format!(
            "ALTER TABLE {tabela} WITH CHECK ADD CONSTRAINT {} FOREIGN KEY ({})\n\
             REFERENCES {} ({})",
            entre_colchetes(&self.nome),
            self.colunas.join(", "),
            self.referencia,
            self.colunas_referidas.join(", ")
        );
        // `NO_ACTION` é o padrão do servidor e não se escreve, como o SSMS
        // também não escreve.
        for (acao, clausula) in [
            (&self.ao_excluir, "ON DELETE"),
            (&self.ao_atualizar, "ON UPDATE"),
        ] {
            if acao != "NO_ACTION" && !acao.is_empty() {
                sql.push_str(&format!("\n{clausula} {}", acao.replace('_', " ")));
            }
        }
        sql
    }
}

async fn estrangeiras(
    cliente: &mut Cliente,
    catalogo: &str,
    alvo: &str,
) -> Resultado<Vec<Estrangeira>> {
    let sql = format!(
        "select f.name as nome, ce.name as coluna, rc.name as coluna_referida, \
                re.name as esquema_referido, rt.name as tabela_referida, \
                f.delete_referential_action_desc as ao_excluir, \
                f.update_referential_action_desc as ao_atualizar \
           from {catalogo}.foreign_keys f \
           join {catalogo}.foreign_key_columns fc on fc.constraint_object_id = f.object_id \
           join {catalogo}.columns ce \
                on ce.object_id = fc.parent_object_id and ce.column_id = fc.parent_column_id \
           join {catalogo}.columns rc \
                on rc.object_id = fc.referenced_object_id \
               and rc.column_id = fc.referenced_column_id \
           join {catalogo}.objects rt on rt.object_id = f.referenced_object_id \
           join {catalogo}.schemas re on re.schema_id = rt.schema_id \
          where f.parent_object_id = object_id(@P1) \
          order by f.name, fc.constraint_column_id"
    );
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    let mut chaves: Vec<Estrangeira> = Vec::new();
    for linha in linhas.iter() {
        let Some(nome) = linha.get::<&str, _>("nome") else {
            continue;
        };
        let coluna = entre_colchetes(linha.get::<&str, _>("coluna").unwrap_or(""));
        let referida = entre_colchetes(linha.get::<&str, _>("coluna_referida").unwrap_or(""));
        match chaves.last_mut() {
            Some(atual) if atual.nome == nome => {
                atual.colunas.push(coluna);
                atual.colunas_referidas.push(referida);
            }
            _ => chaves.push(Estrangeira {
                nome: nome.to_string(),
                colunas: vec![coluna],
                referencia: format!(
                    "{}.{}",
                    entre_colchetes(linha.get::<&str, _>("esquema_referido").unwrap_or("dbo")),
                    entre_colchetes(linha.get::<&str, _>("tabela_referida").unwrap_or(""))
                ),
                colunas_referidas: vec![referida],
                ao_excluir: linha
                    .get::<&str, _>("ao_excluir")
                    .unwrap_or("NO_ACTION")
                    .to_string(),
                ao_atualizar: linha
                    .get::<&str, _>("ao_atualizar")
                    .unwrap_or("NO_ACTION")
                    .to_string(),
            }),
        }
    }
    Ok(chaves)
}

/// Restrições `CHECK`: nome e expressão, já entre parênteses pelo catálogo.
async fn verificacoes(
    cliente: &mut Cliente,
    catalogo: &str,
    alvo: &str,
) -> Resultado<Vec<(String, String)>> {
    let sql = format!(
        "select c.name as nome, c.definition as expressao \
           from {catalogo}.check_constraints c \
          where c.parent_object_id = object_id(@P1) \
          order by c.name"
    );
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    Ok(linhas
        .iter()
        .filter_map(|linha| {
            Some((
                linha.get::<&str, _>("nome")?.to_string(),
                linha.get::<&str, _>("expressao")?.to_string(),
            ))
        })
        .collect())
}

/// Índice comum — o que não nasceu de chave primária nem de restrição única.
struct Indice {
    nome: String,
    unico: bool,
    forma: String,
    colunas: Vec<String>,
    incluidas: Vec<String>,
}

impl Indice {
    fn declaracao(&self, tabela: &str) -> String {
        let unico = if self.unico { "UNIQUE " } else { "" };
        let mut sql = format!(
            "CREATE {unico}{} INDEX {} ON {tabela} ({})",
            self.forma,
            entre_colchetes(&self.nome),
            self.colunas.join(", ")
        );
        if !self.incluidas.is_empty() {
            sql.push_str(&format!("\nINCLUDE ({})", self.incluidas.join(", ")));
        }
        sql
    }
}

async fn indices(cliente: &mut Cliente, catalogo: &str, alvo: &str) -> Resultado<Vec<Indice>> {
    // `type in (1, 2)` é heap à parte: só índice agrupado e não agrupado. Os
    // demais (columnstore, XML, espacial) não se escrevem com esta sintaxe, e
    // um script errado é pior do que um índice de menos.
    let sql = format!(
        "select i.name as nome, i.is_unique as unico, i.type_desc as forma, \
                c.name as coluna, ic.is_descending_key as descendente, \
                ic.is_included_column as incluida \
           from {catalogo}.indexes i \
           join {catalogo}.index_columns ic \
                on ic.object_id = i.object_id and ic.index_id = i.index_id \
           join {catalogo}.columns c \
                on c.object_id = ic.object_id and c.column_id = ic.column_id \
          where i.object_id = object_id(@P1) \
            and i.is_primary_key = 0 and i.is_unique_constraint = 0 and i.type in (1, 2) \
          order by i.name, ic.is_included_column, ic.key_ordinal, ic.index_column_id"
    );
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    let mut indices: Vec<Indice> = Vec::new();
    for linha in linhas.iter() {
        let Some(nome) = linha.get::<&str, _>("nome") else {
            continue;
        };
        let bruta = linha.get::<&str, _>("coluna").unwrap_or("");
        let incluida = linha.get::<bool, _>("incluida").unwrap_or(false);
        // Coluna incluída não tem ordenação: ela não entra na chave do índice.
        let coluna = if incluida {
            entre_colchetes(bruta)
        } else {
            coluna_ordenada(bruta, linha.get::<bool, _>("descendente").unwrap_or(false))
        };

        let atual = match indices.last_mut() {
            Some(atual) if atual.nome == nome => atual,
            _ => {
                indices.push(Indice {
                    nome: nome.to_string(),
                    unico: linha.get::<bool, _>("unico").unwrap_or(false),
                    forma: linha
                        .get::<&str, _>("forma")
                        .unwrap_or("NONCLUSTERED")
                        .to_string(),
                    colunas: Vec::new(),
                    incluidas: Vec::new(),
                });
                indices.last_mut().expect("acabou de ser inserido")
            }
        };
        if incluida {
            atual.incluidas.push(coluna);
        } else {
            atual.colunas.push(coluna);
        }
    }
    Ok(indices)
}

/// `[nome] ASC` — a forma que o SSMS escreve dentro de chave e de índice.
fn coluna_ordenada(nome: &str, descendente: bool) -> String {
    format!(
        "{} {}",
        entre_colchetes(nome),
        if descendente { "DESC" } else { "ASC" }
    )
}

// --- Ir para definição (requisito 22) --------------------------------------

/// Em que categoria da árvore cada tipo do catálogo abre.
///
/// São os mesmos tipos que a árvore lista (requisito 2), e por isso os mesmos
/// que ela sabe abrir. O que fica de fora — restrição, gatilho, tipo de tabela —
/// não tem tela para onde ir.
fn categoria_do_tipo(tipo: &str) -> Option<Categoria> {
    match tipo.trim() {
        "U" => Some(Categoria::Tabelas),
        "V" => Some(Categoria::Views),
        "P" | "PC" => Some(Categoria::Procedures),
        "FN" | "AF" => Some(Categoria::FuncoesEscalares),
        "IF" | "TF" | "FT" => Some(Categoria::FuncoesTabela),
        _ => None,
    }
}

/// Um objeto encontrado no catálogo, e o sinônimo que ele porventura seja.
struct Achado {
    banco: Option<String>,
    esquema: String,
    nome: String,
    tipo: String,
    /// `base_object_name` do sinônimo: para onde ele aponta.
    base: String,
}

/// O destino de um Ctrl+clique no editor (requisito 22).
///
/// Quem resolve o nome é o próprio servidor, pelo `OBJECT_ID`: ele aceita o nome
/// como está escrito na consulta — `cliente`, `dbo.cliente`,
/// `Vendas.dbo.cliente` —, aplica o esquema padrão do login e devolve o nome na
/// grafia do catálogo. O banco é o da aba, a não ser que o nome traga outro.
///
/// Sinônimo é seguido **uma vez**, como no Oracle: um sinônimo para outro
/// sinônimo é raro e ficaria sem destino, em vez de virar um laço.
pub async fn resolver_referencia(
    cliente: &mut Cliente,
    banco: Option<&str>,
    referencia: &str,
) -> Resultado<Option<DestinoDaReferencia>> {
    let Some(achado) = achar(cliente, banco, referencia).await? else {
        return Ok(None);
    };
    let achado = if achado.tipo == "SN" {
        match achar(cliente, achado.banco.as_deref(), &achado.base).await? {
            Some(base) => base,
            None => return Ok(None),
        }
    } else {
        achado
    };

    let Some(categoria) = categoria_do_tipo(&achado.tipo) else {
        return Ok(None);
    };
    Ok(Some(DestinoDaReferencia {
        categoria,
        // Sempre com o esquema: é assim que a árvore nomeia o objeto, e é o que
        // o fonte e a estrutura recebem de volta (requisito 44).
        nome: format!("{}.{}", achado.esquema, achado.nome),
        // A parte é do pacote do PL/SQL; aqui o módulo é um só.
        parte: ParteDoFonte::Corpo,
        banco: achado.banco,
    }))
}

/// Acha no catálogo o objeto que o nome nomeia.
async fn achar(
    cliente: &mut Cliente,
    banco: Option<&str>,
    referencia: &str,
) -> Resultado<Option<Achado>> {
    let Some((banco_alvo, objeto)) = alvo_da_referencia(banco, referencia) else {
        return Ok(None);
    };
    let catalogo = qualificar(banco_alvo.as_deref(), "sys");
    let sql = format!(
        "select e.name as esquema, o.name as nome, o.type as tipo, \
                isnull(s.base_object_name, N'') as base \
           from {catalogo}.objects o \
           join {catalogo}.schemas e on e.schema_id = o.schema_id \
           left join {catalogo}.synonyms s on s.object_id = o.object_id \
          where o.object_id = object_id(@P1)"
    );

    let alvo = nome_completo(banco_alvo.as_deref(), &objeto);
    let parametros: Vec<&dyn ToSql> = vec![&alvo];
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;

    Ok(linhas.first().and_then(|linha| {
        Some(Achado {
            banco: banco_alvo.clone(),
            esquema: linha.get::<&str, _>("esquema")?.to_string(),
            nome: linha.get::<&str, _>("nome")?.to_string(),
            // `type` é `char(2)`: "U " vem com o espaço.
            tipo: linha.get::<&str, _>("tipo")?.trim().to_string(),
            base: linha.get::<&str, _>("base").unwrap_or_default().to_string(),
        })
    }))
}

/// Separa o banco do resto do nome, com o banco da aba quando ele não vem
/// escrito. Devolve `None` no que não é nome de objeto deste servidor.
fn alvo_da_referencia(banco: Option<&str>, referencia: &str) -> Option<(Option<String>, String)> {
    let partes = partes_da_referencia(referencia)?;
    match partes.as_slice() {
        // Quatro partes é servidor ligado, que o Cururu não abre.
        [escrito, esquema, objeto] => Some((
            Some(escrito.clone()),
            format!("{esquema}.{objeto}"),
        )),
        [esquema, objeto] => Some((
            banco.map(str::to_string),
            format!("{esquema}.{objeto}"),
        )),
        [objeto] => Some((banco.map(str::to_string), objeto.clone())),
        _ => None,
    }
}

/// Parte o nome em até três pedaços, sem os colchetes.
///
/// Serve ao que o editor manda e ao `base_object_name` do sinônimo, que o
/// catálogo guarda sempre entre colchetes. Devolve `None` no que não é nome —
/// quatro partes (servidor ligado), aspas, ponto e vírgula.
fn partes_da_referencia(referencia: &str) -> Option<Vec<String>> {
    let referencia = referencia.trim();
    if referencia.is_empty() {
        return None;
    }
    let mut partes = Vec::new();
    for parte in referencia.split('.') {
        let parte = parte.trim();
        let limpo = parte
            .strip_prefix('[')
            .and_then(|resto| resto.strip_suffix(']'))
            .unwrap_or(parte);
        if limpo.is_empty()
            || limpo.contains(['\'', '"', ';', '[', ']'])
            || limpo.chars().any(char::is_control)
        {
            return None;
        }
        partes.push(limpo.to_string());
    }
    (partes.len() <= 3).then_some(partes)
}

/// Quais destas referências o Ctrl+clique abre (requisito 22).
///
/// Uma consulta por banco citado, e não uma por nome: o `OBJECT_ID` resolve
/// cada nome como a consulta o resolveria, e o catálogo diz o tipo. O sinônimo
/// entra pelo destino dele, seguido uma vez, como no clique.
///
/// A consulta é por banco porque `object_id` só vale dentro do banco que o
/// resolveu: juntar o número de um banco com o catálogo de outro acharia o
/// objeto errado.
pub async fn referencias_navegaveis(
    cliente: &mut Cliente,
    banco: Option<&str>,
    referencias: &[String],
) -> Resultado<Vec<String>> {
    let mut por_banco: Vec<(Option<String>, Vec<(usize, String)>)> = Vec::new();
    for (indice, referencia) in referencias.iter().enumerate() {
        let Some((banco_alvo, objeto)) = alvo_da_referencia(banco, referencia) else {
            continue;
        };
        let nome = nome_completo(banco_alvo.as_deref(), &objeto);
        match por_banco.iter_mut().find(|(alvo, _)| *alvo == banco_alvo) {
            Some((_, nomes)) => nomes.push((indice, nome)),
            None => por_banco.push((banco_alvo, vec![(indice, nome)])),
        }
    }

    let mut navegaveis = Vec::new();
    for (banco_alvo, nomes) in por_banco {
        for achados in nomes.chunks(NOMES_POR_CONSULTA) {
            for indice in navegaveis_do_banco(cliente, banco_alvo.as_deref(), achados).await? {
                navegaveis.push(referencias[indice].clone());
            }
        }
    }
    Ok(navegaveis)
}

/// Quantos nomes vão numa consulta. O T-SQL para em 2100 parâmetros, e cada
/// nome gasta um.
const NOMES_POR_CONSULTA: usize = 500;

/// Os índices, entre os informados, cujos nomes levam a algum lugar.
async fn navegaveis_do_banco(
    cliente: &mut Cliente,
    banco: Option<&str>,
    nomes: &[(usize, String)],
) -> Resultado<Vec<usize>> {
    if nomes.is_empty() {
        return Ok(Vec::new());
    }
    let catalogo = qualificar(banco, "sys");
    let lista: Vec<String> = nomes
        .iter()
        .enumerate()
        .map(|(posicao, (indice, _))| format!("({indice}, @P{})", posicao + 1))
        .collect();
    let sql = format!(
        "select n.indice \
           from (values {}) as n(indice, nome) \
           join {catalogo}.objects o on o.object_id = object_id(n.nome) \
           left join {catalogo}.synonyms s on s.object_id = o.object_id \
           left join {catalogo}.objects b on b.object_id = object_id(s.base_object_name) \
          where isnull(b.type, o.type) in ('U', 'V', 'P', 'PC', 'FN', 'AF', 'IF', 'TF', 'FT')",
        lista.join(", ")
    );

    let parametros: Vec<&dyn ToSql> = nomes.iter().map(|(_, nome)| nome as &dyn ToSql).collect();
    let linhas = cliente
        .query(sql, &parametros)
        .await?
        .into_first_result()
        .await?;
    Ok(linhas
        .iter()
        .filter_map(|linha| linha.get::<i32, _>("indice").map(|indice| indice as usize))
        .collect())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_nome_se_parte_em_banco_esquema_e_objeto() {
        assert_eq!(
            alvo_da_referencia(Some("Vendas"), "cliente"),
            Some((Some("Vendas".to_string()), "cliente".to_string()))
        );
        assert_eq!(
            alvo_da_referencia(Some("Vendas"), "dbo.cliente"),
            Some((Some("Vendas".to_string()), "dbo.cliente".to_string()))
        );
        // O banco escrito manda no banco da aba.
        assert_eq!(
            alvo_da_referencia(Some("Vendas"), "Compras.dbo.cliente"),
            Some((Some("Compras".to_string()), "dbo.cliente".to_string()))
        );
        // Como o catálogo guarda o destino de um sinônimo.
        assert_eq!(
            alvo_da_referencia(None, "[Compras].[dbo].[cliente]"),
            Some((Some("Compras".to_string()), "dbo.cliente".to_string()))
        );
    }

    #[test]
    fn o_que_nao_e_nome_de_objeto_nao_vira_consulta() {
        // Servidor ligado tem quatro partes.
        assert_eq!(partes_da_referencia("[srv].[bd].[dbo].[cli]"), None);
        assert_eq!(partes_da_referencia("cli'ente"), None);
        assert_eq!(partes_da_referencia("cliente; drop table x"), None);
        assert_eq!(partes_da_referencia("Vendas..cliente"), None);
        assert_eq!(partes_da_referencia(""), None);
    }

    #[test]
    fn so_os_tipos_que_a_arvore_abre_tem_destino() {
        assert_eq!(categoria_do_tipo("U "), Some(Categoria::Tabelas));
        assert_eq!(categoria_do_tipo("V "), Some(Categoria::Views));
        assert_eq!(categoria_do_tipo("P "), Some(Categoria::Procedures));
        assert_eq!(categoria_do_tipo("IF"), Some(Categoria::FuncoesTabela));
        assert_eq!(categoria_do_tipo("FN"), Some(Categoria::FuncoesEscalares));
        // Sinônimo é seguido antes, e gatilho não tem tela.
        assert_eq!(categoria_do_tipo("SN"), None);
        assert_eq!(categoria_do_tipo("TR"), None);
    }

    #[test]
    fn a_coluna_sai_com_tipo_nulidade_e_identidade() {
        let coluna = ColunaDdl {
            nome: "id".to_string(),
            tipo: "int".to_string(),
            aceita_nulo: false,
            identidade: Some((1, 1)),
            calculada: None,
            padrao: None,
        };
        assert_eq!(coluna.declaracao(), "    [id] int IDENTITY(1,1) NOT NULL");
    }

    #[test]
    fn a_coluna_calculada_nao_declara_tipo() {
        let coluna = ColunaDdl {
            nome: "total".to_string(),
            tipo: "numeric(10,2)".to_string(),
            aceita_nulo: true,
            identidade: None,
            calculada: Some("([preco]*[quantidade])".to_string()),
            padrao: None,
        };
        assert_eq!(coluna.declaracao(), "    [total] AS ([preco]*[quantidade])");
    }

    #[test]
    fn a_chave_primaria_leva_a_forma_e_as_colunas() {
        let chave = Chave {
            nome: "PK_cliente".to_string(),
            primaria: true,
            forma: "CLUSTERED".to_string(),
            colunas: vec!["[id] ASC".to_string()],
        };
        assert_eq!(
            chave.declaracao(),
            "    CONSTRAINT [PK_cliente] PRIMARY KEY CLUSTERED ([id] ASC)"
        );
    }

    #[test]
    fn a_acao_padrao_da_estrangeira_nao_e_escrita() {
        let chave = Estrangeira {
            nome: "FK_pedido_cliente".to_string(),
            colunas: vec!["[cliente_id]".to_string()],
            referencia: "[dbo].[cliente]".to_string(),
            colunas_referidas: vec!["[id]".to_string()],
            ao_excluir: "NO_ACTION".to_string(),
            ao_atualizar: "CASCADE".to_string(),
        };
        let sql = chave.declaracao("[dbo].[pedido]");
        assert!(sql.contains("ON UPDATE CASCADE"), "{sql}");
        assert!(!sql.contains("ON DELETE"), "{sql}");
    }

    #[test]
    fn o_indice_separa_colunas_de_chave_das_incluidas() {
        let indice = Indice {
            nome: "IX_pedido_data".to_string(),
            unico: false,
            forma: "NONCLUSTERED".to_string(),
            colunas: vec!["[data] DESC".to_string()],
            incluidas: vec!["[total]".to_string()],
        };
        assert_eq!(
            indice.declaracao("[dbo].[pedido]"),
            "CREATE NONCLUSTERED INDEX [IX_pedido_data] ON [dbo].[pedido] ([data] DESC)\n\
             INCLUDE ([total])"
        );
    }
}
