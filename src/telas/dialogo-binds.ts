/**
 * Pedido dos valores das bind variables (requisito 15).
 *
 * Aparece antes de executar um comando que use `:variavel`, como no SQL
 * Developer.
 */

import { fecharComEsc } from "../componentes/janela";
import { t } from "../idioma";
export function pedirValoresDosBinds(nomes: string[]): Promise<[string, string][] | null> {
  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";

    const caixa = document.createElement("div");
    caixa.className = "dialogo dialogo-binds";
    caixa.setAttribute("role", "dialog");
    caixa.setAttribute("aria-modal", "true");

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.textContent = t(nomes.length === 1 ? "informe o valor" : "informe os valores");
    barra.append(titulo);

    const corpo = document.createElement("div");
    corpo.className = "dialogo-binds__corpo";
    const campos = new Map<string, HTMLInputElement>();

    for (const nome of nomes) {
      const rotulo = document.createElement("label");
      const texto = document.createElement("span");
      texto.textContent = nome.startsWith("&") ? nome : `:${nome}`;
      const campo = document.createElement("input");
      campo.type = "text";
      campo.autocomplete = "off";
      campos.set(nome, campo);
      rotulo.append(texto, campo);
      corpo.append(rotulo);
    }

    const aviso = document.createElement("p");
    aviso.className = "dialogo-binds__aviso";
    aviso.textContent = t(nomes.some((nome) => nome.startsWith("&")) ? "substituicoes no script" : "binds vao como texto");
    corpo.append(aviso);

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const espaco = document.createElement("span");
    espaco.className = "status";
    const aplicar = document.createElement("button");
    aplicar.textContent = t("executar binds");
    const cancelar = document.createElement("button");
    cancelar.textContent = t("cancelar");
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(aplicar, cancelar);
    rodape.append(espaco, botoes);

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);
    document.body.append(fundo);
    campos.values().next().value?.focus();

    let pararEsc = (): void => {};
    const encerrar = (valores: [string, string][] | null): void => {
      pararEsc();
      fundo.remove();
      resolver(valores);
    };
    pararEsc = fecharComEsc(fundo, () => encerrar(null));
    aplicar.addEventListener("click", () =>
      encerrar([...campos].map(([nome, campo]) => [nome, campo.value])),
    );
    cancelar.addEventListener("click", () => encerrar(null));
    fundo.addEventListener("keydown", (evento) => {
      if (evento.key === "Enter") {
        encerrar([...campos].map(([nome, campo]) => [nome, campo.value]));
      }
    });
  });
}
