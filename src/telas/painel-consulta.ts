/**
 * Aba de consulta SQL: barra de ferramentas, editor, grid e painéis
 * inferiores.
 *
 * Cobre os requisitos 5 (execução com seleção e script), 6 (grid com cópia),
 * 7 (commit/rollback), 8 (Saída DBMS), 10 (fetch ajustável), 13 (compilar
 * PL/SQL), 14 (parar), 15 (binds), 16 (exportar), 17 (rodapé), 18 (mensagens),
 * 19 (histórico) e 39 (script de INSERT).
 */

import "./painel-consulta.css";

import * as api from "../api";
import { fecharComEsc } from "../componentes/janela";
import { atalhoVisivel, eventoEhAtalhoConfigurado } from "../atalhos";
import { t } from "../idioma";
import { formatarDuracao } from "../formatacao";
import { criarDivisor } from "../componentes/divisor";
import { abrirMenuDeContexto } from "../componentes/menu-contexto";
import type { ContextoDeConexao } from "./arvore-objetos";
import { pedirValoresDosBinds } from "./dialogo-binds";
import { EditorSql, tabelasDoComando } from "./editor-sql";
import { Grid } from "./grid";
import { paraCsv } from "./exportacao";
import { copiarInsertParaClipboard } from "./script-insert";
import {
  abrirConfiguracaoDoBotao,
  abrirNovoBotaoRapido,
  atalhoDoEvento,
  botaoValeParaConexao,
  moverBotaoRapido,
  obterConfiguracoesDosBotoes,
  observarConfiguracoesDosBotoes,
  type ConexaoParaFiltro,
  type ConfiguracaoDeBotaoRapido,
} from "./botoes-rapidos";

/** Tamanho de fetch inicial, igual ao padrão do SQL Developer. */
const FETCH_PADRAO = 50;

/**
 * Quantas linhas de cada grid ficam à mostra quando há vários empilhados.
 *
 * O limite existe para caber mais de um conjunto na tela sem rolar, e porque a
 * virtualização do grid depende de a caixa ser menor que o conteúdo. Cada grid
 * rola por dentro; a pilha rola por fora.
 */
const LINHAS_VISIVEIS_NA_PILHA = 10;

/** Altura de uma linha do grid, em pixels: o passo do puxador pelo teclado. */
const ALTURA_DA_LINHA = 22;

/** Tamanho de buffer inicial do DBMS_OUTPUT, como no SQL Developer. */
const BUFFER_DBMS_PADRAO = 20000;

/** Quanto se espera, sem digitação, antes de gravar a cópia de recuperação. */
const ESPERA_DA_COPIA = 2000;

/** Acima deste tamanho o texto não é copiado (ver requisito 12). */
const MAXIMO_DA_COPIA = 5 * 1024 * 1024;

/** O que a aba sabe de si: como se identifica e a que arquivo pertence. */
export interface DadosDaAba {
  /** Identificador da cópia de recuperação; só letras, números, `-` e `_`. */
  id: string;
  titulo: string;
  /** Arquivo a que a aba está amarrada, quando já veio de um. */
  arquivo: string | null;
}

export class PainelDeConsulta {
  readonly elemento = document.createElement("div");

  private readonly editor: EditorSql;
  private readonly areaDeResultado = document.createElement("div");
  private readonly saidaDeScript = document.createElement("pre");
  private readonly painelMensagens = document.createElement("div");
  /**
   * Onde as mensagens realmente entram.
   *
   * É separado do painel porque a barra do **Limpar** fica parada no topo
   * enquanto a lista rola — a rolagem é daqui, não do painel inteiro.
   */
  private readonly listaDeMensagens = document.createElement("div");
  private readonly painelHistorico = document.createElement("div");
  private readonly painelDbms = document.createElement("div");
  private readonly areaInferior = document.createElement("div");
  private readonly rodape = document.createElement("div");
  private readonly informacaoRodape = document.createElement("span");
  private readonly indicadorExecucao = document.createElement("div");
  private readonly campoFetch = document.createElement("input");
  private readonly divisorDoResultado: HTMLElement;
  private grupoBotoesRapidos: HTMLElement | null = null;

  private readonly botoes = new Map<string, HTMLButtonElement>();
  private configuracoesDosBotoes: ConfiguracaoDeBotaoRapido[] = [];
  private pararDeObservarBotoes = (): void => {};
  /**
   * Grid em foco — o que o menu de contexto, a cópia e a exportação usam.
   *
   * Com vários conjuntos empilhados é o último em que se clicou; com um só, é
   * ele mesmo.
   */
  private grid: Grid | null = null;
  /** Os grids do último resultado, na ordem em que o banco os devolveu. */
  private grids: { grid: Grid; cursorId: string | null; comando: string }[] = [];
  private executando = false;
  /** Um Ctrl+clique desta aba está procurando o destino. */
  private navegando = false;
  /** Lista de bancos da barra; só existe em aba de SQL Server. */
  private seletorDeBanco: HTMLSelectElement | null = null;
  /** Banco em que esta aba executa agora. */
  private bancoAtual: string | null = null;
  private abaInferiorAberta: string | null = null;
  /** Auto-commit desta aba: começa no do perfil e segue o interruptor da barra. */
  private autoCommit: boolean;
  /**
   * Há trabalho executado e ainda não confirmado nesta aba.
   *
   * Existe porque a mensagem de uma execução some da vista assim que outra
   * chega, e fechar a aba descarta o pendente sem perguntar (requisito 12). No
   * SQL Server isso alcança também o DDL — um `alter procedure` sem commit se
   * perde inteiro —, então o estado precisa ficar visível na barra enquanto
   * durar, e não só no instante da execução.
   */
  private transacaoPendente = false;
  private temporizadorDaCopia: number | undefined;
  private ultimoTextoCopiado: string;
  private pararDeObservarCatalogo?: Promise<() => void>;
  private arquivoAtual: string | null;
  /**
   * A altura que o divisor tinha dado ao editor quando o resultado saiu de
   * vista. Sem resultado, o editor fica com o espaço inteiro; o próximo
   * resultado traz de volta a divisão que o usuário tinha escolhido.
   */
  private tamanhoDoEditor: string | null = null;

