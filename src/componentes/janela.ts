/**
 * Transforma uma caixa de diálogo em janela: move pela barra de título,
 * redimensiona pelas bordas e maximiza.
 *
 * As caixas do Cururu nascem centradas pelo `flex` de `.dialogo-fundo`, com o
 * tamanho que a folha de estilo der. Isso basta para um formulário curto, mas
 * não para uma tela que carrega um comando inteiro dentro: o T-SQL de uma etapa
 * de trabalho tem cem linhas com facilidade, e uma caixa de tamanho fixo obriga
 * a ler tudo por uma fresta.
 *
 * A partir do primeiro arrasto a caixa sai da centralização e passa a
 * `position: absolute`, nas coordenadas que o usuário escolheu — é o que
 * permite mover e redimensionar sem que o `flex` a puxe de volta para o meio.
 * Enquanto ninguém arrasta nada, o comportamento é o de sempre.
 *
 * Mora aqui também o `fecharComEsc`, que é o outro gesto de janela: fechar.
 */

import "./janela.css";

import { t } from "../idioma";

/** Os mesmos desenhos da barra de título da janela do Windows. */
const ICONE_MAXIMIZAR =
  '<svg viewBox="0 0 10 10" fill="none" stroke="currentColor"><rect x="0.5" y="0.5" width="9" height="9"/></svg>';
const ICONE_RESTAURAR =
  '<svg viewBox="0 0 10 10" fill="none" stroke="currentColor"><rect x="2.5" y="0.5" width="6" height="6"/><rect x="0.5" y="3.5" width="6" height="6"/></svg>';

/** Quanto da janela precisa continuar dentro da tela, em pixels. */
const VISIVEL = 64;

/** As oito alças: quatro bordas e quatro cantos. */
const DIRECOES = ["n", "s", "e", "w", "ne", "nw", "se", "sw"] as const;
type Direcao = (typeof DIRECOES)[number];

export interface OpcoesDaJanela {
  /** Põe as alças de arrasto nas bordas e nos cantos. */
  redimensionavel?: boolean;
  /** Põe o botão de maximizar na barra e liga o duplo clique nela. */
  maximizavel?: boolean;
  larguraMinima?: number;
  alturaMinima?: number;
}

/**
 * Liga o comportamento de janela e devolve como desligá-lo — que a tela precisa
 * chamar ao fechar, para não deixar o observador de redimensionamento apontando
 * para uma caixa que não existe mais.
 */
