/**
 * Grid de resultados (requisitos 6, 10, 17, 20 e 21).
 *
 * Renderiza apenas as linhas visíveis. Uma consulta com dezenas de milhares de
 * linhas carregadas mantém no DOM algumas dezenas de elementos — é o que
 * segura o consumo de memória, que é o foco do projeto.
 */

import "./grid.css";

import type { ColunaResultado, Linha } from "../api";
import { t } from "../idioma";

/** Altura fixa de cada linha, em pixels. Precisa bater com o CSS. */
const ALTURA_DA_LINHA = 22;

/** Quantas linhas extras são desenhadas acima e abaixo da área visível. */
const FOLGA = 6;

/** Largura inicial de qualquer coluna, em pixels. */
const LARGURA_PADRAO = 160;

/**
 * Largura da coluna do número, em pixels. Precisa bater com o CSS.
 *
 * Como essa coluna fica parada na esquerda, ela cobre os primeiros pixels da
 * área rolável: quem move a rolagem horizontal precisa descontá-la para não
 * parar a célula debaixo dela.
 */
const LARGURA_DO_NUMERO = 52;

/** Limites do arrasto: estreito demais some com o conteúdo, largo demais atrapalha o resto. */
const LARGURA_MINIMA = 44;
const LARGURA_MAXIMA = 900;

/** Espaço que a célula gasta fora do texto: os dois padding e a borda direita. */
const SOBRA_DA_CELULA = 14;

/** As mesmas fontes do CSS — medir com outra fonte daria largura errada. */
const FONTE_DA_CELULA = '12px Consolas, "Cascadia Mono", monospace';
const FONTE_DO_CABECALHO = '600 12.5px "Segoe UI", system-ui, sans-serif';

/** Mesmo limite usado pelo formatador HTML do TecladoMacroVirtual. */
const MAXIMO_DE_LINHAS_PARA_TEAMS = 100;

export interface OpcoesDoGrid {
  /** Chamado quando a rolagem chega ao fim e ainda há linhas no banco. */
  aoPedirMais?: () => void;
  /** Abre o visualizador de um valor grande. */
  aoAbrirValor?: (coluna: string, valor: string) => void;
}

type Ordenacao = { coluna: number; crescente: boolean } | null;

export class Grid {
  readonly elemento = document.createElement("div");
  private readonly cabecalho = document.createElement("div");
  /** As células do cabeçalho, na linha que lhes dá a largura de todas juntas. */
  private readonly linhaDoCabecalho = document.createElement("div");
  private readonly rolagem = document.createElement("div");
  private readonly corpo = document.createElement("div");
  private readonly espacador = document.createElement("div");
  /** Reserva, no fim do cabeçalho, o espaço da barra de rolagem do corpo. */
  private readonly folgaDaBarra = document.createElement("div");

  private linhas: Linha[] = [];
  /** Índices de `linhas`, na ordem em que aparecem. */
  private ordem: number[] = [];
  /** Largura escolhida para cada coluna, em pixels. */
  private readonly larguras: number[] = [];
  private ordenacao: Ordenacao = null;
  private selecionadas = new Set<number>();
  /** `null` significa que a seleção abrange a linha inteira. */
  private colunaSelecionada: number | null = null;
  private ultimaClicada: number | null = null;
  private podePedirMais = false;
  /** Folga já aplicada, para não mexer no DOM a cada rolagem. */
  private folgaAplicada = -1;

  constructor(
    private readonly colunas: ColunaResultado[],
    private readonly opcoes: OpcoesDoGrid = {},
  ) {
    this.elemento.className = "grid";
    this.cabecalho.className = "grid__cabecalho";
    this.linhaDoCabecalho.className = "grid__cabecalho-linha";
    this.cabecalho.append(this.linhaDoCabecalho);
    this.rolagem.className = "grid__rolagem";
    this.corpo.className = "grid__corpo";
    this.espacador.className = "grid__espacador";

    this.colunas.forEach((_, indice) => this.definirLargura(indice, LARGURA_PADRAO));
    this.montarCabecalho();
    this.rolagem.append(this.espacador, this.corpo);
    this.elemento.append(this.cabecalho, this.rolagem);

    this.rolagem.addEventListener("scroll", () => this.aoRolar());
    this.elemento.tabIndex = 0;
    this.elemento.addEventListener("keydown", (evento) => this.aoTeclar(evento));
  }

