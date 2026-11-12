/**
 * Tela "Versões": o que mudou em cada versão do Cururu.
 *
 * O conteúdo não é escrito aqui — vem de `versoes.md`, na raiz do projeto, que
 * o Vite embute no pacote como texto. Assim o changelog continua legível no
 * repositório e existe um lugar só para atualizar a cada versão nova, sem o
 * risco de a tela e o arquivo divergirem.
 */

import "./dialogo-versoes.css";

import textoDasVersoes from "../../versoes.md?raw";
import * as api from "../api";
import { t } from "../idioma";
import { comIndicadorDeAbertura } from "../componentes/indicador-de-abertura";
import { fecharComEsc } from "../componentes/janela";
import { comMarcacaoInline } from "../marcacao";

interface Versao {
  numero: string;
  data: string;
  itens: string[];
}

/** Resolve quando a caixa é fechada. */
export async function abrirDialogoDeVersoes(): Promise<void> {
  // Marcar qual versão está rodando é o que responde "o que eu ganhei nesta
  // atualização?". Sem o backend — no navegador — a marca simplesmente some.
  let emExecucao: string;
  try {
    emExecucao = await comIndicadorDeAbertura(t("versoes"), api.versaoEmExecucao);
  } catch {
    emExecucao = "";
  }
  // O backend devolve "0.1.4 · compilado 31/07/2026 07:57". O rodapé mostra a
  // frase inteira; para casar com o título da seção interessa só o número.
  const numeroEmExecucao = emExecucao.split("·")[0].trim();
  const versoes = lerVersoes(textoDasVersoes);

  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";

    const caixa = document.createElement("section");
    caixa.className = "dialogo dialogo-versoes";
    caixa.setAttribute("role", "dialog");
    caixa.setAttribute("aria-modal", "true");
    caixa.setAttribute("aria-labelledby", "titulo-versoes");

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.id = "titulo-versoes";
    titulo.textContent = t("versoes");
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar"));
    barra.append(titulo, fechar);

    const corpo = document.createElement("div");
    corpo.className = "versoes__corpo";
    if (versoes.length === 0) {
      const vazio = document.createElement("p");
      vazio.className = "versoes__vazio";
      vazio.textContent = t("nenhuma versao registrada");
      corpo.append(vazio);
    } else {
      for (const versao of versoes) corpo.append(montarVersao(versao, numeroEmExecucao));
    }

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const situacao = document.createElement("span");
    situacao.className = "status";
    situacao.textContent = emExecucao
      ? `Em execução: versão ${emExecucao}.`
      : "Cada versão nova registra aqui o que mudou.";
    const confirmar = document.createElement("button");
    confirmar.textContent = t("fechar");
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(confirmar);
    rodape.append(situacao, botoes);

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);
    document.body.append(fundo);
    confirmar.focus();

    let pararEsc = (): void => {};
    const encerrar = (): void => {
      pararEsc();
      fundo.remove();
      resolver();
    };
    pararEsc = fecharComEsc(fundo, encerrar);
    fechar.addEventListener("click", encerrar);
    confirmar.addEventListener("click", encerrar);
  });
}

function montarVersao(versao: Versao, numeroEmExecucao: string): HTMLElement {
  const bloco = document.createElement("article");
  bloco.className = "versoes__versao";

  const cabecalho = document.createElement("h2");
  cabecalho.className = "versoes__titulo";
  const numero = document.createElement("span");
  numero.className = "versoes__numero";
  numero.textContent = versao.numero;
  cabecalho.append(numero);
  if (versao.data) {
    const data = document.createElement("span");
    data.className = "versoes__data";
    data.textContent = versao.data;
    cabecalho.append(data);
  }
  if (versao.numero === numeroEmExecucao) {
    const marca = document.createElement("span");
    marca.className = "versoes__atual";
    marca.textContent = t("em execucao marca");
    cabecalho.append(marca);
  }

  const itens = document.createElement("ul");
  itens.className = "versoes__itens";
  for (const texto of versao.itens) {
    const item = document.createElement("li");
    item.append(comMarcacaoInline(texto));
    itens.append(item);
  }

  bloco.append(cabecalho, itens);
  return bloco;
}

/**
 * Lê do markdown só o que a tela mostra: cada `## versão — data` vira um bloco,
 * e as linhas de lista abaixo dele viram os itens. O resto do arquivo —
 * títulos, parágrafos de explicação — fica de fora de propósito.
 */
function lerVersoes(texto: string): Versao[] {
  const versoes: Versao[] = [];
  for (const linha of texto.split(/\r?\n/)) {
    const cabecalho = /^##\s+(.+)$/.exec(linha);
    if (cabecalho) {
      const [numero, data] = cabecalho[1].split("—");
      versoes.push({ numero: numero.trim(), data: data?.trim() ?? "", itens: [] });
      continue;
    }
    const item = /^[-*]\s+(.+)$/.exec(linha);
    if (item && versoes.length > 0) {
      versoes[versoes.length - 1].itens.push(item[1].trim());
    }
  }
  return versoes;
}
