/**
 * Editor de consultas SQL (requisitos 3, 11, 22, 23 e 26).
 *
 * Monta o CodeMirror com o mínimo necessário: destaque de sintaxe, numeração
 * de linha, localizar/substituir, comentar bloco e autocomplete. Nada de
 * `basicSetup` — cada extensão entra explicitamente, para não carregar o que
 * não vai ser usado.
 */

import {
  acceptCompletion,
  autocompletion,
  closeCompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
  startCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import {
  copyLineDown,
  defaultKeymap,
  deleteGroupBackward,
  deleteGroupForwardWin,
  deleteLine,
  deleteToLineEnd,
  deleteToLineStart,
  history,
  historyKeymap,
  indentLess,
  indentMore,
  insertNewlineKeepIndent,
  moveLineDown,
  moveLineUp,
  toggleBlockComment,
  toggleLineComment,
} from "@codemirror/commands";
import { PLSQL, sql } from "@codemirror/lang-sql";
import {
  bracketMatching,
  defaultHighlightStyle,
  HighlightStyle,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { tags } from "@lezer/highlight";
import {
  Compartment,
  EditorSelection,
  EditorState,
  Prec,
  RangeSet,
  StateEffect,
  type Extension,
  type Range,
  type SelectionRange,
  type Text,
} from "@codemirror/state";
import {
  type Command,
  crosshairCursor,
  Decoration,
  type DecorationSet,
  drawSelection,
  EditorView,
  GutterMarker,
  gutterLineClass,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  type PluginValue,
  rectangularSelection,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";

import "./editor-sql.css";

import * as api from "../api";
import { obterAtalhos, observarAtalhos, paraCodeMirror, type IdDeAtalho } from "../atalhos";
import { t } from "../idioma";
import { observarTema, temaAtual } from "../tema";
import { ajustarZoomDoEditor, observarZoomDoEditor } from "../zoom-editor";

/**
 * De que banco e de que esquema é o nome que está sendo digitado.
 *
 * Só o SQL Server preenche isto, e só quando o nome vem qualificado —
 * `dbo.tb`, `Vendas..tb`, `Vendas.dbo.tb`. Vazio, valem o banco da aba e todos
 * os esquemas dele, que é o caso do nome escrito sozinho.
 */
export interface EscopoDoNome {
  banco?: string;
  esquema?: string;
}

/** Quem consulta o banco em nome do editor. */
export interface FonteDeSugestoes {
  /**
   * `apenasRotinas` diz que o cursor está fora de lugar de tabela: ali só cabe
   * o que se chama, e é o que permite responder de índice — inclusive com
   * prefixo vazio, que é o caso do `EXECUTE ` recém-digitado.
   */
  objetos: (
    prefixo: string,
    escopo?: EscopoDoNome,
    apenasRotinas?: boolean,
  ) => Promise<api.Sugestao[]>;
  /** Oracle: a fonte local responde independentemente da consulta remota. */
  relacoes?: (prefixo: string, esquema: string | undefined, local: boolean) => Promise<api.Sugestao[]>;
  /** `banco` só vem no nome de três partes do SQL Server: `Vendas..cliente`. */
  colunas: (objeto: string, banco?: string) => Promise<api.Coluna[]>;
  /** Procedures e funções públicas declaradas numa package. */
  subprogramas?: (pacote: string) => Promise<api.Subprograma[]>;
  /**
   * Os bancos do servidor.
   *
   * Só o SQL Server tem, e é a ausência dela que diz ao editor que ali não há
   * nome de três partes a resolver: no Oracle o ponto depois de um nome pede
   * os subprogramas de uma package, e não os objetos de outro banco.
   */
  bancos?: () => Promise<string[]>;
}

export interface OpcoesDoEditor {
  texto?: string;
  sugestoes: FonteDeSugestoes;
  /** Impede edição, mas mantém seleção, busca e destaque de sintaxe. */
  somenteLeitura?: boolean;
  /**
   * Nome do campo para o leitor de tela.
   *
   * A aba de consulta não precisa — o editor é a tela inteira —, mas dentro de
   * um formulário ele é mais um campo entre outros, e um campo sem nome não se
   * distingue dos vizinhos.
   */
  rotulo?: string;
  /** Chamado por CTRL+Enter (execução do trecho selecionado). */
  aoExecutar?: () => void;
  /** Chamado por F5 (execução em modo script). */
  aoExecutarScript?: () => void;
  /** Chamado por CTRL+SHIFT+C (compilar PL/SQL). */
  aoCompilar?: () => void;
  /** Chamado a cada alteração do texto, para a cópia de recuperação. */
  aoMudar?: () => void;
  /** Chamado por Ctrl+clique sobre um identificador Oracle. */
  aoCtrlClicar?: (referencia: string) => void;
  /**
   * Das referências, as que o Ctrl+clique sabe abrir. É o que sublinha esses
   * nomes enquanto o Ctrl está apertado (requisito 22); sem ela o Ctrl+clique
   * continua valendo, só sem o aviso na tela.
   */
  verificarReferencias?: (referencias: string[]) => Promise<string[]>;
  /** Abre o menu de contexto de um identificador Oracle. */
  aoAbrirMenuDeReferencia?: (evento: MouseEvent, referencia: string) => void;
}

/** Um editor de SQL montado sobre um elemento. */
export class EditorSql {
  readonly elemento = document.createElement("div");
  private readonly visao: EditorView;
  private readonly compartimentoAtalhos = new Compartment();
  private readonly compartimentoDestaque = new Compartment();
  private pararDeObservarAtalhos = (): void => {};
  private pararDeObservarTema = (): void => {};
  private pararDeObservarZoom = (): void => {};

  /** Colunas por tabela, guardadas como a busca. Ver `colunasDe`. */
  private readonly colunasEmMemoria = new Map<string, Promise<api.Coluna[]>>();

  /** Objetos por prefixo e escopo, guardados como a busca. Ver `objetosDoPrefixo`. */
  private readonly objetosEmMemoria = new Map<string, Promise<api.Sugestao[]>>();

  /** Das buscas acima, as que vieram **inteiras**. Ver `derivarDaMemoria`. */
  private readonly objetosInteiros = new Map<string, api.Sugestao[]>();

  /** Prefixos que voltaram sem nada: os maiores também voltariam sem nada. */
  private readonly prefixosVazios = new Set<string>();

  /** Fila das consultas de objeto: uma por vez, para não encher a conexão. */
  private filaDeObjetos: Promise<void> = Promise.resolve();

  /** Número do último pedido de objeto: o que não é o último é abandonado. */
  private pedidoDeObjetos = 0;
  private pedidoDeRelacoes = 0;

  /** Subprogramas por package, guardados como a busca. */
  private readonly subprogramasEmMemoria = new Map<string, Promise<api.Subprograma[]>>();

  /** Bancos do servidor, lidos uma vez por aba. Ver `bancosDoServidor`. */
  private bancosEmMemoria: Promise<string[]> | null = null;

  /** Temporizador da leitura antecipada de colunas. Ver `adiantarColunas`. */
  private adiantamento: number | undefined;

  /** O script limpo do documento inteiro. Ver `textoLimpoDoScript`. */
  private scriptLimpo: { doc: Text; limpo: string } | null = null;

  /** O que se sabe sobre quais nomes o Ctrl+clique abre; nulo sem quem pergunte. */
  private readonly navegaveis: ReferenciasNavegaveis | null;

  /** O sublinhado do Ctrl e o seu plugin, para o editor falar com ele. */
  private readonly navegacao: ReturnType<typeof sublinhadoDeNavegacao> | null;

  /** Resposta que chega depois de a aba fechar não tem onde ser desenhada. */
  private destruido = false;

  constructor(private readonly opcoes: OpcoesDoEditor) {
    this.elemento.className = "editor-sql";
    this.navegaveis =
      opcoes.verificarReferencias && opcoes.aoCtrlClicar
        ? new ReferenciasNavegaveis(opcoes.verificarReferencias, () => {
            if (!this.destruido) this.visao.dispatch({ effects: refazerSublinhado.of(null) });
          })
        : null;
    // É a falta da lista de bancos que diz que este editor é de Oracle — a
    // mesma marca que o autocomplete usa para o nome de três partes.
    this.navegacao = this.navegaveis
      ? sublinhadoDeNavegacao(
          this.navegaveis,
          opcoes.sugestoes.bancos ? RESERVADAS_DO_SQL_SERVER : RESERVADAS_DO_ORACLE,
        )
      : null;
    this.visao = new EditorView({
      parent: this.elemento,
      state: EditorState.create({
        doc: opcoes.texto ?? "",
        extensions: this.extensoes(),
      }),
    });
    this.pararDeObservarAtalhos = observarAtalhos(() =>
      this.visao.dispatch({
        effects: this.compartimentoAtalhos.reconfigure(this.mapaDeAtalhos()),
      }),
    );
    // O destaque de sintaxe já nasce certo (lido em `extensoes()`); só troca ao
    // vivo quando o tema muda enquanto este editor está aberto.
    this.pararDeObservarTema = observarTema(() =>
      this.visao.dispatch({
        effects: this.compartimentoDestaque.reconfigure(this.destaqueDeSintaxe()),
      }),
    );
    // O tamanho da fonte muda pelo CSS, sem passar pelo CodeMirror; sem remedir,
    // ele continuaria posicionando cursor e seleção pela largura de caractere
    // antiga até a próxima rolagem.
    this.pararDeObservarZoom = observarZoomDoEditor(() => this.visao.requestMeasure());

    // A roda vai no elemento inteiro, e não no `domEventHandlers` do
    // CodeMirror: assim o Ctrl+roda também vale sobre a régua de números e a
    // barra de rolagem, e não só sobre o texto. `passive: false` é o que
    // permite o `preventDefault` — sem ele o Chromium ignora o pedido e a
    // página inteira rolaria junto.
    this.elemento.addEventListener("wheel", this.aoGirarARoda, { passive: false });
  }

  /** Ctrl+roda amplia e reduz o editor (requisito 3). */
  private readonly aoGirarARoda = (evento: WheelEvent): void => {
    if (!evento.ctrlKey || evento.altKey || evento.deltaY === 0) return;
    evento.preventDefault();
    // Roda para cima (`deltaY` negativo) aproxima, como em todo editor.
    ajustarZoomDoEditor(evento.deltaY < 0 ? 1 : -1);
  };

  private extensoes(): Extension[] {
    return [
      // Só a mudança do documento interessa: rolar e mover o cursor também
      // produzem atualização, e não mudam nada a ser guardado.
      EditorView.updateListener.of((atualizacao) => {
        if (!atualizacao.docChanged) return;
        this.opcoes.aoMudar?.();
        this.adiantarColunas();
      }),
      EditorState.readOnly.of(this.opcoes.somenteLeitura ?? false),
      EditorView.editable.of(!(this.opcoes.somenteLeitura ?? false)),
      // Fora do modo editável o `.cm-content` não recebe `tabindex`, e sem ele
      // não pega foco de teclado: o clique ainda seleciona texto, mas o CTRL+A
      // escapa para o WebView2, que seleciona a página inteira — árvore, barra,
      // menus — em vez do conteúdo. Tornar o conteúdo focável devolve o
      // "selecionar tudo" (Mod-a) do CodeMirror ao visualizador de DDL.
      ...(this.opcoes.somenteLeitura
        ? [EditorView.contentAttributes.of({ tabindex: "0" })]
        : []),
      ...(this.opcoes.rotulo
        ? [EditorView.contentAttributes.of({ "aria-label": this.opcoes.rotulo })]
        : []),
      lineNumbers(),
      destacarLinhaPrincipalNoGutter,
      destacarLinhaPrincipal,
      highlightSpecialChars(),
      drawSelection(),
      rectangularSelection(),
      crosshairCursor(),
      // Sem este facet o Alt+arrasto não serve para nada: o
      // `rectangularSelection` monta a seleção em coluna, mas a transação a
      // colapsa com `asSingle()` e sobra um intervalo só. Quem normalmente liga
      // isto é o `basicSetup`, que este arquivo dispensa de propósito.
      EditorState.allowMultipleSelections.of(true),
      history(),
      indentOnInput(),
      bracketMatching(),
      closeBrackets(),
      search({ top: true }),
      highlightSelectionMatches(),
      this.compartimentoDestaque.of(this.destaqueDeSintaxe()),
      // PL/SQL é o dialeto certo: reconhece BEGIN/END, /* */, ligação com :bind.
      sql({ dialect: PLSQL, upperCaseKeywords: false }),
      autocompletion({
        override: [
          (contexto) => this.completarRelacoesLocais(contexto),
          (contexto) => this.completarBancos(contexto),
          (contexto) => this.completar(contexto),
        ],
        // Uma consulta ao Oracle não atrasa a publicação da fonte local.
        updateSyncTime: 0,
        // O usuário digita `TBL_` e espera a lista: não exigimos atalho.
        activateOnTyping: true,
        icons: false,
        // O desempate pelo detalhe é o que apaga o nome repetido.
        //
        // As fontes local e remota se sobrepõem de propósito — o índice abre a
        // lista em milissegundos e o dicionário confirma meio segundo depois —,
        // e o CodeMirror já descarta a opção igual à anterior. Só que ele
        // descarta **a anterior**, e o desempate padrão é só pelo rótulo: com
        // duas tabelas `DEM_MOVIMENTOS`, uma do usuário e outra de `EXEMPLO`, os quatro
        // itens empatavam e a ordenação estável os deixava na ordem em que
        // chegaram — índice A, índice B, banco A, banco B. Nunca dois iguais
        // lado a lado, e a lista mostrava o nome quatro vezes.
        //
        // Com o detalhe no desempate a ordem vira A, A, B, B, as cópias ficam
        // vizinhas e a limpeza da própria biblioteca faz o resto. Os homônimos
        // de donos diferentes continuam, que é o que o detalhe existe para
        // distinguir.
        compareCompletions: (a, b) =>
          (a.sortText ?? a.label).localeCompare(b.sortText ?? b.label) ||
          (a.detail ?? "").localeCompare(b.detail ?? ""),
      }),
      EditorView.lineWrapping,
      EditorView.domEventHandlers({
        keydown: (evento, visao) => {
          if (evento.key !== ".") return false;
          evento.preventDefault();
          return inserirPontoEAbrirSugestoes(visao);
        },
        mousedown: (evento, visao) => {
          if (
            evento.button !== 0 ||
            !evento.ctrlKey ||
            evento.altKey ||
            evento.shiftKey ||
            !this.opcoes.aoCtrlClicar
          ) {
            return false;
          }
          const posicao = visao.posAtCoords({ x: evento.clientX, y: evento.clientY });
          if (posicao === null) return false;
          const referencia = identificadorNaPosicao(visao.state.doc.toString(), posicao);
          if (!referencia) return false;

          // O Ctrl+clique é navegação, não uma nova seleção múltipla.
          evento.preventDefault();
          this.opcoes.aoCtrlClicar(referencia);
          return true;
        },
        contextmenu: (evento, visao) => {
          if (!this.opcoes.aoAbrirMenuDeReferencia) return false;
          const posicao = visao.posAtCoords({ x: evento.clientX, y: evento.clientY });
          if (posicao === null) return false;
          const referencia = identificadorNaPosicao(visao.state.doc.toString(), posicao);
          if (!referencia) return false;

          evento.preventDefault();
          this.opcoes.aoAbrirMenuDeReferencia(evento, referencia);
          return true;
        },
      }),
      ...(this.navegacao ? [this.navegacao.extensao] : []),
      this.tema(),

      // Precedência máxima, e **antes** dos atalhos padrão: o `defaultKeymap`
      // do CodeMirror liga Mod-Enter a "inserir linha em branco" e venceria a
      // execução se fosse consultado primeiro.
      Prec.highest(
        keymap.of([
          // Enter repete a indentação da linha atual, e não a indentação
          // automática do SQL (`insertNewlineAndIndent`), que reposicionaria a
          // linha nova pelas regras da linguagem em vez de respeitar o
          // alinhamento escrito à mão. Quem alinhou a segunda coluna de um
          // SELECT continua alinhado ao digitar a terceira; era o
          // `insertNewline` puro que jogava o cursor de volta para a coluna 1.
          // Com o autocomplete aberto, continua aceitando a sugestão primeiro.
          {
            key: "Enter",
            run: (visao) => acceptCompletion(visao) || insertNewlineKeepIndent(visao),
            shift: insertNewlineKeepIndent,
          },

          // Tab aceita a sugestão aberta, e precisa vir antes do
          // `indentWithTab`: os dois pegam Tab, e sem isto a tecla aceitava a
          // sugestão **e** ainda indentava o início da linha. O
          // `completionKeymap` só liga Enter ao aceite, nunca Tab.
          //
          // Sem sugestão aberta o `acceptCompletion` devolve `false`, então o
          // Tab cai para o `indentWithTab` e volta a indentar normalmente.
          { key: "Tab", run: acceptCompletion },

        ]),
      ),
      this.compartimentoAtalhos.of(this.mapaDeAtalhos()),

      keymap.of([
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
        ...completionKeymap,
      ]),
    ];
  }

  /** Atalhos editáveis, reconstruídos quando o usuário salva a configuração. */
  private mapaDeAtalhos(): Extension {
    const atalhos = obterAtalhos();
    // Cada ação pode ter duas teclas; a alternativa vazia ("") não vira entrada.
    const acao = (id: IdDeAtalho, comando: Command) =>
      atalhos[id]
        .filter((tecla) => tecla !== "")
        .map((tecla) => ({ key: paraCodeMirror(tecla), preventDefault: true, run: comando }));
    const chamar = (funcao?: () => void): Command => () => {
      funcao?.();
      return true;
    };

    return Prec.highest(
      keymap.of([
        ...acao("duplicar-linha", duplicarLinhaOuSelecao),
        ...acao("dividir-linhas", dividirLinhasLongas),
        ...acao("juntar-linhas", juntarLinhas),
        ...acao("excluir-linha", deleteLine),
        ...acao("minusculas", alterarCaixa(false)),
        ...acao("maiusculas", alterarCaixa(true)),
        ...acao("mover-linha-acima", moveLineUp),
        ...acao("mover-linha-abaixo", moveLineDown),
        ...acao("apagar-palavra-anterior", deleteGroupBackward),
        ...acao("apagar-palavra-seguinte", deleteGroupForwardWin),
        ...acao("apagar-inicio-linha", deleteToLineStart),
        ...acao("apagar-fim-linha", deleteToLineEnd),
        ...acao("comentar-linhas", toggleLineComment),
        ...acao("comentar-bloco", toggleBlockComment),
        ...acao("selecao-coluna-acima", estenderEmColuna(false)),
        ...acao("selecao-coluna-abaixo", estenderEmColuna(true)),
        ...acao("reduzir-indentacao", indentLess),
        ...acao("aumentar-indentacao", indentMore),
        ...acao("executar", chamar(this.opcoes.aoExecutar)),
        ...acao("executar-script", chamar(this.opcoes.aoExecutarScript)),
        ...acao("compilar", chamar(this.opcoes.aoCompilar)),
      ]),
    );
  }

  /**
   * Cores do editor.
   *
   * Vai como tema do CodeMirror, e não como CSS solto, porque o tema-base dele
   * tem precedência sobre folha de estilo comum — a seleção ficaria naquele
   * cinza quase invisível. As cores em si são `var(--…)`: um valor de tema do
   * CodeMirror é só uma declaração CSS, e resolve variável como qualquer
   * outra, herdada de onde `estilo.css` as define.
   */
  private tema(): Extension {
    return EditorView.theme({
      "&": { color: "var(--cor-texto)", backgroundColor: "var(--cor-fundo-painel)" },
      ".cm-content": { caretColor: "var(--cor-texto)" },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--cor-texto)" },

      // O caminho completo é obrigatório: o tema-base do CodeMirror pinta a
      // seleção com um lilás quase invisível usando exatamente este seletor, e
      // qualquer coisa mais curta perde por especificidade.
      ".cm-selectionLayer .cm-selectionBackground": { backgroundColor: "var(--cor-selecao-editor)" },
      "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground": {
        backgroundColor: "var(--cor-selecao-editor)",
      },
      ".cm-content ::selection": { backgroundColor: "var(--cor-selecao-editor)" },
      ".cm-selectionMatch": { backgroundColor: "var(--cor-selecao-correspondencia)" },

      // O nome que o Ctrl+clique abre, com o Ctrl apertado (requisito 22). A
      // cor vale também para dentro da marca, onde o destaque de sintaxe pode
      // ter posto o próprio `span`; a mãozinha é herdada por ele.
      ".cm-referencia-navegavel, .cm-referencia-navegavel *": {
        color: "var(--cor-destaque)",
      },
      ".cm-referencia-navegavel": {
        textDecoration: "underline",
        textUnderlineOffset: "2px",
        cursor: "pointer",
      },
    });
  }

  /**
   * O destaque padrão do CodeMirror (`defaultHighlightStyle`) é documentado
   * pela própria biblioteca como feito para tema claro — no escuro, palavra
   * reservada em roxo `#708` e string em vermelho `#a11` somem contra um fundo
   * quase preto. `DESTAQUE_ESCURO` cobre os mesmos grupos de tag com cores
   * claras o bastante para o fundo escuro.
   */
  private destaqueDeSintaxe(): Extension {
    return syntaxHighlighting(temaAtual() === "escuro" ? DESTAQUE_ESCURO : defaultHighlightStyle, {
      fallback: true,
    });
  }

  // --- Texto ---------------------------------------------------------------

  get texto(): string {
    return this.visao.state.doc.toString();
  }

  /** Trecho selecionado, ou vazio se não há seleção (requisito 5). */
  get selecao(): string {
    const { from, to } = this.visao.state.selection.main;
    return from === to ? "" : this.visao.state.sliceDoc(from, to);
  }

  /**
   * O que deve ser executado: a seleção quando existe, o documento inteiro
   * quando não existe. É a regra dos requisitos 5 e 6.
   */
  get textoParaExecutar(): string {
    return this.selecao || this.texto;
  }

  /**
   * Linha do editor em que começa o {@link textoParaExecutar}, depois dos
   * espaços que a execução apara. As linhas que o backend devolve são contadas
   * a partir dela (requisito 18).
   */
  get linhaDoTextoParaExecutar(): number {
    const { from, to } = this.visao.state.selection.main;
    const inicio = from === to ? 0 : from;
    const texto = this.textoParaExecutar;
    const aparado = texto.length - texto.trimStart().length;
    return this.visao.state.doc.lineAt(inicio + aparado).number;
  }

  focar(): void {
    this.visao.focus();
  }

  /** Substitui a seleção pelo texto (ou insere no cursor, se não há seleção). */
  inserir(texto: string): void {
    const { from, to } = this.visao.state.selection.main;
    this.visao.dispatch({
      changes: { from, to, insert: texto },
      selection: { anchor: from + texto.length },
      scrollIntoView: true,
    });
    this.focar();
  }

  /** Substitui a seleção e mantém selecionado exatamente o texto inserido. */
  inserirESelecionar(texto: string): void {
    const { from, to } = this.visao.state.selection.main;
    this.visao.dispatch({
      changes: { from, to, insert: texto },
      selection: { anchor: from, head: from + texto.length },
      scrollIntoView: true,
    });
    this.focar();
  }

  /** Troca todo o conteúdo, usado por visualizadores que recarregam o banco. */
  substituirTexto(texto: string): void {
    this.visao.dispatch({
      changes: { from: 0, to: this.visao.state.doc.length, insert: texto },
      selection: { anchor: 0 },
      scrollIntoView: true,
    });
  }

  /**
   * Converte um deslocamento em caracteres na linha correspondente.
   *
   * O Oracle informa o erro por deslocamento dentro do comando; o painel de
   * mensagens precisa da linha (requisito 18).
   */
  linhaDaPosicao(deslocamento: number): number {
    const limite = Math.min(Math.max(deslocamento, 0), this.visao.state.doc.length);
    return this.visao.state.doc.lineAt(limite).number;
  }

  /** Posiciona o cursor no início da linha indicada (1 = primeira). */
  irParaLinha(linha: number): void {
    const total = this.visao.state.doc.lines;
    const alvo = this.visao.state.doc.line(Math.min(Math.max(linha, 1), total));
    this.visao.dispatch({
      selection: { anchor: alvo.from },
      scrollIntoView: true,
    });
    this.focar();
  }

  destruir(): void {
    this.destruido = true;
    window.clearTimeout(this.adiantamento);
    this.pararDeObservarAtalhos();
    this.pararDeObservarTema();
    this.pararDeObservarZoom();
    this.elemento.removeEventListener("wheel", this.aoGirarARoda);
    this.visao.destroy();
  }

  // --- Autocomplete (requisitos 3 e 23) ------------------------------------

  /** Fonte independente: não usa a fila, o cache remoto nem os prefixos vazios. */
  private async completarRelacoesLocais(contexto: CompletionContext): Promise<CompletionResult | null> {
    const fonte = this.opcoes.sugestoes.relacoes;
    if (!fonte) return null;
    contexto.addEventListener("abort", () => {}, { onDocChange: true });
    const analise = analisarSql(contexto.state.doc.toString(), contexto.pos);
    if (analise.dentroDeLiteral) return null;
    const [inicio] = limitesDoComando(analise.limpo, contexto.pos);
    const antes = analise.limpo.slice(inicio, contexto.pos);
    const qualificado = qualificacaoNoCursor(antes);
    let prefixo: string;
    let esquema: string | undefined;
    let de: number;
    if (qualificado) {
      if (qualificado.partes.length !== 1 || lugarDaSugestao(antes.slice(0, qualificado.inicio)) !== "tabela") return null;
      prefixo = qualificado.parcial;
      esquema = qualificado.partes[0];
      de = contexto.pos - prefixo.length;
    } else {
      const palavra = contexto.matchBefore(/[\w$#]+/);
      prefixo = palavra?.text ?? "";
      de = palavra?.from ?? contexto.pos;
      if (lugarDaSugestao(antes.slice(0, antes.length - prefixo.length)) !== "tabela") return null;
      if (!contexto.explicit && prefixo.length < MINIMO_DE_PREFIXO) return null;
    }
    try {
      const achados = await fonte(prefixo, esquema, true);
      if (contexto.aborted || achados.length === 0) return null;
      return { from: de, options: achados.map(paraOpcao) };
    } catch { return null; }
  }

  private relacoesDoBanco(prefixo: string, esquema?: string): Promise<api.Sugestao[]> {
    const pedido = ++this.pedidoDeRelacoes;
    if (prefixo.length < MINIMO_DE_PREFIXO) return Promise.resolve([]);
    return this.emFila(async () => {
      if (this.destruido || pedido !== this.pedidoDeRelacoes) return [];
      return this.opcoes.sugestoes.relacoes!(prefixo, esquema, false);
    }).catch(() => []);
  }

  private async completar(contexto: CompletionContext): Promise<CompletionResult | null> {
    // Descartar a busca em andamento a cada tecla não é economia: é o que
    // impede o autocomplete de travar de vez.
    //
    // Enquanto uma busca está no ar, o CodeMirror não dispara outra para a
    // mesma fonte. Se aquela terminar **sem nada** — e ir ao banco atrás de um
    // objeto chamado `WHE` termina sem nada — com o texto já mudado desde
    // então, ele tenta voltar a fonte para inativa reaplicando as teclas
    // digitadas no meio; as teclas a põem pendente outra vez, e aí ele não
    // publica nem recomeça. A fonte fica pendente para sempre, sem nenhuma
    // busca rodando, e a lista não aparece mais até a tecla seguinte.
    //
    // Foi o que aconteceu no relato de 01/08: uma consulta de 4 s terminou
    // vazia, e depois dela `WHERE IDR` não sugeriu mais nada. Este é o
    // mecanismo que a própria biblioteca documenta para fonte lenta.
    contexto.addEventListener("abort", () => {}, { onDocChange: true });

    const analise = analisarSql(contexto.state.doc.toString(), contexto.pos);

    // Dentro de aspas simples não se sugere nada: ali se escreve o valor de um
    // dado, não o nome de um objeto. Vale igual para comentário.
    if (analise.dentroDeLiteral) return null;

    const [inicio, fim] = limitesDoComando(analise.limpo, contexto.pos);
    const comando = analise.limpo.slice(inicio, fim);
    const ateOCursor = analise.limpo.slice(inicio, contexto.pos);

    // Sai buscando as colunas das tabelas já escritas antes mesmo de saber se
    // elas serão pedidas. Custa nada aqui — a análise já está pronta — e é o
    // que dá ao banco os segundos entre `FROM cliente` e o `WHERE`.
    this.adiantar(comando, /[\w$#]*$/.exec(ateOCursor)?.[0] ?? "");

    // Um nome com ponto tem três leituras, e elas se excluem: depois de
    // "alias." vêm as colunas daquela tabela; depois de "dbo.", "Vendas.." ou
    // "Vendas.dbo.", os objetos daquele esquema e daquele banco (SQL Server,
    // requisito 3); depois de "pacote.", as procedures e funções públicas dela
    // (Oracle). A lista de tabelas do comando e a de bancos do servidor
    // separam as três sem perguntar nada ao banco.
    const qualificado = qualificacaoNoCursor(ateOCursor);
    if (qualificado) {
      const { partes, parcial } = qualificado;
      const de = contexto.pos - parcial.length;
      // Em FROM/JOIN, o ponto do Oracle qualifica o schema, não uma package.
      if (this.opcoes.sugestoes.relacoes && partes.length === 1 &&
          lugarDaSugestao(ateOCursor.slice(0, qualificado.inicio)) === "tabela") {
        return this.completarObjetos({ from: de, text: parcial }, "tabela", { esquema: partes[0] });
      }


      // O apelido vem primeiro: `c.` é o `FROM cliente c` muitas vezes mais do
      // que é o nome de um esquema chamado `c`.
      if (partes.length === 1) {
        const referencia = referenciaDoQualificador(comando, partes[0]);
        if (referencia) return this.listaDeColunas(await this.colunasDe(referencia), de);
      }

      // O que veio **antes** da cadeia é que diz o que cabe nela: `FROM
      // Vendas..tbl` pede tabela, e não procedure. Em lugar de coluna um nome
      // qualificado só faria sentido como apelido — hipótese já descartada
      // acima —, e depois de `use` só cabe o nome de um banco.
      const lugar = lugarDaSugestao(ateOCursor.slice(0, qualificado.inicio));
      // Depois de um operador o nome qualificado é `dbo.f(...)` no SQL Server e
      // `PKG.rotina` no Oracle: os dois caminhos precisam continuar abertos, e
      // é a ausência de `bancos` que separa um do outro logo abaixo.
      if (lugar === "tabela" || lugar === null || lugar === "coluna-ou-rotina") {
        const escopo = await this.escopoDoNome(partes);
        if (escopo) {
          return this.completarObjetos(
            { from: de, text: parcial },
            lugar === "tabela" ? "tabela" : "outro",
            escopo,
          );
        }
      }

      // Sobrou o ponto do Oracle. Com duas ou três partes não sobra nada: lá um
      // nome assim é de outro dono, e o dono já vem na lista sem ponto nenhum.
      if (partes.length !== 1) return null;
      return this.listaDeSubprogramas(await this.subprogramasDe(partes[0]), de);
    }

    // Depois de `EXECUTE ` a lista abre sem palavra nenhuma: é o lugar em que
    // se sabe o que vem a seguir, e a resposta não custa consulta ao banco.
    const emLugarDeRotina = lugarDaSugestao(ateOCursor) === "rotina";
    const palavra =
      contexto.matchBefore(/[\w$#]+/) ??
      // Com Ctrl+Espaço num ponto em branco não há palavra para casar, mas o
      // pedido é explícito e merece resposta.
      (contexto.explicit || emLugarDeRotina
        ? { from: contexto.pos, to: contexto.pos, text: "" }
        : null);
    if (!palavra || (palavra.from === palavra.to && !contexto.explicit && !emLugarDeRotina)) {
      return null;
    }

    const ateAqui = ateOCursor.slice(0, ateOCursor.length - palavra.text.length);

    switch (lugarDaSugestao(ateAqui)) {
      case "rotina":
        return this.completarObjetos(palavra, "outro");

      case "tabela":
        return this.completarObjetos(palavra, "tabela");

      case "coluna":
        // Sem nenhuma tabela escrita no comando não há coluna a oferecer, e é o
        // caso normal de quem começa pelo `SELECT ` e só depois escreve o
        // `FROM`. Devolver nada ali seria pior do que antes desta correção;
        // sobram os objetos que não são tabela, que é onde moram as funções.
        return (
          (await this.completarColunasDoComando(comando, palavra)) ??
          (await this.completarObjetos(palavra, "outro"))
        );

      case "coluna-ou-rotina":
        return this.completarColunasERotinas(comando, palavra);

      case "banco":
        // Depois de `use` só cabe banco (requisito 3, SQL Server): oferecer
        // tabela ali seria oferecer o que o comando não aceita.
        return this.listaDeBancos(await this.bancosQueComecamCom(palavra.text), palavra.from);

      default:
        // Fora do lugar de tabela e do de coluna sobram os objetos que não são
        // tabela — pacote, procedure, função —, que é o que se chama no meio de
        // um bloco PL/SQL (requisito 13). Tabela e view, não: essas só onde a
        // consulta pede uma.
        return this.completarObjetos(palavra, "outro");
    }
  }

  /**
   * Os bancos do servidor em lugar de tabela (SQL Server, requisito 3).
   *
   * O que se digita depois de `FROM` pode ser o começo de um nome de três
   * partes — `BANCO_EXEMPLO..tbl` —, e sem o banco na lista não há como chegar
   * até lá. Com o nome já qualificado é o contrário: ali o banco foi escrito,
   * e repeti-lo não levaria a lugar nenhum.
   *
   * É **fonte à parte**, e não um pedaço da lista de objetos. A lista de
   * bancos está na memória da aba e responde na hora; a de objetos vem do
   * catálogo. Juntas no mesmo `Promise.all`, a que estava pronta esperava a
   * outra — e no log de 30/09 uma consulta de 4 s ao catálogo escondeu por
   * quatro segundos justamente os bancos que estavam sendo digitados. Em fonte
   * própria, o `updateSyncTime: 0` publica esta assim que ela volta.
   */
  private async completarBancos(contexto: CompletionContext): Promise<CompletionResult | null> {
    if (!this.opcoes.sugestoes.bancos) return null;
    contexto.addEventListener("abort", () => {}, { onDocChange: true });
    const analise = analisarSql(contexto.state.doc.toString(), contexto.pos);
    if (analise.dentroDeLiteral) return null;
    const [inicio] = limitesDoComando(analise.limpo, contexto.pos);
    const antes = analise.limpo.slice(inicio, contexto.pos);
    if (qualificacaoNoCursor(antes)) return null;

    const palavra =
      contexto.matchBefore(/[\w$#]+/) ??
      (contexto.explicit ? { from: contexto.pos, to: contexto.pos, text: "" } : null);
    if (!palavra) return null;
    // `#tmp` é temporária do script, e não nome de banco.
    if (palavra.text.startsWith("#")) return null;
    if (lugarDaSugestao(antes.slice(0, antes.length - palavra.text.length)) !== "tabela") {
      return null;
    }

    const bancos = await this.bancosQueComecamCom(palavra.text);
    if (contexto.aborted || bancos.length === 0) return null;
    // Com `validFor`: a lista de bancos vem inteira, e filtrar inteiro é sempre
    // certo — o teto que tirou a marca da lista de objetos não existe aqui.
    return { from: palavra.from, options: bancos.map(paraOpcaoDeBanco), validFor: /^[\w$#]*$/ };
  }

  /**
   * Os objetos que começam pelo prefixo, filtrados pelo lugar do cursor.
   *
   * Os bancos de mesmo prefixo saem em `completarBancos`, que é fonte à parte
   * pelo motivo explicado lá.
   */
  private async completarObjetos(
    palavra: { from: number; text: string },
    lugar: "tabela" | "outro",
    escopo?: EscopoDoNome,
  ): Promise<CompletionResult | null> {
    // Nome que começa por `#` é tabela temporária, e nenhuma delas está no
    // catálogo do banco da aba: perguntar ali é uma consulta certa de voltar
    // vazia, uma por tecla. Quem responde é o script, pelas temporárias que ele
    // próprio define.
    if (palavra.text.startsWith("#")) {
      return lugar === "tabela" ? this.listaDeTemporarias(palavra) : null;
    }

    // Fora de lugar de tabela só cabe o que se chama, e é isso que o outro
    // lado precisa saber para responder do índice local (requisito 40).
    const achados =
      lugar === "tabela" && this.opcoes.sugestoes.relacoes
        ? await this.relacoesDoBanco(palavra.text, escopo?.esquema)
        : await this.objetosDoPrefixo(palavra.text, escopo, lugar === "outro");

    const doLugar = (tipo: string): boolean =>
      lugar === "tabela" ? TIPOS_DE_TABELA.has(tipo) : !TIPOS_DE_TABELA.has(tipo);

    const opcoes = achados.filter((objeto) => doLugar(objeto.tipo)).map(paraOpcao);
    if (opcoes.length === 0) return null;

    // **Sem `validFor` de propósito**, e é a correção de um defeito que parecia
    // ser de maiúsculas. A lista de objetos vem com teto: `DE` responde 300
    // nomes e avisa que cortou. Com `validFor`, o CodeMirror para de consultar
    // e vai filtrando essa lista cortada a cada tecla nova — então digitar o
    // `m` de `dem` nunca perguntava por `dem`, e `DEM_INT` não aparecia, porque
    // o `_` ordena depois das letras e ele fica fora dos 300 primeiros.
    //
    // Sem a marca, cada tecla volta a pedir a lista do prefixo atual. Isso não
    // é uma consulta por tecla: quem responde é o cache desta aba, ou o cache
    // por sessão do outro lado — que sabe derivar de um prefixo menor **só
    // quando ele veio inteiro**, que é a conta que o `validFor` atropelava.
    //
    // As outras listas mantêm a marca: coluna, subprograma e banco vêm
    // completos, e filtrar aqueles localmente é sempre certo.
    return { from: palavra.from, options: opcoes };
  }

  /**
   * Os objetos de um prefixo, sem repetir consulta e sem uma por tecla.
   *
   * Aqui mora a correção do defeito que fazia a lista abrir quinze segundos
   * depois. A conexão de metadados atende **um pedido por vez**, e o
   * autocomplete disparava uma consulta por tecla: digitar `EXECUTE DEM_INT.`
   * enfileirava `EXE`, `EXEC`, `EXECU`, `EXECUTE`, `D`, `DE`, `DEM`… cada uma
   * de um a três segundos, e o pedido do ponto era o último da fila. Medido no
   * log: quatro consultas terminando no mesmo instante, todas com 13 a 15
   * segundos de duração — nenhuma delas lenta, todas esperando.
   *
   * O cache por sessão do lado do Rust não resolvia: ele só ajuda depois de a
   * primeira consulta **voltar**, e ali nenhuma tinha voltado ainda.
   *
   * São três freios, e o terceiro é o que mais rende:
   *
   * 1. Prefixo repetido reaproveita a busca — a promessa, não o resultado, como
   *    nas colunas: quem pede o mesmo prefixo enquanto a consulta está no ar
   *    recebe aquela mesma consulta.
   * 2. Prefixo que voltou **vazio** dispensa os maiores: se `EXE` não achou
   *    nada, `EXEC` também não vai achar. Isso sozinho mata a metade das
   *    consultas de quem digita uma palavra reservada.
   * 3. **Uma consulta de objeto por vez nesta aba**, e a que ficou velha é
   *    abandonada: enquanto `S` está no ar, as teclas seguintes não abrem
   *    consulta nenhuma; quando ela volta, só o prefixo atual é consultado.
   *    Sete consultas viram duas.
   */
  private objetosDoPrefixo(
    prefixo: string,
    escopo?: EscopoDoNome,
    apenasRotinas = false,
  ): Promise<api.Sugestao[]> {
    const alvo = prefixo.toUpperCase();
    // Prefixo curto não é consultado do outro lado, e por isso não se consulta
    // daqui: era uma ida ao banco garantidamente vazia por tecla. A exceção é
    // a lista de rotinas, que vem do índice local e não custa consulta nenhuma
    // — é ela que faz `EXECUTE ` abrir a lista sem nada digitado.
    if (alvo.length < MINIMO_DE_PREFIXO && !apenasRotinas) return Promise.resolve([]);
    if (this.jaVeioVazio(alvo)) return Promise.resolve([]);

    const raiz = `${escopo?.banco ?? ""}|${escopo?.esquema ?? ""}|${apenasRotinas}|`;
    const derivada = this.derivarDaMemoria(raiz, alvo);
    if (derivada) return Promise.resolve(derivada);

    const chave = raiz + alvo;
    const guardada = this.objetosEmMemoria.get(chave);
    if (guardada) return guardada;

    const meu = ++this.pedidoDeObjetos;
    const busca = this.emFila(async () => {
      // Enquanto esta esperava a vez, o texto andou e o CodeMirror já
      // abandonou o pedido. Ir ao banco agora só atrasaria o pedido de
      // verdade, que está atrás dela na fila.
      //
      // A conferência do prefixo vazio se repete aqui, e não é sobra: quem
      // digita rápido cria o pedido de `EXECUT` antes de o de `E` voltar sem
      // nada, e é só na vez dele que já se sabe que não há o que buscar.
      if (meu !== this.pedidoDeObjetos || this.jaVeioVazio(alvo)) {
        // Não vale como resposta guardada: o prefixo pode ser pedido de novo.
        this.objetosEmMemoria.delete(chave);
        return [] as api.Sugestao[];
      }
      const achados = await this.opcoes.sugestoes.objetos(prefixo, escopo, apenasRotinas);
      // Só a lista que não bateu no teto pode responder por prefixo maior.
      if (achados.length < LIMITE_DE_SUGESTOES) this.objetosInteiros.set(chave, achados);
      // Sem escopo o vazio vale para qualquer prefixo maior. Com escopo, não:
      // ali o vazio é daquele esquema, e o prefixo é só parte da chave.
      //
      // O tamanho mínimo não é zelo excessivo: é a correção do defeito que a
      // primeira versão deste freio causou. O `s` de `select` voltava vazio
      // **porque não tinha sido consultado**, era gravado como vazio, e daí em
      // diante nenhum nome começado com S era oferecido na aba — nem
      // `SALDOS`, nem `SAIDAS`. Só vazio de prefixo que o banco realmente
      // olhou vale como resposta.
      // Lista de rotinas vazia não diz nada sobre tabelas com o mesmo prefixo:
      // registrar aquele vazio apagaria da aba nomes que existem.
      if (
        achados.length === 0 &&
        !escopo &&
        !apenasRotinas &&
        alvo.length >= MINIMO_DE_PREFIXO
      ) {
        this.prefixosVazios.add(alvo);
      }
      return achados;
    }).catch(() => {
      // Silêncio é melhor que um erro no meio da digitação — e a busca que
      // falhou não fica guardada, para a próxima tecla tentar de novo.
      this.objetosEmMemoria.delete(chave);
      return [] as api.Sugestao[];
    });
    this.objetosEmMemoria.set(chave, busca);
    return busca;
  }

  /**
   * A lista de um prefixo menor serve a este quando ela veio **inteira**.
   *
   * É a mesma conta que o cache do lado Rust faz, um degrau antes — e é este
   * degrau que importa: aqui não há IPC nem fila, então uma consulta lenta no
   * ar deixa de esconder uma lista que a aba **já tem**. Sem isto, os 3,3 s
   * que a busca de `BANCO_EXEMPLO..tb` levou em 30/09 custaram também todas as
   * teclas digitadas no meio, uma a uma, embora os 82 nomes que chegaram no
   * fim já contivessem a resposta de todas elas.
   *
   * Só a lista inteira serve. A que bateu no teto veio cortada, e filtrar
   * cortado é exatamente o defeito que tirou o `validFor` de
   * `completarObjetos` — por isso `objetosInteiros` recebe só as que couberam.
   */
  private derivarDaMemoria(raiz: string, alvo: string): api.Sugestao[] | null {
    let escolhida: { prefixo: string; lista: api.Sugestao[] } | null = null;
    for (const [chave, lista] of this.objetosInteiros) {
      if (!chave.startsWith(raiz)) continue;
      const prefixo = chave.slice(raiz.length);
      if (!alvo.startsWith(prefixo)) continue;
      // Quanto mais longo o prefixo já buscado, menos sobra para filtrar.
      if (escolhida && escolhida.prefixo.length >= prefixo.length) continue;
      escolhida = { prefixo, lista };
    }
    if (!escolhida) return null;
    return escolhida.lista.filter((objeto) => objeto.nome.toUpperCase().startsWith(alvo));
  }

  /** Se algum prefixo menor já voltou sem nada, este também vem sem nada. */
  private jaVeioVazio(alvo: string): boolean {
    for (const vazio of this.prefixosVazios) {
      if (alvo.startsWith(vazio)) return true;
    }
    return false;
  }

  /**
   * Enfileira o trabalho para não haver dois no ar ao mesmo tempo.
   *
   * A fila é desta aba e serve só às consultas de objeto: são elas que o
   * autocomplete dispara em rajada, e é a rajada que enche a conexão.
   */
  private emFila<T>(trabalho: () => Promise<T>): Promise<T> {
    const proxima = this.filaDeObjetos.then(trabalho, trabalho);
    // A fila não pode parar por causa de uma falha, e o erro segue para quem
    // pediu — daí o `catch` que só engole a rejeição da corrente.
    this.filaDeObjetos = proxima.then(
      () => undefined,
      () => undefined,
    );
    return proxima;
  }

  /**
   * Esquece o que foi lido do dicionário nesta aba.
   *
   * É o que "Atualizar metadados" e a troca de banco precisam: sem isso, o
   * objeto criado agora não apareceria, e o prefixo que voltou vazio ficaria
   * vazio para sempre.
   */
  esquecerObjetos(): void {
    this.objetosEmMemoria.clear();
    this.objetosInteiros.clear();
    this.prefixosVazios.clear();
    this.subprogramasEmMemoria.clear();
    this.esquecerReferencias();
  }

  /**
   * Esquece quais nomes o Ctrl+clique abre (requisito 22).
   *
   * A resposta vale pela vida da aba, menos depois de um DDL: a tabela criada
   * agora voltou "sem destino" da última pergunta, e ficaria sem sublinhado
   * até a aba fechar.
   */
  esquecerReferencias(): void {
    this.navegaveis?.esquecer();
  }

  /**
   * Acende ou apaga o indicador de carregamento do editor — o mesmo do
   * sublinhado do Ctrl. A aba o usa enquanto um Ctrl+clique procura o destino
   * (requisito 22).
   */
  indicarCarregamento(ativo: boolean): void {
    if (this.destruido || !this.navegacao) return;
    const plugin = this.visao.plugin(this.navegacao.plugin);
    if (!plugin || plugin.abrindo === ativo) return;
    plugin.abrindo = ativo;
    this.visao.dispatch({ effects: refazerSublinhado.of(null) });
  }

  /**
   * O banco e o esquema que um nome qualificado nomeia — nulo quando ele não
   * nomeia nenhum dos dois.
   *
   * `dbo.tb` é esquema do banco da aba; `Vendas..tb` é banco com o esquema
   * padrão; `Vendas.dbo.tb` são os dois. Fora do SQL Server não existe nome de
   * três partes, e o nulo é o que deixa `pacote.` seguir para os subprogramas.
   *
   * A primeira parte só vira banco quando **é** um banco do servidor. Sem essa
   * conferência, cada `x.y` digitado viraria uma consulta a `[x].sys.objects`,
   * que não existe e responde com erro em vez de lista vazia.
   */
  private async escopoDoNome(partes: string[]): Promise<EscopoDoNome | null> {
    if (!this.opcoes.sugestoes.bancos) return null;
    if (partes.length === 1) return partes[0] ? { esquema: partes[0] } : null;
    if (partes.length !== 2) return null;

    const banco = await this.bancoChamado(partes[0]);
    if (!banco) return null;
    return partes[1] ? { banco, esquema: partes[1] } : { banco };
  }

  /** O banco de nome igual a este, na grafia do servidor; nulo se não houver. */
  private async bancoChamado(nome: string): Promise<string | null> {
    const alvo = nome.toUpperCase();
    const bancos = await this.bancosDoServidor();
    return bancos.find((banco) => banco.toUpperCase() === alvo) ?? null;
  }

  /** Os bancos que começam pelo prefixo, sem diferenciar maiúsculas. */
  private async bancosQueComecamCom(prefixo: string): Promise<string[]> {
    const alvo = prefixo.toUpperCase();
    const bancos = await this.bancosDoServidor();
    return bancos.filter((banco) => banco.toUpperCase().startsWith(alvo));
  }

  /**
   * A lista de bancos do servidor, lida uma vez por aba.
   *
   * Guarda a **busca**, e não o resultado, pelo mesmo motivo das colunas: a
   * lista é consultada a cada tecla, e sem isso as primeiras teclas abririam
   * uma consulta cada uma. A lista não muda enquanto a aba existe — criar um
   * banco é raro, e a aba seguinte já nasce vendo o novo.
   *
   * O que **não** fica guardado é a falha. Uma leitura que não deu certo — a
   * sessão da conexão ocupada, a rede caindo no meio — valeria pela aba
   * inteira: sem a lista, `escopoDoNome` devolve nulo e o nome de três partes
   * some do autocomplete até a aba ser fechada, em silêncio e sem nada na tela
   * que explique. Descartada a busca falha, a tecla seguinte tenta de novo.
   */
  private bancosDoServidor(): Promise<string[]> {
    const pedir = this.opcoes.sugestoes.bancos;
    if (!pedir) return Promise.resolve([]);

    // Silêncio é melhor que erro no meio da digitação; o backend registra.
    this.bancosEmMemoria ??= Promise.resolve(pedir()).catch(() => {
      this.bancosEmMemoria = null;
      return [];
    });
    return this.bancosEmMemoria;
  }

  private listaDeBancos(bancos: string[], de: number): CompletionResult | null {
    if (bancos.length === 0) return null;
    return { from: de, options: bancos.map(paraOpcaoDeBanco), validFor: /^[\w$#]*$/ };
  }

  /** As colunas de todas as tabelas do comando, para depois de WHERE/AND/OR/`(`. */
  private async completarColunasDoComando(
    comando: string,
    palavra: { from: number; text: string },
  ): Promise<CompletionResult | null> {
    const tabelas = tabelasDoComando(comando);
    if (tabelas.length === 0) return null;

    const listas = await Promise.all(tabelas.map((tabela) => this.colunasDe(tabela)));

    const opcoes: Completion[] = [];
    listas.forEach((colunas, indice) => {
      for (const coluna of colunas) {
        opcoes.push({
          label: coluna.nome,
          // Com mais de uma tabela na consulta, saber de qual é a coluna vale
          // mais que o tipo sozinho — e duas tabelas podem ter a mesma coluna.
          // O tipo pode faltar: numa temporária ele só existe onde o script o
          // escreve, e aí sobra a tabela sozinha em vez de um `·` solto.
          detail: [coluna.tipo, tabelas.length > 1 ? tabelas[indice].tabela : ""]
            .filter((parte) => parte !== "")
            .join(" · "),
          type: "property",
        });
      }
    });
    if (opcoes.length === 0) return null;

    return { from: palavra.from, options: opcoes, validFor: /^[\w$#]*$/ };
  }

  /**
   * As colunas do comando **e** as rotinas do banco, na mesma lista.
   *
   * É o que cabe depois de um operador. Devolver só um dos dois seria escolher
   * pelo usuário: `= '2' || IDR` quer a coluna, `> SYSDATE - 2` quer a função,
   * e o que está digitado ainda não diz qual dos dois é.
   *
   * As colunas vêm com preferência porque são as do comando que está na tela;
   * as rotinas são o dicionário inteiro, e ficam atrás no desempate.
   */
  private async completarColunasERotinas(
    comando: string,
    palavra: { from: number; text: string },
  ): Promise<CompletionResult | null> {
    const [colunas, rotinas] = await Promise.all([
      this.completarColunasDoComando(comando, palavra),
      this.completarObjetos(palavra, "outro"),
    ]);
    if (!colunas) return rotinas;
    if (!rotinas) return colunas;

    return {
      ...colunas,
      options: [
        ...colunas.options.map((opcao) => ({ ...opcao, boost: 1 })),
        ...rotinas.options,
      ],
    };
  }

  /**
   * As temporárias que o script define e que começam pelo que está escrito.
   *
   * Vem com `validFor`: esta lista é completa por construção — são as do texto,
   * não um recorte de uma consulta com teto —, e filtrar localmente é sempre
   * certo.
   */
  private listaDeTemporarias(palavra: { from: number; text: string }): CompletionResult | null {
    const prefixo = palavra.text.toUpperCase();
    const nomes = temporariasDoScript(this.textoLimpoDoScript()).filter((nome) =>
      nome.toUpperCase().startsWith(prefixo),
    );
    if (nomes.length === 0) return null;

    return {
      from: palavra.from,
      options: nomes.map((nome) => ({
        label: nome,
        detail: t("especie tabela temporaria"),
        type: "class",
      })),
      validFor: /^[\w$#]*$/,
    };
  }

  private listaDeColunas(colunas: api.Coluna[], de: number): CompletionResult | null {
    if (colunas.length === 0) return null;
    return {
      from: de,
      options: colunas.map((coluna) => ({
        label: coluna.nome,
        detail: coluna.tipo,
        type: "property",
      })),
      validFor: /^[\w$#]*$/,
    };
  }

  private listaDeSubprogramas(
    subprogramas: api.Subprograma[],
    de: number,
  ): CompletionResult | null {
    if (subprogramas.length === 0) return null;
    return {
      from: de,
      options: subprogramas.map((subprograma) => ({
        label: subprograma.nome,
        detail: t(subprograma.especie === "FUNCTION" ? "especie funcao" : "especie procedure"),
        type: "function",
      })),
      validFor: /^[\w$#]*$/,
    };
  }

  /**
   * Colunas de uma tabela, lidas uma vez por aba.
   *
   * O que fica guardado é a **busca**, e não o resultado dela. A diferença
   * importa: assim, quem pedir as mesmas colunas enquanto a primeira consulta
   * ainda está no ar recebe aquela mesma consulta, em vez de abrir outra. Sem
   * isso, digitar `WHERE STATUS` dispara uma ida ao banco por tecla, porque
   * nenhuma delas termina a tempo de a seguinte encontrar algo guardado.
   *
   * A aba está amarrada a uma conexão (requisito 26), então a memória tem o
   * escopo certo. `listar_colunas` não guarda nada do lado do banco.
   *
   * Guarda também o resultado vazio: um alias que não é tabela nenhuma seria,
   * de outro modo, uma consulta perdida ao banco a cada tecla.
   */
  /**
   * Esquece as colunas guardadas.
   *
   * Trocar o banco na barra da aba (SQL Server, requisito 3) troca também as
   * tabelas: manter as colunas do banco anterior faria o autocomplete oferecer
   * o que não existe mais ali.
   */
  esquecerColunas(): void {
    this.colunasEmMemoria.clear();
  }

  private colunasDe(
    referencia: ReferenciaDeTabela,
    emResolucao: ReadonlySet<string> = new Set(),
  ): Promise<api.Coluna[]> {
    // Tabela temporária do SQL Server não está em catálogo que a sessão da
    // conexão alcance — `#t` mora em `tempdb`, com escopo da sessão que a criou,
    // e quem responde ao autocomplete é a sessão da conexão. Quem sabe as
    // colunas dela é o script; ver `definicaoDeTemporaria`. Não vai ao backend e
    // não passa pelo cache: o texto muda a cada tecla, e uma lista guardada pelo
    // nome `#t` seria a resposta certa da versão errada do script.
    if (referencia.tabela.startsWith("#")) {
      return this.colunasDaTemporaria(referencia.tabela, emResolucao);
    }

    // O banco entra na chave porque entra no nome: `Vendas..cliente` e
    // `cliente` são duas tabelas diferentes, e podem ser de bancos diferentes.
    const chave = `${referencia.banco ?? ""}.${referencia.tabela}`.toUpperCase();
    const guardadas = this.colunasEmMemoria.get(chave);
    if (guardadas) return guardadas;

    // Silêncio é melhor que um erro no meio da digitação.
    const busca = Promise.resolve(
      this.opcoes.sugestoes.colunas(referencia.tabela, referencia.banco ?? undefined),
    ).catch(() => []);
    this.colunasEmMemoria.set(chave, busca);
    return busca;
  }

  /**
   * As colunas de uma tabela temporária, lidas da definição dela no script.
   *
   * As colunas da **origem** essas sim vêm do banco, e do cache: no caso comum a
   * origem do `SELECT INTO` é uma tabela que o comando à vista já cita, e que
   * `adiantar` já mandou ler.
   *
   * `emResolucao` corta o ciclo: um script pode definir uma temporária a partir
   * de outra, e nada impede que ele escreva uma que se refira a si mesma.
   */
  private async colunasDaTemporaria(
    nome: string,
    emResolucao: ReadonlySet<string>,
  ): Promise<api.Coluna[]> {
    const chave = nome.toUpperCase();
    if (emResolucao.has(chave)) return [];

    const definicao = definicaoDeTemporaria(this.textoLimpoDoScript(), nome);
    if (!definicao) return [];

    const visitadas = new Set(emResolucao).add(chave);
    const origens = await Promise.all(
      definicao.origens.map(async (origem) => ({
        origem,
        colunas: await this.colunasDe(origem, visitadas),
      })),
    );
    return colunasDaDefinicao(definicao, origens);
  }

  /**
   * O script inteiro sem literais nem comentários, refeito só quando ele muda.
   *
   * Quem precisa dele é a resolução das temporárias, que olha o documento
   * inteiro — e não só o comando, porque `limitesDoComando` corta o `SELECT
   * INTO` fora do `UPDATE` de baixo. Como isso acontece a cada tecla, o
   * resultado fica guardado: o `doc` do CodeMirror é imutável e trocado a cada
   * alteração, então comparar a identidade dele basta para saber se ainda vale.
   */
  private textoLimpoDoScript(): string {
    const doc = this.visao.state.doc;
    if (this.scriptLimpo?.doc !== doc) {
      this.scriptLimpo = { doc, limpo: analisarSql(doc.toString(), 0).limpo };
    }
    return this.scriptLimpo.limpo;
  }

  /** Procedures e funções de uma package, lidas uma vez por aba. */
  private subprogramasDe(pacote: string): Promise<api.Subprograma[]> {
    if (!this.opcoes.sugestoes.subprogramas) return Promise.resolve([]);

    const chave = pacote.toUpperCase();
    const guardados = this.subprogramasEmMemoria.get(chave);
    if (guardados) return guardados;

    // Autocomplete não deve exibir erro no meio da digitação. A tentativa fica
    // registrada pelo comando Rust e um nome que não é package produz lista vazia.
    const busca = Promise.resolve(this.opcoes.sugestoes.subprogramas(pacote)).catch(() => []);
    this.subprogramasEmMemoria.set(chave, busca);
    return busca;
  }

  /**
   * Manda buscar as colunas das tabelas já escritas, sem esperar o resultado.
   *
   * É o que faz a lista abrir **na primeira vez**. Sem isto, a consulta ao
   * banco só começa quando a sugestão é pedida, e a lista aparece um segundo
   * depois — tarde demais para servir, quando quem digitou já desistiu e
   * apagou. O `FROM cliente` é escrito antes do `WHERE`, e esse intervalo é o
   * tempo que o banco tem para responder.
   *
   * `digitando` é a palavra colada no cursor. Ela fica de fora porque ainda
   * está sendo escrita: pedir as colunas dela mandaria ao banco cada pedaço do
   * nome da tabela, um por tecla.
   */
  private adiantar(comando: string, digitando: string): void {
    const emDigitacao = digitando.toUpperCase();
    for (const referencia of tabelasDoComando(comando)) {
      // Temporária não vai ao banco (ver `colunasDe`), mas a definição dela
      // aponta para tabelas que vão — e é aqui que elas precisam ser alcançadas,
      // porque estão noutro comando do script e ninguém mais as pede. Sem
      // definição escrita, a resolução para em lista vazia e não custa nada.
      if (referencia.tabela.startsWith("#")) {
        void this.colunasDe(referencia);
        continue;
      }

      // A comparação é com a última parte do nome: em `Vendas.dbo.tbl` o que
      // está sendo escrito é o `tbl`, e ir buscar as colunas de `dbo.tbl` a
      // cada tecla é justamente o que esta guarda existe para evitar.
      const ultima = referencia.tabela.split(".").pop() ?? referencia.tabela;
      if (ultima.toUpperCase() !== emDigitacao) void this.colunasDe(referencia);
    }
  }

  /**
   * O mesmo adiantamento, disparado pela digitação parar.
   *
   * O de dentro do `completar` cobre quem escreve sem parar; este cobre quem
   * escreve o `FROM` e para para pensar no resto, que é quando há mais tempo a
   * ganhar. Só roda depois da pausa, então não custa nada por tecla.
   */
  private adiantarColunas(): void {
    window.clearTimeout(this.adiantamento);
    this.adiantamento = window.setTimeout(() => {
      const posicao = this.visao.state.selection.main.head;
      const analise = analisarSql(this.visao.state.doc.toString(), posicao);
      if (analise.dentroDeLiteral) return;

      const [inicio, fim] = limitesDoComando(analise.limpo, posicao);
      const ateOCursor = analise.limpo.slice(inicio, posicao);
      this.adiantar(analise.limpo.slice(inicio, fim), /[\w$#]*$/.exec(ateOCursor)?.[0] ?? "");
    }, PAUSA_PARA_ADIANTAR);
  }
}

/** Quanto tempo de digitação parada dispara a leitura antecipada das colunas. */
const PAUSA_PARA_ADIANTAR = 250;

/**
 * Destaque de sintaxe para o tema escuro — mesmos grupos de tag do
 * `defaultHighlightStyle` do CodeMirror, com cores que se leem sobre fundo
 * escuro em vez das pensadas para fundo branco.
 */
const DESTAQUE_ESCURO = HighlightStyle.define([
  { tag: tags.meta, color: "#9aa79a" },
  { tag: tags.link, textDecoration: "underline" },
  { tag: tags.heading, textDecoration: "underline", fontWeight: "bold" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strong, fontWeight: "bold" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.keyword, color: "#c792ea" },
  { tag: [tags.atom, tags.bool, tags.url, tags.contentSeparator, tags.labelName], color: "#569cd6" },
  { tag: [tags.literal, tags.inserted], color: "#b5cea8" },
  { tag: [tags.string, tags.deleted], color: "#ce9178" },
  { tag: [tags.regexp, tags.escape, tags.special(tags.string)], color: "#d7ba7d" },
  { tag: tags.definition(tags.variableName), color: "#9cdcfe" },
  { tag: tags.local(tags.variableName), color: "#c792ea" },
  { tag: [tags.typeName, tags.namespace], color: "#4ec9b0" },
  { tag: tags.className, color: "#4ec9b0" },
  { tag: [tags.special(tags.variableName), tags.macroName], color: "#9cdcfe" },
  { tag: tags.definition(tags.propertyName), color: "#9cdcfe" },
  { tag: tags.comment, color: "#6a9955" },
  { tag: tags.invalid, color: "#f14c4c" },
]);

/** O que conta como "tabela" para a regra de onde sugerir. */
const TIPOS_DE_TABELA = new Set(["TABLE", "VIEW", "MATERIALIZED VIEW", "SYNONYM"]);

/**
 * O realce padrão do CodeMirror trata toda faixa da seleção múltipla como
 * uma linha ativa. Numa seleção em coluna isso pinta linhas inteiras e esconde
 * visualmente o retângulo. Mantemos o realce somente na faixa principal, como
 * fazem editores de seleção retangular como o Notepad++.
 */
const decoracaoDaLinhaPrincipal = Decoration.line({ class: "cm-activeLine" });

const destacarLinhaPrincipal = ViewPlugin.fromClass(
  class {
    decorations;

    constructor(visao: EditorView) {
      this.decorations = decoracaoDaLinhaPrincipalNa(visao);
    }

    update(atualizacao: { docChanged: boolean; selectionSet: boolean; view: EditorView }): void {
      if (atualizacao.docChanged || atualizacao.selectionSet) {
        this.decorations = decoracaoDaLinhaPrincipalNa(atualizacao.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

function decoracaoDaLinhaPrincipalNa(visao: EditorView) {
  const inicio = visao.state.doc.lineAt(visao.state.selection.main.head).from;
  return Decoration.set([decoracaoDaLinhaPrincipal.range(inicio)]);
}

class MarcadorDaLinhaPrincipal extends GutterMarker {
  elementClass = "cm-activeLineGutter";
}

const marcadorDaLinhaPrincipal = new MarcadorDaLinhaPrincipal();

const destacarLinhaPrincipalNoGutter = gutterLineClass.compute(
  ["doc", "selection"],
  (estado) => {
    const inicio = estado.doc.lineAt(estado.selection.main.head).from;
    return RangeSet.of([marcadorDaLinhaPrincipal.range(inicio)]);
  },
);

/**
 * Estende a seleção em coluna uma linha para baixo ou para cima, mantendo as
 * mesmas colunas em todas as linhas — o que o Notepad++ faz com Alt+Shift+setas.
 *
 * O `addCursorBelow` do CodeMirror não serve aqui: ele usa `moveVertically`, que
 * devolve **cursor sem largura**. O resultado é uma pilha de carets em que só o
 * intervalo original aparece pintado, e não um bloco selecionado.
 *
 * A seta contrária ao crescimento encolhe o bloco, também como no Notepad++. Dá
 * para saber o sentido do crescimento pelo índice da faixa principal: ela é a
 * última quando o bloco cresceu para baixo, e a primeira quando cresceu para
 * cima.
 */
function estenderEmColuna(paraBaixo: boolean): Command {
  return (visao) => {
    const estado = visao.state;
    const faixas = estado.selection.ranges;
    const principal = estado.selection.mainIndex;
    const cresceuParaBaixo = faixas.length > 1 && principal === faixas.length - 1;
    const cresceuParaCima = faixas.length > 1 && principal === 0;

    if (paraBaixo && cresceuParaCima) {
      return aplicar(visao, faixas.slice(1), 0);
    }
    if (!paraBaixo && cresceuParaBaixo) {
      const restantes = faixas.slice(0, -1);
      return aplicar(visao, restantes, restantes.length - 1);
    }

    // As colunas do bloco saem sempre da primeira faixa: usar a faixa da borda
    // faria o bloco encolher de largura ao passar por uma linha curta.
    const origem = faixas[0];
    const linhaDaOrigem = estado.doc.lineAt(origem.from);
    const colunaDe = origem.from - linhaDaOrigem.from;
    const colunaAte = origem.to - linhaDaOrigem.from;

    const borda = paraBaixo ? faixas[faixas.length - 1] : faixas[0];
    const numero = estado.doc.lineAt(borda.head).number + (paraBaixo ? 1 : -1);
    // Na borda do documento o bloco não cresce, mas a tecla tem de ser
    // consumida: devolver `false` aqui deixa o `defaultKeymap` assumir e
    // duplicar a linha com `copyLineUp`/`copyLineDown`.
    if (numero < 1 || numero > estado.doc.lines) return true;

    // Linha mais curta que as colunas do bloco: o intervalo para no fim dela.
    const destino = estado.doc.line(numero);
    const nova = EditorSelection.range(
      Math.min(destino.from + colunaDe, destino.to),
      Math.min(destino.from + colunaAte, destino.to),
    );

    return paraBaixo
      ? aplicar(visao, [...faixas, nova], faixas.length)
      : aplicar(visao, [nova, ...faixas], 0);
  };
}

function aplicar(visao: EditorView, faixas: readonly SelectionRange[], principal: number): boolean {
  visao.dispatch({
    selection: EditorSelection.create([...faixas], principal),
    scrollIntoView: true,
    userEvent: "select",
  });
  return true;
}

/** Duplica a seleção; sem seleção, duplica a linha atual para baixo. */
const duplicarLinhaOuSelecao: Command = (visao) => {
  const estado = visao.state;
  if (estado.selection.ranges.every((faixa) => faixa.empty)) return copyLineDown(visao);

  const alteracao = estado.changeByRange((faixa) => {
    if (faixa.empty) return { range: faixa };
    const texto = estado.sliceDoc(faixa.from, faixa.to);
    return {
      changes: { from: faixa.to, insert: texto },
      // Como no Notepad++, a cópia recém-criada continua selecionada.
      range: EditorSelection.range(faixa.to, faixa.to + texto.length),
    };
  });
  visao.dispatch({ ...alteracao, scrollIntoView: true, userEvent: "input" });
  return true;
};

/**
 * Converte as quebras visuais das linhas alcançadas pela seleção em quebras
 * reais, que é o comportamento de "Dividir linhas" do Notepad++.
 *
 * O editor usa fonte monoespaçada, mas medir as coordenadas reais preserva o
 * ponto exato escolhido pelo navegador para quebrar palavras e pontuação.
 */
/** Insere `.` e reinicia a conclusão no contexto qualificado (`PKG.`/`alias.`). */
function inserirPontoEAbrirSugestoes(visao: EditorView): boolean {
  visao.dispatch(visao.state.replaceSelection("."), {
    scrollIntoView: true,
    userEvent: "input.type",
  });
  window.setTimeout(() => {
    closeCompletion(visao);
    startCompletion(visao);
  }, 120);
  return true;
}

const dividirLinhasLongas: Command = (visao) => {
  const estado = visao.state;
  const numeros = new Set<number>();
  for (const faixa of estado.selection.ranges) {
    const primeira = estado.doc.lineAt(faixa.from).number;
    const fim = faixa.empty ? faixa.to : Math.max(faixa.from, faixa.to - 1);
    const ultima = estado.doc.lineAt(fim).number;
    for (let numero = primeira; numero <= ultima; numero += 1) numeros.add(numero);
  }

  const quebras: number[] = [];
  for (const numero of numeros) {
    const linha = estado.doc.line(numero);
    let coordenadaAnterior = visao.coordsAtPos(linha.from, 1);
    if (!coordenadaAnterior) continue;
    for (let posicao = linha.from + 1; posicao <= linha.to; posicao += 1) {
      const coordenada = visao.coordsAtPos(posicao, 1);
      if (!coordenada) continue;
      if (coordenada.top > coordenadaAnterior.top + 1) quebras.push(posicao);
      coordenadaAnterior = coordenada;
    }
  }

  if (quebras.length === 0) return true;
  visao.dispatch({
    changes: quebras.map((from) => ({ from, insert: "\n" })),
    scrollIntoView: true,
    userEvent: "input",
  });
  return true;
};

/** Junta as linhas selecionadas; sem seleção, junta a atual com a próxima. */
const juntarLinhas: Command = (visao) => {
  const estado = visao.state;
  let mudou = false;
  const alteracao = estado.changeByRange((faixa) => {
    const primeira = estado.doc.lineAt(faixa.from);
    let ultima = estado.doc.lineAt(faixa.to);
    if (faixa.empty) {
      if (primeira.number === estado.doc.lines) return { range: faixa };
      ultima = estado.doc.line(primeira.number + 1);
    }
    if (primeira.number === ultima.number) return { range: faixa };

    const original = estado.sliceDoc(primeira.from, ultima.to);
    const unido = original.replace(/\s*\n\s*/g, " ");
    mudou = true;
    return {
      changes: { from: primeira.from, to: ultima.to, insert: unido },
      range: EditorSelection.range(primeira.from, primeira.from + unido.length),
    };
  });
  if (!mudou) return true;
  visao.dispatch({ ...alteracao, scrollIntoView: true, userEvent: "input" });
  return true;
};

/** Converte somente as seleções, preservando cursores sem texto selecionado. */
function alterarCaixa(paraMaiusculas: boolean): Command {
  return (visao) => {
    const estado = visao.state;
    let mudou = false;
    const alteracao = estado.changeByRange((faixa) => {
      if (faixa.empty) return { range: faixa };
      const original = estado.sliceDoc(faixa.from, faixa.to);
      const convertido = paraMaiusculas ? original.toLocaleUpperCase() : original.toLocaleLowerCase();
      if (convertido === original) return { range: faixa };
      mudou = true;
      return {
        changes: { from: faixa.from, to: faixa.to, insert: convertido },
        range: EditorSelection.range(faixa.from, faixa.from + convertido.length),
      };
    });
    if (!mudou) return true;
    visao.dispatch({ ...alteracao, scrollIntoView: true, userEvent: "input" });
    return true;
  };
}

function paraOpcao(sugestao: api.Sugestao): Completion {
  // O dono aparece quando o objeto é de outro schema — é o que distingue duas
  // tabelas de mesmo nome em donos diferentes.
  const detalhe = sugestao.dono
    ? `${rotuloDoTipo(sugestao.tipo)} · ${sugestao.dono}`
    : rotuloDoTipo(sugestao.tipo);
  return {
    label: sugestao.nome,
    detail: detalhe,
    type: sugestao.tipo === "TABLE" || sugestao.tipo === "SYNONYM" ? "class" : "function",
  };
}

/** O banco entra na lista com o mesmo rótulo que a árvore dá a ele. */
function paraOpcaoDeBanco(banco: string): Completion {
  return { label: banco, detail: t("banco de dados singular"), type: "namespace" };
}

function rotuloDoTipo(tipo: string): string {
  switch (tipo) {
    case "TABLE":
      return t("especie tabela");
    case "VIEW":
      return t("especie view");
    case "MATERIALIZED VIEW":
      return t("especie view materializada");
    case "SYNONYM":
      return t("especie sinonimo");
    case "PACKAGE":
      return t("especie pacote");
    case "PROCEDURE":
      return t("especie procedure");
    case "FUNCTION":
      return t("especie funcao");
    case "SEQUENCE":
      return t("especie sequencia");
    case "TYPE":
      return t("especie tipo");
    default:
      return tipo.toLowerCase();
  }
}

// --- Sublinhado do Ctrl (requisito 22) --------------------------------------

/**
 * Quanto o Ctrl precisa ficar apertado sozinho para os nomes serem
 * sublinhados.
 *
 * Sem espera, todo `Ctrl+C`, `Ctrl+V` e `Ctrl+Enter` piscaria o sublinhado na
 * tela. Num atalho a outra tecla vem bem antes disto; quem segura o Ctrl para
 * navegar não chega a notar a espera. Mexer o mouse com o Ctrl apertado a
 * dispensa, porque aí a intenção já está clara.
 *
 * A espera é só do desenho. A pergunta ao banco sai no instante em que o Ctrl
 * desce: ela custa décimos de segundo, só vai pelo que a aba ainda não sabe, e
 * quando o sublinhado for pedido a resposta já está a caminho — ou chegou.
 */
const ESPERA_DO_CTRL = 200;

/**
 * Quanto a resposta pode demorar, com o sublinhado já pedido, antes de o
 * editor mostrar que está carregando. Resposta mais rápida que isto não pisca
 * indicador nenhum.
 */
const ESPERA_DO_CARREGAMENTO = 120;

/** Quantos nomes vão numa pergunta ao banco. */
const NOMES_POR_PERGUNTA = 200;

/**
 * Quanto esperar, depois de uma pergunta que falhou, antes de fazer outra.
 * Sem isto, com a rede fora, cada Ctrl apertado seria uma tentativa de login.
 */
const PAUSA_APOS_FALHA = 30_000;

/**
 * As palavras reservadas do SQL do Oracle (`V$RESERVED_WORDS` com `RESERVED =
 * 'Y'`).
 *
 * Nenhuma delas pode ser nome de objeto sem aspas, então nem vão ao banco: a
 * resposta — sem destino — é certa de antemão. Palavra-chave **não** reservada
 * fica de fora de propósito: `STATUS`, `LOG` e `DATA` são nomes de tabela
 * comuns.
 *
 * Cada banco tem a sua lista, e não é firula: `FILE`, `SESSION`, `COMMENT` e
 * `NUMBER` são reservadas no Oracle e nomes de tabela perfeitamente possíveis
 * no SQL Server — usar a lista errada esconderia o sublinhado desses nomes.
 */
const RESERVADAS_DO_ORACLE = new Set([
  "ACCESS", "ADD", "ALL", "ALTER", "AND", "ANY", "AS", "ASC", "AUDIT", "BETWEEN", "BY",
  "CHAR", "CHECK", "CLUSTER", "COLUMN", "COMMENT", "COMPRESS", "CONNECT", "CREATE",
  "CURRENT", "DATE", "DECIMAL", "DEFAULT", "DELETE", "DESC", "DISTINCT", "DROP", "ELSE",
  "EXCLUSIVE", "EXISTS", "FILE", "FLOAT", "FOR", "FROM", "GRANT", "GROUP", "HAVING",
  "IDENTIFIED", "IMMEDIATE", "IN", "INCREMENT", "INDEX", "INITIAL", "INSERT", "INTEGER",
  "INTERSECT", "INTO", "IS", "LEVEL", "LIKE", "LOCK", "LONG", "MAXEXTENTS", "MINUS",
  "MLSLABEL", "MODE", "MODIFY", "NOAUDIT", "NOCOMPRESS", "NOT", "NOWAIT", "NULL", "NUMBER",
  "OF", "OFFLINE", "ON", "ONLINE", "OPTION", "OR", "ORDER", "PCTFREE", "PRIOR", "PUBLIC",
  "RAW", "RENAME", "RESOURCE", "REVOKE", "ROW", "ROWID", "ROWNUM", "ROWS", "SELECT",
  "SESSION", "SET", "SHARE", "SIZE", "SMALLINT", "START", "SUCCESSFUL", "SYNONYM",
  "SYSDATE", "TABLE", "THEN", "TO", "TRIGGER", "UID", "UNION", "UNIQUE", "UPDATE", "USER",
  "VALIDATE", "VALUES", "VARCHAR", "VARCHAR2", "VIEW", "WHENEVER", "WHERE", "WITH",
]);

/** As palavras reservadas do T-SQL, pelo mesmo motivo da lista do Oracle. */
const RESERVADAS_DO_SQL_SERVER = new Set([
  "ADD", "ALL", "ALTER", "AND", "ANY", "AS", "ASC", "AUTHORIZATION", "BACKUP", "BEGIN",
  "BETWEEN", "BREAK", "BROWSE", "BULK", "BY", "CASCADE", "CASE", "CHECK", "CHECKPOINT",
  "CLOSE", "CLUSTERED", "COALESCE", "COLLATE", "COLUMN", "COMMIT", "COMPUTE", "CONSTRAINT",
  "CONTAINS", "CONTAINSTABLE", "CONTINUE", "CONVERT", "CREATE", "CROSS", "CURRENT",
  "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "CURRENT_USER", "CURSOR", "DATABASE",
  "DBCC", "DEALLOCATE", "DECLARE", "DEFAULT", "DELETE", "DENY", "DESC", "DISK", "DISTINCT",
  "DISTRIBUTED", "DOUBLE", "DROP", "DUMP", "ELSE", "END", "ERRLVL", "ESCAPE", "EXCEPT",
  "EXEC", "EXECUTE", "EXISTS", "EXIT", "EXTERNAL", "FETCH", "FILE", "FILLFACTOR", "FOR",
  "FOREIGN", "FREETEXT", "FREETEXTTABLE", "FROM", "FULL", "FUNCTION", "GOTO", "GRANT",
  "GROUP", "HAVING", "HOLDLOCK", "IDENTITY", "IDENTITYCOL", "IDENTITY_INSERT", "IF", "IN",
  "INDEX", "INNER", "INSERT", "INTERSECT", "INTO", "IS", "JOIN", "KEY", "KILL", "LEFT",
  "LIKE", "LINENO", "LOAD", "MERGE", "NATIONAL", "NOCHECK", "NONCLUSTERED", "NOT", "NULL",
  "NULLIF", "OF", "OFF", "OFFSETS", "ON", "OPEN", "OPENDATASOURCE", "OPENQUERY",
  "OPENROWSET", "OPENXML", "OPTION", "OR", "ORDER", "OUTER", "OVER", "PERCENT", "PIVOT",
  "PLAN", "PRECISION", "PRIMARY", "PRINT", "PROC", "PROCEDURE", "PUBLIC", "RAISERROR",
  "READ", "READTEXT", "RECONFIGURE", "REFERENCES", "REPLICATION", "RESTORE", "RESTRICT",
  "RETURN", "REVERT", "REVOKE", "RIGHT", "ROLLBACK", "ROWCOUNT", "ROWGUIDCOL", "RULE",
  "SAVE", "SCHEMA", "SECURITYAUDIT", "SELECT", "SEMANTICKEYPHRASETABLE",
  "SEMANTICSIMILARITYDETAILSTABLE", "SEMANTICSIMILARITYTABLE", "SESSION_USER", "SET",
  "SETUSER", "SHUTDOWN", "SOME", "STATISTICS", "SYSTEM_USER", "TABLE", "TABLESAMPLE",
  "TEXTSIZE", "THEN", "TO", "TOP", "TRAN", "TRANSACTION", "TRIGGER", "TRUNCATE",
  "TRY_CONVERT", "TSEQUAL", "UNION", "UNIQUE", "UNPIVOT", "UPDATE", "UPDATETEXT", "USE",
  "USER", "VALUES", "VARYING", "VIEW", "WAITFOR", "WHEN", "WHERE", "WHILE", "WITH",
  "WRITETEXT",
]);

/**
 * O que o editor já sabe sobre quais nomes o Ctrl+clique abre.
 *
 * A resposta de cada nome fica guardada pela vida da aba: o dicionário não muda
 * de um Ctrl para o outro, e o Ctrl é apertado o tempo todo. Só se pergunta
 * pelo que ainda não se sabe, em lotes, e um lote por vez.
 */
class ReferenciasNavegaveis {
  /** Nome em maiúsculas, como o Oracle o resolve → leva ou não a algum lugar. */
  private readonly conhecidas = new Map<string, boolean>();
  /** Nomes na fila ou já perguntados, ainda sem resposta. */
  private readonly pendentes = new Set<string>();
  private fila: string[] = [];
  private perguntando = false;
  /** Muda a cada `esquecer`: a resposta de antes dele não vale mais. */
  private geracao = 0;
  private pausadoAte = 0;

  constructor(
    private readonly perguntar: (referencias: string[]) => Promise<string[]>,
    /** Chamado quando chega resposta, quando a pergunta falha e ao esquecer. */
    private readonly aoMudar: () => void,
  ) {}

  /** `undefined` enquanto o banco não respondeu sobre o nome. */
  situacao(chave: string): boolean | undefined {
    return this.conhecidas.get(chave);
  }

  /** Há nome perguntado e ainda sem resposta. */
  get temPendentes(): boolean {
    return this.pendentes.size > 0;
  }

  /** Pergunta pelos nomes de que ainda não se sabe nada. */
  pedir(chaves: Iterable<string>): void {
    if (Date.now() < this.pausadoAte) return;
    for (const chave of chaves) {
      if (this.conhecidas.has(chave) || this.pendentes.has(chave)) continue;
      this.pendentes.add(chave);
      this.fila.push(chave);
    }
    void this.perguntarAFila();
  }

  esquecer(): void {
    this.conhecidas.clear();
    this.pendentes.clear();
    this.fila = [];
    this.pausadoAte = 0;
    this.geracao += 1;
    this.aoMudar();
  }

  private async perguntarAFila(): Promise<void> {
    if (this.perguntando) return;
    this.perguntando = true;
    try {
      while (this.fila.length > 0) {
        const geracao = this.geracao;
        const lote = this.fila.splice(0, NOMES_POR_PERGUNTA);
        let achadas: Set<string>;
        try {
          achadas = new Set(await this.perguntar(lote));
        } catch {
          if (geracao !== this.geracao) continue;
          // O backend registra o motivo. Sem resposta, o nome só fica sem
          // sublinhado: o Ctrl+clique continua perguntando por conta própria.
          for (const chave of [...lote, ...this.fila]) this.pendentes.delete(chave);
          this.fila = [];
          this.pausadoAte = Date.now() + PAUSA_APOS_FALHA;
          // Sem isto o indicador de carregamento ficaria girando para sempre.
          this.aoMudar();
          return;
        }
        if (geracao !== this.geracao) continue;
        for (const chave of lote) {
          this.conhecidas.set(chave, achadas.has(chave));
          this.pendentes.delete(chave);
        }
        this.aoMudar();
      }
    } finally {
      this.perguntando = false;
    }
  }
}

/** Pede para o sublinhado ser refeito, sem mudar mais nada no editor. */
const refazerSublinhado = StateEffect.define<null>();

const marcaDeReferencia = Decoration.mark({ class: "cm-referencia-navegavel" });

function sublinhadoDeNavegacao(
  referencias: ReferenciasNavegaveis,
  reservadas: ReadonlySet<string>,
): {
  plugin: ViewPlugin<SublinhadoDeNavegacao>;
  extensao: Extension;
} {
  const plugin = ViewPlugin.define(
    (visao) => new SublinhadoDeNavegacao(visao, referencias, reservadas),
    { decorations: (valor) => valor.decorations },
  );
  return {
    plugin,
    extensao: [
      plugin,
      // A classe vai pelo facet, e não direto no elemento: o CodeMirror
      // reescreve o `class` do editor a cada troca de foco, e a levaria junto.
      EditorView.editorAttributes.of((visao) =>
        visao.plugin(plugin)?.carregando ? { class: "cm-navegacao-carregando" } : null,
      ),
    ],
  };
}

/**
 * Sublinha, com o Ctrl apertado, os nomes que o Ctrl+clique abre.
 *
 * Só os visíveis, e só os que o banco confirmou: palavra reservada, coluna,
 * apelido, texto entre aspas e comentário ficam como estão. A mãozinha vem do
 * CSS da própria marca, então fora dela continua o cursor de texto.
 *
 * Enquanto a resposta não chega, o editor diz que está carregando — uma linha
 * que corre no topo e o cursor de "trabalhando" sobre o texto (ver
 * `editor-sql.css`). Sem isso, o Ctrl apertado sem sublinhado nenhum não se
 * distinguia de "não há nada para abrir aqui". O mesmo indicador vale enquanto
 * um Ctrl+clique procura o destino.
 */
class SublinhadoDeNavegacao implements PluginValue {
  decorations: DecorationSet = Decoration.none;
  /** O indicador de carregamento está à vista. */
  carregando = false;
  /** Um Ctrl+clique está procurando o destino (ver `EditorSql.indicarCarregamento`). */
  abrindo = false;
  private ligado = false;
  /** Outra tecla veio junto com este Ctrl: é atalho, e não navegação. */
  private emAtalho = false;
  private espera: number | undefined;
  private esperaDoCarregamento: number | undefined;
  /** O texto sem literais nem comentários, enquanto o documento não muda. */
  private analise: { doc: Text; limpo: string } | null = null;

  constructor(
    private readonly visao: EditorView,
    private readonly referencias: ReferenciasNavegaveis,
    /** As palavras que este banco reserva, e que por isso não vão a ele. */
    private readonly reservadas: ReadonlySet<string>,
  ) {
    visao.dom.addEventListener("mousemove", this.aoMoverOMouse);
    visao.dom.addEventListener("mouseleave", this.desligar);
    // As teclas vêm da janela, e não do editor: o Ctrl vale também com o foco
    // no grid ou na árvore, desde que o ponteiro esteja sobre este editor.
    window.addEventListener("keydown", this.aoApertarTecla, true);
    window.addEventListener("keyup", this.aoSoltarTecla, true);
    window.addEventListener("blur", this.aoPerderAJanela);
  }

  update(atualizacao: ViewUpdate): void {
    const pedido = atualizacao.transactions.some((transacao) =>
      transacao.effects.some((efeito) => efeito.is(refazerSublinhado)),
    );
    const mudouOQueSeVe = atualizacao.docChanged || atualizacao.viewportChanged;
    if (pedido || (this.ligado && mudouOQueSeVe)) {
      this.decorations = this.ligado ? this.sublinhar(atualizacao.view) : Decoration.none;
      this.conferirCarregamento();
    }
  }

  destroy(): void {
    window.clearTimeout(this.espera);
    window.clearTimeout(this.esperaDoCarregamento);
    this.visao.dom.removeEventListener("mousemove", this.aoMoverOMouse);
    this.visao.dom.removeEventListener("mouseleave", this.desligar);
    window.removeEventListener("keydown", this.aoApertarTecla, true);
    window.removeEventListener("keyup", this.aoSoltarTecla, true);
    window.removeEventListener("blur", this.aoPerderAJanela);
  }

  private readonly aoMoverOMouse = (evento: MouseEvent): void => {
    // Ctrl solto com a janela sem foco não chega como `keyup`; o mouse conta.
    if (!evento.ctrlKey) this.emAtalho = false;
    const soCtrl = evento.ctrlKey && !evento.altKey && !evento.shiftKey && !evento.metaKey;
    if (soCtrl && !this.emAtalho) this.ligar();
    else this.desligar();
  };

  private readonly aoApertarTecla = (evento: KeyboardEvent): void => {
    if (evento.key !== "Control") {
      // Outra tecla com o Ctrl apertado é atalho — Ctrl+C, Ctrl+Enter —, e o
      // sublinhado sai até o Ctrl ser solto. O AltGr do teclado ABNT2 também
      // chega assim, como Ctrl seguido de outra tecla.
      if (evento.ctrlKey) this.emAtalho = true;
      this.desligar();
      return;
    }
    if (evento.repeat) return;
    this.emAtalho = false;
    if (evento.altKey || evento.shiftKey || evento.metaKey) return;
    // Só sublinha o editor que está sob o ponteiro: o de uma aba escondida, ou
    // coberto por uma janela, não.
    if (!this.visao.dom.matches(":hover")) return;
    this.perguntarPelosVisiveis();
    window.clearTimeout(this.espera);
    this.espera = window.setTimeout(() => {
      this.espera = undefined;
      this.ligar();
    }, ESPERA_DO_CTRL);
  };

  private readonly aoSoltarTecla = (evento: KeyboardEvent): void => {
    if (evento.key !== "Control") return;
    this.emAtalho = false;
    this.desligar();
  };

  private readonly aoPerderAJanela = (): void => {
    this.emAtalho = false;
    this.desligar();
  };

  private ligar(): void {
    window.clearTimeout(this.espera);
    this.espera = undefined;
    if (this.ligado) return;
    this.ligado = true;
    this.visao.dispatch({ effects: refazerSublinhado.of(null) });
  }

  private readonly desligar = (): void => {
    window.clearTimeout(this.espera);
    this.espera = undefined;
    if (!this.ligado) return;
    this.ligado = false;
    this.visao.dispatch({ effects: refazerSublinhado.of(null) });
  };

  /**
   * Mostra o indicador quando o sublinhado está pedido e a resposta demora, ou
   * quando um Ctrl+clique demora a achar o destino.
   *
   * Chamado dentro da atualização do editor, e por isso não despacha nada: o
   * atributo que o indicador usa é lido logo depois, na mesma atualização. Só
   * o temporizador despacha, e já fora dela.
   */
  private conferirCarregamento(): void {
    if (!this.esperando) {
      window.clearTimeout(this.esperaDoCarregamento);
      this.esperaDoCarregamento = undefined;
      this.carregando = false;
      return;
    }
    if (this.carregando || this.esperaDoCarregamento !== undefined) return;
    this.esperaDoCarregamento = window.setTimeout(() => {
      this.esperaDoCarregamento = undefined;
      if (!this.esperando) return;
      this.carregando = true;
      this.visao.dispatch({ effects: refazerSublinhado.of(null) });
    }, ESPERA_DO_CARREGAMENTO);
  }

  /** Há algo em andamento que merece o indicador. */
  private get esperando(): boolean {
    return this.abrindo || (this.ligado && this.referencias.temPendentes);
  }

  /** Pergunta pelos nomes visíveis de que a aba ainda não sabe nada. */
  private perguntarPelosVisiveis(): void {
    const desconhecidas = new Set<string>();
    for (const achada of this.nomesVisiveis(this.visao)) {
      const chave = achada.referencia.toUpperCase();
      if (this.referencias.situacao(chave) === undefined) desconhecidas.add(chave);
    }
    if (desconhecidas.size > 0) this.referencias.pedir(desconhecidas);
  }

  private sublinhar(visao: EditorView): DecorationSet {
    const marcas: Range<Decoration>[] = [];
    const desconhecidas = new Set<string>();
    for (const achada of this.nomesVisiveis(visao)) {
      const chave = achada.referencia.toUpperCase();
      const situacao = this.referencias.situacao(chave);
      if (situacao === undefined) desconhecidas.add(chave);
      else if (situacao) marcas.push(marcaDeReferencia.range(achada.de, achada.ate));
    }
    // O nome ainda sem resposta fica sem sublinhado até ela chegar, e ela pede
    // outro desenho. Sublinhar antes seria prometer sobre coluna e apelido.
    if (desconhecidas.size > 0) this.referencias.pedir(desconhecidas);
    return Decoration.set(marcas, true);
  }

  /** Os nomes da área visível que podem ter destino — sem as palavras reservadas. */
  private nomesVisiveis(visao: EditorView): TrechoDeReferencia[] {
    const doc = visao.state.doc;
    if (this.analise === null || this.analise.doc !== doc) {
      this.analise = { doc, limpo: analisarSql(doc.toString(), 0).limpo };
    }
    const { limpo } = this.analise;

    const nomes: TrechoDeReferencia[] = [];
    for (const { from, to } of visao.visibleRanges) {
      for (const achada of referenciasNoTrecho(limpo, from, to)) {
        if (!this.reservadas.has(achada.referencia.toUpperCase())) nomes.push(achada);
      }
    }
    return nomes;
  }
}

// --- Onde o cursor está dentro do SQL ---------------------------------------
//
// As três regras de sugestão dependem de saber o que veio antes do cursor, e
// isso só é confiável depois de apagar do texto o que não é código. Um `FROM`
// escrito dentro de um texto entre aspas não abre lugar de tabela, e uma aspa
// solta dentro de um comentário não pode fazer o resto do arquivo parecer
// literal. Por isso a análise vem primeiro e todo o resto trabalha sobre ela.

export interface AnaliseDoSql {
  /**
   * O SQL com literais e comentários trocados por espaços, preservando o
   * comprimento — assim as posições continuam valendo no texto original.
   */
  limpo: string;
  /** O cursor caiu dentro de um texto entre aspas ou de um comentário. */
  dentroDeLiteral: boolean;
}

/**
 * Apaga literais e comentários, e diz se a posição caiu dentro de um deles.
 *
 * Trata `''` como aspas escapada, que é como o Oracle escreve uma aspa dentro
 * do texto. A forma alternativa `q'[…]'` não é reconhecida: é rara e o pior que
 * acontece é a sugestão aparecer onde não devia, dentro dela.
 */
export function analisarSql(sql: string, posicao: number): AnaliseDoSql {
  const limpo = sql.split("");
  let estado: "codigo" | "texto" | "comentario" | "bloco" = "codigo";
  let dentroDeLiteral = false;

  for (let i = 0; i < sql.length; i += 1) {
    // O estado aqui é o que vale *antes* de consumir este caractere, que é
    // exatamente o estado de um cursor parado nesta posição.
    if (i === posicao) dentroDeLiteral = estado !== "codigo";

    const atual = sql[i];
    const proximo = sql[i + 1];

    switch (estado) {
      case "codigo":
        if (atual === "'") {
          estado = "texto";
          limpo[i] = " ";
        } else if (atual === "-" && proximo === "-") {
          estado = "comentario";
          limpo[i] = " ";
        } else if (atual === "/" && proximo === "*") {
          estado = "bloco";
          limpo[i] = " ";
        }
        break;

      case "texto":
        limpo[i] = " ";
        if (atual === "'") {
          if (proximo === "'") {
            // Aspa escapada: as duas somem e o texto continua.
            limpo[i + 1] = " ";
            i += 1;
          } else {
            estado = "codigo";
          }
        }
        break;

      case "comentario":
        if (atual === "\n") estado = "codigo";
        else limpo[i] = " ";
        break;

      case "bloco":
        limpo[i] = " ";
        if (atual === "*" && proximo === "/") {
          limpo[i + 1] = " ";
          i += 1;
          estado = "codigo";
        }
        break;
    }
  }

  // Cursor no fim do texto: vale o estado em que a varredura terminou.
  if (posicao >= sql.length) dentroDeLiteral = estado !== "codigo";

  return { limpo: limpo.join(""), dentroDeLiteral };
}

/**
 * Identificador simples ou qualificado sob o mouse.
 *
 * O texto limpo preserva as posições e troca literal/comentário por espaços;
 * por isso o mesmo extrator que aceita `PKG.PROC` recusa essas formas dentro
 * de `'texto'`, comentário de linha e comentário de bloco.
 */
export function identificadorNaPosicao(sql: string, posicao: number): string | null {
  const limpo = analisarSql(sql, posicao).limpo;

  let ponto = Math.min(Math.max(posicao, 0), limpo.length);
  if (!ehParteDeNome(limpo[ponto]) && ehParteDeNome(limpo[ponto - 1])) ponto -= 1;
  if (!ehParteDeNome(limpo[ponto])) return null;

  let inicio = ponto;
  let fim = ponto + 1;
  while (inicio > 0 && ehParteDeNome(limpo[inicio - 1])) inicio -= 1;
  while (fim < limpo.length && ehParteDeNome(limpo[fim])) fim += 1;

  const identificador = limpo.slice(inicio, fim);
  return ehNomeDeObjeto(identificador) ? identificador : null;
}

/** Um nome que o Ctrl+clique tentaria abrir, e onde ele está no texto. */
export interface TrechoDeReferencia {
  de: number;
  ate: number;
  referencia: string;
}

/**
 * Os nomes que o Ctrl+clique tentaria abrir entre `de` e `ate`.
 *
 * `limpo` é o texto já passado por `analisarSql`. O que conta como nome é o
 * mesmo de `identificadorNaPosicao`, e é isso que faz o sublinhado cobrir
 * exatamente o que o clique ali resolveria: `PKG.PROC` inteiro, e nada dentro
 * de aspas ou de comentário.
 */
export function referenciasNoTrecho(limpo: string, de: number, ate: number): TrechoDeReferencia[] {
  let inicio = Math.min(Math.max(de, 0), limpo.length);
  let fim = Math.min(Math.max(ate, inicio), limpo.length);
  // Um nome cortado pela borda do trecho entra inteiro, e não pela metade.
  if (ehParteDeNome(limpo[inicio])) {
    while (inicio > 0 && ehParteDeNome(limpo[inicio - 1])) inicio -= 1;
  }
  if (ehParteDeNome(limpo[fim - 1])) {
    while (fim < limpo.length && ehParteDeNome(limpo[fim])) fim += 1;
  }

  const achadas: TrechoDeReferencia[] = [];
  for (const achado of limpo.slice(inicio, fim).matchAll(/[A-Za-z0-9_$#.]+/g)) {
    if (!ehNomeDeObjeto(achado[0])) continue;
    const comeco = inicio + (achado.index ?? 0);
    achadas.push({ de: comeco, ate: comeco + achado[0].length, referencia: achado[0] });
  }
  return achadas;
}

/** Caractere que pode estar num nome, contando o ponto entre as partes. */
function ehParteDeNome(caractere: string | undefined): boolean {
  return caractere !== undefined && /[A-Za-z0-9_$#.]/.test(caractere);
}

/** Até três partes separadas por ponto, cada uma um identificador sem aspas. */
function ehNomeDeObjeto(texto: string): boolean {
  const partes = texto.split(".");
  return partes.length <= 3 && partes.every((parte) => /^[A-Za-z_][A-Za-z0-9_$#]*$/.test(parte));
}

/**
 * Palavras que só aparecem abrindo um comando.
 *
 * `from`, `where`, `set` e `values` ficam de fora de propósito: elas continuam
 * um comando começado acima, e cortar ali tiraria justamente o `FROM` que diz
 * de quais tabelas são as colunas. `begin` também fica fora — ele abre o corpo
 * de um bloco PL/SQL que começou no `declare`, e não um comando novo.
 */
const ABERTURA_DE_COMANDO =
  /\n[ \t]*\n\s*(?=(?:select|insert|update|delete|merge|with|create|alter|drop|truncate|grant|revoke|execute|exec|call|declare|comment|explain|use)\b)/gi;

/**
 * Começo e fim do comando em que a posição está.
 *
 * Sugestão de uma consulta não pode vazar para a seguinte: numa aba com vários
 * comandos, as tabelas de um `SELECT` não são as do `UPDATE` de baixo.
 *
 * O corte é pelo `;` — e **também** pela linha em branco seguida de palavra que
 * abre comando, porque na prática ninguém põe `;` numa aba de rascunho. Sem
 * essa segunda regra, uma aba com dez consultas soltas era lida como um comando
 * só: as colunas oferecidas depois do `WHERE` vinham das tabelas de todas elas,
 * misturadas, e a lista ficava inútil exatamente quando a tela tinha mais
 * informação. Exigir a palavra de abertura é o que preserva o comando escrito
 * com linha em branco no meio — `SELECT a, b` em cima, `FROM t` embaixo.
 *
 * Funciona porque o texto já vem sem literais nem comentários: `;` e linha em
 * branco dentro de aspas ou de comentário de bloco não cortam nada.
 */
export function limitesDoComando(limpo: string, posicao: number): [number, number] {
  const anterior = limpo.lastIndexOf(";", Math.max(0, posicao - 1));
  const seguinte = limpo.indexOf(";", posicao);
  let inicio = anterior + 1;
  let fim = seguinte === -1 ? limpo.length : seguinte;

  for (const achado of limpo.matchAll(ABERTURA_DE_COMANDO)) {
    // O corte fica no começo da palavra, e não na linha em branco: o comando
    // de baixo começa nela.
    const corte = (achado.index ?? 0) + achado[0].length;
    if (corte <= posicao) inicio = Math.max(inicio, corte);
    else fim = Math.min(fim, corte);
  }
  return [inicio, fim];
}

/**
 * A partir de quantos caracteres o dicionário é consultado.
 *
 * Precisa bater com `MINIMO_PARA_BUSCAR_NO_BANCO`, do lado do Rust, que
 * devolve lista vazia sem consultar nada abaixo disso. Um valor menor aqui
 * faria o vazio "por não ter consultado" ser confundido com "não existe", e é
 * exatamente esse o defeito que este número evita.
 */
const MINIMO_DE_PREFIXO = 2;

/**
 * Teto de nomes que uma busca de objetos devolve — o mesmo `LIMITE_DE_SUGESTOES`
 * do lado Rust. Lista que bate nele veio **cortada**: há nome de fora, e ela não
 * serve de base para prefixo maior. Ver `derivarDaMemoria`.
 */
const LIMITE_DE_SUGESTOES = 300;

/** O que faz sentido sugerir no ponto em que o cursor está. */
export type LugarDaSugestao =
  | "tabela"
  | "coluna"
  | "banco"
  | "coluna-ou-rotina"
  | "rotina"
  | null;

/**
 * Operadores depois dos quais cabe tanto coluna quanto função.
 *
 * `= '2' || IDR` pede a coluna da tabela que está sendo atualizada, e
 * `> SYSDATE - 2` pede a função — os dois no mesmo lugar, e por isso os dois
 * são oferecidos. O `+` está aqui porque é o `||` do SQL Server.
 *
 * `(` e `,` ficam de fora de propósito: já são lugar de coluna por regra
 * própria, mais específica que esta.
 */
const OPERADORES = /(?:\|\||<>|!=|>=|<=|[+\-*/=<>%])\s*$/;

/**
 * Decide se o cursor está em lugar de tabela, de coluna, de banco, ou de nada.
 *
 * `ateAqui` é o comando desde o início até o cursor, **sem** a palavra parcial
 * que está sendo digitada.
 *
 * Fora desses lugares não se sugere nada. É a diferença entre ajudar e
 * atrapalhar: antes, qualquer palavra digitada em qualquer lugar abria a lista
 * de objetos do banco.
 */
export function lugarDaSugestao(ateAqui: string): LugarDaSugestao {
  // Lugar de banco: `use Vendas` troca o banco da aba (requisito 3, SQL
  // Server). Ali não cabe objeto nenhum — só o nome de um banco.
  if (/\buse\s+$/i.test(ateAqui)) return "banco";

  // Lugar de rotina: depois de `EXECUTE` só cabe o que se chama, e a lista
  // abre **sem nada digitado**. É o único lugar em que isso vale a pena, e só
  // vale porque a resposta vem do índice local (requisito 40), sem consulta.
  if (/\b(?:execute|exec|call)\s+$/i.test(ateAqui)) return "rotina";

  // Lugar de tabela: logo depois da palavra que introduz uma.
  if (/\b(?:from|join|update|delete)\s+$/i.test(ateAqui)) return "tabela";

  // E na continuação da lista do FROM: `FROM cliente c, `.
  if (/,\s*$/.test(ateAqui) && clausulaCorrente(ateAqui) === "from") return "tabela";

  // Lugar de coluna: onde se escreve condição.
  if (/\b(?:where|and|or|having)\s+$/i.test(ateAqui)) return "coluna";
  if (/\(\s*$/.test(ateAqui)) return "coluna";

  // E a própria lista do SELECT, que é o lugar de coluna mais óbvio de todos —
  // e o único que faltava aqui. `DISTINCT` e `ALL` entram na expressão porque
  // ficam entre o `SELECT` e a primeira coluna sem mudar o que cabe ali.
  if (/\bselect\s+(?:distinct\s+|all\s+)?$/i.test(ateAqui)) return "coluna";

  // E o `SET` de um `UPDATE`: à esquerda de cada `=` só cabe coluna da tabela
  // que está sendo atualizada. O `UPDATE financeiro` acima já entrega a tabela.
  if (/\bset\s+$/i.test(ateAqui)) return "coluna";

  // E onde se escreve uma lista de colunas. As três terminam em `BY`, e o que
  // vem depois delas é sempre coluna — nunca tabela, nunca objeto.
  if (/\b(?:group|order|partition)\s+by\s+$/i.test(ateAqui)) return "coluna";

  // A vírgula continua a lista, e a lista pode ter quantas colunas quiser, de
  // quantas tabelas o comando tiver: `ORDER BY c.id_registro,` ainda é lugar de
  // coluna na linha de baixo.
  if (/,\s*$/.test(ateAqui) && CLAUSULAS_DE_COLUNA.has(clausulaCorrente(ateAqui) ?? "")) {
    return "coluna";
  }

  // Depois de um operador cabem as duas coisas, e é por isso que este lugar
  // existe em vez de virar mais um "coluna": `SET ID_REGISTRO = '2' || IDR`
  // pede coluna, `WHERE dt_inc > SYSDATE - 2` pede função, e nenhum dos dois
  // pode sumir para o outro aparecer.
  if (OPERADORES.test(ateAqui)) return "coluna-ou-rotina";

  return null;
}

/** Um nome com ponto colado no cursor. Ver `qualificacaoNoCursor`. */
export interface QualificacaoNoCursor {
  /**
   * As partes antes do último ponto. A parte vazia do meio é significativa: em
   * `Vendas..tb` ela é o esquema omitido do nome de três partes.
   */
  partes: string[];
  /** O que já foi digitado depois do último ponto. */
  parcial: string;
  /** Onde a cadeia começa, para se saber o que veio antes dela. */
  inicio: number;
}

/**
 * O nome qualificado que termina no cursor: `c.`, `dbo.tb`, `Vendas..tb`,
 * `Vendas.dbo.tb`.
 *
 * O `..` do SQL Server é o motivo de a leitura ser por cadeia, e não por um
 * identificador seguido de ponto: com o esquema omitido não há identificador
 * nenhum antes do último ponto, a expressão anterior não casava com nada e a
 * lista simplesmente não abria depois de `FROM BANCO_EXEMPLO..tbl`.
 */
export function qualificacaoNoCursor(ateOCursor: string): QualificacaoNoCursor | null {
  const achado = /(?:^|[\s(,])([A-Za-z_#][\w$#]*(?:\.[\w$#]*)+)$/.exec(ateOCursor);
  if (!achado) return null;

  const partes = achado[1].split(".");
  const parcial = partes.pop() ?? "";
  return { partes, parcial, inicio: ateOCursor.length - achado[1].length };
}

/**
 * Cláusulas cuja lista, separada por vírgula, é de colunas.
 *
 * `select` está aqui pelo mesmo motivo que `group`, `order` e `partition`:
 * `SELECT status,` e `SELECT *,` continuam pedindo coluna na vírgula seguinte.
 * Sem ele, a segunda coluna de um SELECT caía em "lugar de nada" e a lista
 * vinha cheia de tipo e procedure do dicionário.
 *
 * `set` é o `UPDATE ... SET a = 1, b = 2`, e `into` é a lista de colunas do
 * `INSERT INTO t (a, b)` — o parêntese já dava a primeira, e da vírgula em
 * diante caía no mesmo "lugar de nada".
 *
 * `values` fica **fora** de propósito: `VALUES (1, ` é a lista de valores, e
 * ali não cabe coluna. Como `clausulaCorrente` devolve a última palavra vista,
 * é ela que vence o `into` escrito antes.
 */
const CLAUSULAS_DE_COLUNA = new Set(["select", "set", "into", "group", "order", "partition"]);

/** A última palavra de cláusula escrita antes do cursor. */
function clausulaCorrente(ateAqui: string): string | null {
  const palavras = [
    ...ateAqui.matchAll(
      /\b(select|from|where|group|order|having|partition|set|update|delete|insert|into|join|on|using|union|minus|intersect|connect|start|values)\b/gi,
    ),
  ];
  const ultima = palavras[palavras.length - 1];
  return ultima ? ultima[1].toLowerCase() : null;
}

/**
 * Onde uma lista de tabelas termina — a próxima palavra que muda de assunto,
 * ou o parêntese, o que vier primeiro.
 *
 * `from` está aqui por causa do `SELECT ... INTO destino FROM origem`: a lista
 * do `into` tem um nome só, e sem este corte o `from` seguinte era lido como o
 * apelido do destino — de `select * into #t from tbl` saía `#t` apelidada de
 * `from`. Nas outras cláusulas não muda nada: um `from` depois de `from` só
 * aparece dentro de parênteses, e subconsulta já não entrava nesta lista.
 *
 * O parêntese entrou pelo `INSERT INTO cliente (nome, id)`: dali em diante o
 * que está escrito é lista de colunas, e não de tabelas. Sem o corte, a quebra
 * por vírgula entregava `id) values (` ao regex do nome, que lia `id` e
 * inventava uma tabela com esse nome. A inventada não ficava só na lista: ia
 * parar em `tabelasDoComando`, gastava uma consulta de colunas no banco por
 * comando, guardava o vazio no cache da aba sob um nome que não existe, e
 * `id.` passava a resolver para ela. A função de tabela do SQL Server tinha o
 * mesmo defeito, com os argumentos no lugar das colunas: de
 * `from dbo.periodo(inicio, fim) p` saía uma tabela chamada `fim`.
 *
 * Cortar ali não custa tabela nenhuma. A lista do `INSERT INTO` tem um nome
 * só; nas outras cláusulas o que abre parêntese é subconsulta, que já não
 * entrava aqui, ou argumento, que nunca foi tabela.
 */
const FIM_DA_LISTA_DE_TABELAS =
  /\(|\b(?:from|where|group|order|having|set|on|using|join|left|right|inner|outer|full|cross|union|minus|intersect|connect|start|values|select)\b/i;

/** Uma tabela citada no comando, com o apelido que recebeu ali. */
export interface ReferenciaDeTabela {
  /**
   * O nome exatamente como está escrito no comando, banco e tudo. É ele que o
   * script de INSERT usa (requisito 39): remontar o nome a partir das partes
   * trocaria `Vendas..cliente` por outra tabela.
   */
  nomeEscrito: string;
  /**
   * O nome sem o banco — com o esquema quando ele foi escrito, porque é ele
   * que resolve a tabela dentro do banco: `Vendas.rh.cliente` → `rh.cliente`.
   */
  tabela: string;
  /** O apelido, quando o comando deu um: `FROM cliente c` → `c`. */
  alias: string | null;
  /**
   * O banco, quando o nome tem três partes: `FROM Vendas..cliente` → `Vendas`.
   * Só o SQL Server escreve assim; no Oracle é sempre nulo.
   */
  banco: string | null;
}

/**
 * As tabelas citadas no comando, na ordem em que aparecem: as do `FROM`, as de
 * cada `JOIN`, a do `UPDATE` e a do `INSERT INTO`.
 *
 * Lê a **lista inteira** de cada cláusula, e não só a primeira tabela: em
 * `FROM cliente c, pedido p` as duas contam, com os dois apelidos. É o que
 * permite tanto oferecer as colunas das duas depois do `WHERE` quanto resolver
 * `p.` para `pedido`.
 */
export function referenciasDoComando(sql: string): ReferenciaDeTabela[] {
  const referencias: ReferenciaDeTabela[] = [];

  for (const achado of sql.matchAll(/\b(?:from|update|join|into)\b/gi)) {
    const inicio = (achado.index ?? 0) + achado[0].length;
    const resto = sql.slice(inicio);
    const fim = FIM_DA_LISTA_DE_TABELAS.exec(resto);
    const lista = fim ? resto.slice(0, fim.index) : resto;

    for (const item of lista.split(",")) {
      // De `sistema.cliente c` saem `cliente` e `c`.
      //
      // O `#` abre o nome porque tabela temporária do SQL Server começa por ele
      // — `#t`, `##g`. Sem ele o item era descartado inteiro, e com o item iam a
      // tabela **e** o apelido: `join #t t` não deixava o `t.` sugerir coluna
      // nenhuma, e o editor ainda lia o `t` como esquema e consultava o
      // catálogo por nada. Quem diz as colunas de uma temporária é o próprio
      // script — ver `definicaoDeTemporaria`.
      const partes = /^\s*([A-Za-z_#][\w$#.]*)(?:\s+(?:as\s+)?([A-Za-z_][\w$#]*))?/i.exec(item);
      if (!partes) continue;

      const [, nome, apelido] = partes;
      if (ehPalavraReservada(nome)) continue;
      referencias.push({
        ...partirNomeQualificado(nome),
        nomeEscrito: nome,
        alias: apelido && !ehPalavraReservada(apelido) ? apelido : null,
      });
    }
  }
  return referencias;
}

/**
 * As tabelas do comando, sem repetir — a mesma pode estar no `FROM` e num
 * `JOIN`. São elas que têm as colunas oferecidas depois do `WHERE`.
 *
 * O banco entra na comparação: num mesmo comando, `Vendas..cliente` e
 * `cliente` são duas tabelas, e as colunas de uma não são as da outra.
 */
export function tabelasDoComando(sql: string): ReferenciaDeTabela[] {
  const vistas = new Set<string>();
  const tabelas: ReferenciaDeTabela[] = [];

  for (const referencia of referenciasDoComando(sql)) {
    const chave = `${referencia.banco ?? ""}.${referencia.tabela}`.toUpperCase();
    if (vistas.has(chave)) continue;
    vistas.add(chave);
    tabelas.push(referencia);
  }
  return tabelas;
}

/**
 * Resolve um qualificador somente quando ele pertence à lista de tabelas.
 *
 * A ausência é significativa: `PKG.` não deve cair em `listar_colunas`, porque
 * nesse caso o ponto pede os subprogramas da package — e, no SQL Server,
 * `dbo.` e `Vendas..` pedem os objetos daquele esquema e daquele banco.
 *
 * **O apelido vem antes do nome de tabela**, e não é desempate teórico: é o
 * `UPDATE` do T-SQL. Em
 *
 * ```sql
 * update t501
 * set t501.cd_stat_nota = 59
 * from tbl_501_nota_stat t501
 * ```
 *
 * o `t501` do `UPDATE` é o apelido, mas está escrito onde uma tabela estaria —
 * e é lido como tabela. Varrendo na ordem do texto ele casava primeiro, e o
 * `t501.` ia pedir as colunas de uma tabela chamada `t501`, que não existe.
 * Voltava vazio, e vazio aqui não oferece nada: o ponto não sugeria coluna
 * nenhuma. Pior, a lista vazia ficava guardada na aba pelo nome errado.
 *
 * Procurar o apelido primeiro resolve os dois casos sem inventar regra: quem
 * escreve `UPDATE tabela SET` não tem apelido nenhum com aquele nome, e cai na
 * segunda varredura.
 */
export function referenciaDoQualificador(
  sql: string,
  qualificador: string,
): ReferenciaDeTabela | null {
  const alvo = qualificador.toUpperCase();
  const referencias = referenciasDoComando(sql);
  return (
    referencias.find((referencia) => referencia.alias?.toUpperCase() === alvo) ??
    referencias.find((referencia) => referencia.tabela.toUpperCase() === alvo) ??
    null
  );
}

/**
 * Separa o banco do resto do nome escrito no comando.
 *
 * `cliente` continua `cliente`; `sistema.cliente` vira `cliente`, porque no
 * Oracle o escopo é o schema do usuário; `Vendas..cliente` e
 * `Vendas.dbo.cliente` são o nome de três partes do SQL Server, e aí o banco
 * sai na frente — é o banco dali que tem as colunas, e não o da aba.
 */
function partirNomeQualificado(nome: string): { banco: string | null; tabela: string } {
  const partes = nome.split(".");
  if (partes.length < 3) return { banco: null, tabela: partes[partes.length - 1] };

  const [banco, esquema, objeto] = partes;
  return { banco: banco || null, tabela: esquema ? `${esquema}.${objeto}` : objeto };
}

/** Evita confundir `FROM tabela WHERE` com um alias chamado "where". */
function ehPalavraReservada(palavra: string): boolean {
  return ["where", "on", "using", "set", "group", "order", "having", "join", "left", "right", "inner", "outer", "full", "cross", "start", "connect"].includes(
    palavra.toLowerCase(),
  );
}

/**
 * Um item da definição de uma tabela temporária, como o script o escreve.
 *
 * `origem` é o qualificador que apareceu antes do ponto — o `t501` de
 * `t501.cd_stat_nota` e o `t` de `t.*`. É por ele que se acha em qual tabela do
 * `FROM` está a coluna copiada, e com ela o tipo dela.
 */
export type ItemDaTemporaria =
  /** Uma coluna que o texto nomeia. `tipo` só vem preenchido quando o texto o escreve. */
  | { especie: "coluna"; nome: string; tipo: string; origem: string | null }
  /** O `*` de um `SELECT INTO`, ou o `t.*`: as colunas são as da origem. */
  | { especie: "todas"; origem: string | null };

/** O que o texto do script diz sobre as colunas de uma tabela temporária. */
export interface DefinicaoDeTemporaria {
  /** Os itens da definição, na ordem em que estão escritos. */
  itens: ItemDaTemporaria[];
  /** As tabelas do `FROM` da definição, que completam o que o texto não diz. */
  origens: ReferenciaDeTabela[];
}

/**
 * A definição de uma tabela temporária no texto do script — `#t`, `##g`.
 *
 * Existe porque `#t` não está em nenhum catálogo que a sessão da conexão
 * alcance: ela mora em `tempdb`, com escopo da sessão que a criou, e quem
 * responde ao autocomplete é a sessão da **conexão**, não a da aba (o motivo
 * está no cabeçalho de `src-tauri/src/metadados_sqlserver.rs`). Perguntar ao
 * banco também chegaria tarde: o `#t` só passa a existir depois de o
 * `SELECT INTO` rodar, e a sugestão é pedida enquanto o script está sendo
 * escrito. Quem sabe as colunas nesse momento é o próprio texto.
 *
 * Duas formas dizem a resposta, e são as duas que se escrevem na prática:
 * `CREATE TABLE #t (...)`, que traz nome e tipo, e `SELECT <lista> INTO #t FROM
 * <origem>`, em que a lista traz os nomes e a origem traz os tipos.
 *
 * `limpo` é o **documento inteiro** passado por `analisarSql`, e não o comando:
 * `#t` tem escopo de sessão, a definição dela costuma estar num comando acima,
 * e `limitesDoComando` corta justamente ali — a linha em branco antes do
 * `UPDATE` é fronteira de comando.
 *
 * A última definição vence. O script se lê de cima para baixo, e quem recria a
 * temporária no meio dele muda a forma que ela tem a partir daquele ponto.
 */
export function definicaoDeTemporaria(limpo: string, nome: string): DefinicaoDeTemporaria | null {
  const alvo = nome.trim().toUpperCase();
  if (!alvo.startsWith("#")) return null;

  const achadas: { em: number; definicao: DefinicaoDeTemporaria }[] = [];

  // `CREATE TABLE #t (...)`: o texto escreve nome e tipo de cada coluna, e não
  // há origem nenhuma a consultar.
  for (const achado of limpo.matchAll(/\bcreate\s+table\s+(#{1,2}[\w$#]+)\s*\(/gi)) {
    if (achado[1].toUpperCase() !== alvo) continue;
    const abre = (achado.index ?? 0) + achado[0].length - 1;
    achadas.push({
      em: achado.index ?? 0,
      definicao: { itens: colunasDeclaradas(corpoEntreParenteses(limpo, abre)), origens: [] },
    });
  }

  // `SELECT <lista> INTO #t FROM <origem>`.
  for (const achado of limpo.matchAll(/\binto\s+(#{1,2}[\w$#]+)\b/gi)) {
    if (achado[1].toUpperCase() !== alvo) continue;
    const noInto = achado.index ?? 0;
    const [inicio, fim] = limitesDoComando(limpo, noInto);
    const antes = limpo.slice(inicio, noInto);

    // O `select` de fora, e não o último antes do `into`: em `select (select
    // max(x) from y) m into #t` o de dentro é de uma subconsulta, e a lista dele
    // não é esta. Num `INSERT INTO #t SELECT ...` não há `select` nenhum antes
    // do `into`, e está certo não haver definição: o insert preenche a tabela,
    // não a cria.
    const daLista = selectDeFora(antes);
    if (daLista === null) continue;
    achadas.push({
      em: noInto,
      definicao: {
        itens: itensDaLista(antes.slice(daLista)),
        origens: referenciasDoComando(limpo.slice(noInto + achado[0].length, fim)),
      },
    });
  }

  achadas.sort((um, outro) => um.em - outro.em);
  return achadas.length > 0 ? achadas[achadas.length - 1].definicao : null;
}

/**
 * Os nomes de tabela temporária que o script **define** — `#t`, `##g`.
 *
 * É a lista que o autocomplete oferece depois de `join #`, e é também o que
 * evita a pergunta ao catálogo: `#t` não está em `sys.objects` do banco da aba,
 * e perguntar ali é uma consulta certa de voltar vazia, uma por tecla digitada.
 *
 * Definir é diferente de citar. Um `#t` que só aparece num `INSERT INTO #t` veio
 * de outro lugar — de uma procedure, de um script rodado antes — e oferecê-lo
 * seria oferecer um nome de que não se sabe coluna nenhuma. Por isso o filtro
 * final é a própria `definicaoDeTemporaria`: as duas listas não podem divergir.
 */
export function temporariasDoScript(limpo: string): string[] {
  const candidatos = new Map<string, string>();
  for (const achado of limpo.matchAll(/(?:\bcreate\s+table|\binto)\s+(#{1,2}[\w$#]+)/gi)) {
    const chave = achado[1].toUpperCase();
    if (!candidatos.has(chave)) candidatos.set(chave, achado[1]);
  }
  return [...candidatos.values()].filter((nome) => definicaoDeTemporaria(limpo, nome) !== null);
}

/** As colunas de uma das origens de um `SELECT ... INTO`, já lidas do banco. */
export interface ColunasDeOrigem {
  origem: ReferenciaDeTabela;
  colunas: readonly api.Coluna[];
}

/**
 * As colunas que a definição descreve, com o que as origens completam.
 *
 * O `*` vira as colunas das origens, e o nome de coluna copiada vira ele mesmo
 * com o tipo que a origem diz. O que sobra sem tipo é expressão — `count(*) as
 * qtd` —, e ali o nome sozinho já é o que se queria: o tipo vazio só some do
 * detalhe da lista.
 *
 * Nome repetido entra uma vez. Duas origens podem ter a mesma coluna, e o
 * `SELECT *` de um `JOIN` as traz as duas; na temporária resultante é uma só.
 */
export function colunasDaDefinicao(
  definicao: DefinicaoDeTemporaria,
  origens: readonly ColunasDeOrigem[],
): api.Coluna[] {
  const daOrigem = (qualificador: string | null): readonly ColunasDeOrigem[] => {
    if (!qualificador) return origens;
    const alvo = qualificador.toUpperCase();
    return origens.filter(
      ({ origem }) =>
        origem.alias?.toUpperCase() === alvo ||
        origem.tabela.toUpperCase() === alvo ||
        (origem.tabela.split(".").pop() ?? "").toUpperCase() === alvo,
    );
  };

  const colunas: api.Coluna[] = [];
  const vistas = new Set<string>();
  const juntar = (coluna: api.Coluna): void => {
    const chave = coluna.nome.toUpperCase();
    if (vistas.has(chave)) return;
    vistas.add(chave);
    colunas.push(coluna);
  };

  for (const item of definicao.itens) {
    if (item.especie === "todas") {
      for (const { colunas: daTabela } of daOrigem(item.origem)) daTabela.forEach(juntar);
      continue;
    }

    const alvo = item.nome.toUpperCase();
    const copiada = daOrigem(item.origem)
      .flatMap(({ colunas: daTabela }) => [...daTabela])
      .find((coluna) => coluna.nome.toUpperCase() === alvo);
    juntar({
      nome: item.nome,
      tipo: item.tipo || copiada?.tipo || "",
      aceitaNulo: copiada?.aceitaNulo ?? true,
    });
  }
  return colunas;
}

/**
 * Onde a lista de colunas do `SELECT` de fora começa — nulo quando não há
 * `select` fora de parêntese nenhum.
 */
function selectDeFora(trecho: string): number | null {
  let profundidade = 0;
  for (const achado of trecho.matchAll(/[()]|\bselect\b/gi)) {
    if (achado[0] === "(") profundidade += 1;
    else if (achado[0] === ")") profundidade -= 1;
    else if (profundidade === 0) return (achado.index ?? 0) + achado[0].length;
  }
  return null;
}

/** Os itens da lista de um `SELECT ... INTO`, na ordem em que estão escritos. */
function itensDaLista(lista: string): ItemDaTemporaria[] {
  const itens: ItemDaTemporaria[] = [];
  for (const bruto of partirNoNivelZero(lista)) {
    const item = itemDaLista(bruto);
    if (item) itens.push(item);
  }
  return itens;
}

/**
 * O que um item da lista do `SELECT` diz sobre a coluna que ele cria.
 *
 * O `SELECT INTO` exige nome para tudo que não seja coluna copiada, e é por
 * isso que quase todo item aqui tem um: ou é `*`, ou é o nome de uma coluna, ou
 * traz apelido. O que não tem nenhum dos três é expressão sem apelido — SQL que
 * o servidor recusaria —, e sai fora.
 */
function itemDaLista(bruto: string): ItemDaTemporaria | null {
  const item = bruto.trim();
  if (!item) return null;

  // `*` e `t.*`: o texto não nomeia coluna nenhuma; são as da origem.
  const estrela = /^(?:([A-Za-z_#][\w$#]*)\s*\.\s*)?\*$/.exec(item);
  if (estrela) return { especie: "todas", origem: estrela[1] ?? null };

  // `x AS apelido`: quem nomeia a coluna é o apelido, e o tipo o texto não diz.
  // O `as` de dentro de um `cast(x as int)` não confunde porque o apelido tem
  // de estar no fim do item.
  const comAs = /\bas\s+([A-Za-z_][\w$#]*)\s*$/i.exec(item);
  if (comAs) return { especie: "coluna", nome: comAs[1], tipo: "", origem: null };

  // `t501.cd_stat_nota` e `cd_stat_nota`: coluna copiada da origem, e é lá que
  // está o tipo dela.
  const copiada = /^(?:([A-Za-z_#][\w$#]*)\s*\.\s*)?([A-Za-z_][\w$#]*)$/.exec(item);
  if (copiada) {
    return { especie: "coluna", nome: copiada[2], tipo: "", origem: copiada[1] ?? null };
  }

  // Sobra o apelido escrito sem `as`: `count(*) qtd`, `dt + 1 vencimento`. É
  // também por aqui que passam o `distinct` e o `top 10` colados na primeira
  // coluna, que não são apelido de nada mas deixam o nome certo no fim.
  const solto = /\s([A-Za-z_][\w$#]*)\s*$/.exec(item);
  return solto ? { especie: "coluna", nome: solto[1], tipo: "", origem: null } : null;
}

/**
 * As colunas de um `CREATE TABLE #t (...)`, com o tipo que o texto escreve.
 *
 * O que não é coluna sai fora pela primeira palavra: `primary key (a, b)`,
 * `constraint ...` e `unique` declaram restrição, não coluna. A lista delas vem
 * inteira num item só, porque o corte por vírgula ignora o que está entre
 * parênteses.
 */
function colunasDeclaradas(corpo: string): ItemDaTemporaria[] {
  const itens: ItemDaTemporaria[] = [];
  for (const bruto of partirNoNivelZero(corpo)) {
    const achado = /^\s*([A-Za-z_#][\w$#]*)\s+([A-Za-z_][\w$#]*(?:\s*\([^)]*\))?)/.exec(bruto);
    if (!achado || RESTRICOES_DE_TABELA.has(achado[1].toUpperCase())) continue;
    itens.push({
      especie: "coluna",
      nome: achado[1],
      tipo: achado[2].replace(/\s+/g, ""),
      origem: null,
    });
  }
  return itens;
}

/** A primeira palavra do que, dentro de um `CREATE TABLE`, não é coluna. */
const RESTRICOES_DE_TABELA = new Set([
  "CONSTRAINT",
  "PRIMARY",
  "UNIQUE",
  "FOREIGN",
  "CHECK",
  "INDEX",
  "KEY",
  "PERIOD",
]);

/** Parte uma lista por vírgula, ignorando as vírgulas dentro de parênteses. */
function partirNoNivelZero(lista: string): string[] {
  const itens: string[] = [];
  let profundidade = 0;
  let inicio = 0;

  for (let i = 0; i < lista.length; i += 1) {
    const atual = lista[i];
    if (atual === "(") profundidade += 1;
    else if (atual === ")") profundidade -= 1;
    else if (atual === "," && profundidade === 0) {
      itens.push(lista.slice(inicio, i));
      inicio = i + 1;
    }
  }
  itens.push(lista.slice(inicio));
  return itens;
}

/** O texto entre o parêntese que abre em `abre` e o que o fecha. */
function corpoEntreParenteses(limpo: string, abre: number): string {
  let profundidade = 0;
  for (let i = abre; i < limpo.length; i += 1) {
    if (limpo[i] === "(") profundidade += 1;
    else if (limpo[i] === ")") {
      profundidade -= 1;
      if (profundidade === 0) return limpo.slice(abre + 1, i);
    }
  }
  // Parêntese não fechado é script em construção: vale o que já está escrito.
  return limpo.slice(abre + 1);
}
