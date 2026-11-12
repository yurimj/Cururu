/**
 * Escolha de uma conexão encontrada na máquina (requisito 25).
 *
 * Lista o que veio do Oracle SQL Developer e do `tnsnames.ora` para o usuário
 * escolher qual trazer para o formulário.
 */

import "./dialogo-importacao.css";

import type { ConexaoImportavel } from "../api";
import { t } from "../idioma";
import { fecharComEsc } from "../componentes/janela";

export function escolherConexaoImportavel(
  encontradas: ConexaoImportavel[],
): Promise<ConexaoImportavel | null> {
  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";

    const caixa = document.createElement("div");
    caixa.className = "dialogo dialogo-importacao";
    caixa.setAttribute("role", "dialog");
    caixa.setAttribute("aria-modal", "true");

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.textContent = t("importar conexao");
    barra.append(titulo);

    const corpo = document.createElement("div");
    corpo.className = "dialogo-importacao__corpo";

    const tabela = document.createElement("table");
    tabela.innerHTML =
      `<thead><tr><th>${t("nome")}</th><th>${t("endereco")}</th>` +
      `<th>${t("origem")}</th></tr></thead>`;
    const linhas = document.createElement("tbody");
    tabela.append(linhas);

    let selecionada: ConexaoImportavel | null = null;
    const confirmar = document.createElement("button");

    for (const conexao of encontradas) {
      const linha = linhas.insertRow();
      linha.insertCell().textContent = conexao.nome;

      const alvo =
        conexao.perfil.identificacao === "sid"
          ? conexao.perfil.sid
          : conexao.perfil.servico;
      const endereco = linha.insertCell();
      endereco.textContent = `//${conexao.perfil.host}:${conexao.perfil.porta}/${alvo}`;
      endereco.title = endereco.textContent;

      const origem = linha.insertCell();
      origem.textContent = conexao.origem;
      origem.title = conexao.origem;

      const escolher = (): void => {
        selecionada = conexao;
        confirmar.disabled = false;
        for (const outra of linhas.rows) outra.removeAttribute("aria-selected");
        linha.setAttribute("aria-selected", "true");
      };
      linha.addEventListener("click", escolher);
      linha.addEventListener("dblclick", () => {
        escolher();
        encerrar(conexao);
      });
    }

    corpo.append(tabela);

    const aviso = document.createElement("p");
    aviso.className = "dialogo-importacao__aviso";
    aviso.textContent = t("senha nao e importada");
    corpo.append(aviso);

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const contagem = document.createElement("span");
    contagem.className = "status";
    contagem.textContent = t("conexoes encontradas", { n: encontradas.length });
    confirmar.textContent = t("importar");
    confirmar.disabled = true;
    const cancelar = document.createElement("button");
    cancelar.textContent = t("cancelar");
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(confirmar, cancelar);
    rodape.append(contagem, botoes);

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);
    document.body.append(fundo);
    confirmar.focus();

    function encerrar(resultado: ConexaoImportavel | null): void {
      pararEsc();
      fundo.remove();
      resolver(resultado);
    }
    const pararEsc = fecharComEsc(fundo, () => encerrar(null));
    confirmar.addEventListener("click", () => encerrar(selecionada));
    cancelar.addEventListener("click", () => encerrar(null));
  });
}
