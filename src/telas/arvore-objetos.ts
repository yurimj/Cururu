/**
 * Árvore de objetos de uma conexão (requisitos 2 e 4).
 *
 * No Oracle são só as seis categorias acordadas, e sempre no schema do usuário
 * conectado. No SQL Server são dois nós: **Banco de Dados** e **SQL Server
 * Agent** (requisito 36).
 *
 * Os filhos são carregados sob demanda: expandir "Tabelas" num banco com
 * milhares de tabelas não pode custar memória antes da hora.
 */

import "./arvore-objetos.css";

import * as api from "../api";
import { t, type Chave } from "../idioma";
import { sufixoDoAtalho } from "../atalhos";
import { formatarDataHora, formatarDuracao } from "../formatacao";
import { opcoesDaArvore } from "../opcoes-arvore";
import { confirmar, pedirTexto, pedirTextoLongo } from "../componentes/dialogo-simples";
import { comIndicadorDeAbertura } from "../componentes/indicador-de-abertura";
import { abrirMenuDeContexto, type ItemDeMenu } from "../componentes/menu-contexto";
import type { PerfilConexao } from "../tipos";
import { abrirDialogoDeExecucao, rotuloDoResultado } from "./dialogo-execucao-trabalho";
import { abrirDialogoFiltro } from "./dialogo-filtro";
import { abrirDialogoIniciarNaEtapa } from "./dialogo-iniciar-etapa";
import { abrirDialogoTrabalho } from "./dialogo-trabalho";
import { abrirDialogoDeAlertas } from "./dialogo-alertas";
import { abrirLogsDoAlerta } from "./dialogo-logs-alerta";
import { MonitorDeAlertas } from "./monitor-de-alertas";
import type { AbaDoObjeto } from "./painel-objeto";

/** O relógio do SQL Server Agent, o mesmo do Programador do Oracle. */
const SIMBOLO_DO_AGENTE = "◷";

/** O nó não se traduz: é o nome do serviço, como o SSMS o escreve. */
const ROTULO_DO_AGENTE = "SQL Server Agent";

/**
 * Onde o filtro dos trabalhos do Agent é guardado (requisito 4).
 *
 * É a chave do **Programador**, que é a categoria de job do Oracle: uma conexão
 * é de um banco só, então as duas nunca disputam o mesmo lugar, e a tela do
 * filtro já chama o alvo de "Job" nas duas.
 */
const FILTRO_DO_AGENTE: api.Categoria = "programador";

/** O relógio cortado: o serviço está parado e nada agendado vai rodar. */
const SIMBOLO_DE_PARADO = "⊘";

/** O resultado da última execução, escrito por extenso na dica do trabalho. */
function resultadoDoTrabalho(resultado: api.ResultadoDoTrabalho): Chave {
  switch (resultado) {
    case "sucesso":
      return "execucao com sucesso";
    case "falha":
      return "execucao com falha";
    case "cancelado":
      return "execucao cancelada";
    case "novaTentativa":
      return "execucao em nova tentativa";
    default:
      return "desconhecido";
  }
}

/**
 * As seis categorias do Oracle, na ordem em que aparecem.
 *
 * Sem rótulo aqui: o nome de cada categoria sai do catálogo pela própria chave
 * (`t("pacotes")`), e repeti-lo nesta tabela só daria uma segunda origem para o
 * texto — que envelheceria calada ao trocar de idioma.
 */
const CATEGORIAS: ReadonlyArray<{ chave: api.CategoriaOracle; simbolo: string }> = [
  { chave: "tabelas", simbolo: "▤" },
  { chave: "views", simbolo: "◫" },
  { chave: "pacotes", simbolo: "▣" },
  { chave: "procedures", simbolo: "▷" },
  { chave: "funcoes", simbolo: "ƒ" },
  { chave: "programador", simbolo: "◷" },
];

/**
 * Como o filtro se apresenta em cada categoria: o nome do alvo em cada linha,
 * o nome usado na prévia lógica e o símbolo.
 *
 * Cobre as **oito** categorias, e não só as seis do Oracle: as duas famílias de
 * função do SQL Server também são filtráveis (requisito 4), e a tela do filtro
 * precisa saber como chamá-las.
 */
const ALVO_DO_FILTRO: Record<
  api.Categoria,
  { alvo: Chave; alvoSql: string; simbolo: string }
> = {
  tabelas: { alvo: "alvo tabela", alvoSql: "Table", simbolo: "▤" },
  views: { alvo: "alvo view", alvoSql: "View", simbolo: "◫" },
  pacotes: { alvo: "alvo pacote", alvoSql: "Package", simbolo: "▣" },
  procedures: { alvo: "alvo procedure", alvoSql: "Procedure", simbolo: "▷" },
  funcoes: { alvo: "alvo funcao", alvoSql: "Function", simbolo: "ƒ" },
  programador: { alvo: "alvo job", alvoSql: "Job", simbolo: "◷" },
  funcoesTabela: { alvo: "alvo funcao", alvoSql: "Function", simbolo: "▦" },
  funcoesEscalares: { alvo: "alvo funcao", alvoSql: "Function", simbolo: "ƒ" },
};

/** Quantos caracteres do comentário cabem ao lado do nome (requisito 37). */
const CARACTERES_DO_COMENTARIO = 50;

/**
 * Teto do comentário, em caracteres.
 *
 * É o menor dos dois bancos, e vale para os dois: no Oracle,
 * `USER_TAB_COMMENTS.COMMENTS` é um `VARCHAR2(4000)`; no SQL Server a
 * propriedade estendida aceita 7500 **bytes**, que em `nvarchar` dão 3750
 * caracteres. Adotar o menor é o que garante que o texto aceito aqui caiba
 * onde quer que ele acabe sendo gravado.
 */
const LIMITE_DO_COMENTARIO = 3750;

/** Categorias cujo conteúdo é código-fonte abrível. */
const CATEGORIAS_DE_CODIGO: ReadonlySet<api.Categoria> = new Set<api.Categoria>([
  "pacotes",
  "procedures",
  "funcoes",
  "funcoesTabela",
  "funcoesEscalares",
]);

/**
 * As quatro pastas de um banco do SQL Server, na ordem do SSMS.
 *
 * `filhas` é a pasta que não lista nada: **Functions** existe só para separar a
 * função que devolve tabela da que devolve um valor. As duas coisas se escrevem
 * de formas diferentes numa consulta — uma entra no `FROM`, a outra não —, e é
 * por isso que o SSMS as separa.
 */
const CATEGORIAS_DO_BANCO: ReadonlyArray<{
  chave?: api.Categoria;
  chaveDoRotulo: Chave;
  simbolo: string;
  filhas?: ReadonlyArray<{ chave: api.Categoria; chaveDoRotulo: Chave; simbolo: string }>;
}> = [
  { chave: "tabelas", chaveDoRotulo: "tabelas", simbolo: "▤" },
  { chave: "views", chaveDoRotulo: "views", simbolo: "◫" },
  { chave: "procedures", chaveDoRotulo: "stored procedures", simbolo: "▷" },
  {
    chaveDoRotulo: "functions",
    simbolo: "ƒ",
    filhas: [
      {
        chave: "funcoesTabela",
        chaveDoRotulo: "funcoes com valor de tabela",
        simbolo: "▦",
      },
      {
        chave: "funcoesEscalares",
        chaveDoRotulo: "funcoes com valor escalar",
        simbolo: "ƒ",
      },
    ],
  },
];

/**
 * O que o filtro (requisito 4) alcança: a chave em que ele é guardado, o nó que
 * o mostra e como esse nó se reescreve depois da mudança.
 *
 * A categoria põe só o `(Filtrado)` ao lado do nome; o Agent tem também o
 * estado do serviço para dizer no mesmo lugar.
 */
interface AlvoDoFiltro {
  categoria: api.Categoria;
  rotulo: string;
  no: No;
  rotular: () => void;
}

/** Uma categoria selecionada na árvore, pronta para ser filtrada. */
export interface CategoriaSelecionada {
  rotulo: string;
  filtrar: () => Promise<void>;
}

