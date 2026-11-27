/** Ponte com o backend em Rust. Um lugar só para todos os comandos. */

import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

import { t } from "./idioma";
import { opcoesDeAutocomplete } from "./opcoes-autocomplete";
import type { PerfilConexao, ResultadoTeste } from "./tipos";

export interface SituacaoInstantClient {
  diretorio: string | null;
  problema: string | null;
  procuradoEm: string[];
}

export function diagnosticoInstantClient(): Promise<SituacaoInstantClient> {
  return invoke("diagnostico_instantclient");
}

/**
 * O idioma escolhido no instalador, ou `null` quando não há um gravado.
 *
 * Vale só na primeira abertura (requisito 31): depois que alguém escolhe pelo
 * menu Language, é a escolha guardada na máquina que manda.
 */
export function idiomaDoInstalador(): Promise<string | null> {
  return invoke("idioma_do_instalador");
}

/** Versão e data de compilação do executável em uso. */
export function versaoEmExecucao(): Promise<string> {
  return invoke("versao_em_execucao");
}

/** Abre um endereço no navegador padrão do Windows, fora da janela do Cururu. */
export function abrirNoNavegador(endereco: string): Promise<void> {
  return invoke("abrir_no_navegador", { endereco });
}

// --- Atualização ------------------------------------------------------------

/** Versão publicada no repositório de releases, quando é mais nova que esta. */
export interface Atualizacao {
  versao: string;
  /** Versão em execução, para a tela mostrar "0.1.10 → 0.1.11". */
  versaoAtual: string;
  data: string;
  /** Corpo do release em markdown: a lista do que mudou. */
  notas: string;
  instalador: string;
  /** Tamanho do instalador em bytes. */
  tamanho: number;
  /** Página do release no GitHub. */
  pagina: string;
}

export interface AndamentoDoDownload {
  baixados: number;
  /** Total anunciado pelo servidor, ou zero quando ele não informa. */
  total: number;
}

/** `null` quando já se está na versão mais recente — resposta boa, não erro. */
export function verificarAtualizacao(): Promise<Atualizacao | null> {
  return invoke("verificar_atualizacao");
}

/** Baixa o instalador, chamando `andamento` conforme o arquivo desce. */
export function baixarAtualizacao(
  endereco: string,
  andamento: (progresso: AndamentoDoDownload) => void,
): Promise<void> {
  const canal = new Channel<AndamentoDoDownload>();
  canal.onmessage = andamento;
  return invoke("baixar_atualizacao", { endereco, andamento: canal });
}

/**
 * Executa o instalador já baixado.
 *
 * Não recebe caminho: quem baixou foi o backend, e é ele quem sabe onde o
 * arquivo está. Assim o comando que roda um programa não aceita caminho vindo
 * da tela.
 */
export function instalarAtualizacao(): Promise<void> {
  return invoke("instalar_atualizacao");
}

/** Fecha o Cururu para o instalador poder substituir os arquivos em uso. */
export function encerrarAplicativo(): Promise<void> {
  return invoke("encerrar_aplicativo");
}

export function listarPerfis(): Promise<PerfilConexao[]> {
  return invoke("listar_perfis");
}

export function lerSenhaSalva(id: string): Promise<string | null> {
  return invoke("ler_senha_salva", { id });
}

export function salvarPerfil(perfil: PerfilConexao): Promise<PerfilConexao> {
  return invoke("salvar_perfil", { perfil });
}

export function excluirPerfil(id: string): Promise<void> {
  return invoke("excluir_perfil", { id });
}

export function testarConexao(perfil: PerfilConexao): Promise<ResultadoTeste> {
  return invoke(perfil.tipoBanco === "mongodb" ? "testar_mongo" : "testar_conexao", { perfil });
}

/** Abre a conexão e devolve o identificador da sessão criada. */
export function conectar(perfil: PerfilConexao): Promise<string> {
  return invoke(perfil.tipoBanco === "mongodb" ? "conectar_mongo" : "conectar", { perfil });
}

export type ValorJson = null | boolean | number | string | ValorJson[] | { [campo: string]: ValorJson };

export function listarBancosMongo(sessaoId: string): Promise<string[]> {
  return invoke("listar_bancos_mongo", { sessaoId });
}

export function listarColecoesMongo(sessaoId: string, banco: string): Promise<string[]> {
  return invoke("listar_colecoes_mongo", { sessaoId, banco });
}

export function consultarMongo(
  sessaoId: string, banco: string, colecao: string, filtro: string, limite: number,
): Promise<ValorJson[]> {
  return invoke("consultar_mongo", { sessaoId, banco, colecao, filtro, limite });
}

export function camposMongo(sessaoId: string, banco: string, colecao: string): Promise<string[]> {
  return invoke("campos_mongo", { sessaoId, banco, colecao });
}

/**
 * Abre outra sessão na mesma conexão da sessão indicada — uma por aba de
 * consulta (requisito 7). Reaproveita as credenciais já resolvidas, sem repedir
 * a senha. `sessaoId` é o da sessão de metadados (a que a árvore abriu).
 *
 * `banco` abre a aba num banco específico do SQL Server, e não no da conexão: é
 * o caminho de "Nova Consulta" sobre um banco da árvore.
 */
