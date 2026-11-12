/** Tipos trocados entre a interface e o backend em Rust. */

/** Banco de dados a que a conexão se destina. */
export type TipoBanco = "oracle" | "sqlserver" | "mongodb";

/**
 * Modo de autenticação do SQL Server, como no SSMS.
 *
 * `windows` sem usuário informado é a autenticação integrada: vale a identidade
 * de quem está logado no Windows.
 */
export type AutenticacaoSql = "sqlserver" | "windows";

/** Atribuição do usuário na conexão (o "role" do Oracle). */
export type Atribuicao = "padrao" | "sysdba" | "sysoper";

/** Modo de montagem dos dados de conexão. */
export type TipoConexao = "basico" | "tns" | "avancado";

/** Como o banco é identificado no modo Básico. */
export type IdentificacaoBanco = "sid" | "servico";

/** Tipo de autenticação. Hoje só o padrão do Oracle é aceito. */
export type TipoAutenticacao = "padrao";

/**
 * Uma conexão salva.
 *
 * A senha nunca trafega em texto puro para o disco: o backend a protege com o
 * DPAPI do Windows. Aqui ela existe apenas em memória, enquanto a tela está
 * aberta.
 */
export interface PerfilConexao {
  id: string;
  nome: string;
  /** Cor de identificação em `#rrggbb`; `null` só existe em perfis antigos. */
  cor: string | null;
  /** Decide quais campos da tela valem. */
  tipoBanco: TipoBanco;
  tipoAutenticacao: TipoAutenticacao;
  usuario: string;
  senha: string | null;
  salvarSenha: boolean;
  /**
   * Indica que existe senha guardada para este perfil, protegida pelo DPAPI.
   */
  temSenhaSalva: boolean;
  atribuicao: Atribuicao;
  tipoConexao: TipoConexao;
  host: string;
  porta: number;
  identificacao: IdentificacaoBanco;
  sid: string;
  servico: string;
  /** Alias do tnsnames.ora, usado quando `tipoConexao` é `tns`. */
  aliasTns: string;
  /** Cadeia de conexão completa, usada quando `tipoConexao` é `avancado`. */
  urlPersonalizada: string;
  usarProxy: boolean;
  usuarioProxy: string;
  senhaProxy: string | null;
  /**
   * "Nome do Servidor" do SSMS: `HOST`, `HOST\INSTANCIA` ou `HOST,porta`.
   *
   * No MongoDB, guarda a URI sem credenciais.
   */
  servidor: string;
  /**
   * Autenticação do SQL Server ou do Windows.
   *
   * `usuario`, `senha` e `salvarSenha` são os mesmos campos do Oracle: só o
   * rótulo da tela muda ("Logon" ou "Nome de usuário").
   */
  autenticacaoSql: AutenticacaoSql;
  /**
   * Auto-commit da conexão. Desligado por padrão, como no SQL Developer.
   * No Oracle isso é comportamento do cliente, não do servidor.
   */
  autoCommit: boolean;
}

/** Resultado de um teste de conexão. */
export interface ResultadoTeste {
  sucesso: boolean;
  mensagem: string;
  /** Versão do banco, quando a conexão deu certo. */
  versaoBanco: string | null;
  duracaoMs: number;
}

/** Cria um perfil vazio, com os padrões que o SQL Developer usa. */
export function perfilNovo(): PerfilConexao {
  return {
    id: "",
    nome: "",
    cor: null,
    tipoBanco: "oracle",
    tipoAutenticacao: "padrao",
    usuario: "",
    senha: null,
    salvarSenha: false,
    temSenhaSalva: false,
    atribuicao: "padrao",
    tipoConexao: "basico",
    host: "localhost",
    porta: 1521,
    identificacao: "servico",
    sid: "",
    servico: "",
    aliasTns: "",
    urlPersonalizada: "",
    usarProxy: false,
    usuarioProxy: "",
    senhaProxy: null,
    servidor: "",
    autenticacaoSql: "sqlserver",
    autoCommit: false,
  };
}

/** Resumo mostrado na coluna "Detalhes da Conexão" da lista. */
export function detalhesDaConexao(perfil: PerfilConexao): string {
  if (perfil.tipoBanco === "mongodb") return perfil.servidor;
  if (perfil.tipoBanco === "sqlserver") {
    // Na autenticação do Windows sem usuário informado quem conecta é a
    // identidade do Windows, e não há nome a mostrar antes do servidor.
    return perfil.usuario ? `${perfil.usuario}@${perfil.servidor}` : perfil.servidor;
  }

  switch (perfil.tipoConexao) {
    case "tns":
      return `${perfil.usuario}@${perfil.aliasTns}`;
    case "avancado":
      return `${perfil.usuario}@${perfil.urlPersonalizada}`;
    default: {
      const alvo = perfil.identificacao === "sid" ? perfil.sid : perfil.servico;
      return `${perfil.usuario}@//${perfil.host}:${perfil.porta}/${alvo}`;
    }
  }
}