export function tornarJanela(caixa: HTMLElement, opcoes: OpcoesDaJanela = {}): () => void {
  const {
    redimensionavel = false,
    maximizavel = false,
    larguraMinima = 320,
    alturaMinima = 200,
  } = opcoes;

  caixa.classList.add("janela");
  const barra = caixa.querySelector<HTMLElement>(".dialogo__barra");

  /** Onde a caixa estava antes de maximizar. */
  let anterior: Retangulo | null = null;

  const estaMaximizada = (): boolean => caixa.classList.contains("janela--maximizada");

  const posicionar = (r: Retangulo): void => {
    caixa.style.left = r.esquerda;
    caixa.style.top = r.topo;
    caixa.style.width = r.largura;
    caixa.style.height = r.altura;
  };

  /**
   * Congela a caixa onde ela está.
   *
   * Sem isto não há o que arrastar: enquanto o tamanho vem da folha de estilo e
   * a posição vem do `flex`, mudar `left` não move nada.
   */
  const fixar = (): void => {
    if (caixa.classList.contains("janela--posicionada")) return;
    const area = caixa.getBoundingClientRect();
    caixa.classList.add("janela--posicionada");
    posicionar({
      esquerda: `${Math.round(area.left)}px`,
      topo: `${Math.round(area.top)}px`,
      largura: `${Math.round(area.width)}px`,
      altura: `${Math.round(area.height)}px`,
    });
  };

  // --- Maximizar -----------------------------------------------------------

  let botaoMaximizar: HTMLButtonElement | null = null;

  const desenharBotao = (): void => {
    if (!botaoMaximizar) return;
    const rotulo = t(estaMaximizada() ? "restaurar janela" : "maximizar janela");
    botaoMaximizar.innerHTML = estaMaximizada() ? ICONE_RESTAURAR : ICONE_MAXIMIZAR;
    botaoMaximizar.title = rotulo;
    botaoMaximizar.setAttribute("aria-label", rotulo);
  };

  const alternarMaximizada = (): void => {
    if (estaMaximizada()) {
      caixa.classList.remove("janela--maximizada");
      if (anterior) posicionar(anterior);
      anterior = null;
    } else {
      fixar();
      anterior = {
        esquerda: caixa.style.left,
        topo: caixa.style.top,
        largura: caixa.style.width,
        altura: caixa.style.height,
      };
      caixa.classList.add("janela--maximizada");
      // Em porcentagem, e não em pixels: `.dialogo-fundo` cobre a janela
      // inteira, então a caixa continua cheia mesmo se o usuário redimensionar
      // o Cururu com a tela aberta.
      posicionar({ esquerda: "0px", topo: "0px", largura: "100%", altura: "100%" });
    }
    desenharBotao();
  };

  if (maximizavel && barra) {
    botaoMaximizar = document.createElement("button");
    botaoMaximizar.type = "button";
    botaoMaximizar.className = "janela__maximizar";
    botaoMaximizar.addEventListener("click", alternarMaximizada);
    barra.insertBefore(botaoMaximizar, barra.querySelector(".dialogo__fechar"));
    desenharBotao();
    barra.addEventListener("dblclick", (evento) => {
      if ((evento.target as HTMLElement).closest("button")) return;
      alternarMaximizada();
    });
  }

  // --- Mover ---------------------------------------------------------------

  let arrastando = false;
  let pegaX = 0;
  let pegaY = 0;

  const aoMover = (evento: PointerEvent): void => {
    if (!arrastando) return;
    // A janela pode sair pelos lados, mas nunca a ponto de a barra de título
    // ficar fora do alcance do ponteiro — de lá não haveria como trazê-la de
    // volta.
    const esquerda = limitar(
      evento.clientX - pegaX,
      VISIVEL - caixa.offsetWidth,
      window.innerWidth - VISIVEL,
    );
    const topo = limitar(evento.clientY - pegaY, 0, window.innerHeight - VISIVEL);
    caixa.style.left = `${Math.round(esquerda)}px`;
    caixa.style.top = `${Math.round(topo)}px`;
  };

  if (barra) {
    barra.classList.add("janela__barra");
    barra.addEventListener("pointerdown", (evento) => {
      // Só o botão esquerdo, e nunca a partir dos botões da barra: o clique de
      // fechar não pode virar arrasto.
      if (evento.button !== 0 || estaMaximizada()) return;
      if ((evento.target as HTMLElement).closest("button")) return;
      fixar();
      const area = caixa.getBoundingClientRect();
      pegaX = evento.clientX - area.left;
      pegaY = evento.clientY - area.top;
      arrastando = true;
      barra.setPointerCapture(evento.pointerId);
      evento.preventDefault();
    });
    barra.addEventListener("pointermove", aoMover);
    const soltar = (evento: PointerEvent): void => {
      if (!arrastando) return;
      arrastando = false;
      barra.releasePointerCapture(evento.pointerId);
    };
    barra.addEventListener("pointerup", soltar);
    barra.addEventListener("pointercancel", soltar);
  }

  // --- Redimensionar -------------------------------------------------------

  let direcao: Direcao | null = null;
  let inicio = { x: 0, y: 0, esquerda: 0, topo: 0, largura: 0, altura: 0 };

  const aoRedimensionar = (evento: PointerEvent): void => {
    if (!direcao) return;
    const dx = evento.clientX - inicio.x;
    const dy = evento.clientY - inicio.y;
    let { esquerda, topo, largura, altura } = inicio;

    if (direcao.includes("e")) {
      largura = limitar(inicio.largura + dx, larguraMinima, window.innerWidth - esquerda);
    }
    if (direcao.includes("s")) {
      altura = limitar(inicio.altura + dy, alturaMinima, window.innerHeight - topo);
    }
    // Arrastar a borda esquerda e a de cima muda o tamanho **e** a posição: o
    // lado oposto é que fica parado.
    if (direcao.includes("w")) {
      const direita = inicio.esquerda + inicio.largura;
      largura = limitar(inicio.largura - dx, larguraMinima, direita);
      esquerda = direita - largura;
    }
    if (direcao.includes("n")) {
      const base = inicio.topo + inicio.altura;
      altura = limitar(inicio.altura - dy, alturaMinima, base);
      topo = base - altura;
    }

    posicionar({
      esquerda: `${Math.round(esquerda)}px`,
      topo: `${Math.round(topo)}px`,
      largura: `${Math.round(largura)}px`,
      altura: `${Math.round(altura)}px`,
    });
  };

  if (redimensionavel) {
    for (const lado of DIRECOES) {
      const aresta = document.createElement("div");
      aresta.className = `janela__aresta janela__aresta--${lado}`;
      aresta.addEventListener("pointerdown", (evento) => {
        if (evento.button !== 0 || estaMaximizada()) return;
        fixar();
        const area = caixa.getBoundingClientRect();
        inicio = {
          x: evento.clientX,
          y: evento.clientY,
          esquerda: area.left,
          topo: area.top,
          largura: area.width,
          altura: area.height,
        };
        direcao = lado;
        aresta.setPointerCapture(evento.pointerId);
        evento.preventDefault();
      });
      aresta.addEventListener("pointermove", aoRedimensionar);
      const soltar = (evento: PointerEvent): void => {
        if (!direcao) return;
        direcao = null;
        aresta.releasePointerCapture(evento.pointerId);
      };
      aresta.addEventListener("pointerup", soltar);
      aresta.addEventListener("pointercancel", soltar);
      caixa.append(aresta);
    }
  }

  // --- Janela do Cururu diminuindo ------------------------------------------

  /**
   * Traz a caixa de volta para dentro da tela.
   *
   * Uma janela deixada à direita e um Cururu restaurado do maximizado sumiriam
   * juntos: a caixa continuaria nas coordenadas antigas, agora fora da vista.
   */
  const acomodar = (): void => {
    if (!caixa.classList.contains("janela--posicionada") || estaMaximizada()) return;
    const area = caixa.getBoundingClientRect();
    const largura = Math.min(area.width, window.innerWidth);
    const altura = Math.min(area.height, window.innerHeight);
    posicionar({
      esquerda: `${Math.round(limitar(area.left, 0, window.innerWidth - largura))}px`,
      topo: `${Math.round(limitar(area.top, 0, window.innerHeight - altura))}px`,
      largura: `${Math.round(largura)}px`,
      altura: `${Math.round(altura)}px`,
    });
  };

  window.addEventListener("resize", acomodar);
  return () => window.removeEventListener("resize", acomodar);
}