export function conectarIrma(sessaoId: string, banco?: string): Promise<string> {
  return invoke("conectar_irma", { sessaoId, banco: banco ?? null });
}

/**
 * Bancos do servidor a que a conexão tem acesso (SQL Server).
 *
 * `incluirSistema` separa os dois usos: a árvore esconde `master`, `tempdb`,
 * `model` e `msdb`; a lista da aba de consulta os oferece, porque consultar o
 * catálogo do `master` é trabalho legítimo de quem administra.
 */
export function listarBancos(sessaoId: string, incluirSistema = false): Promise<string[]> {
  return invoke("listar_bancos", { sessaoId, incluirSistema });
}

export interface RegraDeAlerta {
  id: string;
  nome: string;
  banco: string | null;
  minutos: number;
  repetirApos: number | null;
  consulta: string;
  ativo: boolean;
}

export interface AlertaSalvo extends RegraDeAlerta {
  temTeams: boolean;
  contagemLegada: boolean;
}

export function listarAlertas(sessaoId: string): Promise<AlertaSalvo[]> {
  return invoke("listar_alertas", { sessaoId });
}

export function lerLogsAlerta(sessaoId: string, id: string): Promise<string[]> {
  return invoke("ler_logs_alerta", { sessaoId, id });
}

export function lerWebhookAlerta(sessaoId: string, id: string): Promise<string | null> {
  return invoke("ler_webhook_alerta", { sessaoId, id });
}

export function salvarAlerta(
  sessaoId: string,
  regra: RegraDeAlerta,
  teams: boolean,
  webhook: string,
): Promise<AlertaSalvo> {
  return invoke("salvar_alerta", { sessaoId, regra, teams, webhook });
}

export function excluirAlerta(sessaoId: string, id: string): Promise<void> {
  return invoke("excluir_alerta", { sessaoId, id });
}

export function verificarAlerta(
  sessaoId: string, id: string, manual = false,
): Promise<{ mensagem: string | null } | null> {
  return invoke("verificar_alerta", { sessaoId, id, manual });
}

export function notificarAlertaNoTeams(sessaoId: string, id: string, mensagem: string, manual = false): Promise<boolean> {
  return invoke("notificar_alerta_teams", { sessaoId, id, mensagem, manual });
}

/**
 * Troca o banco em que a aba executa. Vale só para a sessão indicada — a de uma
 * aba (requisito 7) —, como o `USE` do SSMS vale só para a janela em que foi
 * dado.
 */
export function usarBanco(sessaoId: string, banco: string, sessaoMetadados: string): Promise<void> {
  return invoke("usar_banco", { sessaoId, banco, sessaoMetadados });
}

/**
 * Banco em que a sessão está agora, ou `null` numa conexão Oracle — que não tem
 * banco corrente. A barra da aba de consulta o mostra no lugar da Saída DBMS.
 */
export function bancoDaSessao(sessaoId: string): Promise<string | null> {
  return invoke("banco_da_sessao", { sessaoId });
}

// --- SQL Server Agent (requisito 36) --------------------------------------

/**
 * Situação do serviço SQL Server Agent.
 *
 * `desconhecido` não é o mesmo que `parado`: é a conta não ter permissão de
 * olhar o serviço. A árvore desenha o nó desconectado só no `parado`, porque
 * dizer "parado" a quem apenas não pode ver mandaria o usuário procurar defeito
 * num serviço que está funcionando.
 */
export type EstadoDoAgente = "ativo" | "parado" | "desconhecido";

/** Como terminou a última execução de um trabalho. */
export type ResultadoDoTrabalho =
  | "falha"
  | "sucesso"
  | "novaTentativa"
  | "cancelado"
  | "desconhecido";

/**
 * O que o Agent está fazendo com o trabalho **agora**.
 *
 * São os estados do próprio Agent, com os nomes que o SSMS lhes dá na janela de
 * atividade dos trabalhos.
 */
export type SituacaoDaExecucao =
  | "executando"
  | "aguardandoThread"
  | "entreTentativas"
  | "parado"
  | "suspenso"
  | "finalizando"
  | "desconhecido";

/** Em que pé está a execução de um trabalho, para a tela de acompanhamento. */
export interface ExecucaoDoTrabalho {
  /** `true` enquanto o Agent está com o trabalho na mão. */
  executando: boolean;
  situacao: SituacaoDaExecucao;
  /** A etapa em curso como o Agent a escreve — `2 (Carregar)`; vazia se parado. */
  etapa: string;
  /**
   * A instância do histórico da última execução **concluída**, ou 0 se o
   * trabalho nunca rodou. É o que diz se a execução acompanhada já terminou: a
   * do fim é maior que a de antes do disparo.
   */
  instancia: number;
  resultado: ResultadoDoTrabalho;
  /** A mensagem que o Agent gravou no histórico — a mesma que o SSMS mostra. */
  mensagem: string;
  /** `yyyy-MM-dd HH:mm:ss` do início dessa execução. */
  iniciouEm: string | null;
  /** Quanto ela durou, em segundos, segundo o próprio servidor. */
  duracao: number | null;
}

