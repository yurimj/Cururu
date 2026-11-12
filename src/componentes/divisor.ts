/**
 * Divisor arrastável entre dois painéis vizinhos.
 *
 * `vertical` divide painéis lado a lado, arrasta na horizontal e muda a largura
 * do da esquerda. `horizontal` divide painéis empilhados, arrasta na vertical e
 * muda a altura do de cima. Em ambos os casos o vizinho fica com o resto.
 *
 * O painel redimensionado sai da divisão proporcional do flex: a partir do
 * primeiro arrasto o tamanho dele é o que o usuário escolheu, e não mais a
 * porcentagem da folha de estilo.
 */

import "./divisor.css";
import { t } from "../idioma";

export interface OpcoesDoDivisor {
  orientacao: "horizontal" | "vertical";
  /** O painel que muda de tamanho: o de cima, ou o da esquerda. */
  redimensionado: HTMLElement;
  /** O vizinho, que absorve o espaço restante. */
  vizinho: HTMLElement;
  /** Menor tamanho do painel redimensionado, em pixels. */
  minimo?: number;
  /** Quanto se deve deixar para o vizinho, no mínimo, em pixels. */
  minimoDoVizinho?: number;
}

export function criarDivisor(opcoes: OpcoesDoDivisor): HTMLElement {
  const {
    orientacao,
    redimensionado,
    vizinho,
    minimo = 80,
    minimoDoVizinho = 120,
  } = opcoes;
  const vertical = orientacao === "vertical";

  const divisor = document.createElement("div");
  divisor.className = `divisor divisor--${orientacao}`;
  divisor.setAttribute("role", "separator");
  divisor.setAttribute("aria-orientation", vertical ? "vertical" : "horizontal");
  divisor.title = t("arraste para redimensionar");
  divisor.tabIndex = 0;

  const medir = (elemento: HTMLElement): number => {
    const caixa = elemento.getBoundingClientRect();
    return vertical ? caixa.width : caixa.height;
  };

  const aplicar = (pedido: number): void => {
    // A soma dos dois painéis não muda durante o arrasto, então o teto sai
    // dela: é o quanto dá para crescer sem esmagar o vizinho.
    const disponivel = medir(redimensionado) + medir(vizinho);
    const teto = Math.max(minimo, disponivel - minimoDoVizinho);
    const alvo = Math.round(Math.min(Math.max(pedido, minimo), teto));
    redimensionado.style.flex = `0 0 ${alvo}px`;
  };

  let inicio = 0;
  let tamanhoInicial = 0;
  let arrastando = false;

  divisor.addEventListener("pointerdown", (evento) => {
    arrastando = true;
    inicio = vertical ? evento.clientX : evento.clientY;
    tamanhoInicial = medir(redimensionado);
    // A captura mantém os eventos vindo para cá mesmo quando o ponteiro corre
    // por cima do editor ou do grid, que de outro modo os engoliriam.
    divisor.setPointerCapture(evento.pointerId);
    divisor.classList.add("divisor--ativo");
    // Sem isto o arrasto seleciona o texto dos painéis dos dois lados.
    evento.preventDefault();
  });

  divisor.addEventListener("pointermove", (evento) => {
    if (!arrastando) return;
    const atual = vertical ? evento.clientX : evento.clientY;
    aplicar(tamanhoInicial + (atual - inicio));
  });

  const terminar = (evento: PointerEvent): void => {
    if (!arrastando) return;
    arrastando = false;
    divisor.releasePointerCapture(evento.pointerId);
    divisor.classList.remove("divisor--ativo");
  };
  divisor.addEventListener("pointerup", terminar);
  divisor.addEventListener("pointercancel", terminar);

  /** Duplo clique devolve o tamanho que a folha de estilo define. */
  divisor.addEventListener("dblclick", () => {
    redimensionado.style.flex = "";
  });

  // Teclado, para quem não usa mouse: as setas do eixo do divisor.
  divisor.addEventListener("keydown", (evento) => {
    const encolher = vertical ? "ArrowLeft" : "ArrowUp";
    const crescer = vertical ? "ArrowRight" : "ArrowDown";
    const passo = evento.key === encolher ? -16 : evento.key === crescer ? 16 : 0;
    if (passo === 0) return;
    evento.preventDefault();
    aplicar(medir(redimensionado) + passo);
  });

  return divisor;
}