  // --- Dados ---------------------------------------------------------------

  acrescentar(novas: Linha[]): void {
    const inicio = this.linhas.length;
    this.linhas.push(...novas);
    for (let i = inicio; i < this.linhas.length; i += 1) this.ordem.push(i);
    if (this.ordenacao) this.reordenar();
    this.desenhar();
  }

  definirTemMais(tem: boolean): void {
    this.podePedirMais = tem;
  }

  get totalDeLinhas(): number {
    return this.linhas.length;
  }

  /**
   * Altura, em pixels, que deixa `maximo` linhas à mostra — ou todas, se forem
   * menos.
   *
   * É o que a pilha de grids usa para dar a cada conjunto a sua altura natural
   * sem esticá-lo até o conteúdo inteiro: a virtualização depende de a caixa
   * ser menor que o que ela mostra, e um grid alto o bastante para caber tudo
   * desenharia todas as linhas de uma vez.
   *
   * Precisa ser chamada com o elemento já na tela: a altura do cabeçalho é
   * medida, não fixada. Fora da tela sobra a estimativa, que erra por pouco.
   */
  alturaPara(maximo: number): number {
    const visiveis = Math.max(1, Math.min(this.linhas.length, maximo));
    const cabecalho = this.cabecalho.offsetHeight || ALTURA_DA_LINHA + 2;
    return cabecalho + visiveis * ALTURA_DA_LINHA + this.alturaDaBarraDeitada() + 1;
  }

  /**
   * O que a barra de rolagem horizontal come da altura do corpo — zero quando
   * não há barra, ou quando o sistema a desenha por cima do conteúdo.
   *
   * Sem reservá-la, a conta acima entrega a altura de `maximo` linhas e a barra
   * fica com uns quinze pixels dela: a última linha aparece cortada ao meio. Um
   * conjunto de uma linha e muitas colunas — o caso de uma procedure de
   * depuração — perde justamente a única linha que tinha para mostrar.
   */
  private alturaDaBarraDeitada(): number {
    return Math.max(0, this.rolagem.offsetHeight - this.rolagem.clientHeight);
  }

  // --- Montagem ------------------------------------------------------------

  private montarCabecalho(): void {
    const canto = document.createElement("div");
    canto.className = "grid__celula grid__celula--numero grid__celula--cabecalho";
    this.linhaDoCabecalho.append(canto);

    this.colunas.forEach((coluna, indice) => {
      const celula = document.createElement("div");
      celula.className = "grid__celula grid__celula--cabecalho";
      if (coluna.numerica) celula.classList.add("grid__celula--numero-dado");
      celula.title = `${coluna.nome} — ${coluna.tipo}`;
      celula.dataset.coluna = String(indice);
      this.aplicarLargura(celula, indice);

      const texto = document.createElement("span");
      texto.className = "grid__nome-da-coluna";
      texto.textContent = coluna.nome;
      const seta = document.createElement("span");
      seta.className = "grid__seta";
      celula.append(texto, seta, this.montarAlca(indice));

      celula.addEventListener("click", () => {
        this.selecionarColuna(indice);
        this.ordenarPor(indice);
      });
      this.linhaDoCabecalho.append(celula);
    });

    // Depois de todas as colunas, e por isso no fim: é o espaço que o corpo
    // gasta com a barra de rolagem vertical e o cabeçalho não gastaria.
    this.folgaDaBarra.className = "grid__folga-da-barra";
    this.linhaDoCabecalho.append(this.folgaDaBarra);
  }

  /**
   * Reserva no cabeçalho o espaço que a barra de rolagem vertical ocupa no
   * corpo.
   *
   * Sem ela o cabeçalho é mais largo que a área rolável do corpo — exatamente a
   * largura da barra — e portanto **rola menos** que ele. Nos últimos pixels da
   * rolagem horizontal o navegador corta o `scrollLeft` do cabeçalho no máximo
   * dele, o corpo continua andando, e as colunas aparecem deslocadas dos seus
   * títulos.
   *
   * Isso explica o "às vezes" do defeito: só se vê no fim da rolagem horizontal
   * e só quando há linhas bastantes para existir barra vertical. Medido com 68
   * colunas: 15 px de deslocamento com 200 linhas, nenhum com 5.
   *
   * A largura vem medida do próprio corpo porque varia com o sistema, e é zero
   * onde a barra é sobreposta.
   */
  private ajustarFolgaDaBarra(): void {
    const barra = this.rolagem.offsetWidth - this.rolagem.clientWidth;
    if (barra === this.folgaAplicada) return;
    this.folgaAplicada = barra;
    this.folgaDaBarra.style.flexBasis = `${barra}px`;
    // A folga acabou de mudar o quanto o cabeçalho pode rolar. Sem repetir a
    // cópia aqui, ele fica parado no valor cortado de antes.
    this.cabecalho.scrollLeft = this.rolagem.scrollLeft;
  }

