/**
 * Menu de contexto do botão direito.
 *
 * Usado pela árvore de objetos (filtro de Tabelas e Views) e pelo nó da
 * conexão (Nova Consulta SQL).
 */

import "./menu-contexto.css";

export interface ItemDeMenu {
  rotulo: string;
  acao?: () => void;
  /** Itens exibidos ao lado deste item, como num submenu convencional. */
  subitens?: ItemDeMenu[];
  desabilitado?: boolean;
  /** Desenha um separador antes deste item. */
  separadorAntes?: boolean;
}

let menuAberto: HTMLElement | null = null;
let submenuAberto: HTMLElement | null = null;
let donoDoSubmenu: HTMLButtonElement | null = null;
let temporizadorDeAbertura: number | undefined;
let temporizadorDeFechamento: number | undefined;

/**
 * Os dois atrasos do hover, e o problema que eles resolvem.
 *
 * O caminho do mouse de um item até o submenu dele é uma diagonal, não uma
 * linha reta: para alcançar o terceiro item de um submenu, o ponteiro
 * atravessa por cima dos itens que ficam **abaixo** no menu principal. Reagir
 * na hora a esses itens de passagem é o que fazia o submenu sumir — ou pior,
 * ser trocado pelo do vizinho — antes de o mouse chegar ao destino.
 *
 * O de abrir é o mais importante dos dois: sem ele, passar por um vizinho que
 * também tem submenu (Tema, logo abaixo de Language) troca o submenu na hora.
 * O de fechar é mais longo de propósito — precisa sobreviver ao trajeto
 * inteiro, inclusive quando ele passa por um item sem submenu.
 */
const ATRASO_ABRIR_SUBMENU_MS = 220;
const ATRASO_FECHAR_SUBMENU_MS = 320;

function fecharSubmenu(): void {
  window.clearTimeout(temporizadorDeAbertura);
  window.clearTimeout(temporizadorDeFechamento);
  submenuAberto?.remove();
  submenuAberto = null;
  donoDoSubmenu?.setAttribute("aria-expanded", "false");
  donoDoSubmenu = null;
}

/** Abre o submenu só se o mouse ficar no item; sair antes disso desarma. */
function agendarAberturaDoSubmenu(dono: HTMLButtonElement, itens: ItemDeMenu[]): void {
  // Voltar para o dono do submenu que já está aberto é intenção de ficar,
  // não de reabrir: basta desarmar o fechamento que estivesse pendente.
  cancelarFechamentoAgendado();
  if (donoDoSubmenu === dono && submenuAberto) return;

  cancelarAberturaAgendada();
  temporizadorDeAbertura = window.setTimeout(
    () => abrirSubmenu(dono, itens),
    ATRASO_ABRIR_SUBMENU_MS,
  );
}

function cancelarAberturaAgendada(): void {
  window.clearTimeout(temporizadorDeAbertura);
}

/** Fecha o submenu depois de um instante, em vez de na hora. */
function agendarFechamentoDoSubmenu(): void {
  window.clearTimeout(temporizadorDeFechamento);
  temporizadorDeFechamento = window.setTimeout(fecharSubmenu, ATRASO_FECHAR_SUBMENU_MS);
}

function cancelarFechamentoAgendado(): void {
  window.clearTimeout(temporizadorDeFechamento);
}

/** Fecha o menu que estiver aberto, se houver. */
export function fecharMenuDeContexto(): void {
  fecharSubmenu();
  menuAberto?.remove();
  menuAberto = null;
}

/** Abre o menu na posição do evento. */
export function abrirMenuDeContexto(evento: MouseEvent, itens: ItemDeMenu[]): void {
  evento.preventDefault();
  evento.stopPropagation();
  abrir(itens, evento.clientX, evento.clientY);
}

/** Abre o mesmo menu ancorado logo abaixo de um botão. */
export function abrirMenuDeContextoNoElemento(alvo: HTMLElement, itens: ItemDeMenu[]): void {
  const retangulo = alvo.getBoundingClientRect();
  abrir(itens, retangulo.left, retangulo.bottom);
}

function abrir(itens: ItemDeMenu[], x: number, y: number): void {
  fecharMenuDeContexto();
  if (itens.length === 0) return;

  const menu = document.createElement("div");
  menu.className = "menu-contexto";
  menu.setAttribute("role", "menu");

  preencher(menu, itens);

  document.body.append(menu);
  menuAberto = menu;
  posicionar(menu, x, y);
  menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
}

/**
 * `dentroDeSubmenu` diz se `menu` é o submenu, e não o menu principal. Um
 * item sem subitens dentro do submenu é o destino do mouse — passar por cima
 * dele significa que o mouse chegou, e não que é hora de fechar. Só um item
 * sem subitens do menu **principal** agenda o fechamento.
 */