  constructor(
    private readonly contexto: ContextoDeConexao,
    /**
     * Sessão própria desta aba: nela rodam execução, cancelamento, transação
     * (commit/rollback, auto-commit) e Saída DBMS, para uma consulta pesada
     * aqui não travar as outras abas nem o autocomplete (requisito 7). O
     * autocomplete e o compilar seguem na sessão de metadados (`contexto`).
     */
    private readonly sessaoExecucao: string,
    private readonly aoRelatar: (mensagem: string, erro?: boolean) => void,
    textoInicial = "",
    private readonly dadosDaAba: DadosDaAba | null = null,
    /** Abre o destino de um Ctrl+clique; a promessa acaba quando ele abriu. */
    private readonly aoAbrirReferencia?: (
      referencia: string,
      banco: string | null,
    ) => Promise<void>,
    /** Avisa a aba para acender ou apagar o símbolo de execução (requisito 17). */
    private readonly aoMudarExecucao?: (executando: boolean) => void,
  ) {
    this.elemento.className = "painel-consulta";
    this.arquivoAtual = dadosDaAba?.arquivo ?? null;
    // A aba já nasce sabendo em que banco vai executar (requisito 3): sem isto,
    // o autocomplete das primeiras teclas perguntaria pelo banco da conexão
    // enquanto a lista da barra ainda estivesse carregando.
    this.bancoAtual = contexto.banco ?? null;
    this.autoCommit = contexto.perfil.autoCommit;
    // O texto de partida já conta como copiado: abrir um fonte ou um arquivo e
    // não mexer nele não precisa gerar cópia de nada.
    this.ultimoTextoCopiado = textoInicial;
    this.editor = new EditorSql({
      texto: textoInicial,
      // O autocomplete pergunta à sessão da **conexão**, e não à da aba. No
      // Oracle isso é economia — o cache fica compartilhado entre as abas —, e
      // no SQL Server é necessidade: a conexão da aba pode estar com uma
      // consulta correndo ou com um grid pela metade, e no TDS ela atende um
      // pedido de cada vez. Em troca é preciso dizer em que banco a aba está,
      // que é o que `bancoAtual` carrega (requisito 3).
      sugestoes: {
        // O escopo vem do nome de três partes que estiver sendo escrito
        // (`Vendas..cliente`); sem ele valem o banco da aba e todos os
        // esquemas dele.
        objetos: (prefixo, escopo, apenasRotinas) =>
          api.sugerirObjetos(
            contexto.sessaoId,
            prefixo,
            escopo?.banco ?? this.bancoAtual,
            escopo?.esquema ?? null,
            apenasRotinas,
          ),
        ...(contexto.perfil.tipoBanco === "oracle" ? {
          relacoes: (prefixo: string, esquema: string | undefined, local: boolean) =>
            api.sugerirRelacoes(contexto.sessaoId, prefixo, esquema ?? null, local),
        } : {}),
        colunas: (objeto, banco) =>
          api.listarColunas(contexto.sessaoId, objeto, banco ?? this.bancoAtual, true),
        subprogramas: (pacote) => api.listarSubprogramas(contexto.sessaoId, pacote),
        // Só o SQL Server tem bancos a sugerir; no Oracle a função não existe,
        // e é a falta dela que diz ao editor que ali não há nome de três partes.
        ...(contexto.perfil.tipoBanco === "sqlserver"
          ? { bancos: () => api.listarBancos(contexto.sessaoId, true) }
          : {}),
      },
      aoExecutar: () => void this.executar(false),
      aoExecutarScript: () => void this.executar(true),
      aoCompilar: () => void this.compilar(),
      aoMudar: () => this.agendarCopia(),
      aoCtrlClicar: (referencia) => this.abrirReferencia(referencia),
      // O sublinhado do Ctrl pergunta à sessão da conexão (requisito 22). No
      // Oracle ela responde por uma conexão à parte; no SQL Server, pelo
      // catálogo do banco em que a aba está — e é por isso que o banco vai
      // junto, e não o da conexão.
      verificarReferencias: (referencias: string[]) =>
        api.referenciasNavegaveis(contexto.sessaoId, referencias, this.bancoAtual),
      aoAbrirMenuDeReferencia: (evento, referencia) =>
        abrirMenuDeContexto(evento, [
          {
            rotulo: t("ir para definicao"),
            acao: () => this.abrirReferencia(referencia),
          },
        ]),
    });

    if (contexto.perfil.tipoBanco === "sqlserver") {
      this.pararDeObservarCatalogo = api.aoAtualizarCatalogo((sessaoId) => {
        if (sessaoId !== contexto.sessaoId) return;
        this.editor.esquecerObjetos();
        this.editor.esquecerColunas();
      }).catch(() => () => {});
    }

    this.divisorDoResultado = criarDivisor({
      orientacao: "horizontal",
      redimensionado: this.editor.elemento,
      vizinho: this.areaDeResultado,
      minimo: 60,
      minimoDoVizinho: 80,
    });
    this.divisorDoResultado.hidden = true;

    this.elemento.append(
      this.montarBarra(),
      this.editor.elemento,
      this.divisorDoResultado,
      this.montarAreaDeResultado(),
      this.montarAreaInferior(),
      this.montarRodape(),
    );
    this.elemento.addEventListener("keydown", this.aoPressionarAtalho, true);
    this.pararDeObservarBotoes = observarConfiguracoesDosBotoes(() =>
      this.atualizarBotoesRapidos(),
    );
    this.atualizarBotoesRapidos();
    this.atualizarBotoes();

    // A conexão do sublinhado do Ctrl abre junto com a aba, e não no primeiro
    // Ctrl: o login e a primeira execução do bloco eram quase todo o segundo
    // que o primeiro sublinhado levava (requisito 22). Falhar aqui não pesa: o
    // primeiro Ctrl tenta de novo, e o backend registra o motivo.
    if (contexto.perfil.tipoBanco !== "sqlserver") {
      void api.prepararNavegacao(contexto.sessaoId).catch(() => undefined);
    }
  }

  focar(): void {
    this.editor.focar();
    for (const { grid } of this.grids) grid.ajustar();
  }

  /**
   * Abre o destino de um Ctrl+clique ou do **Ir para definição**, um de cada
   * vez, com o editor mostrando que está carregando (requisito 22).
   *
   * Clicar de novo enquanto o primeiro não voltou não faz nada: no PRD, três
   * cliques em `TABELA_EXEMPLO` sem nada na tela viraram três perguntas em fila
   * — 21, 34 e 38 s —, e a estrutura da tabela ficou esperando atrás delas.
   */
  private abrirReferencia(referencia: string): void {
    if (!this.aoAbrirReferencia || this.navegando) return;
    this.navegando = true;
    this.editor.indicarCarregamento(true);
    void this.aoAbrirReferencia(referencia, this.bancoAtual).finally(() => {
      this.navegando = false;
      this.editor.indicarCarregamento(false);
    });
  }

  /** Texto do editor, para salvar a consulta em arquivo (requisito 12). */
  get texto(): string {
    return this.editor.texto;
  }

  /**
   * Arquivo a que a aba está amarrada, ou `null` enquanto ela nunca foi salva
   * nem veio de um. É o que decide se o Salvar pergunta o caminho.
   */
  get arquivo(): string | null {
    return this.arquivoAtual;
  }

  /** Amarra a aba a um arquivo, depois de salvar nele. */
  amarrarAoArquivo(caminho: string): void {
    this.arquivoAtual = caminho;
  }

  /** Fecha o cursor aberto e a sessão da aba; chamado quando a aba é fechada. */
  encerrar(): void {
    this.fecharCursores();
    // Fechar a aba encerra a sessão dela. O Oracle desfaz no logoff o que não
    // foi confirmado — como fechar não pergunta nada (requisito 12), um DML sem
    // commit é descartado, igual a fechar uma janela do SQL Developer.
    void api.desconectar(this.sessaoExecucao);
    this.pararDeObservarBotoes();
    void this.pararDeObservarCatalogo?.then((parar) => parar());
    // Fechar a aba não pode engolir o que foi digitado nos últimos segundos: a
    // cópia pendente vai agora, e o que já está gravado permanece na pasta de
    // recuperação até vencer (requisito 12).
    window.clearTimeout(this.temporizadorDaCopia);
    void this.copiar();
    this.editor.destruir();
  }

  // --- Cópia de recuperação (requisito 12) ---------------------------------

  /** Espera a digitação parar: uma gravação por pausa, não uma por tecla. */
  private agendarCopia(): void {
    if (!this.dadosDaAba) return;
    window.clearTimeout(this.temporizadorDaCopia);
    this.temporizadorDaCopia = window.setTimeout(() => void this.copiar(), ESPERA_DA_COPIA);
  }

  private async copiar(): Promise<void> {
    const alvo = this.dadosDaAba;
    if (!alvo) return;

    const texto = this.editor.texto;
    // Texto imenso reescrito a cada pausa é o único caso em que isto custaria
    // algo; e um documento inalterado não precisa ser regravado.
    if (texto === this.ultimoTextoCopiado || texto.length > MAXIMO_DA_COPIA) return;

    try {
      await api.gravarRecuperacao(alvo.id, texto, {
        titulo: alvo.titulo,
        conexao: this.contexto.perfil.nome,
        // Salvar em outro arquivo muda a origem: a tela de recuperação passa a
        // apontar para onde a consulta realmente está hoje.
        origem: this.arquivoAtual,
      });
      this.ultimoTextoCopiado = texto;
    } catch {
      // Falhar em copiar não pode atrapalhar quem está trabalhando. O backend
      // registra o motivo no log, e a próxima pausa tenta de novo.
    }
  }

  // --- Barra de ferramentas ------------------------------------------------

  private montarBarra(): HTMLElement {
    const barra = document.createElement("div");
    barra.className = "barra-consulta";

    const sqlServer = this.contexto.perfil.tipoBanco === "sqlserver";

    barra.append(
      this.botao("executar", "▶", t("executar"), () => void this.executar(false)),
      this.botao("script", "▶≡", t("executar como script"), () => void this.executar(true)),
      this.botao("parar", "■", t("parar a execucao"), () => void this.parar()),
      separador(),
      this.botao("commit", "✔", "Commit", () => void this.transacao(true)),
      this.botao("rollback", "↶", "Rollback", () => void this.transacao(false)),
      this.marcacao("Auto-commit", this.autoCommit, (ligado) => this.trocarAutoCommit(ligado)),
      separador(),
      // Compilar é PL/SQL (requisito 13): não existe equivalente no SQL Server,
      // e um botão que só sabe dar erro não deve ocupar a barra.
      ...(sqlServer ? [] : [this.botao("compilar", "⚙", t("compilar"), () => void this.compilar()), separador()]),
      // Saída DBMS também é do Oracle (requisito 8). No lugar dela, a aba de
      // SQL Server traz a lista de bancos — que é o que muda de aba para aba
      // num servidor com vários deles.
      sqlServer
        ? this.montarSeletorDeBanco()
        : this.marcacao(t("saida dbms"), false, (ligada) => this.trocarSaidaDbms(ligada)),
      separador(),
      this.montarBotoesRapidos(),
    );
    return barra;
  }

