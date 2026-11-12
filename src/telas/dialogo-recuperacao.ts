/**
 * Tela "Recuperar Consultas" (requisito 12).
 *
 * Mostra as cópias de socorro guardadas nos últimos sete dias e devolve a
 * escolhida para ser aberta numa aba. Sem esta tela a cópia não serviria para
 * nada: guardar texto que ninguém consegue trazer de volta é só ocupar disco.
 */

import "./dialogo-recuperacao.css";

import * as api from "../api";
import { t } from "../idioma";
import { comIndicadorDeAbertura } from "../componentes/indicador-de-abertura";
import { fecharComEsc } from "../componentes/janela";

/** Resolve com o texto escolhido, ou `null` se a caixa foi fechada sem escolha. */
export async function abrirDialogoDeRecuperacao(): Promise<{
  texto: string;
  entrada: api.EntradaDeRecuperacao;
} | null> {
  let entradas: api.EntradaDeRecuperacao[];
  try {
    entradas = await comIndicadorDeAbertura(t("recuperar consultas titulo"), api.listarRecuperacoes);
  } catch (erro) {
    entradas = [];
    window.alert(api.mensagemDeErro(erro));
  }

  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";

    const caixa = document.createElement("section");
    caixa.className = "dialogo dialogo-recuperacao";
    caixa.setAttribute("role", "dialog");
    caixa.setAttribute("aria-modal", "true");
    caixa.setAttribute("aria-labelledby", "titulo-recuperacao");

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.id = "titulo-recuperacao";
    titulo.textContent = t("recuperar consultas titulo");
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar"));
    barra.append(titulo, fechar);

    const corpo = document.createElement("div");
    corpo.className = "recuperacao__corpo";

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const situacao = document.createElement("span");
    situacao.className = "status";
    const abrir = document.createElement("button");
    abrir.textContent = t("abrir numa aba");
    abrir.disabled = true;
    const descartar = document.createElement("button");
    descartar.textContent = t("descartar");
    descartar.disabled = true;
    const cancelar = document.createElement("button");
    cancelar.textContent = t("fechar");
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(descartar, abrir, cancelar);
    rodape.append(situacao, botoes);

    let selecionada: api.EntradaDeRecuperacao | null = null;

    const escreverSituacao = (texto: string, erro = false): void => {
      situacao.textContent = texto;
      situacao.className = erro ? "status status--erro" : "status";
    };

    const desenhar = (): void => {
      corpo.replaceChildren();
      if (entradas.length === 0) {
        const vazio = document.createElement("p");
        vazio.className = "recuperacao__vazio";
        vazio.textContent = t("nenhuma consulta guardada");
        corpo.append(vazio);
        escreverSituacao(t("copias duram sete dias"));
        return;
      }
      for (const entrada of entradas) {
        corpo.append(montarEntrada(entrada));
      }
      escreverSituacao(t("consultas guardadas", { n: entradas.length }));
    };

    const selecionar = (entrada: api.EntradaDeRecuperacao, item: HTMLElement): void => {
      selecionada = entrada;
      for (const outro of corpo.querySelectorAll(".recuperacao__item")) {
        outro.classList.toggle("recuperacao__item--ativa", outro === item);
      }
      abrir.disabled = false;
      descartar.disabled = false;
    };

    function montarEntrada(entrada: api.EntradaDeRecuperacao): HTMLElement {
      const item = document.createElement("article");
      item.className = "recuperacao__item";
      item.tabIndex = 0;

      const cabecalho = document.createElement("div");
      cabecalho.className = "recuperacao__cabecalho";
      cabecalho.append(
        rotulo("recuperacao__titulo", entrada.titulo),
        rotulo("recuperacao__quando", entrada.quando),
      );

      const detalhe = document.createElement("div");
      detalhe.className = "recuperacao__detalhe";
      detalhe.append(rotulo("recuperacao__conexao", entrada.conexao || t("sem conexao")));
      if (entrada.origem) {
        // Saber que a cópia veio de um arquivo é o que evita a dúvida de "isto
        // aqui já está salvo em algum lugar?".
        detalhe.append(rotulo("recuperacao__origem", entrada.origem));
      }

      const amostra = document.createElement("pre");
      amostra.className = "recuperacao__amostra";
      amostra.textContent = entrada.amostra || t("vazia");

      item.append(cabecalho, detalhe, amostra);
      item.addEventListener("click", () => selecionar(entrada, item));
      item.addEventListener("dblclick", () => {
        selecionar(entrada, item);
        void confirmarAbertura();
      });
      return item;
    }

    const confirmarAbertura = async (): Promise<void> => {
      const escolhida = selecionada;
      if (!escolhida) return;
      try {
        const texto = await comIndicadorDeAbertura(escolhida.titulo, () =>
          api.lerRecuperacao(escolhida.id),
        );
        fundo.remove();
        resolver({ texto, entrada: escolhida });
      } catch (erro) {
        escreverSituacao(api.mensagemDeErro(erro), true);
      }
    };

    const confirmarDescarte = async (): Promise<void> => {
      if (!selecionada) return;
      if (!window.confirm(t("descartar copia", { titulo: selecionada.titulo }))) return;
      try {
        await api.descartarRecuperacao(selecionada.id);
        entradas = entradas.filter((entrada) => entrada.id !== selecionada?.id);
        selecionada = null;
        abrir.disabled = true;
        descartar.disabled = true;
        desenhar();
      } catch (erro) {
        escreverSituacao(api.mensagemDeErro(erro), true);
      }
    };

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);
    document.body.append(fundo);
    desenhar();
    cancelar.focus();

    let pararEsc = (): void => {};
    const encerrar = (): void => {
      pararEsc();
      fundo.remove();
      resolver(null);
    };
    pararEsc = fecharComEsc(fundo, encerrar);
    fechar.addEventListener("click", encerrar);
    cancelar.addEventListener("click", encerrar);
    abrir.addEventListener("click", () => void confirmarAbertura());
    descartar.addEventListener("click", () => void confirmarDescarte());
  });
}

function rotulo(classe: string, texto: string): HTMLSpanElement {
  const elemento = document.createElement("span");
  elemento.className = classe;
  elemento.textContent = texto;
  return elemento;
}
