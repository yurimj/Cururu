import "./alertas.css";

import { lerLogsAlerta, mensagemDeErro, type AlertaSalvo } from "../api";
import { fecharComEsc } from "../componentes/janela";
import { t } from "../idioma";

export function abrirLogsDoAlerta(sessaoId: string, regra: AlertaSalvo): void {
  if (document.querySelector(".dialogo-logs-alerta")) return;
  const anterior = document.activeElement;
  const fundo = document.createElement("div");
  fundo.className = "dialogo-fundo";
  fundo.innerHTML = `
    <section class="dialogo dialogo-logs-alerta" role="dialog" aria-modal="true"
             aria-labelledby="titulo-logs-alerta">
      <header class="dialogo__barra">
        <h1 id="titulo-logs-alerta"></h1>
        <button type="button" class="dialogo__fechar" aria-label="${t("fechar")}">×</button>
      </header>
      <pre class="alertas__logs" tabindex="0" aria-labelledby="titulo-logs-alerta"></pre>
      <footer class="dialogo__rodape">
        <output class="status" aria-live="polite"></output>
        <button type="button" data-acao="atualizar">${t("atualizar")}</button>
        <button type="button" data-acao="fechar">${t("fechar")}</button>
      </footer>
    </section>`;
  fundo.querySelector("h1")!.textContent = t("logs do alerta titulo", { nome: regra.nome });
  const registros = fundo.querySelector("pre")!;
  const status = fundo.querySelector("output")!;
  const atualizar = fundo.querySelector<HTMLButtonElement>('[data-acao="atualizar"]')!;
  const fechar = fundo.querySelector<HTMLButtonElement>('[data-acao="fechar"]')!;
  const encerrar = () => {
    pararEsc();
    fundo.remove();
    if (anterior instanceof HTMLElement) anterior.focus();
  };
  const pararEsc = fecharComEsc(fundo, encerrar);
  fechar.addEventListener("click", encerrar);
  fundo.querySelector(".dialogo__fechar")!.addEventListener("click", encerrar);

  const carregar = async () => {
    atualizar.disabled = true;
    status.classList.remove("alertas__erro");
    status.textContent = t("carregando");
    try {
      const linhas = await lerLogsAlerta(sessaoId, regra.id);
      registros.textContent = linhas.join("\n");
      registros.scrollTop = 0;
      status.textContent = t(linhas.length > 0 ? "logs do alerta ordem" : "logs do alerta vazios");
    } catch (erro) {
      const mensagem = mensagemDeErro(erro);
      status.textContent = mensagem === "alerta inexistente" ? t("alerta inexistente") : mensagem;
      status.classList.add("alertas__erro");
    } finally {
      atualizar.disabled = false;
    }
  };
  atualizar.addEventListener("click", () => void carregar());
  document.body.append(fundo);
  fechar.focus();
  void carregar();
}
