/** Configuração persistente dos atalhos gerais e do editor SQL. */

import { t, type Chave } from "./idioma";

export type IdDeAtalho =
  | "abrir-consulta"
  | "salvar-consulta"
  | "nova-consulta"
  | "fechar-aba"
  | "executar"
  | "executar-script"
  | "compilar"
  | "duplicar-linha"
  | "dividir-linhas"
  | "juntar-linhas"
  | "excluir-linha"
  | "minusculas"
  | "maiusculas"
  | "mover-linha-acima"
  | "mover-linha-abaixo"
  | "apagar-palavra-anterior"
  | "apagar-palavra-seguinte"
  | "apagar-inicio-linha"
  | "apagar-fim-linha"
  | "comentar-linhas"
  | "comentar-bloco"
  | "selecao-coluna-acima"
  | "selecao-coluna-abaixo"
  | "reduzir-indentacao"
  | "aumentar-indentacao";

export interface DefinicaoDeAtalho {
  id: IdDeAtalho;
  padrao: string;
  acao: Chave;
  escopo: "janela" | "editor";
}

/**
 * As duas teclas aceitas para a mesma ação. Qualquer posição pode ser `""` —
 * uma ação sem tecla, seja porque o usuário limpou o campo, seja porque outra
 * ação tomou a combinação dela. `""` nunca combina com evento nenhum, então a
 * ação sem tecla apenas deixa de responder.
 */
export type ParDeAtalhos = [string, string];

export type ConfiguracaoDeAtalhos = Record<IdDeAtalho, ParDeAtalhos>;

export const DEFINICOES_DE_ATALHOS: readonly DefinicaoDeAtalho[] = [
  { id: "abrir-consulta", padrao: "Ctrl+O", acao: "atalho abrir consulta", escopo: "janela" },
  { id: "salvar-consulta", padrao: "Ctrl+S", acao: "atalho salvar consulta", escopo: "janela" },
  { id: "nova-consulta", padrao: "Ctrl+T", acao: "atalho nova consulta", escopo: "janela" },
  { id: "fechar-aba", padrao: "Ctrl+W", acao: "atalho fechar aba", escopo: "janela" },
  { id: "executar", padrao: "Ctrl+Enter", acao: "atalho executar", escopo: "editor" },
  { id: "executar-script", padrao: "F5", acao: "atalho executar script", escopo: "editor" },
  { id: "compilar", padrao: "Ctrl+Shift+C", acao: "atalho compilar", escopo: "editor" },
  { id: "duplicar-linha", padrao: "Ctrl+D", acao: "atalho duplicar linha", escopo: "editor" },
  { id: "dividir-linhas", padrao: "Ctrl+I", acao: "atalho dividir linhas", escopo: "editor" },
  { id: "juntar-linhas", padrao: "Ctrl+J", acao: "atalho juntar linhas", escopo: "editor" },
  { id: "excluir-linha", padrao: "Ctrl+L", acao: "atalho excluir linha", escopo: "editor" },
  { id: "minusculas", padrao: "Ctrl+U", acao: "atalho minusculas", escopo: "editor" },
  { id: "maiusculas", padrao: "Ctrl+Shift+U", acao: "atalho maiusculas", escopo: "editor" },
  { id: "mover-linha-acima", padrao: "Ctrl+Shift+Seta acima", acao: "atalho mover linha acima", escopo: "editor" },
  { id: "mover-linha-abaixo", padrao: "Ctrl+Shift+Seta abaixo", acao: "atalho mover linha abaixo", escopo: "editor" },
  { id: "apagar-palavra-anterior", padrao: "Ctrl+Backspace", acao: "atalho apagar palavra anterior", escopo: "editor" },
  { id: "apagar-palavra-seguinte", padrao: "Ctrl+Delete", acao: "atalho apagar palavra seguinte", escopo: "editor" },
  { id: "apagar-inicio-linha", padrao: "Ctrl+Shift+Backspace", acao: "atalho apagar inicio linha", escopo: "editor" },
  { id: "apagar-fim-linha", padrao: "Ctrl+Shift+Delete", acao: "atalho apagar fim linha", escopo: "editor" },
  { id: "comentar-linhas", padrao: "Ctrl+-", acao: "atalho comentar linhas", escopo: "editor" },
  { id: "comentar-bloco", padrao: "Ctrl+/", acao: "atalho comentar bloco", escopo: "editor" },
  { id: "selecao-coluna-acima", padrao: "Alt+Shift+Seta acima", acao: "atalho selecao coluna acima", escopo: "editor" },
  { id: "selecao-coluna-abaixo", padrao: "Alt+Shift+Seta abaixo", acao: "atalho selecao coluna abaixo", escopo: "editor" },
  { id: "reduzir-indentacao", padrao: "Shift+Tab", acao: "atalho reduzir indentacao", escopo: "editor" },
  { id: "aumentar-indentacao", padrao: "Tab", acao: "atalho aumentar indentacao", escopo: "editor" },
] as const;