  // --- Largura das colunas (requisito 20) ----------------------------------

  /**
   * A largura de cada coluna mora numa variável do CSS na raiz do grid, e as
   * células só apontam para ela. Assim o arrasto muda uma propriedade só e o
   * cabeçalho e as milhares de linhas seguem juntos, sem redesenhar nada.
   */
  private aplicarLargura(celula: HTMLElement, coluna: number): void {
    celula.style.flexBasis = `var(--coluna-${coluna})`;
  }

  private definirLargura(coluna: number, largura: number): void {
    const alvo = Math.round(Math.min(Math.max(largura, LARGURA_MINIMA), LARGURA_MAXIMA));
    this.larguras[coluna] = alvo;
    this.elemento.style.setProperty(`--coluna-${coluna}`, `${alvo}px`);
  }

  /** Divisa arrastável no canto direito do cabeçalho, como no SQL Developer. */
  private montarAlca(coluna: number): HTMLElement {
    const alca = document.createElement("span");
    alca.className = "grid__alca";
    alca.title = t("arraste para mudar a largura");

    let inicio = 0;
    let larguraInicial = 0;
    let arrastando = false;

    alca.addEventListener("pointerdown", (evento) => {
      arrastando = true;
      inicio = evento.clientX;
      larguraInicial = this.larguras[coluna];
      // A captura mantém o arrasto vivo mesmo com o ponteiro correndo por cima
      // das outras colunas ou saindo da janela.
      alca.setPointerCapture(evento.pointerId);
      alca.classList.add("grid__alca--ativa");
      // Sem isto o arrasto seleciona o texto do cabeçalho.
      evento.preventDefault();
      evento.stopPropagation();
    });

    alca.addEventListener("pointermove", (evento) => {
      if (!arrastando) return;
      this.definirLargura(coluna, larguraInicial + (evento.clientX - inicio));
    });

    const terminar = (evento: PointerEvent): void => {
      if (!arrastando) return;
      arrastando = false;
      alca.releasePointerCapture(evento.pointerId);
      alca.classList.remove("grid__alca--ativa");
    };
    alca.addEventListener("pointerup", terminar);
    alca.addEventListener("pointercancel", terminar);

    // O clique do fim do arrasto não pode virar ordenação da coluna.
    alca.addEventListener("click", (evento) => evento.stopPropagation());
    alca.addEventListener("dblclick", (evento) => {
      evento.stopPropagation();
      this.definirLargura(coluna, this.larguraDoConteudo(coluna));
    });
    return alca;
  }

  /** Largura que mostra por inteiro o cabeçalho e o maior valor já carregado. */
  private larguraDoConteudo(coluna: number): number {
    // O cabeçalho ainda tem a seta de ordenação ao lado do nome.
    let maior = medirTexto(this.colunas[coluna].nome, FONTE_DO_CABECALHO) + 12;

    // A fonte das células é monoespaçada: o valor mais comprido é também o mais
    // largo, então basta procurar por comprimento e medir uma vez só — o que
    // mantém isto barato mesmo com dezenas de milhares de linhas carregadas.
    let maisComprido = "";
    for (const linha of this.linhas) {
      const valor = linha[coluna];
      if (valor !== null && valor.length > maisComprido.length) maisComprido = valor;
    }
    if (maisComprido !== "") {
      maior = Math.max(maior, medirTexto(maisComprido, FONTE_DA_CELULA));
    }
    return maior + SOBRA_DA_CELULA;
  }

  /** Ordena pelas linhas já carregadas (requisito 20). */
  private ordenarPor(coluna: number): void {
    if (this.ordenacao?.coluna === coluna) {
      this.ordenacao = this.ordenacao.crescente
        ? { coluna, crescente: false }
        : null;
    } else {
      this.ordenacao = { coluna, crescente: true };
    }
    this.reordenar();
    this.atualizarSetas();
    this.desenhar();
  }