/** Um trabalho do Agent, como a árvore o mostra. */
export interface Trabalho {
  id: string;
  nome: string;
  habilitado: boolean;
  descricao: string;
  categoria: string;
  proprietario: string;
  /** `yyyy-MM-dd HH:mm:ss`, ou `null` quando o servidor não informa. */
  criadoEm: string | null;
  modificadoEm: string | null;
  /** `null` quando o trabalho nunca rodou. */
  ultimaExecucao: string | null;
  resultado: ResultadoDoTrabalho;
}

/** O que a tela de Novo Trabalho precisa saber antes de abrir. */
export interface DadosDeTrabalho {
  categorias: string[];
  logins: string[];
  loginAtual: string;
  /** Nome do servidor: é o que a tela mostra em **Origem**. */
  servidor: string;
}

/** Uma etapa a criar. */
export interface EtapaDeTrabalho {
  nome: string;
  /** O `@subsystem` do `sp_add_jobstep`. */
  tipo: "TSQL" | "CmdExec" | "PowerShell";
  /** Só vale no `TSQL`. */
  banco: string | null;
  comando: string;
  /** 1 = sai com êxito, 2 = sai com falha, 3 = vai para a próxima etapa. */
  aoTerExito: number;
  aoFalhar: number;
}

/**
 * Uma agenda, nos mesmos números do `sp_add_jobschedule`.
 *
 * A tela fala a língua da procedure de propósito — é onde o SSMS também traduz
 * "toda segunda às 3h" para `freqTipo: 8, freqIntervalo: 2`. Um vocabulário
 * intermediário só acrescentaria mais uma tradução para errar.
 */
export interface AgendaDeTrabalho {
  nome: string;
  habilitada: boolean;
  freqTipo: number;
  freqIntervalo: number;
  freqSubtipo: number;
  freqSubintervalo: number;
  freqIntervaloRelativo: number;
  freqFator: number;
  /** `yyyyMMdd`. */
  inicioData: number;
  fimData: number;
  /** `HHmmss`. */
  inicioHora: number;
  fimHora: number;
}

/** Um trabalho a criar, com as etapas e agendas dele. */
export interface NovoTrabalho {
  nome: string;
  proprietario: string | null;
  categoria: string | null;
  descricao: string;
  habilitado: boolean;
  etapas: EtapaDeTrabalho[];
  agendas: AgendaDeTrabalho[];
}

/** Um trabalho por inteiro, com as etapas e as agendas dele. */
export interface DetalheDoTrabalho {
  id: string;
  nome: string;
  habilitado: boolean;
  descricao: string;
  categoria: string;
  proprietario: string;
  criadoEm: string | null;
  modificadoEm: string | null;
  ultimaExecucao: string | null;
  etapas: EtapaDeTrabalho[];
  agendas: AgendaDeTrabalho[];
}

export function estadoDoAgente(sessaoId: string): Promise<EstadoDoAgente> {
  return invoke("estado_do_agente", { sessaoId });
}

export function listarTrabalhos(sessaoId: string): Promise<Trabalho[]> {
  return invoke("listar_trabalhos", { sessaoId });
}

export function dadosDeTrabalho(sessaoId: string): Promise<DadosDeTrabalho> {
  return invoke("dados_de_trabalho", { sessaoId });
}

/** Cria o trabalho, as etapas e as agendas de uma vez — tudo ou nada. */
export function criarTrabalho(sessaoId: string, trabalho: NovoTrabalho): Promise<void> {
  return invoke("criar_trabalho", { sessaoId, trabalho });
}

/** Um trabalho por inteiro, para a tela de propriedades. */
export function detalheDoTrabalho(
  sessaoId: string,
  trabalhoId: string,
): Promise<DetalheDoTrabalho> {
  return invoke("detalhe_do_trabalho", { sessaoId, trabalhoId });
}

/**
 * O script que recria o trabalho, no formato do "Script Job as CREATE" do SSMS.
 *
 * Quem monta o texto é o backend, a partir da mesma leitura da tela de
 * propriedades: assim o formato fica testável sem um servidor por perto.
 */
export function ddlDoTrabalho(sessaoId: string, trabalhoId: string): Promise<string> {
  return invoke("ddl_do_trabalho", { sessaoId, trabalhoId });
}

/** Grava o que a tela de propriedades mudou — também tudo ou nada. */
export function atualizarTrabalho(
  sessaoId: string,
  trabalhoId: string,
  trabalho: NovoTrabalho,
): Promise<void> {
  return invoke("atualizar_trabalho", { sessaoId, trabalhoId, trabalho });
}

/** Liga ou desliga o trabalho, sem tocar em mais nada dele. */
export function habilitarTrabalho(
  sessaoId: string,
  trabalhoId: string,
  habilitado: boolean,
): Promise<void> {
  return invoke("habilitar_trabalho", { sessaoId, trabalhoId, habilitado });
}

export function renomearTrabalho(
  sessaoId: string,
  trabalhoId: string,
  nome: string,
): Promise<void> {
  return invoke("renomear_trabalho", { sessaoId, trabalhoId, nome });
}

