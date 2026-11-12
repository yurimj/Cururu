/**
 * Tema da interface (Claro/Escuro), no mesmo padrão de idioma.ts: escolha
 * guardada na máquina, aplicada na hora, sem precisar reabrir o aplicativo.
 *
 * A troca em si é responsabilidade do CSS: tudo em estilo.css e nas folhas de
 * estilo das telas lê cor de variável, e o atributo `data-tema` no `<html>`
 * decide qual bloco de variáveis vale. Este módulo só guarda a escolha e
 * avisa quem precisa reagir em JavaScript — hoje, só o destaque de sintaxe do
 * editor SQL, que não é CSS.
 */

import type { Chave } from "./idioma";

export type Tema = "claro" | "escuro";

/** Os dois temas oferecidos, com a chave do catálogo do rótulo de cada um. */
export const TEMAS: { codigo: Tema; chave: Chave }[] = [
  { codigo: "claro", chave: "tema claro" },
  { codigo: "escuro", chave: "tema escuro" },
];

const CHAVE_ARMAZENAMENTO = "cururu.tema";
const EVENTO_DE_ALTERACAO = "cururu:tema-alterado";

/** Sem escolha guardada, o padrão é o tema claro de sempre — nada de detectar o SO. */
function lerGuardado(): Tema {
  return localStorage.getItem(CHAVE_ARMAZENAMENTO) === "escuro" ? "escuro" : "claro";
}

let atual: Tema = lerGuardado();
// Aplicado já na carga do módulo, antes de a tela ser montada: evita um
// lampejo do tema claro no instante em que o aplicativo abre no escuro.
document.documentElement.setAttribute("data-tema", atual);

export function temaAtual(): Tema {
  return atual;
}

export function definirTema(novo: Tema): void {
  if (novo === atual) return;
  atual = novo;
  localStorage.setItem(CHAVE_ARMAZENAMENTO, novo);
  document.documentElement.setAttribute("data-tema", novo);
  window.dispatchEvent(new CustomEvent(EVENTO_DE_ALTERACAO));
}

/**
 * Avisa só as trocas futuras, sem chamar de imediato — mesmo contrato de
 * `observarAtalhos`. Serve para reconfigurar algo que já nasceu lendo
 * `temaAtual()` na hora da criação (o destaque de sintaxe do editor), não
 * para escrever o rótulo inicial de uma tela.
 */
export function observarTema(aoTrocar: () => void): () => void {
  window.addEventListener(EVENTO_DE_ALTERACAO, aoTrocar);
  return () => window.removeEventListener(EVENTO_DE_ALTERACAO, aoTrocar);
}
