/**
 * Zoom do editor de SQL, pelo Ctrl+roda do mouse.
 *
 * Guardado na máquina e aplicado na hora, no mesmo padrão de tema.ts. O ajuste
 * em si é responsabilidade do CSS: `estilo.css` calcula o tamanho da fonte do
 * editor a partir de `--zoom-editor`, e este módulo só reescreve essa variável
 * no `<html>`. Um multiplicador, e não um tamanho em pixels, porque assim o
 * tamanho-base continua morando num lugar só, o CSS.
 *
 * **Só a roda**, sem tecla: o Ctrl+- do teclado já é o "Comentar linhas" do
 * editor, e o Cururu não deixa dois comandos dividirem a mesma tecla.
 *
 * O zoom é do editor, não da janela: o Tauri nasce com `zoomHotkeysEnabled`
 * desligado, então o WebView2 não disputa o Ctrl+roda.
 */

const CHAVE_ARMAZENAMENTO = "cururu.zoom-editor";
const EVENTO_DE_ALTERACAO = "cururu:zoom-editor-alterado";
const PROPRIEDADE_CSS = "--zoom-editor";

/** 100%: o tamanho escrito em `estilo.css`. */
export const ZOOM_PADRAO = 1;

const MINIMO = 0.6;
const MAXIMO = 3;
const PASSO = 0.1;

/**
 * Arredonda para uma casa antes de limitar.
 *
 * Somar 0.1 dez vezes em ponto flutuante não devolve exatamente 1, e sem o
 * arredondamento a tela mostraria "110.00000000000001%" e o zoom nunca voltaria
 * a bater com `ZOOM_PADRAO`.
 */
function limitar(valor: number): number {
  return Math.min(Math.max(Math.round(valor * 10) / 10, MINIMO), MAXIMO);
}

function lerGuardado(): number {
  const salvo = Number(localStorage.getItem(CHAVE_ARMAZENAMENTO));
  return Number.isFinite(salvo) && salvo > 0 ? limitar(salvo) : ZOOM_PADRAO;
}

let atual = lerGuardado();

function aplicar(): void {
  document.documentElement.style.setProperty(PROPRIEDADE_CSS, String(atual));
}

// Aplicado já na carga do módulo, antes de a tela ser montada: evita o editor
// abrir no tamanho de fábrica e saltar para o do usuário no primeiro quadro.
aplicar();

function definirZoomDoEditor(valor: number): void {
  const novo = limitar(valor);
  if (novo === atual) return;
  atual = novo;
  localStorage.setItem(CHAVE_ARMAZENAMENTO, String(novo));
  aplicar();
  window.dispatchEvent(new Event(EVENTO_DE_ALTERACAO));
}

/** Um passo para cada entalhe da roda: positivo aumenta, negativo reduz. */
export function ajustarZoomDoEditor(passos: number): void {
  definirZoomDoEditor(atual + passos * PASSO);
}

/**
 * Avisa só as trocas futuras, sem chamar de imediato — mesmo contrato de
 * `observarTema`. Serve para o editor remedir a fonte depois do zoom; o
 * tamanho inicial já veio do CSS.
 */
export function observarZoomDoEditor(aoMudar: () => void): () => void {
  window.addEventListener(EVENTO_DE_ALTERACAO, aoMudar);
  return () => window.removeEventListener(EVENTO_DE_ALTERACAO, aoMudar);
}