/** Exclui o trabalho e, junto, as agendas que ficarem sem dono. */
export function excluirTrabalho(sessaoId: string, trabalhoId: string): Promise<void> {
  return invoke("excluir_trabalho", { sessaoId, trabalhoId });
}

/**
 * Manda o Agent rodar o trabalho, opcionalmente a partir de uma etapa.
 *
 * O comando **agenda** a execução e volta na hora: quem executa é o serviço, e
 * ele não avisa quando termina. Quem descobre o fim é `execucaoDoTrabalho`,
 * perguntando.
 */
export function iniciarTrabalho(
  sessaoId: string,
  trabalhoId: string,
  etapa?: string,
): Promise<void> {
  return invoke("iniciar_trabalho", { sessaoId, trabalhoId, etapa: etapa ?? null });
}

/**
 * Em que pé está a execução do trabalho.
 *
 * `instanciaConhecida` é a instância do histórico que a tela viu antes de
 * disparar, e serve ao log: é o que deixa o backend registrar o fim **desta**
 * execução, e não o resultado da anterior.
 */
export function execucaoDoTrabalho(
  sessaoId: string,
  trabalhoId: string,
  instanciaConhecida?: number,
): Promise<ExecucaoDoTrabalho> {
  return invoke("execucao_do_trabalho", {
    sessaoId,
    trabalhoId,
    instanciaConhecida: instanciaConhecida ?? null,
  });
}

export function pararTrabalho(sessaoId: string, trabalhoId: string): Promise<void> {
  return invoke("parar_trabalho", { sessaoId, trabalhoId });
}

// --- Árvore de objetos ----------------------------------------------------

/** As seis categorias da árvore (requisito 2). */
/**
 * As categorias da árvore.
 *
 * As seis primeiras são as do Oracle. As duas últimas só existem no SQL Server,
 * que separa a função que devolve tabela da que devolve um valor — como o SSMS
 * separa, e por uma razão prática: uma entra no `FROM` e a outra não.
 */
export type CategoriaOracle =
  | "tabelas"
  | "views"
  | "pacotes"
  | "procedures"
  | "funcoes"
  | "programador";

export type Categoria = CategoriaOracle | "funcoesTabela" | "funcoesEscalares";

/** O esquema padrão do SQL Server, o único que não se escreve. */
const ESQUEMA_PADRAO = "dbo.";

/**
 * O nome como ele aparece na tela: sem o `dbo.` da frente.
 *
 * No SQL Server o objeto vem qualificado — `dbo.cliente`, `vendas.pedido` —,
 * porque um banco tem vários esquemas e o nome sozinho seria ambíguo. Mas `dbo`
 * é o padrão: ninguém o escreve numa consulta, e repeti-lo em cada linha da
 * árvore só gasta a largura do painel dizendo o que já se sabe. Esquema
 * diferente continua aparecendo, porque ali a informação é necessária — é ela
 * que distingue `vendas.pedido` de `compras.pedido`.
 *
 * **Só para exibir.** O que vai ao servidor continua sendo o nome inteiro: um
 * login cujo esquema padrão não seja `dbo` resolveria `cliente` para outro
 * objeto, e a árvore passaria a mostrar uma tabela e abrir outra.
 *
 * No Oracle não muda nada: lá o nome nunca vem com esquema (requisito 2).
 */
export function nomeVisivel(nome: string): string {
  return nome.startsWith(ESQUEMA_PADRAO) ? nome.slice(ESQUEMA_PADRAO.length) : nome;
}

export interface Objeto {
  nome: string;
  situacao: string | null;
  /**
   * O comentário da view, por inteiro (requisito 37).
   *
   * Vem inteiro, e não já encurtado, porque serve a dois usos: a árvore mostra
   * os primeiros caracteres ao lado do nome, e a tela de edição precisa do
   * texto completo para não devolvê-lo truncado ao banco.
   *
   * Só as views o trazem; nas demais categorias vem nulo.
   */
  comentario: string | null;
}

/** Um pedaço da lista de objetos. Há schemas com dezenas de milhares. */
export interface PaginaDeObjetos {
  objetos: Objeto[];
  temMais: boolean;
}

// --- Filtro de objetos (requisito 4) ---------------------------------------

export type CampoFiltro = "nome" | "situacao";

export type OperadorFiltro =
  | "like"
  | "naoLike"
  | "igual"
  | "diferente"
  | "em"
  | "naoEm"
  | "comeca"
  | "termina"
  | "naoComeca"
  | "naoTermina";

export interface CriterioFiltro {
  campo: CampoFiltro;
  operador: OperadorFiltro;
  valor: string;
  distingueMaiusculas: boolean;
}

export interface Filtro {
  criterios: CriterioFiltro[];
  /** Campo legado mantido para ler filtros gravados pela interface anterior. */
  corresponderTodos: boolean;
  excluirGerados: boolean;
  incluirSinonimos: boolean;
}