  private reordenar(): void {
    if (!this.ordenacao) {
      this.ordem = this.linhas.map((_, indice) => indice);
      return;
    }
    const { coluna, crescente } = this.ordenacao;
    const numerica = this.colunas[coluna]?.numerica ?? false;
    const sinal = crescente ? 1 : -1;

    this.ordem.sort((a, b) => {
      const valorA = this.linhas[a][coluna];
      const valorB = this.linhas[b][coluna];
      // NULL sempre no fim, em qualquer sentido — como no SQL Developer.
      if (valorA === null && valorB === null) return 0;
      if (valorA === null) return 1;
      if (valorB === null) return -1;
      if (numerica) {
        return sinal * (paraNumero(valorA) - paraNumero(valorB));
      }
      return sinal * valorA.localeCompare(valorB, "pt-BR");
    });
  }

  private atualizarSetas(): void {
    for (const celula of this.cabecalho.querySelectorAll<HTMLElement>("[data-coluna]")) {
      const indice = Number(celula.dataset.coluna);
      const seta = celula.querySelector(".grid__seta");
      if (!seta) continue;
      seta.textContent =
        this.ordenacao?.coluna === indice ? (this.ordenacao.crescente ? "▲" : "▼") : "";
    }
  }

  // --- Desenho -------------------------------------------------------------

  private aoRolar(): void {
    // `desenhar` ajusta a folga do cabeçalho, e ela precisa estar certa **antes**
    // da cópia: o navegador limita `scrollLeft` ao que couber, e um valor
    // cortado aqui não se recupera depois.
    this.desenhar();
    // O cabeçalho não rola sozinho: acompanha o corpo na horizontal.
    this.cabecalho.scrollLeft = this.rolagem.scrollLeft;
    const { scrollTop, scrollHeight, clientHeight } = this.rolagem;
    if (this.podePedirMais && scrollTop + clientHeight >= scrollHeight - ALTURA_DA_LINHA * 3) {
      this.podePedirMais = false;
      this.opcoes.aoPedirMais?.();
    }
  }

  private desenhar(): void {
    // Aqui porque a barra vertical aparece quando as linhas chegam, e não só
    // quando se rola.
    this.ajustarFolgaDaBarra();
    this.atualizarCabecalhoSelecionado();
    this.espacador.style.height = `${this.ordem.length * ALTURA_DA_LINHA}px`;

    const primeira = Math.max(0, Math.floor(this.rolagem.scrollTop / ALTURA_DA_LINHA) - FOLGA);
    const visiveis = Math.ceil(this.rolagem.clientHeight / ALTURA_DA_LINHA) + FOLGA * 2;
    const ultima = Math.min(this.ordem.length, primeira + visiveis);

    const fragmento = document.createDocumentFragment();
    for (let posicao = primeira; posicao < ultima; posicao += 1) {
      fragmento.append(this.montarLinha(posicao));
    }
    this.corpo.style.transform = `translateY(${primeira * ALTURA_DA_LINHA}px)`;
    this.corpo.replaceChildren(fragmento);
  }