  private montarBotoesRapidos(): HTMLElement {
    const grupo = document.createElement("div");
    grupo.className = "botoes-rapidos";
    grupo.setAttribute("role", "toolbar");
    grupo.setAttribute("aria-label", t("textos rapidos"));
    this.grupoBotoesRapidos = grupo;
    return grupo;
  }

  private atualizarBotoesRapidos(): void {
    this.configuracoesDosBotoes = obterConfiguracoesDosBotoes();
    const grupo = this.grupoBotoesRapidos;
    if (!grupo) return;

    // O índice é o da lista **inteira**, e não o da lista visível: é ele que
    // `moverBotaoRapido`, o menu de contexto e os atalhos usam. Filtrar sem
    // guardá-lo faria a aba de uma conexão editar o botão de outra.
    const visiveis = this.configuracoesDosBotoes
      .map((configuracao, indice) => ({ configuracao, indice }))
      .filter(({ configuracao }) => botaoValeParaConexao(configuracao, this.contexto.perfil.id));

    const botoes = visiveis.map(({ configuracao, indice }) => {
      const botao = document.createElement("button");
      botao.className = "botao-rapido";
      botao.textContent = configuracao.rotulo;
      botao.draggable = true;
      botao.dataset.indice = String(indice);
      botao.addEventListener("click", () => this.inserirConteudoDoBotao(indice));
      botao.addEventListener("contextmenu", (evento) => {
        evento.preventDefault();
        evento.stopPropagation();
        void this.editarBotaoRapido(indice);
      });
      const situacao = configuracao.conteudo ? t("inserir no editor") : t("sem conteudo definido");
      const visivel = atalhoVisivel(configuracao.atalho);
      const atalho = visivel ? ` — ${visivel}` : "";
      botao.title = t("dica do botao rapido", { situacao, atalho });
      botao.setAttribute(
        "aria-label",
        `${configuracao.rotulo}: ${situacao}${visivel ? `. ${t("atalho")} ${visivel}` : ""}`,
      );
      botao.addEventListener("dragstart", (evento) => {
        evento.dataTransfer?.setData("text/x-cururu-botao", String(indice));
        if (evento.dataTransfer) evento.dataTransfer.effectAllowed = "move";
        botao.classList.add("botao-rapido--arrastando");
      });
      botao.addEventListener("dragend", () => botao.classList.remove("botao-rapido--arrastando"));
      botao.addEventListener("dragover", (evento) => {
        evento.preventDefault();
        if (evento.dataTransfer) evento.dataTransfer.dropEffect = "move";
      });
      botao.addEventListener("drop", (evento) => {
        evento.preventDefault();
        const origem = Number(evento.dataTransfer?.getData("text/x-cururu-botao"));
        if (Number.isInteger(origem)) moverBotaoRapido(origem, indice);
      });
      return botao;
    });

    const adicionar = document.createElement("button");
    adicionar.className = "botao-rapido botao-rapido--adicionar";
    adicionar.textContent = "+";
    adicionar.title = t("adicionar botao rapido");
    adicionar.setAttribute("aria-label", t("adicionar botao rapido"));
    adicionar.addEventListener("click", () => void this.editarBotaoRapido(null));
    grupo.replaceChildren(...botoes, adicionar);
  }

  /**
   * Abre a configuração de um botão — `null` cria um novo.
   *
   * A lista de conexões é lida aqui, e não lá dentro, para o diálogo continuar
   * sendo tela pura: quem sabe falar com o banco é o painel.
   */
  private async editarBotaoRapido(indice: number | null): Promise<void> {
    let conexoes: ConexaoParaFiltro[] = [];
    try {
      conexoes = (await api.listarPerfis()).map(({ id, nome, cor }) => ({ id, nome, cor }));
    } catch (erro) {
      // Sem a lista sobra só "Todas" no filtro; o resto do diálogo — rótulo,
      // atalho, conteúdo — continua editável, que é melhor do que não abrir.
      this.aoRelatar(api.mensagemDeErro(erro), true);
    }
    if (indice === null) abrirNovoBotaoRapido(conexoes);
    else abrirConfiguracaoDoBotao(indice, conexoes);
  }

  private inserirConteudoDoBotao(indice: number): void {
    const configuracao = this.configuracoesDosBotoes[indice];
    // O botão escondido nesta conexão também não responde ao atalho dele: a
    // tecla não pode fazer o que o botão que ela representa não faria.
    if (!configuracao || !botaoValeParaConexao(configuracao, this.contexto.perfil.id)) return;
    if (configuracao.naoInserirNaJanela) {
      // Executa o conteúdo direto, sem passar pelo editor: nem insere, nem
      // seleciona, nem deixa o texto aparecer na tela. Tem prioridade sobre
      // "executar automaticamente", que depende do texto estar selecionado.
      if (configuracao.conteudo.trim()) void this.executar(false, configuracao.conteudo);
    } else if (configuracao.executarAutomaticamente) {
      this.editor.inserirESelecionar(configuracao.conteudo);
      // Conteúdo vazio não pode cair na regra de executar o documento inteiro.
      if (this.editor.selecao) void this.executar(false);
    } else {
      this.editor.inserir(configuracao.conteudo);
    }
  }

  private readonly aoPressionarAtalho = (evento: KeyboardEvent): void => {
    // Navegação de célula é comportamento nativo do grid, não atalho de botão
    // rápido. Além de não aparecer na tela "Teclas de Atalho", ela não pode ser
    // roubada por uma configuração antiga que use uma dessas teclas.
    if (
      (evento.target as Element).closest(".grid") &&
      ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"].includes(
        evento.key,
      )
    ) {
      return;
    }

    // Os atalhos gerais e do editor têm prioridade sobre os botões rápidos.
    if (eventoEhAtalhoConfigurado(evento)) return;