/** Conexão a que uma aba pertence. */
export interface ContextoDeConexao {
  perfil: PerfilConexao;
  sessaoId: string;
  /**
   * Banco em que a aba deve nascer (SQL Server). Ausente, a aba nasce no banco
   * em que a conexão está — que é o caminho de quem abre a consulta pela
   * conexão, e não por um banco específico.
   */
  banco?: string;
  colecao?: string;
}

/** O que a árvore precisa saber fazer com o mundo lá fora. */
export interface LigacoesDaArvore {
  /** Abre a conexão desta árvore e devolve o identificador da sessão. */
  conectar: (perfil: PerfilConexao) => Promise<string>;
  /** Abre uma aba de consulta SQL vazia (requisito 3). */
  aoNovaConsulta: (contexto: ContextoDeConexao) => void;
  /**
   * A conexão acabou de abrir — a de verdade, e não reexpansão nem Atualizar.
   * Abre o que vem junto: a aba vazia do requisito 3, ou os arquivos que
   * esperavam uma conexão (requisito 12).
   */
  aoConectar: (contexto: ContextoDeConexao) => void;
  /** Abre um fonte numa aba do editor. */
  aoAbrirFonte: (titulo: string, texto: string, contexto: ContextoDeConexao) => Promise<boolean>;
  /** Abre uma tabela ou view no visualizador de Dados, DDL e Estrutura. */
  aoAbrirObjeto: (
    nome: string,
    categoria: "tabelas" | "views",
    aba: AbaDoObjeto,
    contexto: ContextoDeConexao,
  ) => void;
  /** Escreve na barra de status da janela. */
  aoRelatar: (mensagem: string, erro?: boolean) => void;
  /** Avisa qual categoria está selecionada, para o botão de filtro da barra. */
  aoSelecionarCategoria: (selecao: CategoriaSelecionada | null) => void;
  /**
   * Avisa em que conexão está o nó selecionado, para o botão de nova consulta
   * da barra. `null` quando a conexão selecionada não está aberta.
   */
  aoSelecionarConexao: (contexto: ContextoDeConexao | null) => void;
  /**
   * Avisa qual banco está sob o ponteiro do mouse, ou `null` ao sair dele.
   *
   * É o que faz o atalho de nova consulta (Ctrl+T de fábrica) abrir a consulta
   * no banco apontado sem exigir um clique antes.
   */
  aoPairarBanco: (contexto: ContextoDeConexao | null) => void;
  /** Avisa que a sessão foi encerrada, para fechar as abas dela. */
  aoEncerrarSessao: (sessaoId: string) => void;
}

export class ArvoreDeObjetos {
  private readonly raiz = document.createElement("ul");
  /** Filtro ativo por categoria (requisito 4). */
  private readonly filtros = new Map<api.Categoria, api.Filtro>();
  /** O que o serviço do Agent respondeu por último (requisito 36). */
  private estadoDoAgente: api.EstadoDoAgente = "desconhecido";
  /** Nulo enquanto a conexão não foi aberta. */
  private sessaoId: string | null = null;
  private monitorDeAlertas: MonitorDeAlertas | null = null;
  private noDeAlertasAtual: No | null = null;

  encerrarAlertas(): void {
    this.monitorDeAlertas?.encerrar();
    this.monitorDeAlertas = null;
    this.noDeAlertasAtual = null;
  }

  traduzirAlertas(): void {
    this.noDeAlertasAtual?.renomear(t("alertas"));
    this.noDeAlertasAtual?.definirDica(t("alertas"));
  }

  constructor(
    private readonly perfil: PerfilConexao,
    private readonly ligacoes: LigacoesDaArvore,
  ) {
    this.raiz.className = "arvore";
  }

  get conectado(): boolean {
    return this.sessaoId !== null;
  }

  /**
   * Anuncia o que ficou selecionado, para os botões da barra saberem sobre o
   * que agir. Sem categoria, o botão de filtro desliga; sem conexão aberta, o
   * de nova consulta também.
   */
  private anunciarSelecao(categoria: CategoriaSelecionada | null, banco?: string): void {
    this.ligacoes.aoSelecionarConexao(this.sessaoId ? this.contexto(banco) : null);
    this.ligacoes.aoSelecionarCategoria(categoria);
  }

  /** Atalho para os nós que não são categoria. */
  private readonly semCategoria = (): void => this.anunciarSelecao(null);

  /**
   * Monta o nó da conexão. Ele aparece **sem conectar**: a conexão só é aberta
   * quando o usuário expande o nó.
   */
  montar(sessaoInicial?: string): HTMLElement {
    let sessaoRecebida = sessaoInicial;
    const conexao = new No({
      rotulo: this.perfil.nome,
      simbolo: "◈",
      cor: this.perfil.cor,
      classe: "no--conexao no--desconectado",
      dica: t("clique para conectar", { conexao: this.perfil.nome }),
      aoSelecionar: this.semCategoria,
      carregar: async () => {
        await this.garantirConexao(conexao, sessaoRecebida);
        sessaoRecebida = undefined;
        // O filtro por categoria (requisito 4) vale nos dois bancos: o mesmo
        // critério sai em SQL do Oracle ou em T-SQL, conforme a conexão.
        if (this.perfil.tipoBanco !== "mongodb") await this.carregarFiltrosSalvos();
        const nos = this.nosDaConexao();
        // Conectar já mostra os bancos: quem abre uma conexão de SQL Server vai
        // olhar a lista, e exigir um segundo clique para isso seria só atrito.
        if (this.perfil.tipoBanco !== "oracle") await nos[0]?.alternar();
        return nos;
      },
      menu: (evento) =>
        abrirMenuDeContexto(evento, [
          {
            // Requisito 3: botão direito no banco abre uma nova consulta.
            rotulo: this.rotuloNovaConsulta(),
            desabilitado: !this.conectado,
            acao: () => this.ligacoes.aoNovaConsulta(this.contexto()),
          },
          {
            rotulo: t(this.conectado ? "atualizar" : "conectar"),
            separadorAntes: true,
            acao: () => void conexao.recarregar(),
          },
          {
            rotulo: t("desconectar"),
            separadorAntes: true,
            desabilitado: !this.conectado,
            acao: () => void this.desconectar(conexao),
          },
        ]),
    });
    this.raiz.append(conexao.elemento);
    if (sessaoRecebida) void conexao.alternar();
    return this.raiz;
  }

  /** Abre a conexão, se ainda não estiver aberta. */
  private async garantirConexao(no: No, sessaoInicial?: string): Promise<void> {
    if (this.sessaoId) return;
    this.sessaoId = await (sessaoInicial ?? this.ligacoes.conectar(this.perfil));
    if (this.raiz.isConnected && this.perfil.tipoBanco !== "mongodb") {
      this.monitorDeAlertas = new MonitorDeAlertas(this.sessaoId, this.perfil.nome);
    }
    no.marcarEstado("no--conectado");
    no.definirDica(t("conectado", { conexao: this.perfil.nome }));
    this.ligacoes.aoRelatar(t("conectado em", { conexao: this.perfil.nome }));

    // Conectar é o gesto de quem vai consultar: a aba já abre junto, com o
    // cursor dentro dela. Aqui e não no `carregar` do nó porque só se passa por
    // aqui na conexão de verdade — reexpandir ou "Atualizar" para no começo
    // deste método, e não vai empilhando aba vazia a cada clique. Vem antes do
    // reanúncio abaixo: arquivo esperando conexão abre no lugar da aba vazia,
    // e o reanúncio o abriria primeiro, deixando as duas.
    this.ligacoes.aoConectar(this.contexto());

    // O clique já anunciou a seleção, mas naquele instante a conexão ainda não
    // estava aberta e o botão de nova consulta ficou desligado. Reanuncia agora
    // que há sessão, senão ele só liberaria no próximo clique.
    this.anunciarSelecao(null);

    // O índice anterior já atende o autocomplete enquanto uma conexão separada
    // confere packages, procedures e functions em background.
    // A falha não aparece na tela: sem índice, vale o caminho antigo, e o motivo
    // fica no log.
    if (this.perfil.tipoBanco !== "mongodb") void api.indexarObjetos(this.sessaoId).catch(() => {});
  }