const CHAVE_ARMAZENAMENTO = "cururu.atalhos.v1";
const EVENTO_DE_ALTERACAO = "cururu:atalhos-alterados";

export function atalhosPadrao(): ConfiguracaoDeAtalhos {
  return Object.fromEntries(
    DEFINICOES_DE_ATALHOS.map((item) => [item.id, [item.padrao, ""]]),
  ) as ConfiguracaoDeAtalhos;
}

/**
 * Aceita tanto o par atual (`[principal, secundario]`) quanto uma única
 * string — o formato salvo por uma versão anterior do Cururu, que só tinha
 * uma tecla por ação.
 */
function paraPar(candidato: unknown): ParDeAtalhos | null {
  if (typeof candidato === "string") return [candidato, ""];
  if (
    Array.isArray(candidato) &&
    candidato.length <= 2 &&
    candidato.every((item) => typeof item === "string")
  ) {
    return [candidato[0] ?? "", candidato[1] ?? ""];
  }
  return null;
}

function normalizar(valor: unknown): ConfiguracaoDeAtalhos | null {
  if (!valor || typeof valor !== "object" || Array.isArray(valor)) return null;
  const recebido = valor as Record<string, unknown>;
  const resultado = atalhosPadrao();
  const usados = new Set<string>();

  // Só a tecla ausente ("") pode se repetir — ela não é atalho de ninguém.
  const registrar = (atalho: string): boolean => {
    if (!atalho) return true;
    if (usados.has(atalho)) return false;
    usados.add(atalho);
    return true;
  };

  for (const definicao of DEFINICOES_DE_ATALHOS) {
    const [principal, secundario] = paraPar(recebido[definicao.id]) ?? [definicao.padrao, ""];
    if (!registrar(principal) || !registrar(secundario)) return null;
    resultado[definicao.id] = [principal, secundario];
  }
  return resultado;
}

function carregar(): ConfiguracaoDeAtalhos {
  try {
    const salvo = localStorage.getItem(CHAVE_ARMAZENAMENTO);
    return salvo ? normalizar(JSON.parse(salvo)) ?? atalhosPadrao() : atalhosPadrao();
  } catch {
    return atalhosPadrao();
  }
}

let configuracao = carregar();

export function obterAtalhos(): ConfiguracaoDeAtalhos {
  // Cópia rasa não basta: o valor de cada ação é um array, e devolver a mesma
  // referência deixaria quem editar o rascunho da tela mexer sem querer na
  // configuração vigente antes de clicar em Salvar.
  return Object.fromEntries(
    Object.entries(configuracao).map(([id, par]) => [id, [...par]]),
  ) as ConfiguracaoDeAtalhos;
}

/**
 * O sufixo " (tecla)" da ação, para pôr num rótulo de botão ou de menu.
 *
 * Sai da configuração vigente, e **não** de um texto fixo: a tecla é
 * configurável (requisito 32), e um rótulo que anuncie a combinação de fábrica
 * depois de o usuário tê-la trocado ensina o caminho errado.
 *
 * Devolve vazio quando a ação está sem atalho principal — o que é permitido
 * desde a 0.1.35 —, para o rótulo não terminar num parêntese oco.
 */
export function sufixoDoAtalho(id: IdDeAtalho): string {
  const principal = configuracao[id]?.[0];
  return principal ? ` (${atalhoVisivel(principal)})` : "";
}