/**
 * Fecha a tela com Esc, com o foco onde estiver.
 *
 * **É por aqui que toda janela extra do Cururu fecha com Esc** — a exigência
 * vale para todas, sem exceção, e sair assim descarta o que estava sendo
 * editado, sem gravar e sem perguntar.
 *
 * O `keydown` preso ao fundo do diálogo — o jeito antigo, que cada tela repetia
 * por conta própria — só recebe a tecla enquanto o foco está dentro dele. Basta
 * clicar numa linha da grade, que não é focável, para o foco voltar ao `body` e
 * o Esc deixar de fechar a tela. Daí a escuta ir na janela, e a tela devolver
 * como desligá-la ao fechar.
 *
 * `podeFechar` é para a tela que não pode sumir no meio de uma gravação.
 *
 * Um campo que captura teclas — o de atalho, na tela de Configurações e na de
 * botões rápidos — precisa deixar o Esc passar sem `preventDefault` nem
 * `stopPropagation`: Esc é a tecla de fechar, e não uma combinação gravável.
 */
export function fecharComEsc(
  fundo: HTMLElement,
  fechar: () => void,
  podeFechar: () => boolean = () => true,
): () => void {
  const aoTeclar = (evento: KeyboardEvent): void => {
    // Tecla já tratada não é desta tela: dentro do editor, o Esc fecha primeiro
    // o autocomplete ou o localizar, e fechar a tela ali levaria junto o
    // comando digitado.
    if (evento.key !== "Escape" || evento.defaultPrevented || !podeFechar()) return;
    // Com outra tela aberta por cima, o Esc é dela — e é por isso que quem
    // fecha marca a tecla como tratada, antes que a tela de baixo se descubra
    // a última da pilha.
    const telas = document.querySelectorAll(".dialogo-fundo");
    if (telas[telas.length - 1] !== fundo) return;
    evento.preventDefault();
    fechar();
  };
  window.addEventListener("keydown", aoTeclar);
  return () => window.removeEventListener("keydown", aoTeclar);
}

/** Posição e tamanho, já em texto de CSS. */
interface Retangulo {
  esquerda: string;
  topo: string;
  largura: string;
  altura: string;
}

function limitar(valor: number, minimo: number, maximo: number): number {
  return Math.min(Math.max(valor, minimo), Math.max(minimo, maximo));
}