  private montarLinha(posicao: number): HTMLElement {
    const indice = this.ordem[posicao];
    const dados = this.linhas[indice];

    const linha = document.createElement("div");
    linha.className = "grid__linha";
    const selecionada = this.selecionadas.has(indice);
    if (selecionada && this.colunaSelecionada === null) {
      linha.classList.add("grid__linha--selecionada");
    }
    linha.addEventListener("mousedown", (evento) => {
      // O botão direito sobre uma linha já escolhida não mexe na escolha: quem
      // clica ali quer o menu para todas as linhas marcadas, e refazer a
      // escolha deixaria só a de baixo do ponteiro. Sobre linha não escolhida
      // ele escolhe, que é o costume — sem isso o menu agiria sobre outra.
      if (evento.button === 2 && this.selecionadas.has(indice)) return;
      const celula = (evento.target as Element).closest<HTMLElement>(".grid__celula");
      const coluna = celula?.dataset.coluna;
      this.aoClicarLinha(evento, posicao, coluna === undefined ? null : Number(coluna));
    });

    const numero = document.createElement("div");
    numero.className = "grid__celula grid__celula--numero";
    // Vale também quando a escolha é de uma célula só: a coluna do número é o
    // que continua à vista depois de a rolagem horizontal levar a célula
    // escolhida para fora da tela.
    if (selecionada) numero.classList.add("grid__celula--numero-marcado");
    numero.textContent = String(posicao + 1);
    linha.append(numero);

    this.colunas.forEach((coluna, c) => {
      const valor = dados[c];
      const celula = document.createElement("div");
      celula.className = "grid__celula";
      celula.dataset.coluna = String(c);
      this.aplicarLargura(celula, c);
      if (selecionada && this.colunaSelecionada === c) {
        celula.classList.add("grid__celula--selecionada");
      }
      if (coluna.numerica) celula.classList.add("grid__celula--numero-dado");

      if (valor === null) {
        // Requisito 20: NULL precisa ser distinguível de texto vazio.
        celula.classList.add("grid__celula--nula");
        celula.textContent = "(null)";
      } else {
        celula.textContent = valor;
        celula.title = valor.length > 80 ? valor : "";
        if (valor.length > 80) {
          celula.classList.add("grid__celula--longa");
          celula.addEventListener("dblclick", () =>
            this.opcoes.aoAbrirValor?.(coluna.nome, valor),
          );
        }
      }
      linha.append(celula);
    });
    return linha;
  }

  // --- Seleção e cópia (requisito 6) --------------------------------------

  private aoClicarLinha(evento: MouseEvent, posicao: number, coluna: number | null): void {
    const indice = this.ordem[posicao];
    const mudouDeColuna = this.colunaSelecionada !== coluna;
    if (mudouDeColuna) {
      this.selecionadas.clear();
      this.ultimaClicada = null;
    }
    this.colunaSelecionada = coluna;

    if (evento.shiftKey && this.ultimaClicada !== null) {
      const anterior = this.ordem.indexOf(this.ultimaClicada);
      const [de, ate] = anterior < posicao ? [anterior, posicao] : [posicao, anterior];
      for (let p = de; p <= ate; p += 1) this.selecionadas.add(this.ordem[p]);
    } else if (evento.ctrlKey) {
      if (this.selecionadas.has(indice)) this.selecionadas.delete(indice);
      else this.selecionadas.add(indice);
      this.ultimaClicada = indice;
    } else {
      this.selecionadas.clear();
      this.selecionadas.add(indice);
      this.ultimaClicada = indice;
    }
    this.desenhar();
    this.elemento.focus();
  }

  /** O cabeçalho seleciona todos os valores da coluna, além de manter a ordenação. */
  private selecionarColuna(coluna: number): void {
    this.colunaSelecionada = coluna;
    this.selecionadas = new Set(this.ordem);
    this.ultimaClicada = null;
    this.elemento.focus();
  }

  private atualizarCabecalhoSelecionado(): void {
    for (const celula of this.cabecalho.querySelectorAll<HTMLElement>("[data-coluna]")) {
      celula.classList.toggle(
        "grid__celula--coluna-selecionada",
        Number(celula.dataset.coluna) === this.colunaSelecionada,
      );
    }
  }

  private aoTeclar(evento: KeyboardEvent): void {
    if (evento.altKey || evento.shiftKey) return;

    const posicaoAtual = this.posicaoDaLinhaAtiva();
    const colunaAtual = this.colunaSelecionada ?? 0;
    if (posicaoAtual !== null) {
      let destino: [number, number] | null = null;
      switch (evento.key) {
        case "ArrowUp":
          if (!evento.ctrlKey) destino = [posicaoAtual - 1, colunaAtual];
          break;
        case "ArrowDown":
          if (!evento.ctrlKey) destino = [posicaoAtual + 1, colunaAtual];
          break;
        case "ArrowLeft":
          if (!evento.ctrlKey) destino = [posicaoAtual, colunaAtual - 1];
          break;
        case "ArrowRight":
          if (!evento.ctrlKey) destino = [posicaoAtual, colunaAtual + 1];
          break;
        case "Home":
          destino = evento.ctrlKey ? [0, 0] : [posicaoAtual, 0];
          break;
        case "End":
          destino = evento.ctrlKey
            ? [this.ordem.length - 1, this.colunas.length - 1]
            : [posicaoAtual, this.colunas.length - 1];
          break;
        case "PageUp":
          if (!evento.ctrlKey) destino = [posicaoAtual - this.linhasPorPagina(), colunaAtual];
          break;
        case "PageDown":
          if (!evento.ctrlKey) destino = [posicaoAtual + this.linhasPorPagina(), colunaAtual];
          break;
      }

      if (destino) {
        evento.preventDefault();
        this.irParaCelula(...destino);
        return;
      }
    }

    if (!evento.ctrlKey) return;
    if (evento.key === "a" || evento.key === "A") {
      evento.preventDefault();
      this.selecionarTudo();
    }
    if (evento.key === "c" || evento.key === "C") {
      evento.preventDefault();
      // Com Shift, vai o cabeçalho junto.
      void this.copiar(evento.shiftKey);
    }
  }