/** As teclas cujo nome na notação é palavra, e não símbolo. */
const NOMES_DE_TECLAS: Record<string, Chave> = {
  "Espaço": "tecla espaco",
  "Seta acima": "tecla seta acima",
  "Seta abaixo": "tecla seta abaixo",
  "Seta esquerda": "tecla seta esquerda",
  "Seta direita": "tecla seta direita",
};

/**
 * A combinação como ela aparece na tela.
 *
 * A notação **guardada** é sempre a portuguesa — é ela que vai para o
 * `localStorage`, para o backup e para o `paraCodeMirror` —, e só o nome da
 * tecla é traduzido na hora de mostrar. Traduzir o que se guarda deixaria sem
 * dono, ao abrir o Cururu em English, todo atalho gravado em português; e um
 * backup feito num idioma não abriria no outro.
 *
 * `Ctrl`, `Alt` e `Shift` ficam como estão: é como o Windows os escreve nos
 * três idiomas.
 */
export function atalhoVisivel(atalho: string): string {
  if (!atalho) return "";
  const partes = atalho.split("+");
  const tecla = partes.pop() ?? "";
  const chave = NOMES_DE_TECLAS[tecla];
  return [...partes, chave ? t(chave) : tecla].join("+");
}

export function substituirAtalhos(valor: unknown): void {
  const novos = normalizar(valor);
  if (!novos) throw new Error(t("atalhos invalidos"));
  configuracao = novos;
  localStorage.setItem(CHAVE_ARMAZENAMENTO, JSON.stringify(configuracao));
  window.dispatchEvent(new Event(EVENTO_DE_ALTERACAO));
}

export function restaurarAtalhosPadrao(): void {
  substituirAtalhos(atalhosPadrao());
}

export function observarAtalhos(aoAlterar: () => void): () => void {
  window.addEventListener(EVENTO_DE_ALTERACAO, aoAlterar);
  return () => window.removeEventListener(EVENTO_DE_ALTERACAO, aoAlterar);
}

export function eventoCorrespondeAoAtalho(evento: KeyboardEvent, id: IdDeAtalho): boolean {
  const texto = textoDoEvento(evento);
  // "" é a marca de "sem atalho alternativo": não pode combinar com um evento
  // vazio (só modificador solto), senão toda ação sem o segundo atalho
  // passaria a responder a Ctrl sozinho.
  return texto !== "" && configuracao[id].includes(texto);
}

export function eventoEhAtalhoConfigurado(evento: KeyboardEvent): boolean {
  const texto = textoDoEvento(evento);
  if (!texto) return false;
  return Object.values(configuracao).some((par) => par.includes(texto));
}

/** Notação única usada na tela, no armazenamento e pelos eventos do navegador. */
export function textoDoEvento(evento: KeyboardEvent): string {
  if (["Control", "Shift", "Alt", "Meta"].includes(evento.key)) return "";
  const nomes: Record<string, string> = {
    " ": "Espaço",
    ArrowUp: "Seta acima",
    ArrowDown: "Seta abaixo",
    ArrowLeft: "Seta esquerda",
    ArrowRight: "Seta direita",
    PageUp: "Page Up",
    PageDown: "Page Down",
  };
  let tecla = nomes[evento.key] ?? evento.key;
  if (/^Key[A-Z]$/.test(evento.code)) tecla = evento.code.slice(3);
  else if (/^Digit[0-9]$/.test(evento.code)) tecla = evento.code.slice(5);
  const partes: string[] = [];
  if (evento.ctrlKey || evento.metaKey) partes.push("Ctrl");
  if (evento.altKey) partes.push("Alt");
  if (evento.shiftKey) partes.push("Shift");
  partes.push(tecla.length === 1 ? tecla.toUpperCase() : tecla);
  return partes.join("+");
}

/** Converte a notação visível para a aceita pelo keymap do CodeMirror. */
export function paraCodeMirror(atalho: string): string {
  const partes = atalho.split("+");
  const tecla = partes.pop() ?? "";
  const nomes: Record<string, string> = {
    "Seta acima": "ArrowUp",
    "Seta abaixo": "ArrowDown",
    "Seta esquerda": "ArrowLeft",
    "Seta direita": "ArrowRight",
    "Page Up": "PageUp",
    "Page Down": "PageDown",
    "Espaço": "Space",
  };
  return [...partes, nomes[tecla] ?? tecla].join("-");
}
