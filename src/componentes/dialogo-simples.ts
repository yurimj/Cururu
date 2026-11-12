/**
 * Confirmação e perguntas de texto, no estilo do aplicativo.
 *
 * Existe para não se usar `window.confirm()` e `window.prompt()`, que são
 * desenhados pelo sistema operacional e não seguem o tema: no escuro, apareceria
 * um popup claro do Windows por cima de uma tela escura. Nasceu na tela de
 * botões rápidos, e saiu de lá quando o SQL Server Agent (requisito 36) passou a
 * precisar das mesmas duas perguntas.
 */

import "./dialogo-simples.css";

import { t } from "../idioma";
import { fecharComEsc } from "./janela";

export interface OpcoesDeConfirmacao {
  titulo: string;
  mensagem: string;
  /** Rótulo do botão que confirma; o padrão é "Excluir". */
  rotuloConfirmar?: string;
  /** Desenha o botão de confirmação como ação destrutiva. Ligado por padrão. */
  destrutivo?: boolean;
}

/** Pergunta sim ou não. O padrão seguro é **não**: é ele que recebe o foco. */
export function confirmar(opcoes: OpcoesDeConfirmacao): Promise<boolean> {
  return new Promise((resolver) => {
    const fundo = criarFundo();
    const destrutivo = opcoes.destrutivo ?? true;
    fundo.innerHTML = `
      <section class="dialogo dialogo-simples" role="alertdialog" aria-modal="true"
               aria-labelledby="titulo-simples">
        <header class="dialogo__barra"><h1 id="titulo-simples">${escapar(opcoes.titulo)}</h1></header>
        <div class="dialogo-simples__corpo"><p>${escapar(opcoes.mensagem)}</p></div>
        <footer class="dialogo__rodape">
          <span class="status"></span>
          <span class="dialogo__botoes">
            <button class="${destrutivo ? "botao-excluir" : "botao-principal"}"
                    data-acao="confirmar">${escapar(opcoes.rotuloConfirmar ?? t("excluir"))}</button>
            <button data-acao="cancelar">${t("cancelar")}</button>
          </span>
        </footer>
      </section>`;

    let pararEsc = (): void => {};
    const encerrar = (resposta: boolean): void => {
      pararEsc();
      fundo.remove();
      resolver(resposta);
    };
    pararEsc = ligar(fundo, (acao) => encerrar(acao === "confirmar"), () => encerrar(false));

    document.body.append(fundo);
    // O cancelamento é o padrão seguro: quem der Enter sem querer não perde nada.
    fundo.querySelector<HTMLButtonElement>('[data-acao="cancelar"]')?.focus();
  });
}

export interface OpcoesDeTexto {
  titulo: string;
  rotulo: string;
  valor: string;
  rotuloConfirmar?: string;
}

/** Pede um texto de uma linha. Devolve `null` ao cancelar ou ao deixar vazio. */
export function pedirTexto(opcoes: OpcoesDeTexto): Promise<string | null> {
  return new Promise((resolver) => {
    const fundo = criarFundo();
    fundo.innerHTML = `
      <section class="dialogo dialogo-simples" role="dialog" aria-modal="true"
               aria-labelledby="titulo-simples">
        <header class="dialogo__barra"><h1 id="titulo-simples">${escapar(opcoes.titulo)}</h1></header>
        <div class="dialogo-simples__corpo">
          <label class="dialogo-simples__campo">
            <span>${escapar(opcoes.rotulo)}</span>
            <input type="text" data-campo="valor" spellcheck="false" />
          </label>
        </div>
        <footer class="dialogo__rodape">
          <span class="status"></span>
          <span class="dialogo__botoes">
            <button class="botao-principal" data-acao="confirmar">${escapar(
              opcoes.rotuloConfirmar ?? t("salvar"),
            )}</button>
            <button data-acao="cancelar">${t("cancelar")}</button>
          </span>
        </footer>
      </section>`;

    const campo = fundo.querySelector<HTMLInputElement>('[data-campo="valor"]')!;
    let pararEsc = (): void => {};
    const encerrar = (texto: string | null): void => {
      pararEsc();
      fundo.remove();
      resolver(texto);
    };
    const confirmarTexto = (): void => {
      const informado = campo.value.trim();
      encerrar(informado || null);
    };
    pararEsc = ligar(fundo, (acao) => (acao === "confirmar" ? confirmarTexto() : encerrar(null)), () =>
      encerrar(null),
    );
    // Enter no campo é o gesto natural de quem acabou de digitar o nome.
    campo.addEventListener("keydown", (evento) => {
      if (evento.key === "Enter") confirmarTexto();
    });

    document.body.append(fundo);
    campo.value = opcoes.valor;
    campo.focus();
    campo.select();
  });
}