  /**
   * Relê os filtros guardados desta conexão.
   *
   * Falhar aqui não pode impedir a árvore de abrir: sem filtro, ela apenas
   * mostra tudo.
   */
  private async carregarFiltrosSalvos(): Promise<void> {
    this.filtros.clear();
    try {
      const guardados = await api.lerFiltros(this.perfil.id);
      for (const [categoria, filtro] of Object.entries(guardados)) {
        if (filtro) this.filtros.set(categoria as api.Categoria, filtro);
      }
    } catch (erro) {
      this.ligacoes.aoRelatar(
        t("falha ao ler filtros", { erro: api.mensagemDeErro(erro) }),
        true,
      );
    }
  }

  private async desconectar(no: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    this.sessaoId = null;
    this.encerrarAlertas();
    // Os filtros continuam guardados em disco; só saem da memória.
    this.filtros.clear();
    this.ligacoes.aoEncerrarSessao(sessaoId);
    no.recolher();
    no.marcarEstado("no--desconectado");
    no.definirDica(t("clique para conectar", { conexao: this.perfil.nome }));
    try {
      await api.desconectar(sessaoId);
      this.ligacoes.aoRelatar(t("conexao encerrada", { conexao: this.perfil.nome }));
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  /**
   * Os nós fixos abaixo da conexão — que dependem do banco.
   *
   * O Oracle tem as seis categorias do schema conectado (requisito 2). O SQL
   * Server tem os bancos do servidor e o SQL Server Agent (requisito 36), na
   * mesma ordem em que o SSMS os mostra.
   */
  private nosDaConexao(): No[] {
    if (this.perfil.tipoBanco === "mongodb") return [this.noDeBancos()];
    const alertas = this.noDeAlertas();
    if (this.perfil.tipoBanco === "sqlserver") {
      return [this.noDeBancos(), alertas, this.noDoAgente()];
    }
    const categorias = this.nosDeCategoria();
    categorias.splice(CATEGORIAS.findIndex(({ chave }) => chave === "programador"), 0, alertas);
    return categorias;
  }

  private noDeAlertas(): No {
    const abrir = (regra: api.AlertaSalvo | null) => {
      if (this.sessaoId && this.monitorDeAlertas && !document.querySelector(".dialogo-alertas")) {
        abrirDialogoDeAlertas(this.contexto(), this.monitorDeAlertas, regra, () => {
          void no.recarregar();
        });
      }
    };
    const novo = () => abrir(null);
    const no = new No({
      rotulo: t("alertas"),
      simbolo: "🔔︎",
      classe: "no--categoria",
      aoSelecionar: this.semCategoria,
      carregar: async () => {
        if (!this.sessaoId) return [];
        const regras = await api.listarAlertas(this.sessaoId);
        if (regras.length === 0) {
          return [new No({
            rotulo: t("novo alerta"),
            simbolo: "+",
            aoSelecionar: this.semCategoria,
            aoAbrir: novo,
          })];
        }
        return regras.map((regra) => new No({
          rotulo: regra.banco ? `${regra.nome} · ${regra.banco}` : regra.nome,
          simbolo: "▷",
          classe: regra.ativo ? undefined : "no--desabilitado",
          detalhe: regra.ativo ? undefined : t("alerta pausado"),
          aoSelecionar: this.semCategoria,
          aoAbrir: () => abrir(regra),
          menu: (evento) => abrirMenuDeContexto(evento, [
            {
              rotulo: t("executar alerta"),
              acao: () => { void this.monitorDeAlertas?.executar(regra.id); },
            },
            {
              rotulo: t("logs do alerta"),
              acao: () => {
                if (this.sessaoId) abrirLogsDoAlerta(this.sessaoId, regra);
              },
            },
            { rotulo: t("propriedades"), acao: () => abrir(regra) },
          ]),
        }));
      },
      menu: (evento) => abrirMenuDeContexto(evento, [
        { rotulo: t("novo alerta"), acao: novo },
        { rotulo: t("atualizar"), separadorAntes: true, acao: () => void no.recarregar() },
      ]),
    });
    this.noDeAlertasAtual = no;
    return no;
  }

  /**
   * O nó "Banco de Dados": os bancos a que este login tem acesso.
   *
   * Sem paginação nem filtro, ao contrário das categorias do Oracle: um
   * servidor tem dezenas de bancos, não as dezenas de milhares de objetos que
   * um schema pode ter. Os bancos ainda não expandem — é o que vem depois.
   */
  private noDeBancos(): No {
    return new No({
      rotulo: t("banco de dados"),
      simbolo: "◱",
      classe: "no--categoria",
      aoSelecionar: this.semCategoria,
      carregar: async () => {
        const sessao = this.exigirSessao();
        const bancos = this.perfil.tipoBanco === "mongodb"
          ? await api.listarBancosMongo(sessao)
          : await api.listarBancos(sessao);
        if (bancos.length === 0) {
          return [
            new No({
              rotulo: t("nenhum banco acessivel"),
              simbolo: "·",
              aoSelecionar: this.semCategoria,
            }),
          ];
        }
        return bancos.map((banco) => this.noDoBanco(banco));
      },
    });
  }

  /**
   * O nó "SQL Server Agent" (requisito 36).
   *
   * Os trabalhos ficam pendurados **direto** nele, sem a pasta "Trabalhos" que o
   * SSMS interpõe: lá dentro do Agent há também alertas, operadores e proxies, e
   * a pasta existe para separá-los. Aqui só há trabalho, e a pasta seria um
   * clique a mais para chegar ao único conteúdo.
   *
   * O nó se marca como parado quando o serviço está fora do ar. Não é enfeite: um
   * trabalho agendado num Agent parado não roda, e ver a lista cheia sem essa
   * informação é a maneira mais fácil de perder uma manhã.
   */
  private noDoAgente(): No {
    // O tipo é explícito porque `carregar` menciona o próprio nó — os trabalhos
    // precisam dele para relistar o Agent depois de uma exclusão ou renomeação —,
    // e o TypeScript não infere um tipo que se refere a si mesmo.
    const no: No = new No({
      rotulo: ROTULO_DO_AGENTE,
      simbolo: SIMBOLO_DO_AGENTE,
      classe: "no--categoria",
      // Como nas categorias, o botão de filtro da barra age sobre o nó
      // selecionado (requisito 4).
      aoSelecionar: () =>
        this.anunciarSelecao({
          rotulo: ROTULO_DO_AGENTE,
          filtrar: () => this.pedirFiltro(this.alvoDoAgente(no)),
        }),
      carregar: async () => {
        const trabalhos = await api.listarTrabalhos(this.exigirSessao());
        if (trabalhos.length === 0) {
          return [
            new No({
              rotulo: t("nenhum trabalho"),
              simbolo: "·",
              aoSelecionar: this.semCategoria,
            }),
          ];
        }
        return trabalhos.map((trabalho) => this.noDoTrabalho(trabalho, no));
      },
      menu: (evento) =>
        abrirMenuDeContexto(evento, [
          {
            rotulo: t("novo trabalho"),
            desabilitado: !this.conectado,
            acao: () => void this.novoTrabalho(no),
          },
          // Os trabalhos se filtram como as categorias se filtram (requisito
          // 4): num servidor com dezenas deles, achar o seu é o mesmo problema
          // de achar uma tabela.
          {
            rotulo: t("aplicar filtro"),
            separadorAntes: true,
            acao: () => void this.pedirFiltro(this.alvoDoAgente(no)),
          },
          {
            rotulo: t("limpar filtro"),
            desabilitado: !this.filtros.has(FILTRO_DO_AGENTE),
            acao: () => void this.definirFiltro(this.alvoDoAgente(no), null),
          },
          {
            rotulo: t("atualizar"),
            separadorAntes: true,
            acao: () => {
              // O serviço pode ter subido — ou caído — desde que a árvore abriu.
              void this.marcarEstadoDoAgente(no);
              void no.recarregar();
            },
          },
        ]),
    });
    this.desenharAgente(no);
    // Sem `await`: a árvore não pode ficar esperando a resposta do serviço para
    // aparecer. O nó nasce neutro e se marca quando a resposta chega.
    void this.marcarEstadoDoAgente(no);
    return no;
  }

  /**
   * Escreve no nó do Agent o que ele tem a dizer: o serviço e o filtro.
   *
   * Os dois disputam o mesmo rótulo e a mesma dica, e por isso saem juntos
   * daqui — o estado vem do serviço (requisito 36) e o `(Filtrado)`, do filtro
   * da conexão (requisito 4).
   */
  private desenharAgente(no: No): void {
    const filtro = this.filtros.get(FILTRO_DO_AGENTE);
    no.renomear(filtro ? `${ROTULO_DO_AGENTE} ${t("filtrado")}` : ROTULO_DO_AGENTE);
    no.definirDica(
      [
        this.estadoDoAgente === "ativo"
          ? t("agente ativo")
          : this.estadoDoAgente === "parado"
            ? t("agente parado")
            : t("agente desconhecido"),
        filtro ? t("filtro ativo", { filtro: api.resumoDoFiltro(filtro) }) : "",
      ]
        .filter(Boolean)
        .join(" · "),
    );
  }

  /** Pergunta pelo serviço e desenha o nó conforme a resposta. */
  private async marcarEstadoDoAgente(no: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;

    try {
      this.estadoDoAgente = await api.estadoDoAgente(sessaoId);
    } catch {
      // Não vale ocupar a barra de status com isto: quem não conseguir falar com
      // o Agent descobre ao expandir o nó, e ali o motivo vem escrito.
      this.estadoDoAgente = "desconhecido";
    }

    const parado = this.estadoDoAgente === "parado";
    no.marcarEstado(parado ? "no--parado" : null);
    no.definirSimbolo(parado ? SIMBOLO_DE_PARADO : SIMBOLO_DO_AGENTE);
    no.definirDetalhe(parado ? t("parado") : "");
    this.desenharAgente(no);
  }

  /**
   * Um trabalho do Agent.
   *
   * Ele não expande: o que há dentro de um trabalho — etapas e agendas — a tela
   * de propriedades mostra melhor do que uma árvore mostraria. O que o nó carrega
   * é a dica do mouse, com o que a lista não tem espaço para exibir: categoria,
   * proprietário, criação e última execução.
   */
  private noDoTrabalho(trabalho: api.Trabalho, agente: No): No {
    const execucao = trabalho.ultimaExecucao
      ? `${formatarDataHora(trabalho.ultimaExecucao)} (${t(resultadoDoTrabalho(trabalho.resultado))})`
      : t("nunca executado");
    const dica = [
      trabalho.nome,
      trabalho.descricao,
      `${t("categoria")}: ${trabalho.categoria}`,
      `${t("proprietario")}: ${trabalho.proprietario}`,
      `${t("criado em")}: ${formatarDataHora(trabalho.criadoEm)}`,
      `${t("ultima execucao")}: ${execucao}`,
    ].filter((linha) => linha.length > 0);

    const no = new No({
      rotulo: trabalho.nome,
      simbolo: "▷",
      classe: trabalho.habilitado ? undefined : "no--desabilitado",
      detalhe: trabalho.habilitado ? undefined : t("trabalho desabilitado"),
      dica: dica.join("\n"),
      aoSelecionar: this.semCategoria,
      // Duplo clique abre as propriedades, como no resto da árvore o duplo
      // clique abre o que o nó tem para mostrar.
      aoAbrir: () => void this.propriedadesDoTrabalho(trabalho, agente),
      menu: (evento) => this.menuDoTrabalho(evento, trabalho, agente),
    });
    return no;
  }

  /** As ações do botão direito sobre um trabalho, na ordem do SSMS. */
  private menuDoTrabalho(evento: MouseEvent, trabalho: api.Trabalho, agente: No): void {
    abrirMenuDeContexto(evento, [
      { rotulo: t("novo trabalho"), acao: () => void this.novoTrabalho(agente) },
      {
        rotulo: t("iniciar trabalho na etapa"),
        separadorAntes: true,
        acao: () => void this.iniciarTrabalho(trabalho, agente),
      },
      { rotulo: t("parar trabalho"), acao: () => void this.pararTrabalho(trabalho) },
      {
        rotulo: t("exibir ddl"),
        separadorAntes: true,
        acao: () => void this.ddlDoTrabalho(trabalho),
      },
      {
        rotulo: t("habilitar"),
        separadorAntes: true,
        // Habilitar um trabalho já habilitado não faria nada, e oferecer a ação
        // assim mesmo faz o usuário duvidar do que está vendo.
        desabilitado: trabalho.habilitado,
        acao: () => void this.habilitarTrabalho(trabalho, true, agente),
      },
      {
        rotulo: t("desabilitar"),
        desabilitado: !trabalho.habilitado,
        acao: () => void this.habilitarTrabalho(trabalho, false, agente),
      },
      { rotulo: t("renomear"), acao: () => void this.renomearTrabalho(trabalho, agente) },
      { rotulo: t("excluir"), acao: () => void this.excluirTrabalho(trabalho, agente) },
      {
        rotulo: t("atualizar"),
        separadorAntes: true,
        acao: () => void agente.recarregar(),
      },
      {
        rotulo: t("propriedades"),
        acao: () => void this.propriedadesDoTrabalho(trabalho, agente),
      },
    ]);
  }

  /** Abre a tela de Novo Trabalho e relista o Agent se algo tiver sido criado. */
  private async novoTrabalho(no: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    if (!(await abrirDialogoTrabalho(sessaoId))) return;
    this.ligacoes.aoRelatar(t("trabalho criado"));
    await no.recarregar();
  }

  /** Abre a mesma tela preenchida, para editar. */
  private async propriedadesDoTrabalho(trabalho: api.Trabalho, agente: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    if (!(await abrirDialogoTrabalho(sessaoId, trabalho.id, trabalho.nome))) return;
    this.ligacoes.aoRelatar(t("trabalho gravado", { trabalho: trabalho.nome }));
    await agente.recarregar();
  }

  /**
   * Pergunta a etapa, manda o Agent rodar e acompanha até o fim.
   *
   * O comando **agenda** a execução e volta na hora: quem executa é o serviço, e
   * ele não avisa quando termina. Quem vai saber o fim é a tela de
   * acompanhamento, perguntando ao Agent de segundo em segundo — antes a barra
   * de status apenas avisava que não haveria resposta, e um trabalho disparado
   * daqui parecia não ter acontecido.
   */
  private async iniciarTrabalho(trabalho: api.Trabalho, agente: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    let etapa: string | null;
    try {
      const detalhe = await comIndicadorDeAbertura(trabalho.nome, () =>
        api.detalheDoTrabalho(sessaoId, trabalho.id),
      );
      etapa = await abrirDialogoIniciarNaEtapa(trabalho.nome, detalhe.etapas);
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
      return;
    }
    if (etapa === null) return;

    let execucao: api.ExecucaoDoTrabalho | null;
    try {
      execucao = await abrirDialogoDeExecucao(sessaoId, trabalho, etapa);
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
      return;
    }
    if (!execucao || this.sessaoId !== sessaoId) return;
    this.relatarExecucao(trabalho, execucao);
    // A lista volta do servidor com a última execução e o resultado dela, que é
    // o que a dica do mouse mostra. Sem isto a árvore continuaria contando a
    // execução de antes.
    if (!execucao.executando) await agente.recarregar();
  }

  /** O veredito na barra de status, depois que a tela de acompanhamento fecha. */
  private relatarExecucao(trabalho: api.Trabalho, execucao: api.ExecucaoDoTrabalho): void {
    const nome = trabalho.nome;
    if (execucao.executando) {
      this.ligacoes.aoRelatar(t("trabalho continua rodando", { trabalho: nome }));
      return;
    }
    if (execucao.resultado === "falha") {
      this.ligacoes.aoRelatar(
        t("trabalho concluido com falha", { trabalho: nome, mensagem: execucao.mensagem }),
        true,
      );
      return;
    }
    if (execucao.resultado === "sucesso") {
      this.ligacoes.aoRelatar(
        t("trabalho concluido ok", {
          trabalho: nome,
          tempo: formatarDuracao((execucao.duracao ?? 0) * 1_000),
        }),
      );
      return;
    }
    // Cancelado, em nova tentativa ou sem resultado registrado: o rótulo da
    // própria tela diz melhor do que uma frase inventada para cada caso.
    this.ligacoes.aoRelatar(`${nome}: ${rotuloDoResultado(execucao.resultado)}`);
  }

  private async pararTrabalho(trabalho: api.Trabalho): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    try {
      await api.pararTrabalho(sessaoId, trabalho.id);
      this.ligacoes.aoRelatar(t("trabalho parado", { trabalho: trabalho.nome }));
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  /**
   * O script que recria o trabalho, aberto numa aba do editor.
   *
   * É o mesmo destino do fonte de uma procedure do Oracle: texto com destaque de
   * sintaxe, numa aba que se pode salvar e levar para outro servidor.
   */
  private async ddlDoTrabalho(trabalho: api.Trabalho): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    try {
      await comIndicadorDeAbertura(trabalho.nome, async () => {
        const script = await api.ddlDoTrabalho(sessaoId, trabalho.id);
        await this.ligacoes.aoAbrirFonte(trabalho.nome, script, this.contexto());
      });
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private async habilitarTrabalho(
    trabalho: api.Trabalho,
    ligado: boolean,
    agente: No,
  ): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    try {
      await api.habilitarTrabalho(sessaoId, trabalho.id, ligado);
      this.ligacoes.aoRelatar(
        t(ligado ? "trabalho habilitado" : "trabalho desabilitado aviso", {
          trabalho: trabalho.nome,
        }),
      );
      await agente.recarregar();
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private async renomearTrabalho(trabalho: api.Trabalho, agente: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    const nome = await pedirTexto({
      titulo: t("renomear trabalho"),
      rotulo: t("nome"),
      valor: trabalho.nome,
    });
    if (nome === null || nome === trabalho.nome) return;
    try {
      await api.renomearTrabalho(sessaoId, trabalho.id, nome);
      this.ligacoes.aoRelatar(t("trabalho renomeado", { de: trabalho.nome, para: nome }));
      await agente.recarregar();
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private async excluirTrabalho(trabalho: api.Trabalho, agente: No): Promise<void> {
    const sessaoId = this.sessaoId;
    if (!sessaoId) return;
    const confirmado = await confirmar({
      titulo: t("excluir trabalho"),
      mensagem: t("excluir trabalho mensagem", { trabalho: trabalho.nome }),
    });
    if (!confirmado) return;
    try {
      await api.excluirTrabalho(sessaoId, trabalho.id);
      this.ligacoes.aoRelatar(t("trabalho excluido", { trabalho: trabalho.nome }));
      await agente.recarregar();
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  /**
   * Um banco da lista.
   *
   * Ele responde de três formas ao pedido de uma consulta nova, todas abrindo a
   * aba **naquele banco**: o botão direito, o atalho de nova consulta com o
   * ponteiro em cima (daí o `aoPairar`) e a seleção, que alimenta o botão da
   * barra.
   */
  private noDoBanco(banco: string): No {
    return new No({
      rotulo: banco,
      simbolo: "▤",
      carregar: async () => this.perfil.tipoBanco === "mongodb"
        ? this.nosDeColecao(banco)
        : this.categoriasDoBanco(banco),
      aoSelecionar: () => this.anunciarSelecao(null, banco),
      aoPairar: (dentro) =>
        this.ligacoes.aoPairarBanco(dentro && this.sessaoId ? this.contexto(banco) : null),
      menu: (evento) =>
        abrirMenuDeContexto(evento, [
          {
            rotulo: this.rotuloNovaConsulta(),
            desabilitado: !this.conectado,
            acao: () => this.ligacoes.aoNovaConsulta(this.contexto(banco)),
          },
        ]),
    });
  }

  private rotuloNovaConsulta(): string {
    return t(this.perfil.tipoBanco === "mongodb" ? "nova consulta mongodb" : "nova consulta sql", {
      atalho: sufixoDoAtalho("nova-consulta"),
    });
  }

  private async nosDeColecao(banco: string): Promise<No[]> {
    const colecoes = await api.listarColecoesMongo(this.exigirSessao(), banco);
    if (colecoes.length === 0) {
      return [new No({ rotulo: t("nenhuma colecao"), simbolo: "·", aoSelecionar: this.semCategoria })];
    }
    return colecoes.map((colecao) => {
      const contexto = () => ({ ...this.contexto(banco), colecao });
      return new No({
        rotulo: colecao,
        simbolo: "▧",
        aoSelecionar: () => {
          this.ligacoes.aoSelecionarConexao(contexto());
          this.ligacoes.aoSelecionarCategoria(null);
        },
        aoPairar: (dentro) => this.ligacoes.aoPairarBanco(dentro && this.sessaoId ? contexto() : null),
        aoAbrir: () => this.ligacoes.aoNovaConsulta(contexto()),
      });
    });
  }

  /**
   * As categorias de um banco do SQL Server.
   *
   * O escopo aqui é o **banco**, e não o schema como no Oracle (requisito 2):
   * um banco tem vários esquemas, e por isso cada objeto aparece como
   * `esquema.nome` — que é o nome do SSMS e, não por acaso, o que se escreve
   * numa consulta.
   */
  private categoriasDoBanco(banco: string): No[] {
    return CATEGORIAS_DO_BANCO.map((pasta) => {
      if (!pasta.chave) {
        return new No({
          rotulo: t(pasta.chaveDoRotulo),
          simbolo: pasta.simbolo,
          classe: "no--categoria",
          aoSelecionar: this.semCategoria,
          carregar: async () =>
            (pasta.filhas ?? []).map((filha) =>
              this.noDeCategoriaDoBanco(filha.chave, t(filha.chaveDoRotulo), filha.simbolo, banco),
            ),
        });
      }
      return this.noDeCategoriaDoBanco(
        pasta.chave,
        t(pasta.chaveDoRotulo),
        pasta.simbolo,
        banco,
      );
    });
  }

  /**
   * Uma categoria dentro de um banco.
   *
   * Filtrável como as do Oracle (requisito 4). O filtro é **da conexão**, e não
   * do banco: a mesma categoria aparece embaixo de cada banco do servidor, e
   * todas passam a usar o critério guardado. Um nó de outro banco já aberto só
   * mostra o rótulo novo depois de recarregado.
   */
  private noDeCategoriaDoBanco(
    categoria: api.Categoria,
    rotulo: string,
    simbolo: string,
    banco: string,
  ): No {
    const no: No = new No({
      rotulo,
      simbolo,
      classe: "no--categoria",
      carregar: () => this.paginaDeObjetos(categoria, 0, banco),
      menu: (evento) => this.menuDaCategoria(evento, categoria, rotulo, no),
      // O botão de filtro da barra age sobre a categoria selecionada.
      aoSelecionar: () =>
        this.anunciarSelecao({
          rotulo,
          filtrar: () => this.pedirFiltro(this.alvoDeCategoria(categoria, rotulo, no)),
        }),
    });
    this.aplicarRotuloDeFiltro(no, categoria, rotulo);
    return no;
  }

  /** Os seis nós fixos do Oracle. */
  private nosDeCategoria(): No[] {
    return CATEGORIAS.map(({ chave, simbolo }) => {
      // O rótulo da categoria vem do idioma; o resto da tabela é estrutura.
      const rotulo = t(chave);
      const no = new No({
        rotulo,
        simbolo,
        classe: "no--categoria",
        carregar: () => this.paginaDeObjetos(chave, 0),
        menu: (evento) => this.menuDaCategoria(evento, chave, rotulo, no),
        // O botão de filtro da barra age sobre a categoria selecionada.
        aoSelecionar: () =>
          this.anunciarSelecao({
            rotulo,
            filtrar: () => this.pedirFiltro(this.alvoDeCategoria(chave, rotulo, no)),
          }),
      });
      this.aplicarRotuloDeFiltro(no, chave, rotulo);
      return no;
    });
  }

  /**
   * Uma página de objetos. Havendo mais, o último nó é o botão que traz a
   * próxima.
   *
   * Quantos vêm por vez é a opção da seção Geral das Configurações (requisito
   * 41), lida a cada página: trocá-la vale a partir da próxima listagem, sem
   * reconectar.
   */
  private async paginaDeObjetos(
    categoria: api.Categoria,
    deslocamento: number,
    banco?: string,
    tamanho = opcoesDaArvore().objetosPorVez,
  ): Promise<No[]> {
    const pagina = await api.listarObjetos(
      this.exigirSessao(),
      categoria,
      this.filtros.get(categoria) ?? null,
      deslocamento,
      tamanho,
      banco ?? null,
    );

    const nos = pagina.objetos.map((objeto) => {
      const invalido = objeto.situacao === "INVALID";
      // O nó é declarado antes de ser usado no menu de propósito: o menu só
      // roda quando alguém clica, e precisa da mão no próprio nó para redesenhar
      // o comentário sem relistar a categoria inteira.
      const no: No = new No({
        // O nome vai qualificado para o servidor e sem o `dbo.` para a tela
        // (requisito 44): o que se abre é `objeto.nome`, o que se lê é este.
        rotulo: api.nomeVisivel(objeto.nome),
        simbolo: simboloDoObjeto(categoria),
        classe: invalido ? "no--invalido" : undefined,
        // O comentário da view (requisito 37) vai ao lado do nome, e o texto
        // inteiro fica na dica.
        detalhe: resumoDoComentario(objeto.comentario),
        dica: dicaDoObjeto(objeto),
        aoSelecionar: this.semCategoria,
        carregar: this.temFilhos(categoria)
          ? () => this.filhosDoObjeto(categoria, objeto.nome, banco)
          : undefined,
        aoAbrir: CATEGORIAS_DE_CODIGO.has(categoria)
          ? () => this.abrirFonte(categoria, objeto.nome, "corpo", banco)
          : () => this.abrirFonte(categoria, objeto.nome, "especificacao", banco),
        menu:
          categoria === "tabelas" || categoria === "views"
            ? (evento) => this.menuDoObjeto(evento, categoria, objeto, no, banco)
            : undefined,
      });
      return no;
    });

    if (pagina.temMais) {
      nos.push(
        this.noDeListarMais(categoria, deslocamento + pagina.objetos.length, banco),
      );
    }
    return nos;
  }

  /**
   * O "Listar mais…" do fim da lista; ao ser clicado, cede o lugar aos que
   * trouxe. Traz quantos o rótulo promete, mesmo que a opção mude antes do
   * clique: o número que está na tela é o que vale.
   */
  private noDeListarMais(categoria: api.Categoria, deslocamento: number, banco?: string): No {
    const tamanho = opcoesDaArvore().objetosPorVez;
    const no: No = new No({
      rotulo: t("listar mais", { n: tamanho }),
      simbolo: "⋯",
      classe: "no--listar-mais",
      dica: t("ja listados", { n: deslocamento }),
      aoSelecionar: this.semCategoria,
      aoClicar: async () => {
        no.renomear(t("carregando"));
        try {
          no.substituirPor(await this.paginaDeObjetos(categoria, deslocamento, banco, tamanho));
        } catch (erro) {
          no.renomear(api.mensagemDeErro(erro));
          no.marcarComoErro();
        }
      },
    });
    return no;
  }

  /** Tabelas e views abrem em colunas; pacotes, em spec, body e subprogramas. */
  private temFilhos(categoria: api.Categoria): boolean {
    return categoria === "tabelas" || categoria === "views" || categoria === "pacotes";
  }

  /** A sessão precisa existir: só se chega aqui com o nó da conexão aberto. */
  private exigirSessao(): string {
    if (!this.sessaoId) throw new Error(t("conexao nao esta aberta"));
    return this.sessaoId;
  }

  private contexto(banco?: string): ContextoDeConexao {
    return { perfil: this.perfil, sessaoId: this.exigirSessao(), banco };
  }

  private async filhosDoObjeto(
    categoria: api.Categoria,
    nome: string,
    banco?: string,
  ): Promise<No[]> {
    if (categoria === "pacotes") {
      const subprogramas = await api.listarSubprogramas(this.exigirSessao(), nome);
      return [
        new No({
          rotulo: t("especificacao"),
          simbolo: "≡",
          classe: "no--fonte",
          aoSelecionar: this.semCategoria,
          aoAbrir: () => this.abrirFonte(categoria, nome, "especificacao"),
        }),
        new No({
          rotulo: "Body",
          simbolo: "≡",
          classe: "no--fonte",
          aoSelecionar: this.semCategoria,
          aoAbrir: () => this.abrirFonte(categoria, nome, "corpo"),
        }),
        ...subprogramas.map(
          (sub) =>
            new No({
              rotulo: sub.nome,
              simbolo: sub.especie === "FUNCTION" ? "ƒ" : "▷",
              dica: `${sub.especie} ${nome}.${sub.nome}`,
              aoSelecionar: this.semCategoria,
              aoAbrir: () => this.abrirFonte(categoria, nome, "corpo"),
            }),
        ),
      ];
    }

    const colunas = await api.listarColunas(this.exigirSessao(), nome, banco ?? null);
    return colunas.map(
      (coluna) =>
        new No({
          rotulo: coluna.nome,
          simbolo: "•",
          classe: "no--coluna",
          detalhe: coluna.tipo,
          dica: `${coluna.nome} ${coluna.tipo}${coluna.aceitaNulo ? "" : " NOT NULL"}`,
          aoSelecionar: this.semCategoria,
        }),
    );
  }

  private async abrirFonte(
    categoria: api.Categoria,
    nome: string,
    parte: api.ParteDoFonte,
    banco?: string,
  ): Promise<void> {
    const sufixo = categoria === "pacotes" ? (parte === "corpo" ? " (Body)" : " (Spec)") : "";
    const titulo = `${api.nomeVisivel(nome)}${sufixo}`;
    try {
      await comIndicadorDeAbertura(titulo, async () => {
        const texto = await api.fonteDoObjeto(
          this.exigirSessao(),
          categoria,
          nome,
          parte,
          banco ?? null,
        );
        await this.ligacoes.aoAbrirFonte(titulo, texto, this.contexto(banco));
      });
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  /** Ações de tabela e view: as três abas do painel, e o comentário da view. */
  private menuDoObjeto(
    evento: MouseEvent,
    categoria: "tabelas" | "views",
    objeto: api.Objeto,
    no: No,
    banco?: string,
  ): void {
    const abrir = (aba: AbaDoObjeto): void =>
      this.ligacoes.aoAbrirObjeto(objeto.nome, categoria, aba, this.contexto(banco));
    const itens: ItemDeMenu[] = [
      { rotulo: t("exibir dados"), acao: () => abrir("dados") },
      { rotulo: t("exibir ddl"), acao: () => abrir("ddl") },
      { rotulo: t("exibir estrutura"), acao: () => abrir("estrutura") },
    ];
    // Só na view (requisito 37): é nela que o nome sozinho não diz o que o
    // objeto é. A tabela se explica pelas colunas; a view, não.
    if (categoria === "views") {
      itens.push({
        rotulo: t("editar comentario"),
        separadorAntes: true,
        acao: () => void this.editarComentario(objeto, no, banco),
      });
    }
    abrirMenuDeContexto(evento, itens);
  }

  /**
   * Edita e regrava o comentário da view (requisito 37).
   *
   * O texto que a tela recebe é o que veio com a listagem, e não uma leitura
   * nova: é o mesmo comentário que a linha está exibindo, e reler o daria a
   * quem editou a chance de ver na tela um texto diferente do que ele acabou
   * de ler na árvore.
   *
   * Gravado, o nó se redesenha no lugar. Relistar a categoria custaria uma
   * página inteira do banco para mudar uma linha — e, numa categoria expandida
   * com "Listar mais" já clicado várias vezes, jogaria fora tudo o que foi
   * trazido.
   */
  private async editarComentario(objeto: api.Objeto, no: No, banco?: string): Promise<void> {
    const informado = await pedirTextoLongo({
      titulo: t("comentario da view", { view: api.nomeVisivel(objeto.nome) }),
      rotulo: t("comentario"),
      valor: objeto.comentario ?? "",
      limite: LIMITE_DO_COMENTARIO,
    });
    // Vazio **não** é desistência: é como se apaga o comentário. Só o
    // cancelamento devolve nulo, e é só ele que não faz nada.
    if (informado === null) return;

    try {
      await api.gravarComentario(this.exigirSessao(), objeto.nome, informado, banco ?? null);
    } catch (erro) {
      this.ligacoes.aoRelatar(api.mensagemDeErro(erro), true);
      return;
    }

    objeto.comentario = informado || null;
    no.definirDetalhe(resumoDoComentario(objeto.comentario));
    no.definirDica(dicaDoObjeto(objeto));
    this.ligacoes.aoRelatar(
      t(informado ? "comentario gravado" : "comentario apagado", { view: objeto.nome }),
    );
  }

  // --- Filtro (requisito 4) ------------------------------------------------

  /** O nó que o filtro alcança, e como ele se reescreve depois da mudança. */
  private alvoDeCategoria(categoria: api.Categoria, rotulo: string, no: No): AlvoDoFiltro {
    return {
      categoria,
      rotulo,
      no,
      rotular: () => this.aplicarRotuloDeFiltro(no, categoria, rotulo),
    };
  }

  /** O mesmo para o Agent, que escreve o estado do serviço junto (requisito 36). */
  private alvoDoAgente(no: No): AlvoDoFiltro {
    return {
      categoria: FILTRO_DO_AGENTE,
      rotulo: ROTULO_DO_AGENTE,
      no,
      rotular: () => this.desenharAgente(no),
    };
  }

  /** Todas as seis categorias podem ser filtradas. */
  private menuDaCategoria(
    evento: MouseEvent,
    categoria: api.Categoria,
    rotulo: string,
    no: No,
  ): void {
    const alvo = this.alvoDeCategoria(categoria, rotulo, no);
    abrirMenuDeContexto(evento, [
      { rotulo: t("aplicar filtro"), acao: () => void this.pedirFiltro(alvo) },
      {
        rotulo: t("limpar filtro"),
        desabilitado: !this.filtros.has(categoria),
        acao: () => void this.definirFiltro(alvo, null),
      },
      {
        rotulo: t("atualizar"),
        separadorAntes: true,
        acao: () => void no.recarregar(async () => {
          const indexada = ["tabelas", "views", "pacotes", "procedures"].includes(categoria);
          if (indexada && this.perfil.tipoBanco !== "sqlserver" && this.sessaoId) {
            await api.listarObjetos(this.sessaoId, categoria, null, 0, 0, null, true);
          }
        }),
      },
    ]);
  }

  /** Abre a tela do filtro e aplica o que ela devolver. */
  private async pedirFiltro({ categoria, rotulo, no, rotular }: AlvoDoFiltro): Promise<void> {
    const dados = ALVO_DO_FILTRO[categoria];
    if (!dados) throw new Error(`categoria de filtro desconhecida: ${categoria}`);
    const sqlServer = this.perfil.tipoBanco === "sqlserver";
    const informado = await abrirDialogoFiltro(rotulo, this.filtros.get(categoria) ?? null, {
      alvo: t(dados.alvo),
      alvoSql: dados.alvoSql,
      simbolo: dados.simbolo,
      // "Objetos gerados" vem de USER_OBJECTS, que não cobre o Programador. No
      // SQL Server a opção não existe por outro motivo: `is_ms_shipped = 0` já
      // vale sempre na listagem, e uma caixa que não muda nada é pior do que
      // caixa nenhuma.
      excluirGerados: !sqlServer && categoria !== "programador",
    });
    if (informado === null) return;
    await this.definirFiltro({ categoria, rotulo, no, rotular }, informado);
  }

  /** Aplica o filtro, guarda em disco e relista o nó. */
  private async definirFiltro(
    { categoria, no, rotular }: AlvoDoFiltro,
    filtro: api.Filtro | null,
  ): Promise<void> {
    const efetivo = api.filtroAtivo(filtro) ? filtro : null;
    if (efetivo) {
      this.filtros.set(categoria, efetivo);
    } else {
      this.filtros.delete(categoria);
    }
    rotular();

    try {
      await api.gravarFiltro(this.perfil.id, categoria, efetivo);
    } catch (erro) {
      // O filtro vale nesta sessão mesmo sem conseguir guardar.
      this.ligacoes.aoRelatar(
        t("filtro aplicado sem guardar", { erro: api.mensagemDeErro(erro) }),
        true,
      );
    }
    await no.recarregar();
  }

  /** Acrescenta "(Filtrado)" ao nó, como o SQL Developer faz. */
  private aplicarRotuloDeFiltro(no: No, categoria: api.Categoria, rotulo: string): void {
    const filtro = this.filtros.get(categoria);
    no.renomear(filtro ? `${rotulo} ${t("filtrado")}` : rotulo);
    no.definirDica(
      filtro ? t("filtro ativo", { filtro: api.resumoDoFiltro(filtro) }) : rotulo,
    );
  }
}

// --- Nó da árvore ----------------------------------------------------------

/** As classes de estado que um nó pode ter — nunca mais de uma ao mesmo tempo. */
const CLASSES_DE_ESTADO = ["no--conectado", "no--desconectado", "no--parado"] as const;

type ClasseDeEstado = (typeof CLASSES_DE_ESTADO)[number];

interface OpcoesDoNo {
  rotulo: string;
  simbolo: string;
  classe?: string;
  cor?: string | null;
  dica?: string;
  /** Texto secundário, à direita (o tipo de uma coluna, por exemplo). */
  detalhe?: string;
  /** Presente quando o nó é expansível. */
  carregar?: () => Promise<No[]>;
  /** Duplo clique. */
  aoAbrir?: () => void;
  /** Clique simples, para nós que agem em vez de expandir. */
  aoClicar?: () => void;
  /** Chamado quando este nó passa a ser o selecionado. */
  aoSelecionar?: () => void;
  /** Chamado quando o ponteiro do mouse entra (`true`) e sai (`false`). */
  aoPairar?: (dentro: boolean) => void;
  menu?: (evento: MouseEvent) => void;
}

class No {
  readonly elemento = document.createElement("li");
  private readonly linha = document.createElement("div");
  private readonly alternador = document.createElement("span");
  private readonly simbolo = document.createElement("span");
  private readonly rotulo = document.createElement("span");
  private readonly listaFilhos = document.createElement("ul");
  /** Só existe depois de o nó ter algum detalhe a mostrar. */
  private detalhe: HTMLElement | null = null;
  private carregado = false;
  private expandido = false;
  private carregando = false;

  constructor(private readonly opcoes: OpcoesDoNo) {
    this.linha.className = `no ${opcoes.classe ?? ""}`.trim();
    this.linha.tabIndex = 0;

    this.alternador.className = "no__alternador";
    this.alternador.textContent = opcoes.carregar ? "▸" : "";

    this.simbolo.className = "no__simbolo";
    this.simbolo.textContent = opcoes.simbolo;
    if (opcoes.cor) this.simbolo.style.color = opcoes.cor;

    this.rotulo.className = "no__rotulo";
    this.rotulo.textContent = opcoes.rotulo;

    this.linha.append(this.alternador, this.simbolo, this.rotulo);
    if (opcoes.detalhe) this.definirDetalhe(opcoes.detalhe);
    this.linha.title = opcoes.dica ?? opcoes.rotulo;

    this.listaFilhos.className = "no__filhos";
    this.listaFilhos.hidden = true;
    this.elemento.append(this.linha, this.listaFilhos);

    this.ligarEventos();
  }

  private ligarEventos(): void {
    this.linha.addEventListener("click", () => {
      this.selecionar();
      this.opcoes.aoClicar?.();
      if (this.opcoes.carregar) void this.alternar();
    });
    this.linha.addEventListener("dblclick", () => this.opcoes.aoAbrir?.());
    if (this.opcoes.aoPairar) {
      this.linha.addEventListener("mouseenter", () => this.opcoes.aoPairar?.(true));
      this.linha.addEventListener("mouseleave", () => this.opcoes.aoPairar?.(false));
    }
    this.linha.addEventListener("keydown", (evento) => {
      if (evento.key === "F5" && !evento.ctrlKey && !evento.metaKey && !evento.altKey && !evento.shiftKey) {
        evento.preventDefault();
        evento.stopPropagation();
        if (!evento.repeat && !document.querySelector(".dialogo-fundo")) void this.recarregar();
        return;
      }
      if (evento.key === "Enter") {
        if (this.opcoes.aoAbrir) this.opcoes.aoAbrir();
        else void this.alternar();
      }
    });
    // Nenhum nó da árvore pode cair no menu nativo da webview (o "Voltar,
    // Imprimir, Salvar como…" do navegador): ou mostra o menu próprio do nó, ou
    // ao menos "Abrir" quando há o que abrir — o mesmo destino do duplo clique —,
    // e, quando não há nada a oferecer, apenas engole o evento.
    this.linha.addEventListener("contextmenu", (evento) => {
      evento.preventDefault();
      const { menu, aoAbrir } = this.opcoes;
      if (menu) {
        this.selecionar();
        menu(evento);
      } else if (aoAbrir) {
        this.selecionar();
        abrirMenuDeContexto(evento, [{ rotulo: t("abrir"), acao: aoAbrir }]);
      }
    });
  }

  private selecionar(): void {
    for (const outra of document.querySelectorAll(".no--selecionado")) {
      outra.classList.remove("no--selecionado");
    }
    this.linha.classList.add("no--selecionado");
    this.opcoes.aoSelecionar?.();
  }

  renomear(texto: string): void {
    this.rotulo.textContent = texto;
  }

  definirDica(texto: string): void {
    this.linha.title = texto;
  }

  definirSimbolo(texto: string): void {
    this.simbolo.textContent = texto;
  }

  /**
   * Texto secundário à direita; o elemento nasce na primeira vez que é usado.
   *
   * Some quando o texto fica vazio — apagar o comentário de uma view deixaria,
   * senão, um espaço pendurado depois do nome.
   */
  definirDetalhe(texto: string): void {
    if (!this.detalhe) {
      if (!texto) return;
      this.detalhe = document.createElement("span");
      this.detalhe.className = "no__detalhe";
      this.linha.append(this.detalhe);
    }
    this.detalhe.hidden = !texto;
    this.detalhe.textContent = texto;
  }

  marcarComoErro(): void {
    this.linha.classList.add("no--invalido");
  }

  /**
   * Aplica **uma** das classes de estado, tirando a que estivesse valendo.
   *
   * Um nó só pode estar num estado de cada vez, e trocar sem tirar a anterior é
   * como o nó da conexão acabaria conectado e desconectado ao mesmo tempo.
   */
  marcarEstado(classe: ClasseDeEstado | null): void {
    this.linha.classList.remove(...CLASSES_DE_ESTADO);
    if (classe) this.linha.classList.add(classe);
  }

  /** Fecha o nó e esquece o que já havia sido carregado. */
  recolher(): void {
    this.expandido = false;
    this.carregado = false;
    this.listaFilhos.replaceChildren();
    this.listaFilhos.hidden = true;
    this.alternador.textContent = "▸";
  }

  /** Troca este nó pelos que vieram no lugar dele. */
  substituirPor(novos: No[]): void {
    this.elemento.replaceWith(emFragmento(novos));
  }

  async alternar(): Promise<void> {
    if (!this.opcoes.carregar) return;
    if (this.expandido) {
      this.expandido = false;
      this.listaFilhos.hidden = true;
      this.alternador.textContent = "▸";
      return;
    }
    this.expandido = true;
    this.listaFilhos.hidden = false;
    this.alternador.textContent = "▾";
    if (!this.carregado) await this.carregarFilhos();
  }

  /** Relê os filhos do banco, mantendo o nó aberto. */
  async recarregar(antesDeCarregar?: () => Promise<void>): Promise<void> {
    if (!this.opcoes.carregar) return;
    this.carregado = false;
    this.expandido = true;
    this.listaFilhos.hidden = false;
    this.alternador.textContent = "▾";
    await this.carregarFilhos(antesDeCarregar);
  }

  private async carregarFilhos(antesDeCarregar?: () => Promise<void>): Promise<void> {
    if (this.carregando) return;
    this.carregando = true;
    this.listaFilhos.replaceChildren(mensagem(t("carregando")));
    try {
      await antesDeCarregar?.();
      const filhos = await this.opcoes.carregar!();
      // Nunca espalhar a lista em argumentos (`replaceChildren(...filhos)`):
      // com muitos itens isso estoura a pilha de chamadas do navegador.
      this.listaFilhos.replaceChildren(
        filhos.length > 0 ? emFragmento(filhos) : mensagem(t("vazio")),
      );
      this.carregado = true;
    } catch (erro) {
      this.listaFilhos.replaceChildren(mensagem(api.mensagemDeErro(erro), true));
    } finally {
      this.carregando = false;
    }
  }
}

/**
 * Junta os nós num fragmento.
 *
 * É o jeito seguro de inserir muitos elementos de uma vez: espalhar a lista em
 * argumentos (`replaceChildren(...nos)`) estoura a pilha de chamadas quando a
 * lista é grande.
 */
function emFragmento(nos: No[]): DocumentFragment {
  const fragmento = document.createDocumentFragment();
  for (const no of nos) fragmento.append(no.elemento);
  return fragmento;
}

/** Símbolo de um objeto, herdado do símbolo da categoria a que pertence. */
/**
 * O comentário como ele aparece ao lado do nome: entre parênteses, numa linha
 * só e cortado no que couber (requisito 37).
 *
 * A quebra de linha desaparece porque a linha da árvore é uma só — um
 * comentário de três parágrafos precisa virar uma frase para caber ao lado do
 * nome. Nada se perde: o texto inteiro continua na dica do mouse e na tela de
 * edição.
 */
function resumoDoComentario(comentario: string | null): string {
  const linha = (comentario ?? "").replace(/\s+/g, " ").trim();
  if (!linha) return "";
  const curto =
    linha.length > CARACTERES_DO_COMENTARIO
      ? `${linha.slice(0, CARACTERES_DO_COMENTARIO)}…`
      : linha;
  return `(${curto})`;
}

/** Nome, situação e o comentário inteiro — o que a linha não teve espaço de mostrar. */
function dicaDoObjeto(objeto: api.Objeto): string {
  const cabecalho = objeto.situacao ? `${objeto.nome} — ${objeto.situacao}` : objeto.nome;
  return objeto.comentario ? `${cabecalho}\n${objeto.comentario}` : cabecalho;
}

function simboloDoObjeto(categoria: api.Categoria): string {
  const oracle = CATEGORIAS.find((c) => c.chave === categoria)?.simbolo;
  if (oracle) return oracle;
  // As duas categorias do SQL Server moram na outra tabela, e uma delas dentro
  // da pasta Functions.
  for (const pasta of CATEGORIAS_DO_BANCO) {
    if (pasta.chave === categoria) return pasta.simbolo;
    const filha = pasta.filhas?.find((f) => f.chave === categoria);
    if (filha) return filha.simbolo;
  }
  return "•";
}

function mensagem(texto: string, erro = false): HTMLElement {
  const item = document.createElement("li");
  item.className = erro ? "no__aviso no__aviso--erro" : "no__aviso";
  item.textContent = texto;
  return item;
}