/** O filtro restringe alguma coisa de fato? */
export function filtroAtivo(filtro: Filtro | null): boolean {
  if (!filtro) return false;
  return (
    filtro.excluirGerados ||
    filtro.incluirSinonimos ||
    filtro.criterios.some((criterio) => criterio.valor.trim() !== "")
  );
}

/** Filtros guardados de uma conexão, por categoria. */
export type FiltrosDaConexao = Partial<Record<Categoria, Filtro>>;

/**
 * Filtros guardados de uma conexão. Sobrevivem a fechar o aplicativo: o filtro
 * é configuração da conexão, como a cor.
 */
export function lerFiltros(perfilId: string): Promise<FiltrosDaConexao> {
  return invoke("ler_filtros", { perfilId });
}

/** Guarda o filtro de uma categoria. `null` apaga o que estava guardado. */
export function gravarFiltro(
  perfilId: string,
  categoria: Categoria,
  filtro: Filtro | null,
): Promise<void> {
  return invoke("gravar_filtro", { perfilId, categoria, filtro });
}

/** Resumo do filtro para a dica do nó da árvore. */
export function resumoDoFiltro(filtro: Filtro): string {
  const negativos = new Map<OperadorFiltro, string>([
    ["naoLike", "like"],
    ["diferente", "="],
    ["naoEm", "in"],
    ["naoComeca", t("operador comeca com")],
    ["naoTermina", t("operador termina com")],
  ]);
  const validos = filtro.criterios.filter((criterio) => criterio.valor.trim() !== "");
  const inclusoes = validos
    .filter((criterio) => !negativos.has(criterio.operador))
    .map((criterio) => `${criterio.campo} ${criterio.operador} ${criterio.valor}`);
  const exclusoes = validos
    .filter((criterio) => negativos.has(criterio.operador))
    .map(
      (criterio) =>
        `${criterio.campo} ${negativos.get(criterio.operador) ?? criterio.operador} ${criterio.valor}`,
    );

  const grupos: string[] = [];
  const ou = ` ${t("resumo do filtro ou")} `;
  if (inclusoes.length > 0) grupos.push(`(${inclusoes.join(ou)})`);
  if (exclusoes.length > 0) grupos.push(`${t("resumo do filtro nao")} (${exclusoes.join(ou)})`);
  if (filtro.excluirGerados) grupos.push(t("resumo sem objetos gerados"));
  if (filtro.incluirSinonimos) grupos.push(t("resumo com sinonimos"));
  return grupos.join(` ${t("resumo do filtro e")} `);
}

export interface Coluna {
  nome: string;
  tipo: string;
  aceitaNulo: boolean;
}

export interface Subprograma {
  nome: string;
  especie: string;
}

export type ParteDoFonte = "especificacao" | "corpo";

/**
 * Uma página de objetos de uma categoria.
 *
 * `banco` só vale para SQL Server, onde a árvore pendura as categorias de cada
 * banco. No Oracle vai vazio, e o escopo é o schema do usuário conectado
 * (requisito 2).
 */
export function listarObjetos(
  sessaoId: string,
  categoria: Categoria,
  filtro: Filtro | null,
  deslocamento: number,
  limite: number,
  banco: string | null = null,
  atualizar = false,
): Promise<PaginaDeObjetos> {
  return invoke("listar_objetos", {
    sessaoId,
    categoria,
    banco,
    filtro,
    deslocamento,
    limite,
    atualizar,
  });
}

/**
 * Colunas de um objeto.
 *
 * `banco` só vale para SQL Server: quem responde ao autocomplete é a sessão da
 * **conexão** — a da aba pode estar com uma consulta correndo ou com um grid
 * pela metade —, e ela precisa saber de que banco a aba está falando
 * (requisito 3). No Oracle vai vazio.
 */
export function listarColunas(
  sessaoId: string,
  objeto: string,
  banco: string | null = null,
  paraEditor = false,
): Promise<Coluna[]> {
  return invoke("listar_colunas", { sessaoId, objeto, banco, paraEditor });
}

export function listarSubprogramas(sessaoId: string, pacote: string): Promise<Subprograma[]> {
  return invoke("listar_subprogramas", { sessaoId, pacote });
}

export function aoAtualizarCatalogo(acao: (sessaoId: string) => void): Promise<() => void> {
  return listen<string>("catalogo-sqlserver-atualizado", ({ payload }) => acao(payload));
}

/**
 * O fonte do objeto — ou, nas tabelas, o DDL que o recria.
 *
 * `parte` só faz sentido no Oracle, onde o pacote tem especificação e corpo
 * separados; no SQL Server o módulo é um só e o parâmetro é ignorado. `banco`
 * é o contrário: só o SQL Server o usa.
 */
export function fonteDoObjeto(
  sessaoId: string,
  categoria: Categoria,
  nome: string,
  parte: ParteDoFonte,
  banco: string | null = null,
): Promise<string> {
  return invoke("fonte_do_objeto", { sessaoId, categoria, nome, parte, banco });
}

/**
 * Grava o comentário de uma view; texto vazio apaga o que havia (requisito 37).
 *
 * O mecanismo é diferente nos dois bancos — `COMMENT ON` no Oracle, *extended
 * property* `MS_Description` no SQL Server —, mas isso fica do lado de lá: aqui
 * é o mesmo texto para os dois.
 *
 * `banco` só vale para SQL Server, como no resto da árvore.
 */