function preencher(menu: HTMLElement, itens: ItemDeMenu[], dentroDeSubmenu = false): void {
  for (const item of itens) {
    if (item.separadorAntes) {
      const separador = document.createElement("div");
      separador.className = "menu-contexto__separador";
      menu.append(separador);
    }
    const botao = document.createElement("button");
    botao.className = "menu-contexto__item";
    botao.setAttribute("role", "menuitem");
    botao.textContent = item.rotulo;
    botao.disabled = item.desabilitado ?? false;

    if (item.subitens?.length) {
      botao.classList.add("menu-contexto__item--com-submenu");
      botao.setAttribute("aria-haspopup", "menu");
      botao.setAttribute("aria-expanded", "false");
      botao.addEventListener("mouseenter", () => agendarAberturaDoSubmenu(botao, item.subitens!));
      // Sair antes do atraso vencer desarma a abertura: quem só passou por
      // cima a caminho do submenu do vizinho não abre este aqui depois.
      botao.addEventListener("mouseleave", cancelarAberturaAgendada);
      // Clique e ArrowRight são pedido explícito: abrem na hora, sem atraso.
      botao.addEventListener("click", (evento) => {
        evento.stopPropagation();
        abrirSubmenu(botao, item.subitens!, true);
      });
      botao.addEventListener("keydown", (evento) => {
        if (evento.key !== "ArrowRight") return;
        evento.preventDefault();
        abrirSubmenu(botao, item.subitens!, true);
      });
    } else {
      botao.addEventListener(
        "mouseenter",
        dentroDeSubmenu ? cancelarFechamentoAgendado : agendarFechamentoDoSubmenu,
      );
      botao.addEventListener("click", () => {
        fecharMenuDeContexto();
        item.acao?.();
      });
    }
    menu.append(botao);
  }
}

/** Abre um submenu encostado à direita do item; inverte o lado perto da borda. */
function abrirSubmenu(
  dono: HTMLButtonElement,
  itens: ItemDeMenu[],
  focarPrimeiro = false,
): void {
  cancelarAberturaAgendada();
  cancelarFechamentoAgendado();
  if (donoDoSubmenu === dono && submenuAberto) {
    if (focarPrimeiro) {
      submenuAberto.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    }
    return;
  }

  fecharSubmenu();
  const submenu = document.createElement("div");
  submenu.className = "menu-contexto menu-contexto--submenu";
  submenu.setAttribute("role", "menu");
  // Cobre o hover na folga entre os botões (padding, bordas): sem isto, só
  // entrar num item já cancelava o fechamento, e passar pela borda do
  // submenu sem tocar em nenhum botão não cancelava nada.
  submenu.addEventListener("mouseenter", cancelarFechamentoAgendado);
  preencher(submenu, itens, true);
  document.body.append(submenu);

  submenuAberto = submenu;
  donoDoSubmenu = dono;
  dono.setAttribute("aria-expanded", "true");

  const ancora = dono.getBoundingClientRect();
  const tamanho = submenu.getBoundingClientRect();
  const esquerda = ancora.right + tamanho.width <= window.innerWidth
    ? ancora.right
    : Math.max(0, ancora.left - tamanho.width);
  const topo = Math.min(ancora.top, Math.max(0, window.innerHeight - tamanho.height));
  submenu.style.left = `${esquerda}px`;
  submenu.style.top = `${topo}px`;

  submenu.addEventListener("keydown", (evento) => {
    if (evento.key !== "ArrowLeft") return;
    evento.preventDefault();
    const anterior = donoDoSubmenu;
    fecharSubmenu();
    anterior?.focus();
  });
  if (focarPrimeiro) {
    submenu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }
}

/** Mantém o menu inteiro dentro da janela. */
function posicionar(menu: HTMLElement, x: number, y: number): void {
  const { width, height } = menu.getBoundingClientRect();
  const esquerda = x + width > window.innerWidth ? Math.max(0, x - width) : x;
  const topo = y + height > window.innerHeight ? Math.max(0, y - height) : y;
  menu.style.left = `${esquerda}px`;
  menu.style.top = `${topo}px`;
}

// Qualquer clique, rolagem ou Esc fora do menu o encerra.
document.addEventListener("mousedown", (evento) => {
  const alvo = evento.target as Node;
  if (menuAberto && !menuAberto.contains(alvo) && !submenuAberto?.contains(alvo)) {
    fecharMenuDeContexto();
  }
});
document.addEventListener("keydown", (evento) => {
  if (evento.key === "Escape") fecharMenuDeContexto();
});
window.addEventListener("blur", fecharMenuDeContexto);
window.addEventListener("resize", fecharMenuDeContexto);