    const atalho = atalhoDoEvento(evento);
    if (!atalho) return;
    // Só entre os botões desta conexão: engolir a tecla de um botão que a aba
    // nem mostra tiraria a combinação de quem mais pudesse querê-la.
    const indice = this.configuracoesDosBotoes.findIndex(
      (item) =>
        item.atalho === atalho && botaoValeParaConexao(item, this.contexto.perfil.id),
    );
    if (indice < 0) return;
    evento.preventDefault();
    evento.stopPropagation();
    if (!evento.repeat) this.inserirConteudoDoBotao(indice);
  };

  private botao(
    chave: string,
    simbolo: string,
    dica: string,
    acao: () => void,
  ): HTMLButtonElement {
    const botao = document.createElement("button");
    botao.className = "botao-barra";
    botao.textContent = simbolo;
    botao.title = dica;
    botao.setAttribute("aria-label", dica);
    botao.addEventListener("click", acao);
    this.botoes.set(chave, botao);
    return botao;
  }

  private marcacao(
    texto: string,
    inicial: boolean,
    aoTrocar: (ligado: boolean) => void,
  ): HTMLElement {
    const rotulo = document.createElement("label");
    rotulo.className = "caixa-marcacao";
    const caixa = document.createElement("input");
    caixa.type = "checkbox";
    caixa.checked = inicial;
    caixa.addEventListener("change", () => aoTrocar(caixa.checked));
    rotulo.append(caixa, document.createTextNode(texto));
    return rotulo;
  }

  /**
   * A lista de bancos desta aba, no lugar da Saída DBMS.
   *
   * Os do sistema **entram** aqui, ao contrário da árvore: consultar o catálogo
   * do `master` é trabalho legítimo de quem administra, e esconder o banco da
   * lista impediria justamente isso.
   *
   * A troca vale só para esta aba, como o `USE` do SSMS vale só para a janela em
   * que foi dado — cada aba tem a sua sessão (requisito 7).
   */
  private montarSeletorDeBanco(): HTMLElement {
    const lista = document.createElement("select");
    lista.className = "banco-em-uso";
    lista.title = t("banco em uso");
    lista.addEventListener("change", () => void this.trocarBanco(lista));
    this.seletorDeBanco = lista;
    void this.preencherBancos(lista);
    return lista;
  }

  /**
   * Enche a lista sem segurar a montagem da barra.
   *
   * Falhar aqui deixa a lista com o banco atual e mais nada: a aba continua
   * utilizável, e o backend já registrou o motivo no log.
   */
  private async preencherBancos(lista: HTMLSelectElement): Promise<void> {
    try {
      const [bancos, atual] = await Promise.all([
        api.listarBancos(this.sessaoExecucao, true),
        api.bancoDaSessao(this.sessaoExecucao),
      ]);
      this.bancoAtual = atual;

      // O banco corrente entra na lista mesmo se não vier dela: a sessão pode
      // estar num banco que saiu do ar entre a abertura e agora, e a tela não
      // pode mentir sobre onde está executando.
      const nomes = atual && !bancos.includes(atual) ? [atual, ...bancos] : bancos;
      lista.replaceChildren();
      for (const banco of nomes) {
        const opcao = document.createElement("option");
        opcao.value = banco;
        opcao.textContent = banco;
        lista.append(opcao);
      }
      if (atual) lista.value = atual;
    } catch {
      // O backend registra o motivo; a barra fica com a lista vazia.
    }
  }

  private async trocarBanco(lista: HTMLSelectElement): Promise<void> {
    const banco = lista.value;
    if (!banco || banco === this.bancoAtual) return;

    lista.disabled = true;
    try {
      await api.usarBanco(this.sessaoExecucao, banco, this.contexto.sessaoId);
      this.bancoAtual = banco;
      // O que foi lido do dicionário era do banco anterior, e as tabelas do
      // banco novo são outras: oferecê-las aqui seria oferecer o que não
      // existe.
      this.editor.esquecerColunas();
      this.editor.esquecerObjetos();
      this.aoRelatar(t("banco trocado", { banco }));
    } catch (erro) {
      // Sem permissão no banco escolhido, a sessão continua onde estava — e a
      // lista precisa voltar a mostrar isso, senão a tela passa a mentir.
      lista.value = this.bancoAtual ?? "";
      this.aoRelatar(api.mensagemDeErro(erro), true);
    } finally {
      lista.disabled = false;
    }
  }

  private atualizarBotoes(): void {
    for (const [chave, botao] of this.botoes) {
      botao.disabled = chave === "parar" ? !this.executando : this.executando;
    }
    // O ✔ acende enquanto há o que confirmar: é o único aviso permanente de que
    // o que rodou ainda não vale no banco.
    const commit = this.botoes.get("commit");
    if (commit) {
      commit.classList.toggle("botao-barra--pendente", this.transacaoPendente);
      const dica = this.transacaoPendente ? t("transacao aberta") : "Commit";
      commit.title = dica;
      commit.setAttribute("aria-label", dica);
    }
    // Trocar de banco no meio de uma execução mudaria o chão sob os pés da
    // consulta que está correndo.
    if (this.seletorDeBanco) this.seletorDeBanco.disabled = this.executando;
  }

  // --- Áreas ---------------------------------------------------------------

  private montarAreaDeResultado(): HTMLElement {
    this.areaDeResultado.className = "area-resultado";
    this.areaDeResultado.hidden = true;
    this.saidaDeScript.className = "saida-script";
    this.saidaDeScript.hidden = true;
    this.areaDeResultado.append(this.saidaDeScript);
    return this.areaDeResultado;
  }

  /** Painéis inferiores, minimizados por padrão (requisito 8). */
  private montarAreaInferior(): HTMLElement {
    this.areaInferior.className = "area-inferior";

    const conteudo = document.createElement("div");
    conteudo.className = "area-inferior__conteudo";
    this.painelMensagens.className = "painel-inferior";
    this.painelHistorico.className = "painel-inferior";
    this.painelDbms.className = "painel-inferior";
    conteudo.append(this.montarDbms(), this.montarMensagens(), this.painelHistorico);

    const faixa = document.createElement("div");
    faixa.className = "area-inferior__faixa";
    // O histórico continua sendo preenchido internamente, mas seu botão fica
    // oculto por enquanto: pode voltar sem perder os comandos da sessão.
    for (const [chave, rotulo] of [
      ["mensagens", t("mensagens e log")],
      ["dbms", t("saida dbms")],
    ] as const) {
      const botao = document.createElement("button");
      botao.className = "aba-inferior";
      botao.textContent = rotulo;
      botao.addEventListener("click", () => this.alternarAbaInferior(chave));
      botao.dataset.aba = chave;
      faixa.append(botao);
    }

    this.areaInferior.append(conteudo, faixa);
    this.alternarAbaInferior(null);
    return this.areaInferior;
  }

  /** Mensagens e log (requisito 18), com a barra do Limpar como a Saída DBMS. */
  private montarMensagens(): HTMLElement {
    const cabecalho = document.createElement("div");
    cabecalho.className = "painel-inferior__barra";
    const limpar = document.createElement("button");
    limpar.className = "botao-discreto";
    limpar.textContent = t("limpar");
    limpar.title = t("limpar mensagens");
    limpar.setAttribute("aria-label", t("limpar mensagens"));
    limpar.addEventListener("click", () => this.limparMensagens());
    cabecalho.append(limpar);

    this.listaDeMensagens.className = "painel-inferior__lista";
    this.painelMensagens.append(cabecalho, this.listaDeMensagens);
    return this.painelMensagens;
  }

  private montarDbms(): HTMLElement {
    const cabecalho = document.createElement("div");
    cabecalho.className = "painel-inferior__barra";
    const rotulo = document.createElement("label");
    rotulo.textContent = t("tamanho do buffer");
    const buffer = document.createElement("input");
    buffer.type = "number";
    buffer.value = String(BUFFER_DBMS_PADRAO);
    buffer.min = "2000";
    buffer.style.width = "90px";
    buffer.dataset.papel = "buffer";
    const limpar = document.createElement("button");
    limpar.className = "botao-discreto";
    limpar.textContent = t("limpar");
    limpar.addEventListener("click", () => this.limparDbms());

    cabecalho.append(rotulo, buffer, limpar);
    const texto = document.createElement("pre");
    texto.className = "painel-inferior__texto";
    texto.dataset.papel = "texto";

    this.painelDbms.append(cabecalho, texto);
    return this.painelDbms;
  }

  private alternarAbaInferior(chave: string | null): void {
    this.abaInferiorAberta = this.abaInferiorAberta === chave ? null : chave;
    const aberta = this.abaInferiorAberta;

    this.areaInferior.classList.toggle("area-inferior--aberta", aberta !== null);
    this.painelMensagens.hidden = aberta !== "mensagens";
    this.painelHistorico.hidden = aberta !== "historico";
    this.painelDbms.hidden = aberta !== "dbms";

    for (const botao of this.areaInferior.querySelectorAll<HTMLElement>("[data-aba]")) {
      botao.classList.toggle("aba-inferior--ativa", botao.dataset.aba === aberta);
    }
  }

  /**
   * Abre a aba pedida, sem alternar. Alternar é o gesto do clique na faixa;
   * quem mostra um erro precisa do painel aberto — com a alternância, o
   * segundo erro seguido fechava o painel que o primeiro tinha aberto, e a
   * mensagem sumia justamente quando havia o que ler.
   */
  private abrirAbaInferior(chave: string): void {
    if (this.abaInferiorAberta !== chave) this.alternarAbaInferior(chave);
  }

  private montarRodape(): HTMLElement {
    this.rodape.className = "rodape-consulta";

    this.informacaoRodape.className = "rodape-consulta__informacao";

    this.indicadorExecucao.className = "rodape-consulta__execucao";
    this.indicadorExecucao.hidden = true;
    this.indicadorExecucao.setAttribute("role", "status");
    this.indicadorExecucao.setAttribute("aria-live", "polite");
    const progresso = document.createElement("progress");
    progresso.className = "rodape-consulta__progresso";
    const textoExecucao = document.createElement("span");
    textoExecucao.textContent = t("executando");
    this.indicadorExecucao.append(progresso, textoExecucao);

    const rotuloFetch = document.createElement("label");
    rotuloFetch.textContent = t("linhas por vez");
    this.campoFetch.type = "number";
    this.campoFetch.value = String(FETCH_PADRAO);
    this.campoFetch.min = "1";
    this.campoFetch.max = "10000";
    this.campoFetch.title =
      t("linhas por vez explicacao");
    rotuloFetch.append(this.campoFetch);

    this.rodape.append(this.informacaoRodape, this.indicadorExecucao, rotuloFetch);
    return this.rodape;
  }

  private escreverRodape(texto: string): void {
    this.informacaoRodape.textContent = texto;
  }

  private definirExecutando(executando: boolean): void {
    this.executando = executando;
    this.informacaoRodape.hidden = executando;
    this.indicadorExecucao.hidden = !executando;
    // O mesmo estado acende o símbolo na aba, para a execução aparecer também
    // quando a aba está de fundo (requisito 17).
    this.aoMudarExecucao?.(executando);
    this.atualizarBotoes();
  }

  private exibirAreaDeResultado(): void {
    if (this.tamanhoDoEditor !== null) {
      this.editor.elemento.style.flex = this.tamanhoDoEditor;
      this.tamanhoDoEditor = null;
    }
    this.divisorDoResultado.hidden = false;
    this.areaDeResultado.hidden = false;
  }

  /**
   * Tira de vista o resultado da execução anterior (requisito 6).
   *
   * Depois de uma execução que falhou, o grid na tela era o da execução de
   * antes: não respondia mais pelo que está no editor, e disputava o olho com
   * o erro, espremido no painel de mensagens. Os cursores dele são largados
   * como seriam pelo próximo resultado.
   */
  private ocultarAreaDeResultado(): void {
    this.fecharCursores();
    this.saidaDeScript.hidden = true;
    this.areaDeResultado.replaceChildren(this.saidaDeScript);
    this.areaDeResultado.hidden = true;
    this.divisorDoResultado.hidden = true;
    // O divisor fixa a altura do editor no primeiro arrasto; com o resultado
    // fora, essa altura deixaria um vão vazio onde ele estava.
    if (this.editor.elemento.style.flex) {
      this.tamanhoDoEditor = this.editor.elemento.style.flex;
      this.editor.elemento.style.flex = "";
    }
  }

  private get tamanhoDoFetch(): number {
    const valor = Number(this.campoFetch.value);
    return Number.isFinite(valor) && valor > 0 ? Math.min(valor, 10000) : FETCH_PADRAO;
  }

  // --- Execução (requisitos 5, 14 e 15) ------------------------------------

  /**
   * `textoExplicito` é o caminho da execução silenciosa dos botões rápidos
   * (item "Não inserir SELECT na janela de consulta"): substitui a leitura do
   * editor sem tocar nele. Omitido, vale a regra normal do requisito 5 — a
   * seleção, ou a tela inteira sem seleção.
   */
  private async executar(modoScript: boolean, textoExplicito?: string): Promise<void> {
    if (this.executando) return;

    const texto = (textoExplicito ?? this.editor.textoParaExecutar).trim();
    if (!texto) {
      this.aoRelatar(t("nada para executar"), true);
      return;
    }
    // As linhas que o backend devolve são contadas no texto executado. O dos
    // botões rápidos não está no editor, e para ele não há onde saltar.
    const linhaBase = textoExplicito === undefined ? this.editor.linhaDoTextoParaExecutar : null;

    let binds: [string, string][] = [];
    try {
      const nomes = await api.analisarBinds(texto, modoScript && this.contexto.perfil.tipoBanco === "oracle");
      if (nomes.length > 0) {
        const valores = await pedirValoresDosBinds(nomes);
        if (valores === null) return;
        binds = valores;
      }
    } catch (erro) {
      this.aoRelatar(api.mensagemDeErro(erro), true);
      return;
    }

    // Recomeça a barra de status geral a cada execução: o caminho de sucesso só
    // atualiza o rodapé da aba (requisito 17), então sem isto o erro `ORA-` de
    // uma falha anterior — escrito por `mostrarErro` — ficaria na barra para
    // sempre, mesmo depois de uma consulta bem-sucedida.
    this.aoRelatar("");
    this.definirExecutando(true);

    const comeco = performance.now();
    try {
      const resultados = await api.executarSql(
        this.sessaoExecucao,
        texto,
        modoScript,
        binds,
        this.tamanhoDoFetch,
      );
      this.mostrarResultados(resultados, modoScript, linhaBase);
    } catch (erro) {
      // O lote pode ter mandado grids e falhado depois — uma procedure que
      // mostra o que carregou e estoura no fim. O que já veio é exibido antes
      // do erro, que fica por último para ser a última palavra do rodapé.
      // Sem nada desta execução para mostrar, o resultado da anterior sai de
      // vista: ele já não responde pelo editor, e quem precisa ser lido agora
      // é o erro (requisito 6).
      const parciais = (erro as api.ErroBackend | null)?.resultados;
      if (parciais && parciais.length > 0) this.mostrarResultados(parciais, modoScript, linhaBase);
      else this.ocultarAreaDeResultado();
      this.mostrarErro(erro, linhaBase);
      this.escreverRodape(`Falhou em ${formatarDuracao(performance.now() - comeco)}`);
    } finally {
      this.definirExecutando(false);
      void this.conferirBancoEmUso();
    }
  }

  /**
   * Reacerta a lista de bancos depois de uma execução.
   *
   * Um `use Vendas` digitado no editor troca o banco da aba de verdade — é o
   * mesmo comando que o SSMS manda —, e a lista da barra não pode continuar
   * mostrando o banco de antes. A pergunta é ao backend, que já sabe a resposta
   * sem ir ao servidor.
   */
  private async conferirBancoEmUso(): Promise<void> {
    const lista = this.seletorDeBanco;
    if (!lista) return;
    try {
      const agora = await api.bancoDaSessao(this.sessaoExecucao);
      if (!agora || agora === this.bancoAtual) return;
      this.bancoAtual = agora;
      this.editor.esquecerColunas();
      this.editor.esquecerObjetos();
      if (!Array.from(lista.options).some((opcao) => opcao.value === agora)) {
        const opcao = document.createElement("option");
        opcao.value = agora;
        opcao.textContent = agora;
        lista.append(opcao);
      }
      lista.value = agora;
    } catch {
      // O backend registra o motivo; a barra fica como está.
    }
  }

  private async parar(): Promise<void> {
    try {
      await api.cancelarExecucao(this.sessaoExecucao);
      this.aoRelatar(t("interrupcao enviada"));
    } catch (erro) {
      this.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private mostrarResultados(
    resultados: api.ResultadoDaExecucao[],
    modoScript: boolean,
    linhaBase: number | null,
  ): void {
    // A consulta rodou, mas numa conexão nova: a anterior tinha caído e o
    // backend reabriu a sessão. O aviso não é decoração — a sessão nova é outra
    // sessão do banco, e uma transação aberta sem commit ficou para trás.
    if (resultados.some((resultado) => resultado.reconectado)) {
      this.avisarReconexao(t("conexao restabelecida"));
    }

    for (const resultado of resultados) {
      this.registrarNoHistorico(resultado);
      const erros = resultado.errosDeCompilacao;
      this.acrescentarMensagem(resultado.mensagem, erros.some((erro) => !erro.aviso));
      this.listarErrosDeCompilacao(erros, linhaBase);
      if (resultado.saidaDbms.length > 0) this.acrescentarDbms(resultado.saidaDbms);
    }
    // Para o Oracle, o `CREATE` que compila com erro deu certo — e o script
    // segue. Mas o objeto ficou inválido no banco, e isso não pode passar como
    // mais uma linha no painel.
    const errosGraves = resultados
      .flatMap((resultado) => resultado.errosDeCompilacao)
      .filter((erro) => !erro.aviso).length;
    if (errosGraves > 0) {
      this.abrirAbaInferior("mensagens");
      this.aoRelatar(t("erros de compilacao", { n: errosGraves }), true);
    }

    // Quem diz o que ficou por confirmar é o backend, comando a comando, e vale
    // o último que mexeu na transação: no Oracle um DDL confirma sozinho o que
    // estava pendente, e um `commit` digitado vale o mesmo que o botão. O
    // caminho do erro também passa por aqui, com os resultados parciais do
    // lote: o que rodou antes da falha está tão pendente quanto o de uma
    // execução inteira.
    this.definirTransacaoPendente(
      resultados.reduce<boolean>((pendente, resultado) => {
        if (resultado.transacao === "aberta") return true;
        if (resultado.transacao === "encerrada") return false;
        return pendente;
      }, this.transacaoPendente),
    );
    // No Oracle é o DDL que volta `encerrada`: a tabela criada agora precisa
    // ganhar o sublinhado do Ctrl, e o que se sabia dela era "não existe". O
    // `commit` digitado cai aqui junto, e custa só uma pergunta a mais.
    if (resultados.some((resultado) => resultado.transacao === "encerrada")) {
      this.editor.esquecerReferencias();
    }

    if (modoScript) {
      this.mostrarSaidaDeScript(resultados, linhaBase);
      return;
    }

    // Requisito 6: um grid por conjunto de resultados, na ordem em que o banco
    // os devolveu.
    const consultas = resultados.filter((r) => r.consulta !== null);
    if (consultas.length > 0) {
      this.mostrarGrids(consultas);
    } else {
      this.mostrarSaidaDeScript(resultados, linhaBase);
      // DDL e `commit` não afetam linhas: o rodapé fica com a contagem de
      // comandos que a saída escreveu, e não com um "0 linha(s) afetada(s)"
      // que não diz nada.
      const afetadas = resultados.filter((r) => r.linhasAfetadas !== null);
      if (afetadas.length > 0) {
        const total = afetadas.reduce((soma, r) => soma + (r.linhasAfetadas ?? 0), 0);
        const tempo = resultados.reduce((soma, r) => soma + r.duracaoMs, 0);
        this.escreverRodape(`${total} linha(s) afetada(s) — ${formatarDuracao(tempo)}`);
      }
    }
  }

  /**
   * Monta um grid por conjunto de resultados, um embaixo do outro.
   *
   * Um lote do SQL Server devolve vários conjuntos — uma procedure de
   * depuração devolve dezenas —, e uma aba do Oracle com várias consultas
   * soltas devolve um por comando. Mostrar só o último escondia todo o resto,
   * que já tinha vindo do banco: esta é a mesma pilha que o SSMS mostra.
   *
   * Com um conjunto só o grid ocupa a área inteira, como sempre ocupou; a
   * pilha só aparece quando há mais de um, e aí cada grid fica na sua altura,
   * limitada, com a rolagem da pilha percorrendo todos.
   */
  private mostrarGrids(resultados: api.ResultadoDaExecucao[]): void {
    this.fecharCursores();

    const abertos = resultados.map((resultado) => {
      const consulta = resultado.consulta!;
      const grid: Grid = new Grid(consulta.colunas, {
        aoPedirMais: () => void this.trazerMaisLinhas(grid),
        aoAbrirValor: (coluna, valor) => this.abrirValor(coluna, valor),
      });
      grid.acrescentar(consulta.linhas);
      grid.definirTemMais(consulta.temMais);
      // Copiar, exportar e o menu de contexto agem sobre o grid em que se
      // mexeu por último; sem isto agiriam sempre sobre o primeiro da pilha.
      grid.elemento.addEventListener("pointerdown", () => {
        this.grid = grid;
      });
      grid.elemento.addEventListener("contextmenu", (evento) => {
        this.grid = grid;
        this.menuDoGrid(evento);
      });
      return {
        grid,
        cursorId: consulta.temMais ? consulta.cursorId : null,
        comando: resultado.comando,
      };
    });

    this.grids = abertos;
    this.grid = abertos[0]?.grid ?? null;
    this.exibirAreaDeResultado();
    this.saidaDeScript.hidden = true;

    if (abertos.length === 1) {
      this.areaDeResultado.replaceChildren(abertos[0].grid.elemento, this.saidaDeScript);
    } else {
      const pilha = document.createElement("div");
      pilha.className = "pilha-de-grids";
      for (const { grid } of abertos) {
        pilha.append(grid.elemento, this.criarPuxadorDeAltura(grid));
      }
      this.areaDeResultado.replaceChildren(pilha, this.saidaDeScript);
      // A altura vem depois de o grid estar na tela: ela mede o cabeçalho e a
      // barra de rolagem deitada.
      for (const { grid } of abertos) {
        grid.elemento.style.height = `${grid.alturaPara(LINHAS_VISIVEIS_NA_PILHA)}px`;
      }
    }
    for (const { grid } of abertos) grid.ajustar();

    if (resultados.length === 1) {
      const consulta = resultados[0].consulta!;
      this.escreverRodape(
        `${consulta.linhas.length} linha(s)${consulta.temMais ? "+" : ""} — ${formatarDuracao(resultados[0].duracaoMs)}`,
      );
      return;
    }
    const linhas = abertos.reduce((soma, { grid }) => soma + grid.totalDeLinhas, 0);
    const tempo = resultados.reduce((soma, r) => soma + r.duracaoMs, 0);
    this.escreverRodape(
      `${abertos.length} conjunto(s) — ${linhas} linha(s) — ${formatarDuracao(tempo)}`,
    );
  }

  /**
   * Faixa arrastável embaixo de cada grid da pilha, para o conjunto receber a
   * altura que o usuário quiser.
   *
   * Dez linhas é um começo, não uma resposta: um conjunto de duzentas linhas
   * merece mais espaço que o vizinho de uma, e um conjunto largo fica espremido
   * contra a própria barra de rolagem. Como o SSMS, cada conjunto passa a ser
   * regulável.
   *
   * Diferente do divisor dos painéis, aqui nada sai do vizinho: o grid cresce,
   * a pilha cresce junto e rola. Por isso o componente não serve e o puxador
   * mora aqui, onde a pilha vive.
   */
  private criarPuxadorDeAltura(grid: Grid): HTMLElement {
    const puxador = document.createElement("div");
    puxador.className = "puxador-de-altura";
    puxador.setAttribute("role", "separator");
    puxador.setAttribute("aria-orientation", "horizontal");
    puxador.title = t("arraste para mudar a altura");
    puxador.tabIndex = 0;

    /**
     * Teto do arrasto: a área de resultado inteira.
     *
     * É o que segura a memória, que é o foco do projeto: a virtualização
     * desenha o que couber na caixa, e um grid esticado para além da janela
     * desenharia linhas que ninguém chega a ver.
     *
     * O teto não é o conteúdo. Parar no conteúdo deixaria o puxador de um
     * conjunto de uma linha sem resposta nenhuma ao arrasto — e puxador que
     * não anda parece defeito, mesmo quando não há mais o que mostrar.
     */
    const teto = (): number => Math.max(this.areaDeResultado.clientHeight, grid.alturaPara(1));

    /** Altura que mostra o conjunto inteiro, sem passar do teto. */
    const cheio = (): number => Math.min(teto(), grid.alturaPara(grid.totalDeLinhas));

    const aplicar = (pedido: number): void => {
      const minimo = grid.alturaPara(1);
      const alvo = Math.round(Math.min(Math.max(pedido, minimo), Math.max(minimo, teto())));
      grid.elemento.style.height = `${alvo}px`;
      // A caixa mudou de tamanho: sem redesenhar, o espaço que acabou de
      // aparecer fica em branco até alguém rolar o grid.
      grid.ajustar();
    };

    let inicio = 0;
    let alturaInicial = 0;
    let arrastando = false;

    puxador.addEventListener("pointerdown", (evento) => {
      arrastando = true;
      inicio = evento.clientY;
      alturaInicial = grid.elemento.offsetHeight;
      // Sem a captura o ponteiro entra no grid de baixo e os eventos param de
      // chegar aqui no meio do arrasto.
      puxador.setPointerCapture(evento.pointerId);
      puxador.classList.add("puxador-de-altura--ativo");
      // Sem isto o arrasto sai selecionando as células por onde passa.
      evento.preventDefault();
    });

    puxador.addEventListener("pointermove", (evento) => {
      if (!arrastando) return;
      aplicar(alturaInicial + (evento.clientY - inicio));
    });

    const terminar = (evento: PointerEvent): void => {
      if (!arrastando) return;
      arrastando = false;
      puxador.releasePointerCapture(evento.pointerId);
      puxador.classList.remove("puxador-de-altura--ativo");
    };
    puxador.addEventListener("pointerup", terminar);
    puxador.addEventListener("pointercancel", terminar);

    // Duplo clique alterna entre mostrar o conjunto inteiro e as dez linhas do
    // começo: é um clique para ver tudo, e outro para devolver o espaço aos
    // outros conjuntos.
    puxador.addEventListener("dblclick", () => {
      const inteiro = cheio();
      const padrao = grid.alturaPara(LINHAS_VISIVEIS_NA_PILHA);
      aplicar(Math.abs(grid.elemento.offsetHeight - inteiro) < 2 ? padrao : inteiro);
    });

    // Teclado, para quem não usa mouse: cada seta vale uma linha.
    puxador.addEventListener("keydown", (evento) => {
      const passo =
        evento.key === "ArrowUp" ? -ALTURA_DA_LINHA : evento.key === "ArrowDown" ? ALTURA_DA_LINHA : 0;
      if (passo === 0) return;
      evento.preventDefault();
      aplicar(grid.elemento.offsetHeight + passo);
    });

    return puxador;
  }

  /**
   * Larga os cursores da execução anterior.
   *
   * Cada conjunto pode ter deixado o seu: no Oracle todos podem; no SQL Server
   * só o primeiro, porque alcançar o segundo obriga a ler o primeiro até o fim
   * (requisito 10).
   */
  private fecharCursores(): void {
    for (const { cursorId } of this.grids) {
      if (cursorId) void api.fecharCursor(cursorId);
    }
    this.grids = [];
    this.grid = null;
  }

  private async trazerMaisLinhas(grid: Grid): Promise<void> {
    const aberto = this.grids.find((candidato) => candidato.grid === grid);
    if (!aberto?.cursorId) return;
    try {
      const bloco = await api.buscarMaisLinhas(aberto.cursorId, this.tamanhoDoFetch);
      grid.acrescentar(bloco.linhas);
      grid.definirTemMais(bloco.temMais);
      if (!bloco.temMais) aberto.cursorId = null;
      this.escreverRodape(`${grid.totalDeLinhas} linha(s)${bloco.temMais ? "+" : " — fim"}`);
    } catch (erro) {
      this.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private mostrarSaidaDeScript(
    resultados: api.ResultadoDaExecucao[],
    linhaBase: number | null,
  ): void {
    const partes: string[] = [];
    for (const resultado of resultados) {
      partes.push(`> ${primeiraLinha(resultado.comando)}`);
      if (resultado.consulta) {
        partes.push(formatarComoTexto(resultado.consulta));
      }
      partes.push(resultado.mensagem);
      for (const erro of resultado.errosDeCompilacao) {
        partes.push(`  ${this.textoDoErroDeCompilacao(erro, linhaBase)}`);
      }
      partes.push("");
    }
    this.saidaDeScript.textContent = partes.join("\n");
    this.exibirAreaDeResultado();
    this.saidaDeScript.hidden = false;
    this.fecharCursores();
    this.areaDeResultado.replaceChildren(this.saidaDeScript);
    const tempo = resultados.reduce((soma, r) => soma + r.duracaoMs, 0);
    this.escreverRodape(`${resultados.length} comando(s) — ${formatarDuracao(tempo)}`);
  }

  /**
   * `linhaBase` é a linha do editor em que o texto executado começa; sem ela —
   * texto que não está no editor —, não há para onde saltar.
   */
  private mostrarErro(erro: unknown, linhaBase: number | null): void {
    this.acrescentarMensagem(api.mensagemDeErro(erro), true);
    this.abrirAbaInferior("mensagens");
    this.relatarFalha(erro, true);

    // Requisito 18: saltar até a linha do comando que falhou. O backend a conta
    // no texto executado — num script, é a do comando que parou —, e a
    // seleção pode não começar na primeira linha do editor.
    if (linhaBase === null) return;
    const backend = erro as api.ErroBackend | null;
    if (typeof backend?.linha === "number") {
      this.editor.irParaLinha(linhaBase + backend.linha - 1);
      return;
    }
    if (typeof backend?.posicao === "number") {
      this.editor.irParaLinha(this.editor.linhaDaPosicao(backend.posicao));
    }
  }

  // --- Transação (requisito 7) ---------------------------------------------

  private async transacao(confirmar: boolean): Promise<void> {
    try {
      if (confirmar) await api.confirmarTransacao(this.sessaoExecucao);
      else await api.desfazerTransacao(this.sessaoExecucao);
      const texto = confirmar ? t("commit efetuado") : t("rollback efetuado");
      this.definirTransacaoPendente(false);
      this.acrescentarMensagem(texto);
      if (!this.saidaDeScript.hidden) {
        const separador = this.saidaDeScript.textContent ? "\n" : "";
        this.saidaDeScript.textContent += `${separador}${texto}`;
        this.saidaDeScript.scrollTop = this.saidaDeScript.scrollHeight;
      }
      this.aoRelatar(texto);
    } catch (erro) {
      this.relatarFalha(erro);
    }
  }

  private trocarAutoCommit(ligado: boolean): void {
    void api
      .definirAutoCommit(this.sessaoExecucao, ligado)
      .then(() => {
        // Ligar o auto-commit confirma o que estava pendente (é o que o backend
        // faz ao trocar), então não sobra o que avisar.
        this.autoCommit = ligado;
        if (ligado) this.definirTransacaoPendente(false);
        this.aoRelatar(ligado ? t("auto commit ligado") : t("auto commit desligado"));
      })
      .catch((erro) => this.aoRelatar(api.mensagemDeErro(erro), true));
  }

  private definirTransacaoPendente(pendente: boolean): void {
    if (this.transacaoPendente === pendente) return;
    this.transacaoPendente = pendente;
    this.atualizarBotoes();
  }

  // --- Saída DBMS (requisito 8) --------------------------------------------

  private trocarSaidaDbms(ligada: boolean): void {
    const buffer = this.painelDbms.querySelector<HTMLInputElement>('[data-papel="buffer"]');
    const tamanho = Number(buffer?.value) || BUFFER_DBMS_PADRAO;
    void api
      .definirSaidaDbms(this.sessaoExecucao, ligada, tamanho)
      .then(() => {
        if (ligada) this.abrirAbaInferior("dbms");
        this.aoRelatar(ligada ? "Saída DBMS ligada." : "Saída DBMS desligada.");
      })
      .catch((erro) => this.relatarFalha(erro));
  }

  private acrescentarDbms(linhas: string[]): void {
    const alvo = this.painelDbms.querySelector<HTMLElement>('[data-papel="texto"]');
    if (!alvo) return;
    alvo.textContent = `${alvo.textContent ?? ""}${linhas.join("\n")}\n`;
    alvo.scrollTop = alvo.scrollHeight;
  }

  private limparDbms(): void {
    const alvo = this.painelDbms.querySelector<HTMLElement>('[data-papel="texto"]');
    if (alvo) alvo.textContent = "";
  }

  /**
   * Esvazia o painel de mensagens.
   *
   * Só a tela: o log em disco não é tocado, e é dele que sai o diagnóstico de
   * um defeito. Aqui se limpa o que atrapalha ler o erro da execução seguinte.
   */
  private limparMensagens(): void {
    this.listaDeMensagens.replaceChildren();
  }

  // --- Mensagens e histórico (requisitos 18 e 19) --------------------------

  /**
   * Relata um erro vindo do banco, acrescentando o aviso de reconexão quando a
   * sessão precisou ser reaberta para tratá-lo.
   *
   * Sem o aviso, o `ORA-03113` sozinho continuaria dando a impressão de que a
   * aba morreu junto com a conexão — que é o que o usuário aprendeu a esperar.
   *
   * `jaNoPainel` diz que quem chamou já escreveu o erro no painel de mensagens.
   * Quando não escreveu e houve reconexão, o erro vai junto: o aviso sozinho no
   * painel não diria o que falhou, e a barra de status é apagada pela mensagem
   * seguinte.
   */
  private relatarFalha(erro: unknown, jaNoPainel = false): void {
    const mensagem = api.mensagemDeErro(erro);
    this.aoRelatar(mensagem, true);
    if (!(erro as api.ErroBackend | null)?.reconectado) return;
    if (!jaNoPainel) this.acrescentarMensagem(mensagem, true);
    this.avisarReconexao(t("conexao restabelecida execute outra vez"));
  }

  /**
   * Conta que a sessão desta aba foi reaberta.
   *
   * Vai para o painel de mensagens e para a barra de status, porque as duas
   * coisas que o usuário precisa saber são diferentes: que não é preciso fechar
   * a aba, e que a sessão de antes — com a transação aberta — não existe mais.
   */
  private avisarReconexao(texto: string): void {
    this.acrescentarMensagem(texto, true);
    this.abrirAbaInferior("mensagens");
    this.aoRelatar(texto, true);
  }

  private acrescentarMensagem(texto: string, erro = false): void {
    const item = document.createElement("div");
    item.className = erro ? "mensagem mensagem--erro" : "mensagem";
    item.textContent = `${new Date().toLocaleTimeString("pt-BR")}  ${texto}`;
    this.listaDeMensagens.append(item);
    this.listaDeMensagens.scrollTop = this.listaDeMensagens.scrollHeight;
  }

  private registrarNoHistorico(resultado: api.ResultadoDaExecucao): void {
    const item = document.createElement("div");
    item.className = "historico__item";
    item.title = resultado.comando;
    item.textContent = `${new Date().toLocaleTimeString("pt-BR")}  ${primeiraLinha(resultado.comando)}`;
    // Duplo clique devolve o comando ao editor, como no SQL Developer.
    item.addEventListener("dblclick", () => this.editor.inserir(resultado.comando));
    this.painelHistorico.prepend(item);
  }

  // --- Compilar PL/SQL (requisito 13) --------------------------------------

  private async compilar(): Promise<void> {
    // O botão sai da barra no SQL Server, mas o atalho (Ctrl+Shift+C) continua
    // ligado ao editor. Sem esta guarda ele iria ao backend só para voltar com
    // um erro que não explica por que a tecla não fez nada.
    if (this.contexto.perfil.tipoBanco === "sqlserver") {
      this.aoRelatar(t("compilar so no oracle"), true);
      return;
    }

    const fonte = this.editor.textoParaExecutar.trim();
    if (!fonte) {
      this.aoRelatar(t("sem fonte para compilar"), true);
      return;
    }
    const linhaBase = this.editor.linhaDoTextoParaExecutar;
    this.definirExecutando(true);
    try {
      const erros = await api.compilar(this.contexto.sessaoId, fonte);
      this.abrirAbaInferior("mensagens");
      if (erros.length === 0) {
        this.acrescentarMensagem(t("compilado com sucesso"));
        this.aoRelatar(t("compilado com sucesso"));
      } else {
        this.listarErrosDeCompilacao(erros, linhaBase);
        this.aoRelatar(t("erros de compilacao", { n: erros.length }), true);
      }
    } catch (erro) {
      this.mostrarErro(erro, linhaBase);
    } finally {
      this.definirExecutando(false);
    }
  }

  /**
   * Um item por erro de compilação no painel de mensagens, cada um levando o
   * cursor até a sua linha. Serve ao botão Compilar e ao `CREATE` executado no
   * editor, que o Oracle aceita e só avisa.
   */
  private listarErrosDeCompilacao(
    erros: api.ErroDeCompilacao[],
    linhaBase: number | null,
  ): void {
    if (erros.length === 0) return;
    for (const erro of erros) {
      const item = document.createElement("div");
      item.className = erro.aviso ? "mensagem" : "mensagem mensagem--erro";
      item.textContent = this.textoDoErroDeCompilacao(erro, linhaBase);
      if (linhaBase !== null) {
        const linha = linhaBase + erro.linha - 1;
        item.classList.add("mensagem--clicavel");
        item.addEventListener("click", () => this.editor.irParaLinha(linha));
      }
      this.listaDeMensagens.append(item);
    }
    this.listaDeMensagens.scrollTop = this.listaDeMensagens.scrollHeight;
  }

  /** "Linha 12, coluna 5: PLS-00103…", com a linha do editor quando se sabe. */
  private textoDoErroDeCompilacao(erro: api.ErroDeCompilacao, linhaBase: number | null): string {
    return t("linha coluna erro", {
      linha: linhaBase === null ? erro.linha : linhaBase + erro.linha - 1,
      coluna: erro.coluna,
      texto: erro.texto,
    });
  }

  // --- Grid: menu, cópia e exportação (requisitos 6, 16 e 39) --------------

  private menuDoGrid(evento: MouseEvent): void {
    const grid = this.grid;
    if (!grid) return;
    const tabela = this.tabelaDoInsert(grid);
    abrirMenuDeContexto(evento, [
      { rotulo: t("copiar"), acao: () => void grid.copiar(false) },
      { rotulo: t("copiar com cabecalho"), acao: () => void grid.copiar(true) },
      {
        rotulo: t("copiar para teams"),
        acao: () => void grid.copiarParaTeams(),
      },
      { rotulo: t("selecionar tudo"), acao: () => grid.selecionarTudo() },
      // Requisito 39: o item só existe quando o comando lê uma tabela só,
      // porque é dela que o INSERT fala. Com duas no `FROM` não há resposta
      // certa para "inserir onde", e um palpite escreveria na tabela errada.
      ...(tabela
        ? [
            {
              rotulo: t("gerar script de insert"),
              separadorAntes: true,
              acao: () => void this.copiarScriptDeInsert(grid, tabela),
            },
          ]
        : []),
      { rotulo: t("exportar para csv"), separadorAntes: true, acao: () => void this.exportar("csv") },
      { rotulo: t("exportar para excel"), acao: () => void this.exportar("excel") },
    ]);
  }

  /**
   * A tabela para o script de INSERT: a única do comando daquele grid, ou
   * `null` quando o comando lê mais de uma — ou nenhuma.
   *
   * Subconsulta conta como outra tabela, e por isso `SELECT * FROM cliente
   * WHERE id IN (SELECT ...)` não oferece o item. É de propósito: dali para
   * frente a conta deixa de ser confiável, e é melhor não oferecer do que
   * oferecer errado.
   */
  private tabelaDoInsert(grid: Grid): string | null {
    const comando = this.grids.find((aberto) => aberto.grid === grid)?.comando;
    if (!comando) return null;
    const tabelas = tabelasDoComando(comando);
    return tabelas.length === 1 ? tabelas[0].nomeEscrito : null;
  }

  /** Um `INSERT` por linha escolhida, no clipboard (requisito 39). */
  private copiarScriptDeInsert(grid: Grid, tabela: string): Promise<void> {
    return copiarInsertParaClipboard(
      grid.paraScriptDeInsert(),
      tabela,
      this.contexto.perfil.tipoBanco === "sqlserver",
      this.aoRelatar,
    );
  }

  private async exportar(formato: "csv" | "excel"): Promise<void> {
    const grid = this.grid;
    if (!grid) return;
    const dados = grid.paraExportacao();
    const extensao = formato === "csv" ? "csv" : "xlsx";

    try {
      const caminho = await api.escolherArquivoParaSalvar(
        `consulta.${extensao}`,
        t(formato === "csv" ? "arquivo csv" : "arquivo planilha do excel"),
        extensao,
      );
      if (!caminho) return;
      if (formato === "csv") {
        await api.gravarTexto(caminho, paraCsv(dados.colunas, dados.linhas));
      } else {
        await api.gravarPlanilha(caminho, dados.colunas, dados.linhas);
      }
      this.aoRelatar(t("exportado para", { caminho }));
    } catch (erro) {
      this.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private abrirValor(coluna: string, valor: string): void {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";
    const caixa = document.createElement("div");
    caixa.className = "dialogo visor-valor";

    let pararEsc = (): void => {};
    const encerrar = (): void => {
      pararEsc();
      fundo.remove();
    };

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.textContent = coluna;
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.addEventListener("click", encerrar);
    barra.append(titulo, fechar);

    const corpo = document.createElement("pre");
    corpo.className = "visor-valor__corpo";
    corpo.textContent = valor;

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const espaco = document.createElement("span");
    espaco.className = "status";
    espaco.textContent = t("caracteres", { n: valor.length });
    const copiar = document.createElement("button");
    copiar.textContent = t("copiar");
    copiar.addEventListener("click", () => void navigator.clipboard.writeText(valor));
    const ok = document.createElement("button");
    ok.textContent = t("fechar");
    ok.addEventListener("click", encerrar);
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(copiar, ok);
    rodape.append(espaco, botoes);

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);
    pararEsc = fecharComEsc(fundo, encerrar);
    document.body.append(fundo);
    ok.focus();
  }
}

function separador(): HTMLElement {
  const traco = document.createElement("span");
  traco.className = "barra-consulta__separador";
  return traco;
}

function primeiraLinha(texto: string): string {
  const linha = texto.split("\n")[0].trim();
  return linha.length > 90 ? `${linha.slice(0, 90)}…` : linha;
}

/** Resultado de consulta em texto, para o modo script. */
function formatarComoTexto(consulta: api.ResultadoConsulta): string {
  const larguras = consulta.colunas.map((coluna, indice) =>
    Math.max(
      coluna.nome.length,
      ...consulta.linhas.map((linha) => (linha[indice] ?? "(null)").length),
    ),
  );
  const cabecalho = consulta.colunas
    .map((coluna, i) => coluna.nome.padEnd(larguras[i]))
    .join(" ");
  const traco = larguras.map((largura) => "-".repeat(largura)).join(" ");
  const corpo = consulta.linhas.map((linha) =>
    linha.map((valor, i) => (valor ?? "(null)").padEnd(larguras[i])).join(" "),
  );
  return [cabecalho, traco, ...corpo].join("\n");
}