  /** Posição visual da linha ativa, que pode diferir do índice após ordenar. */
  private posicaoDaLinhaAtiva(): number | null {
    if (this.ordem.length === 0 || this.colunas.length === 0) return null;
    if (this.ultimaClicada === null) return 0;
    const posicao = this.ordem.indexOf(this.ultimaClicada);
    return posicao < 0 ? 0 : posicao;
  }

  private linhasPorPagina(): number {
    return Math.max(1, Math.floor(this.rolagem.clientHeight / ALTURA_DA_LINHA) - 1);
  }

  /** Seleciona uma célula e move as barras só o necessário para mostrá-la. */
  private irParaCelula(posicao: number, coluna: number): void {
    const linhaLimitada = Math.min(Math.max(posicao, 0), this.ordem.length - 1);
    const colunaLimitada = Math.min(Math.max(coluna, 0), this.colunas.length - 1);
    const indice = this.ordem[linhaLimitada];

    this.ultimaClicada = indice;
    this.selecionadas = new Set([indice]);
    this.colunaSelecionada = colunaLimitada;

    const topo = linhaLimitada * ALTURA_DA_LINHA;
    const base = topo + ALTURA_DA_LINHA;
    if (topo < this.rolagem.scrollTop) this.rolagem.scrollTop = topo;
    else if (base > this.rolagem.scrollTop + this.rolagem.clientHeight) {
      this.rolagem.scrollTop = base - this.rolagem.clientHeight;
    }

    const esquerda =
      LARGURA_DO_NUMERO +
      this.larguras.slice(0, colunaLimitada).reduce((soma, valor) => soma + valor, 0);
    const direita = esquerda + this.larguras[colunaLimitada];
    // A coluna do número não rola: parar a célula em `esquerda` a esconderia
    // debaixo dela.
    const inicioAVista = esquerda - LARGURA_DO_NUMERO;
    if (inicioAVista < this.rolagem.scrollLeft) this.rolagem.scrollLeft = inicioAVista;
    else if (direita > this.rolagem.scrollLeft + this.rolagem.clientWidth) {
      this.rolagem.scrollLeft = direita - this.rolagem.clientWidth;
    }

    this.desenhar();
    this.cabecalho.scrollLeft = this.rolagem.scrollLeft;
  }

  selecionarTudo(): void {
    this.colunaSelecionada = null;
    this.selecionadas = new Set(this.ordem);
    this.desenhar();
  }

  /** Texto das linhas selecionadas, separado por tabulação. */
  texto(comCabecalho: boolean): string {
    const escolhidas = this.indicesEscolhidos();
    const colunas = this.indicesDeColunasEscolhidas();
    const partes: string[] = [];
    if (comCabecalho) {
      partes.push(colunas.map((indice) => this.colunas[indice].nome).join("\t"));
    }
    for (const indice of escolhidas) {
      partes.push(colunas.map((coluna) => this.linhas[indice][coluna] ?? "").join("\t"));
    }
    return partes.join("\r\n");
  }

  async copiar(comCabecalho: boolean): Promise<void> {
    await navigator.clipboard.writeText(this.texto(comCabecalho));
  }

