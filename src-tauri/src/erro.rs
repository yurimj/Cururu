//! Erro único do backend, já em português e pronto para exibição na tela.
//!
//! Erros do Oracle preservam o código `ORA-` e a posição dentro do comando,
//! para o painel de mensagens poder saltar até a linha que falhou
//! (requisito 18).

use serde::Serialize;

use crate::execucao::ResultadoDaExecucao;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Erro {
    pub mensagem: String,
    /// Código do banco, quando houver: `ORA-01017`, `DPI-1047`…
    pub codigo: Option<String>,
    /// Deslocamento do erro dentro do comando SQL, quando o Oracle informa.
    pub posicao: Option<u32>,
    /// Linha onde o erro caiu, contada no texto executado — a tela soma a linha
    /// em que esse texto começa no editor, para o salto do requisito 18.
    ///
    /// O SQL Server conta a linha dentro do lote, e quem executa soma a linha em
    /// que o lote começa. O Oracle informa o deslocamento dentro do comando, e
    /// quem executa o converte na linha do comando que falhou.
    pub linha: Option<u32>,
    /// A conexão tinha caído e foi restabelecida ao tratar deste erro.
    ///
    /// O comando não valeu, mas a sessão já está de pé outra vez: a tela usa
    /// isto para dizer que basta executar de novo, em vez de deixar o usuário
    /// achando que precisa fechar a aba.
    pub reconectado: bool,
    /// O que a execução já tinha devolvido quando o erro chegou.
    ///
    /// No T-SQL um lote manda vários conjuntos de resultados e só **depois**
    /// falha — uma procedure de depuração que mostra dez grids e estoura no
    /// último comando, por exemplo. Jogar fora o que já veio é perder informação
    /// que o servidor mandou e não volta: a aba mostra os grids e o erro juntos.
    ///
    /// Fica de fora do JSON quando está vazio, que é o caso de quase todo erro.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resultados: Vec<ResultadoDaExecucao>,
}

pub type Resultado<T> = Result<T, Erro>;

impl Erro {
    pub fn novo(mensagem: impl Into<String>) -> Self {
        Erro {
            mensagem: mensagem.into(),
            codigo: None,
            posicao: None,
            linha: None,
            reconectado: false,
            resultados: Vec::new(),
        }
    }

    /// Marca o erro de um comando que caiu junto com a conexão, depois de a
    /// sessão ter sido reaberta.
    pub fn reconectado(mut self) -> Self {
        self.reconectado = true;
        self
    }

    /// Guarda no erro o que a execução já tinha produzido antes de falhar.
    ///
    /// A ordem é a da saída do banco: `anteriores` são os lotes que terminaram,
    /// e o que já estava aqui é do lote que falhou no meio.
    pub fn apos(mut self, mut anteriores: Vec<ResultadoDaExecucao>) -> Self {
        if anteriores.is_empty() {
            return self;
        }
        anteriores.append(&mut self.resultados);
        self.resultados = anteriores;
        self
    }
}

impl std::fmt::Display for Erro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.mensagem)
    }
}

impl From<oracle::Error> for Erro {
    fn from(erro: oracle::Error) -> Self {
        match erro.db_error() {
            // A mensagem do Oracle já vem prefixada pelo código
            // ("ORA-01017: nome de usuário/senha inválidos"), então não
            // repetimos o prefixo no texto.
            Some(banco) => {
                let codigo = (banco.code() != 0).then(|| format!("ORA-{:05}", banco.code()));
                Erro {
                    mensagem: banco.message().trim().to_string(),
                    codigo,
                    posicao: (banco.offset() != 0).then_some(banco.offset()),
                    linha: None,
                    reconectado: false,
                    resultados: Vec::new(),
                }
            }
            None => Erro::novo(erro.to_string()),
        }
    }
}

impl From<tiberius::error::Error> for Erro {
    fn from(erro: tiberius::error::Error) -> Self {
        match erro {
            // Erro vindo do próprio servidor: `18456` é login recusado, `4060`
            // é banco inacessível. O número é o equivalente do `ORA-`, e o
            // painel de mensagens o mostra do mesmo jeito.
            // A linha vem contada **dentro do lote**; quem executa a converte
            // para a linha do editor antes de a tela saltar até ela.
            tiberius::error::Error::Server(token) => Erro {
                mensagem: token.message().trim().to_string(),
                codigo: Some(format!("MSSQL-{}", token.code())),
                posicao: None,
                linha: (token.line() != 0).then_some(token.line()),
                reconectado: false,
                resultados: Vec::new(),
            },
            // O Windows já devolve o texto do erro de rede traduzido ("Este host
            // não é conhecido"); o `tiberius` o embrulha num prefixo em inglês
            // que não acrescenta nada e quebra o idioma da tela.
            tiberius::error::Error::Io { message, .. } => Erro::novo(message),
            // O texto do TLS sozinho ("invalid certificate") não diz a quem se
            // refere, e é o erro que aparece quando o servidor exige cifra que
            // o Windows já não aceita — comum em SQL Server antigo.
            tiberius::error::Error::Tls(detalhe) => {
                Erro::novo(format!("falha ao cifrar a conexão: {detalhe}"))
            }
            outro => Erro::novo(outro.to_string()),
        }
    }
}

impl From<std::io::Error> for Erro {
    fn from(erro: std::io::Error) -> Self {
        Erro::novo(erro.to_string())
    }
}

impl From<serde_json::Error> for Erro {
    fn from(erro: serde_json::Error) -> Self {
        Erro::novo(format!("arquivo de conexões inválido: {erro}"))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn resultado(comando: &str) -> ResultadoDaExecucao {
        ResultadoDaExecucao {
            comando: comando.to_string(),
            linha_no_editor: 1,
            duracao_ms: 0,
            consulta: None,
            linhas_afetadas: None,
            transacao: crate::execucao::Transacao::Inalterada,
            mensagem: String::new(),
            saida_dbms: Vec::new(),
            reconectado: false,
            erros_de_compilacao: Vec::new(),
        }
    }

    fn comandos(erro: &Erro) -> Vec<&str> {
        erro.resultados.iter().map(|r| r.comando.as_str()).collect()
    }

    #[test]
    fn erro_comum_nao_carrega_resultado_nenhum() {
        assert!(Erro::novo("falhou").resultados.is_empty());
    }

    #[test]
    fn o_que_ja_tinha_vindo_fica_na_ordem_da_saida_do_banco() {
        // O lote que falhou já tinha mandado o "select b"; os lotes anteriores
        // do script vêm antes dele, como sairiam na tela.
        let erro = Erro {
            resultados: vec![resultado("select b")],
            ..Erro::novo("uncommittable transaction")
        };
        let erro = erro.apos(vec![resultado("select a")]);
        assert_eq!(comandos(&erro), ["select a", "select b"]);
    }

    #[test]
    fn guardar_nada_deixa_o_erro_como_estava() {
        let erro = Erro {
            resultados: vec![resultado("select b")],
            ..Erro::novo("uncommittable transaction")
        };
        assert_eq!(comandos(&erro.apos(Vec::new())), ["select b"]);
    }
}
