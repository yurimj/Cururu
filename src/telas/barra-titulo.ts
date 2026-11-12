/**
 * Barra de título própria, no lugar da nativa do Windows.
 *
 * A nativa é sempre clara e não acompanha o tema do Cururu — no escuro,
 * ficaria uma tira branca por cima do resto da janela escura. `decorations:
 * false` em tauri.conf.json tira a barra do sistema; esta a repõe, com os
 * mesmos três botões (minimizar, maximizar/restaurar, fechar) e a mesma
 * faixa arrastável, mas pintada com as variáveis de tema como o resto da
 * tela.
 *
 * Fora da janela do Tauri (navegador de desenvolvimento) os comandos de
 * janela não existem — os botões ficam desenhados, mas sem efeito, a mesma
 * situação da ponte `invoke` descrita em AGENTS.md.
 */

import "./barra-titulo.css";

import { Window, getCurrentWindow } from "@tauri-apps/api/window";

import { t } from "../idioma";

const ICONE_MINIMIZAR =
  '<svg viewBox="0 0 10 10" fill="none" stroke="currentColor"><line x1="0.5" y1="5" x2="9.5" y2="5"/></svg>';
const ICONE_MAXIMIZAR =
  '<svg viewBox="0 0 10 10" fill="none" stroke="currentColor"><rect x="0.5" y="0.5" width="9" height="9"/></svg>';
const ICONE_RESTAURAR =
  '<svg viewBox="0 0 10 10" fill="none" stroke="currentColor"><rect x="2.5" y="0.5" width="6" height="6"/><rect x="0.5" y="3.5" width="6" height="6"/></svg>';
const ICONE_FECHAR =
  '<svg viewBox="0 0 10 10" stroke="currentColor"><line x1="0.5" y1="0.5" x2="9.5" y2="9.5"/><line x1="9.5" y1="0.5" x2="0.5" y2="9.5"/></svg>';

export function montarBarraDeTitulo(): HTMLElement {
  const barra = document.createElement("div");
  barra.className = "barra-titulo";

  const arraste = document.createElement("div");
  arraste.className = "barra-titulo__arraste";
  // "deep", e não o atributo vazio: sem valor, o Tauri só arrasta quando o
  // clique cai exatamente neste `div` — clicar no ícone ou no texto "Cururu",
  // que são filhos, não arrastaria nada. O próprio Tauri também maximiza no
  // duplo clique daqui, então não há handler nosso para isso.
  arraste.setAttribute("data-tauri-drag-region", "deep");

  const icone = document.createElement("img");
  icone.className = "barra-titulo__icone";
  icone.src = "/cururu.svg";
  icone.alt = "";

  const nome = document.createElement("span");
  nome.className = "barra-titulo__nome";
  nome.textContent = "Cururu";

  arraste.append(icone, nome);

  const controles = document.createElement("div");
  controles.className = "barra-titulo__controles";

  const minimizar = botao(t("minimizar janela"), ICONE_MINIMIZAR, () =>
    comandar("minimizar", (alvo) => alvo.minimize()),
  );
  const maximizar = botao(t("maximizar janela"), ICONE_MAXIMIZAR, () =>
    comandar("maximizar", (alvo) => alvo.toggleMaximize()),
  );
  const fechar = botao(t("fechar"), ICONE_FECHAR, () => comandar("fechar", (alvo) => alvo.close()));
  fechar.classList.add("barra-titulo__botao--fechar");

  controles.append(minimizar, maximizar, fechar);
  barra.append(arraste, controles);

  sincronizarIconeDeMaximizar(maximizar);

  return barra;
}

/** `null` fora da janela do Tauri — não existe janela nenhuma para comandar. */
function janela(): Window | null {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

/**
 * Comanda a janela e **registra** a falha, em vez de engoli-la.
 *
 * Cada comando de janela do Tauri 2 exige permissão declarada em
 * `capabilities/default.json`; sem ela a chamada é recusada. Um `catch` mudo
 * transformava isso num botão que simplesmente não faz nada — sem erro na
 * tela, sem erro no console, nada para investigar. Fora da janela do Tauri
 * não há o que comandar, e aí o silêncio é o correto.
 */
function comandar(nome: string, operacao: (alvo: Window) => Promise<void>): void {
  const alvo = janela();
  if (!alvo) return;
  operacao(alvo).catch((erro) => {
    console.error(`Barra de título: falha ao ${nome} a janela.`, erro);
  });
}

function botao(rotulo: string, icone: string, aoClicar: () => void): HTMLButtonElement {
  const elemento = document.createElement("button");
  elemento.type = "button";
  elemento.className = "barra-titulo__botao";
  elemento.innerHTML = icone;
  elemento.title = rotulo;
  elemento.setAttribute("aria-label", rotulo);
  elemento.addEventListener("click", aoClicar);
  return elemento;
}

/**
 * Troca entre o ícone de maximizar e o de restaurar, acompanhando o estado
 * real da janela — que muda tanto pelo próprio botão quanto por arrastar
 * até a borda da tela (Aero Snap) ou pelo duplo clique na barra.
 */
function sincronizarIconeDeMaximizar(botaoMaximizar: HTMLButtonElement): void {
  const alvo = janela();
  if (!alvo) return;

  const atualizar = (maximizada: boolean): void => {
    botaoMaximizar.innerHTML = maximizada ? ICONE_RESTAURAR : ICONE_MAXIMIZAR;
    const rotulo = t(maximizada ? "restaurar janela" : "maximizar janela");
    botaoMaximizar.title = rotulo;
    botaoMaximizar.setAttribute("aria-label", rotulo);
  };

  const conferir = (): void => {
    alvo
      .isMaximized()
      .then(atualizar)
      .catch((erro) => console.error("Barra de título: falha ao ler o estado da janela.", erro));
  };

  conferir();
  // Maximizar também acontece fora do botão — Aero Snap, duplo clique na
  // barra, Win+seta —, então o ícone acompanha o redimensionamento em vez de
  // só o próprio clique.
  void alvo
    .onResized(conferir)
    .catch((erro) => console.error("Barra de título: falha ao observar a janela.", erro));
}