  /** Copia uma tabela rica, pronta para ser colada no Microsoft Teams. */
  async copiarParaTeams(): Promise<void> {
    const textoPlano = this.texto(true);

    // ClipboardItem faz o WebView2 publicar CF_HTML no clipboard do Windows.
    // O texto tabulado continua disponível para aplicativos que não aceitam HTML.
    if (typeof ClipboardItem === "undefined" || typeof navigator.clipboard.write !== "function") {
      await navigator.clipboard.writeText(textoPlano);
      return;
    }

    try {
      const item = new ClipboardItem({
        "text/html": new Blob([this.htmlParaTeams()], { type: "text/html" }),
        "text/plain": new Blob([textoPlano], { type: "text/plain" }),
      });
      await navigator.clipboard.write([item]);
    } catch {
      // Mantém a cópia utilizável em WebViews sem suporte a HTML no clipboard.
      await navigator.clipboard.writeText(textoPlano);
    }
  }

  private indicesEscolhidos(): number[] {
    return this.selecionadas.size > 0
      ? this.ordem.filter((indice) => this.selecionadas.has(indice))
      : this.ordem;
  }

  private indicesDeColunasEscolhidas(): number[] {
    return this.colunaSelecionada === null
      ? this.colunas.map((_, indice) => indice)
      : [this.colunaSelecionada];
  }

  private htmlParaTeams(): string {
    const escolhidas = this.indicesEscolhidos();
    const colunas = this.indicesDeColunasEscolhidas();
    const exibidas = escolhidas.slice(0, MAXIMO_DE_LINHAS_PARA_TEAMS);
    const partes = [
      '<table border="1" style="border-collapse:collapse;font-family:Arial,sans-serif;font-size:12px">',
      "<thead><tr>",
    ];

    for (const indice of colunas) {
      partes.push(
        `<th style="padding:5px;background:#f2f2f2">${escaparHtml(this.colunas[indice].nome)}</th>`,
      );
    }
    partes.push("</tr></thead><tbody>");

    for (const indice of exibidas) {
      partes.push("<tr>");
      for (const coluna of colunas) {
        const valor = this.linhas[indice][coluna];
        partes.push(`<td style="padding:5px">${escaparHtml(valor ?? "")}</td>`);
      }
      partes.push("</tr>");
    }

    if (escolhidas.length > MAXIMO_DE_LINHAS_PARA_TEAMS) {
      partes.push(
        `<tr><td colspan="${Math.max(1, colunas.length)}" ` +
          'style="padding:5px;text-align:center;color:red">... mais registros omitidos</td></tr>',
      );
    }

    partes.push("</tbody></table>");
    return partes.join("");
  }

  /**
   * Colunas e linhas escolhidas — a base do script de INSERT (requisito 39).
   *
   * As linhas são as marcadas, ou todas quando não há marca nenhuma, como em
   * copiar e exportar. As colunas são **todas**, mesmo que a escolha seja de
   * uma célula só: um INSERT de uma coluna não serve para nada, e quem clicou
   * numa célula quer o registro dela.
   */
  paraScriptDeInsert(): { colunas: ColunaResultado[]; linhas: Linha[] } {
    return {
      colunas: this.colunas,
      linhas: this.indicesEscolhidos().map((indice) => this.linhas[indice]),
    };
  }

  /** Linhas atualmente carregadas, na ordem exibida — base da exportação. */
  paraExportacao(): { colunas: string[]; linhas: Linha[] } {
    return {
      colunas: this.colunas.map((coluna) => coluna.nome),
      linhas: this.ordem.map((indice) => this.linhas[indice]),
    };
  }

  /** Precisa ser chamado quando o grid entra na tela, para medir a altura. */
  ajustar(): void {
    this.desenhar();
  }
}

/**
 * Régua de texto: um canvas fora da tela, criado uma vez só, para saber a
 * largura de um texto sem escrevê-lo em lugar nenhum.
 */
let regua: CanvasRenderingContext2D | null = null;

function medirTexto(texto: string, fonte: string): number {
  regua ??= document.createElement("canvas").getContext("2d");
  // Sem canvas — situação que não deve acontecer no WebView2 —, a estimativa
  // por caractere ainda deixa a coluna utilizável.
  if (!regua) return texto.length * 7;
  regua.font = fonte;
  return regua.measureText(texto).width;
}

/** `1.234,56` → 1234.56, para ordenar coluna numérica formatada em pt-BR. */
function paraNumero(texto: string): number {
  const numero = Number(texto.replace(/\./g, "").replace(",", "."));
  return Number.isNaN(numero) ? 0 : numero;
}

function escaparHtml(texto: string): string {
  return texto.replace(
    /[&<>"']/g,
    (caractere) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        caractere
      ] ?? caractere,
  );
}
