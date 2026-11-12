/**
 * "Iniciar Trabalho na Etapa…" do SQL Server Agent (requisito 36).
 *
 * A mesma grade da página Etapas, só que para escolher por onde começar. Ela
 * existe porque um trabalho de cinco etapas quase nunca precisa rodar inteiro
 * quando se está consertando a terceira.
 */

import "./dialogo-trabalho.css";

import type { EtapaDeTrabalho } from "../api";
import { fecharComEsc } from "../componentes/janela";
import { t } from "../idioma";
import { ACOES } from "./dialogo-etapa";

/**
 * Abre a tela e devolve o **nome** da etapa escolhida, ou `null` ao cancelar.
 *
 * O nome, e não a ordem, porque é o que o `sp_start_job` recebe: ele aceita
 * `@step_name`, e não um índice.
 */
export function abrirDialogoIniciarNaEtapa(
  trabalho: string,
  etapas: EtapaDeTrabalho[],
): Promise<string | null> {
  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";
    fundo.innerHTML = `
      <div class="dialogo dialogo-iniciar-etapa" role="dialog" aria-modal="true"
           aria-label="${t("iniciar trabalho na etapa")}">
        <header class="dialogo__barra">
          <h1>${t("iniciar trabalho na etapa")}</h1>
          <button class="dialogo__fechar" data-acao="cancelar" title="${t("fechar")}"
                  aria-label="${t("fechar")}">✕</button>
        </header>
        <div class="trabalho__corpo">
          <p class="trabalho__nota" data-elemento="aviso"></p>
          <div class="trabalho__lista">
            <table class="trabalho__grade">
              <thead>
                <tr>
                  <th class="trabalho__ordem">#</th>
                  <th>${t("nome")}</th>
                  <th>${t("tipo")}</th>
                  <th>${t("ao ter exito")}</th>
                  <th>${t("ao falhar")}</th>
                </tr>
              </thead>
              <tbody data-elemento="etapas"></tbody>
            </table>
          </div>
        </div>
        <footer class="dialogo__rodape">
          <span class="status"></span>
          <div class="dialogo__botoes">
            <button data-acao="iniciar" class="botao-principal">${t("iniciar")}</button>
            <button data-acao="cancelar">${t("cancelar")}</button>
          </div>
        </footer>
      </div>`;

    const caixa = fundo.firstElementChild as HTMLElement;
    const corpo = caixa.querySelector<HTMLElement>('[data-elemento="etapas"]')!;
    const botaoIniciar = caixa.querySelector<HTMLButtonElement>('[data-acao="iniciar"]')!;
    caixa.querySelector<HTMLElement>('[data-elemento="aviso"]')!.textContent = t(
      "escolha a etapa inicial",
      { trabalho },
    );

    let escolhida = etapas.length > 0 ? 0 : -1;

    let pararEsc = (): void => {};
    const encerrar = (nome: string | null): void => {
      pararEsc();
      fundo.remove();
      resolver(nome);
    };
    pararEsc = fecharComEsc(fundo, () => encerrar(null));
    const iniciar = (): void => {
      const etapa = etapas[escolhida];
      if (etapa) encerrar(etapa.nome);
    };

    const redesenhar = (): void => {
      corpo.replaceChildren();
      for (const [ordem, etapa] of etapas.entries()) {
        const linha = document.createElement("tr");
        if (ordem === escolhida) linha.className = "trabalho__linha--selecionada";
        for (const texto of [
          String(ordem + 1),
          etapa.nome,
          etapa.tipo,
          rotuloDaAcao(etapa.aoTerExito),
          rotuloDaAcao(etapa.aoFalhar),
        ]) {
          const celula = document.createElement("td");
          celula.textContent = texto;
          celula.title = texto;
          linha.append(celula);
        }
        linha.addEventListener("click", () => {
          escolhida = ordem;
          redesenhar();
        });
        // Duplo clique é o gesto de quem já decidiu: escolhe e inicia.
        linha.addEventListener("dblclick", iniciar);
        corpo.append(linha);
      }
      if (etapas.length === 0) {
        const linha = document.createElement("tr");
        const celula = document.createElement("td");
        celula.colSpan = 5;
        celula.className = "trabalho__vazio";
        celula.textContent = t("nenhuma etapa");
        linha.append(celula);
        corpo.append(linha);
      }
      botaoIniciar.disabled = escolhida < 0;
    };

    fundo.addEventListener("click", (evento) => {
      const acao = (evento.target as HTMLElement).closest<HTMLElement>("[data-acao]")?.dataset
        .acao;
      if (acao === "iniciar") iniciar();
      else if (acao === "cancelar") encerrar(null);
    });

    document.body.append(fundo);
    redesenhar();
    botaoIniciar.focus();
  });
}

function rotuloDaAcao(valor: number): string {
  const acao = ACOES.find((opcao) => opcao.valor === valor);
  return acao ? t(acao.chave) : String(valor);
}