export interface OpcoesDeTextoLongo {
  titulo: string;
  rotulo: string;
  valor: string;
  /** Teto de caracteres aceito pelo destino; o rodapé mostra quanto falta. */
  limite: number;
  rotuloConfirmar?: string;
}

/**
 * Pede um texto de várias linhas. Devolve `null` só ao **cancelar**.
 *
 * A diferença para [`pedirTexto`] não é o tamanho do campo, é o vazio: lá, o
 * campo em branco equivale a desistir, porque quem pede um nome não quer um
 * nome vazio. Aqui o branco é uma resposta legítima — é assim que se apaga um
 * comentário —, então cancelar e esvaziar precisam ser distinguíveis, e só o
 * cancelamento devolve `null`.
 */
export function pedirTextoLongo(opcoes: OpcoesDeTextoLongo): Promise<string | null> {
  return new Promise((resolver) => {
    const fundo = criarFundo();
    fundo.innerHTML = `
      <section class="dialogo dialogo-simples dialogo-simples--longo" role="dialog"
               aria-modal="true" aria-labelledby="titulo-simples">
        <header class="dialogo__barra"><h1 id="titulo-simples">${escapar(opcoes.titulo)}</h1></header>
        <div class="dialogo-simples__corpo">
          <label class="dialogo-simples__campo">
            <span>${escapar(opcoes.rotulo)}</span>
            <textarea data-campo="valor" rows="4" spellcheck="false"
                      maxlength="${opcoes.limite}"></textarea>
          </label>
        </div>
        <footer class="dialogo__rodape">
          <span class="status" data-campo="contagem"></span>
          <span class="dialogo__botoes">
            <button class="botao-principal" data-acao="confirmar">${escapar(
              opcoes.rotuloConfirmar ?? t("salvar"),
            )}</button>
            <button data-acao="cancelar">${t("cancelar")}</button>
          </span>
        </footer>
      </section>`;

    const campo = fundo.querySelector<HTMLTextAreaElement>('[data-campo="valor"]')!;
    const contagem = fundo.querySelector<HTMLElement>('[data-campo="contagem"]')!;
    let pararEsc = (): void => {};
    const encerrar = (texto: string | null): void => {
      pararEsc();
      fundo.remove();
      resolver(texto);
    };
    const contar = (): void => {
      contagem.textContent = t("caracteres restantes", {
        n: opcoes.limite - campo.value.length,
      });
    };
    pararEsc = ligar(fundo, (acao) => encerrar(acao === "confirmar" ? campo.value.trim() : null), () =>
      encerrar(null),
    );
    campo.addEventListener("input", contar);
    // Enter quebra linha, que é o gesto certo num campo de várias linhas; quem
    // terminou confirma com Ctrl+Enter, como no editor SQL.
    campo.addEventListener("keydown", (evento) => {
      if (evento.key === "Enter" && (evento.ctrlKey || evento.metaKey)) {
        evento.preventDefault();
        encerrar(campo.value.trim());
      }
    });

    document.body.append(fundo);
    campo.value = opcoes.valor;
    contar();
    campo.focus();
    campo.select();
  });
}

function criarFundo(): HTMLElement {
  const fundo = document.createElement("div");
  fundo.className = "dialogo-fundo";
  return fundo;
}

/**
 * Liga os botões e o Esc, e devolve como desligar a escuta da tecla — que fica
 * na janela, e não no fundo do diálogo, para o Esc valer com o foco onde
 * estiver (ver `fecharComEsc` em componentes/janela.ts).
 */
function ligar(
  fundo: HTMLElement,
  aoAgir: (acao: string) => void,
  aoEscapar: () => void,
): () => void {
  fundo.addEventListener("click", (evento) => {
    const acao = (evento.target as HTMLElement).closest<HTMLElement>("[data-acao]")?.dataset.acao;
    if (acao) aoAgir(acao);
  });
  return fecharComEsc(fundo, aoEscapar);
}

/**
 * O texto entra por `innerHTML`, e nome de trabalho vem do servidor: um `<` no
 * meio dele não pode virar marcação.
 */
function escapar(texto: string): string {
  const alvo = document.createElement("span");
  alvo.textContent = texto;
  return alvo.innerHTML;
}
