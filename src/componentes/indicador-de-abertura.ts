import "./indicador-de-abertura.css";
import { t } from "../idioma";

const aberturasVisiveis = new Map<symbol, string>();
let indicador: HTMLElement | undefined;

/** Sinaliza uma abertura lenta sem piscar em operações rápidas. */
export async function comIndicadorDeAbertura<T>(
  nome: string,
  abrir: () => Promise<T>,
): Promise<T> {
  const elemento = indicador ??= montarIndicador();
  const id = Symbol();
  const espera = window.setTimeout(() => {
    aberturasVisiveis.set(id, t("carregando abertura", { nome }));
    atualizar(elemento);
  }, 1000);

  try {
    return await abrir();
  } finally {
    window.clearTimeout(espera);
    if (aberturasVisiveis.delete(id)) atualizar(elemento);
  }
}

function montarIndicador(): HTMLElement {
  const elemento = document.createElement("div");
  elemento.className = "indicador-de-abertura";
  elemento.setAttribute("role", "status");
  document.body.append(elemento);
  return elemento;
}

function atualizar(elemento: HTMLElement): void {
  // Se uma abertura termina antes de outra, ainda há algo para mostrar.
  const mensagem = Array.from(aberturasVisiveis.values()).at(-1) ?? "";
  if (elemento.textContent !== mensagem) elemento.textContent = mensagem;
}