export function gravarComentario(
  sessaoId: string,
  nome: string,
  comentario: string,
  banco: string | null = null,
): Promise<void> {
  return invoke("gravar_comentario", { sessaoId, nome, comentario, banco });
}

/** Destino já resolvido pelo banco para um Ctrl+clique no editor. */
export interface DestinoDaReferencia {
  categoria: Categoria;
  nome: string;
  parte: ParteDoFonte;
  /**
   * Em que banco o objeto está — só no SQL Server, onde um nome de três partes
   * leva a outro banco do servidor. No Oracle vem vazio.
   */
  banco: string | null;
}

/**
 * O destino de um Ctrl+clique. `banco` é o banco **da aba**, e só o SQL Server
 * o usa: é nele que o nome escrito na consulta é resolvido.
 */
export function resolverReferencia(
  sessaoId: string,
  referencia: string,
  banco: string | null = null,
): Promise<DestinoDaReferencia | null> {
  return invoke("resolver_referencia", { sessaoId, referencia, banco });
}

/**
 * Abre e prepara a conexão que responde ao sublinhado do Ctrl (requisito 22),
 * para o primeiro Ctrl não pagar o login. Numa conexão de SQL Server não faz
 * nada: lá quem responde é a sessão da conexão, já aberta.
 */
export function prepararNavegacao(sessaoId: string): Promise<void> {
  return invoke("preparar_navegacao", { sessaoId });
}

/**
 * Das referências, as que o Ctrl+clique abre — as que o editor sublinha com o
 * Ctrl apertado (requisito 22). As regras são as de `resolverReferencia`, para
 * muitos nomes de uma vez, e `banco` é o mesmo de lá.
 */
export function referenciasNavegaveis(
  sessaoId: string,
  referencias: string[],
  banco: string | null = null,
): Promise<string[]> {
  return invoke("referencias_navegaveis", { sessaoId, referencias, banco });
}

export function desconectar(id: string): Promise<void> {
  return invoke("desconectar", { id });
}

// --- Autocomplete (requisitos 3 e 23) --------------------------------------

/** Um nome do schema que o editor pode sugerir. */
export interface Sugestao {
  nome: string;
  /** `TABLE`, `VIEW`, `SYNONYM`, `PACKAGE`… */
  tipo: string;
  /** Dono, quando não é o próprio usuário conectado. */
  dono: string | null;
}

/**
 * Nomes que começam pelo prefixo digitado.
 *
 * `banco` e `esquema` são as duas primeiras partes do nome de três partes do
 * SQL Server: em `Vendas.dbo.cli`, o banco é `Vendas` e o esquema é `dbo`.
 * Vazios, valem o banco da aba e todos os esquemas dele. No Oracle vão sempre
 * vazios — lá o dono já vem ao lado de cada nome, sem ser preciso escrevê-lo.
 */
export function sugerirObjetos(
  sessaoId: string,
  prefixo: string,
  banco: string | null = null,
  esquema: string | null = null,
  apenasRotinas = false,
): Promise<Sugestao[]> {
  return invoke("sugerir_objetos", {
    sessaoId,
    prefixo,
    banco,
    esquema,
    apenasRotinas,
    // A opção **Tabelas** (requisito 43) é lida aqui, e não passada por quem
    // chama: ela vale para o aplicativo inteiro, e são três telas pedindo
    // sugestão — a aba de consulta, o DDL do objeto e a etapa do Agent. Uma
    // delas esquecendo de repassar a escolha seria um autocomplete que obedece
    // à configuração em dois lugares e a ignora no terceiro.
    apenasTabelasFiltradas: opcoesDeAutocomplete().tabelas === "filtradas",
  });
}

/** Fonte local e complemento remoto das tabelas, views e sinônimos Oracle. */
export function sugerirRelacoes(sessaoId: string, prefixo: string, esquema: string | null, local: boolean): Promise<Sugestao[]> {
  return invoke("sugerir_relacoes", {
    sessaoId, prefixo, esquema, local,
    apenasTabelasFiltradas: opcoesDeAutocomplete().tabelas === "filtradas",
  });
}

/** Descarta o cache de nomes, para reler depois de criar ou remover objetos. */
export function atualizarMetadados(sessaoId: string): Promise<void> {
  return invoke("atualizar_metadados", { sessaoId });
}

/**
 * Monta o índice local dos objetos do usuário (requisito 40) e devolve quantos
 * foram guardados.
 *
 * Chamado logo depois de conectar, **sem esperar**. O JSON anterior atende o
 * autocomplete enquanto uma conexão separada procura novidades. Devolve zero
 * somente quando a conexão é SQL Server.
 */
export function indexarObjetos(sessaoId: string): Promise<number> {
  return invoke("indexar_objetos", { sessaoId });
}

// --- Execução de SQL (requisitos 5, 6, 7, 8, 10, 14, 15) -------------------

export interface ColunaResultado {
  nome: string;
  tipo: string;
  numerica: boolean;
}

