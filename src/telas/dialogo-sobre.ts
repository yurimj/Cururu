/**
 * Caixa "Sobre o Cururu", aberta pela opção Sobre do menu principal.
 *
 * Mostra a versão do executável em uso junto com o nome e o autor: é a resposta
 * rápida para "estou olhando a versão nova?" depois de uma atualização.
 */

import "./dialogo-sobre.css";

import * as api from "../api";
import { t } from "../idioma";
import { comIndicadorDeAbertura } from "../componentes/indicador-de-abertura";
import { fecharComEsc } from "../componentes/janela";

/** Perfil do autor, aberto no navegador do sistema. */
const PERFIL_DO_AUTOR = "https://www.instagram.com/yurijiacomo";

/** Resolve quando a caixa é fechada. */
export async function abrirDialogoSobre(): Promise<void> {
  // A versão é lida antes de montar para a caixa já nascer completa, sem texto
  // aparecendo depois e mudando a altura na frente do usuário. Não conseguir
  // ler não impede de abrir: sem o dado, a linha da versão fica de fora.
  let versao: string;
  try {
    versao = await comIndicadorDeAbertura(t("sobre"), api.versaoEmExecucao);
  } catch {
    versao = "";
  }

  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";

    const caixa = document.createElement("div");
    caixa.className = "dialogo dialogo-sobre";
    caixa.setAttribute("role", "dialog");
    caixa.setAttribute("aria-modal", "true");

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.textContent = t("sobre o cururu");
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar"));
    barra.append(titulo, fechar);

    const icone = document.createElement("img");
    icone.className = "sobre__icone";
    icone.src = "/cururu.svg";
    icone.alt = "";

    const corpo = document.createElement("div");
    corpo.className = "sobre__corpo";
    corpo.append(icone, linha("sobre__nome", "Cururu"), linha("sobre__descricao", t("cliente sql")));
    if (versao) corpo.append(linha("sobre__versao", t("versao rotulo", { versao })));

    const autor = linha("sobre__autor", t("desenvolvido por"));
    const perfil = document.createElement("a");
    perfil.className = "sobre__perfil";
    perfil.href = PERFIL_DO_AUTOR;
    perfil.textContent = "@yurijiacomo";
    perfil.title = PERFIL_DO_AUTOR;
    autor.append(perfil);
    corpo.append(autor);

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const espaco = document.createElement("span");
    espaco.className = "status";

    // A janela do Cururu não tem barra de endereço nem botão de voltar: deixar
    // o link navegar aqui dentro prenderia a tela no site, sem volta. O
    // endereço vai para o navegador do sistema.
    perfil.addEventListener("click", (evento) => {
      evento.preventDefault();
      api.abrirNoNavegador(PERFIL_DO_AUTOR).catch((erro) => {
        espaco.textContent = api.mensagemDeErro(erro);
        espaco.className = "status status--erro";
      });
    });
    const confirmar = document.createElement("button");
    confirmar.textContent = t("fechar");
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(confirmar);
    rodape.append(espaco, botoes);

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);
    document.body.append(fundo);
    // Com o foco no "Fechar", o Enter já encerra sem precisar de atalho.
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

function linha(classe: string, texto: string): HTMLParagraphElement {
  const paragrafo = document.createElement("p");
  paragrafo.className = classe;
  paragrafo.textContent = texto;
  return paragrafo;
}