/** `null` é NULL do banco; `""` é texto vazio. A diferença aparece no grid. */
export type Linha = (string | null)[];

export interface ResultadoConsulta {
  cursorId: string;
  colunas: ColunaResultado[];
  linhas: Linha[];
  temMais: boolean;
}

export interface ResultadoDaExecucao {
  comando: string;
  linhaNoEditor: number;
  duracaoMs: number;
  consulta: ResultadoConsulta | null;
  linhasAfetadas: number | null;
  /**
   * Como a transação da aba ficou depois deste comando — é o que acende e
   * apaga o ✔ (requisito 7). No Oracle um DDL volta `encerrada`: ele confirma
   * sozinho, e leva junto o que estava pendente.
   */
  transacao: "inalterada" | "aberta" | "encerrada";
  mensagem: string;
  saidaDbms: string[];
  /**
   * A conexão tinha caído — VPN, rede, sessão derrubada — e foi restabelecida
   * antes deste comando rodar. Vem marcado só no primeiro resultado do lote.
   */
  reconectado: boolean;
  /**
   * Erros do objeto que o comando compilou, quando o `CREATE` o deixou
   * inválido — o Oracle aceita o comando e só avisa (requisito 13). A linha é
   * contada no texto executado.
   */
  errosDeCompilacao: ErroDeCompilacao[];
}

export interface BlocoDeLinhas {
  linhas: Linha[];
  temMais: boolean;
}

/** Nomes das bind variables presentes no texto. */
export function analisarBinds(texto: string, substituicoesOracle = false): Promise<string[]> {
  return invoke("analisar_binds", { texto, substituicoesOracle });
}

export function executarSql(
  sessaoId: string,
  texto: string,
  modoScript: boolean,
  binds: [string, string][],
  tamanhoDoFetch: number,
): Promise<ResultadoDaExecucao[]> {
  return invoke("executar_sql", { sessaoId, texto, modoScript, binds, tamanhoDoFetch });
}

export function buscarMaisLinhas(cursorId: string, quantidade: number): Promise<BlocoDeLinhas> {
  return invoke("buscar_mais_linhas", { cursorId, quantidade });
}

export function fecharCursor(cursorId: string): Promise<void> {
  return invoke("fechar_cursor", { cursorId });
}

export function cancelarExecucao(sessaoId: string): Promise<void> {
  return invoke("cancelar_execucao", { sessaoId });
}

export function confirmarTransacao(sessaoId: string): Promise<void> {
  return invoke("confirmar_transacao", { sessaoId });
}

export function desfazerTransacao(sessaoId: string): Promise<void> {
  return invoke("desfazer_transacao", { sessaoId });
}

export function definirAutoCommit(sessaoId: string, ligado: boolean): Promise<void> {
  return invoke("definir_auto_commit", { sessaoId, ligado });
}

export function definirSaidaDbms(
  sessaoId: string,
  ligada: boolean,
  tamanhoDoBuffer: number,
): Promise<void> {
  return invoke("definir_saida_dbms", { sessaoId, ligada, tamanhoDoBuffer });
}

// --- PL/SQL e exportação ---------------------------------------------------

export interface ErroDeCompilacao {
  /** Contada no texto enviado para compilar ou executar. */
  linha: number;
  coluna: number;
  texto: string;
  /** Aviso do compilador (`PLW-`): não deixa o objeto inválido. */
  aviso: boolean;
}

export function compilar(sessaoId: string, fonte: string): Promise<ErroDeCompilacao[]> {
  return invoke("compilar", { sessaoId, fonte });
}

/** Abre o diálogo "salvar como" do Windows e devolve o caminho, ou `null`. */
export async function escolherArquivoParaSalvar(
  nomeSugerido: string,
  descricao: string,
  extensao: string,
): Promise<string | null> {
  const escolhido = await save({
    defaultPath: nomeSugerido,
    filters: [{ name: descricao, extensions: [extensao] }],
  });
  return escolhido ?? null;
}

export function gravarTexto(caminho: string, conteudo: string): Promise<void> {
  return invoke("gravar_texto", { caminho, conteudo });
}

/**
 * Grava o resultado do grid como planilha do Excel (requisito 16). O arquivo
 * `.xlsx` é binário, e quem o monta é o Rust: daqui vão só os dados.
 */
export function gravarPlanilha(caminho: string, colunas: string[], linhas: Linha[]): Promise<void> {
  return invoke("gravar_planilha", { caminho, colunas, linhas });
}

/** Lê um arquivo de texto do disco, descobrindo a codificação pelo conteúdo. */
export function lerTexto(caminho: string): Promise<string> {
  return invoke("ler_texto", { caminho });
}

/**
 * Arquivos que o Windows mandou abrir — duplo clique, **Abrir com** — e que a
 * janela ainda não abriu (requisito 12). Quem pergunta leva: a fila sai vazia.
 */
export function arquivosRecebidos(): Promise<string[]> {
  return invoke("arquivos_recebidos");
}

/** Chama `acao` quando chega arquivo com a janela já aberta. */
export function aoReceberArquivos(acao: () => void): Promise<() => void> {
  return listen("arquivos-recebidos", acao);
}

// --- Cópia de recuperação (requisito 12) -----------------------------------

/** O que acompanha o texto guardado, para a tela de recuperação exibir. */
export interface DescricaoDaRecuperacao {
  titulo: string;
  conexao: string;
  /** Arquivo de onde a consulta veio, quando veio de um. */
  origem: string | null;
}

export interface EntradaDeRecuperacao extends DescricaoDaRecuperacao {
  id: string;
  /** Data e hora da última gravação, já formatada. */
  quando: string;
  /** Primeiras linhas do texto, para reconhecer qual consulta era. */
  amostra: string;
}

export function gravarRecuperacao(
  id: string,
  texto: string,
  descricao: DescricaoDaRecuperacao,
): Promise<void> {
  return invoke("gravar_recuperacao", { id, texto, descricao });
}

export function listarRecuperacoes(): Promise<EntradaDeRecuperacao[]> {
  return invoke("listar_recuperacoes");
}

export function lerRecuperacao(id: string): Promise<string> {
  return invoke("ler_recuperacao", { id });
}

export function descartarRecuperacao(id: string): Promise<void> {
  return invoke("descartar_recuperacao", { id });
}

/** Abre o diálogo de seleção de um arquivo de consulta (requisito 12). */
export async function escolherArquivoDeConsulta(): Promise<string | null> {
  const escolhido = await open({
    multiple: false,
    directory: false,
    filters: [
      { name: t("arquivo script sql"), extensions: ["sql"] },
      { name: t("arquivo todos"), extensions: ["*"] },
    ],
  });
  return typeof escolhido === "string" ? escolhido : null;
}

export interface BotaoRapidoBackup {
  rotulo: string;
  atalho: string;
  conteudo: string;
  executarAutomaticamente: boolean;
  naoInserirNaJanela: boolean;
}

/** Par de teclas de uma ação: posição 1 vazia ("") quando não há alternativa. */
export type ParDeAtalhosBackup = [string, string];

export function exportarBackup(
  caminho: string,
  botoes: BotaoRapidoBackup[],
  atalhos: Record<string, ParDeAtalhosBackup>,
): Promise<void> {
  return invoke("exportar_backup", { caminho, botoes, atalhos });
}

export interface ConteudoRestauradoDoBackup {
  botoes: BotaoRapidoBackup[];
  atalhos: Record<string, ParDeAtalhosBackup>;
}

export function importarBackup(caminho: string): Promise<ConteudoRestauradoDoBackup> {
  return invoke("importar_backup", { caminho });
}

/** Abre o diálogo de seleção de um arquivo de backup. */
export async function escolherArquivoDeBackup(): Promise<string | null> {
  const escolhido = await open({
    multiple: false,
    directory: false,
    filters: [{ name: t("arquivo backup do cururu"), extensions: ["json"] }],
  });
  return typeof escolhido === "string" ? escolhido : null;
}

// --- Importação de conexões (requisito 25) ---------------------------------

export interface ConexaoImportavel {
  nome: string;
  /** De onde veio: "SQL Developer" ou o caminho do tnsnames.ora. */
  origem: string;
  perfil: PerfilConexao;
}

export function procurarConexoesImportaveis(): Promise<ConexaoImportavel[]> {
  return invoke("procurar_conexoes_importaveis");
}

/** Espelha `erro::Erro` do backend. */
export interface ErroBackend {
  mensagem: string;
  codigo: string | null;
  posicao: number | null;
  /**
   * Linha onde o erro caiu, contada no texto executado: a tela soma a linha em
   * que esse texto começa no editor (requisito 18). No SQL Server o backend já
   * somou a linha em que o lote começa; no Oracle, converteu o deslocamento
   * dentro do comando na linha do comando que falhou.
   */
  linha: number | null;
  /**
   * A conexão tinha caído e foi restabelecida ao tratar deste erro: o comando
   * não valeu, mas a sessão já está de pé e basta executar outra vez.
   */
  reconectado: boolean;
  /**
   * O que a execução já tinha devolvido quando o erro chegou.
   *
   * Um lote do SQL Server manda vários conjuntos de resultados e só **depois**
   * falha — é o caso de uma procedure de depuração que mostra grids e estoura
   * no último comando. Os grids que já vieram aparecem mesmo assim; o erro
   * continua sendo erro. Ausente quando não veio nada antes da falha.
   */
  resultados?: ResultadoDaExecucao[];
}

function ehErroBackend(valor: unknown): valor is ErroBackend {
  return typeof valor === "object" && valor !== null && "mensagem" in valor;
}

/**
 * Traduz um erro em texto exibível.
 *
 * Falhas internas do próprio navegador têm mensagem em inglês, que não temos
 * como traduzir; ficam ao menos dentro de uma frase no idioma da tela, como
 * detalhe técnico.
 */
export function mensagemDeErro(erro: unknown): string {
  if (ehErroBackend(erro)) return erro.mensagem;
  if (typeof erro === "string") return erro;
  const detalhe = erro instanceof Error ? erro.message : String(erro);
  return t("falha interna do aplicativo", { detalhe });
}
