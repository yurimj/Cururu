/**
 * Acompanhamento da execução de um trabalho do Agent (requisito 36).
 *
 * O `sp_start_job` **agenda** e volta na hora: quem executa é o serviço, e ele
 * não avisa quando termina. Antes isso era tudo o que a tela tinha a dizer — a
 * barra de status avisava que não haveria resposta —, e um trabalho disparado
 * daqui parecia não ter acontecido.
 *
 * Esta tela pergunta. De segundo em segundo ela lê o estado do Agent e mostra a
 * etapa em curso; quando o trabalho sai das mãos do serviço, mostra o resultado
 * e a mensagem que ele gravou no histórico — a mesma que o SSMS mostra.
 *
 * Fechar a tela **não** para o trabalho: ele é do Agent, e segue sem ninguém
 * olhando. Quem quer interromper tem o botão Parar, que é outra coisa.
 */

import "./dialogo-execucao-trabalho.css";

import * as api from "../api";
import { isTauri } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatarDataHora, formatarDuracao } from "../formatacao";
import { t } from "../idioma";
import { temaAtual } from "../tema";

/** De quanto em quanto tempo se pergunta ao Agent. */
const INTERVALO_MS = 1_000;

/**
 * Abre a tela, dispara o trabalho e acompanha até o fim.
 *
 * Devolve o último estado conhecido quando a tela fechou, ou `null` se o
 * trabalho nem chegou a ser disparado — o chamador usa isso para relatar na
 * barra de status e para relistar a árvore, que passa a ter uma última execução
 * nova a mostrar.
 */
export async function abrirDialogoDeExecucao(
  sessaoId: string,
  trabalho: { id: string; nome: string },
  etapa: string | null,
): Promise<api.ExecucaoDoTrabalho | null> {
  const rotulo = `execucao-trabalho-${crypto.randomUUID()}`;
  const parametros = new URLSearchParams({ sessaoId, id: trabalho.id, nome: trabalho.nome });
  if (etapa !== null) parametros.set("etapa", etapa);

  let concluir!: (execucao: api.ExecucaoDoTrabalho | null) => void;
  const resultado = new Promise<api.ExecucaoDoTrabalho | null>((resolver) => {
    concluir = resolver;
  });
  const desligar = await getCurrentWindow().listen<api.ExecucaoDoTrabalho | null>(rotulo, ({ payload }) =>
    concluir(payload),
  );
  try {
    const janela = new WebviewWindow(rotulo, {
      url: `execucao-trabalho.html?${parametros}`,
      title: t("execucao de", { trabalho: trabalho.nome }),
      theme: temaAtual() === "escuro" ? "dark" : "light",
      parent: "main",
      center: true,
      width: 480,
      height: 290,
      minWidth: 360,
      minHeight: 210,
      maximizable: false,
    });
    return await new Promise((resolver, rejeitar) => {
      void janela.once("tauri://error", ({ payload }) => rejeitar(payload)).catch(rejeitar);
      void resultado.then(resolver);
    });
  } finally {
    desligar();
  }
}

export async function montarJanelaDeExecucao(): Promise<void> {
  const parametros = new URLSearchParams(location.search);
  const sessaoId = parametros.get("sessaoId");
  const id = parametros.get("id");
  const nome = parametros.get("nome");
  const etapa = parametros.get("etapa");
  if (!sessaoId || !id || nome === null) return;

  const tela = montarTela(nome, etapa);
  document.body.append(tela.caixa);
  if (!isTauri()) return;

  const janela = getCurrentWindow();
  let ultima: api.ExecucaoDoTrabalho | null = null;
  let acompanhando = true;
  await janela.onCloseRequested(async () => {
    acompanhando = false;
    try {
      await janela.emitTo("main", janela.label, ultima);
    } catch (erro) {
      console.error(erro);
    }
  });
  const fechar = (): void => {
    void janela.close().catch((erro: unknown) =>
      anunciar(tela, "erro", api.mensagemDeErro(erro)),
    );
  };
  tela.sair.addEventListener("click", fechar);
  document.addEventListener("keydown", (evento) => {
    if (evento.key === "Escape") fechar();
  });
  tela.parar.addEventListener("click", () => {
    tela.parar.disabled = true;
    void api
      .pararTrabalho(sessaoId, id)
      .then(() => anunciar(tela, "aviso", t("parada pedida")))
      .catch((erro: unknown) => anunciar(tela, "erro", api.mensagemDeErro(erro)));
  });
  tela.sair.focus();

  await acompanhar(sessaoId, { id, nome }, etapa, tela, {
    ativo: () => acompanhando,
    registrar: (execucao) => { ultima = execucao; },
  });
}

interface Observador {
  ativo: () => boolean;
  registrar: (execucao: api.ExecucaoDoTrabalho) => void;
}

/**
 * O laço: lê o estado de antes, dispara, pergunta até o Agent soltar.
 *
 * A leitura **antes** do disparo não é desperdício: é ela que dá a instância do
 * histórico já existente. Sem esse número, o primeiro "não está executando" —
 * que acontece sempre, porque o Agent leva alguns instantes para pegar o
 * trabalho — seria lido como fim, e a tela anunciaria o resultado da execução
 * anterior como se fosse desta.
 */
async function acompanhar(
  sessaoId: string,
  trabalho: { id: string; nome: string },
  etapa: string | null,
  tela: Tela,
  observador: Observador,
): Promise<void> {
  let anterior = 0;
  try {
    const antes = await api.execucaoDoTrabalho(sessaoId, trabalho.id);
    if (!observador.ativo()) return;
    anterior = antes.instancia;
    if (antes.executando) {
      // O Agent já está com ele: disparar agora é recusado pelo servidor, com
      // uma mensagem que não explica nada a quem não sabia disso.
      observador.registrar(antes);
      mostrarSituacao(tela, antes);
      anunciar(tela, "aviso", t("trabalho ja estava rodando"));
      tela.parar.disabled = false;
      await esperarOFim(sessaoId, trabalho, tela, observador, anterior);
      return;
    }
  } catch (erro) {
    // Falhar aqui é falhar a leitura, não o disparo: o trabalho ainda merece
    // ser iniciado, e o acompanhamento segue sem a referência do histórico.
    mostrarDetalhe(tela, api.mensagemDeErro(erro));
  }

  if (!observador.ativo()) return;
  try {
    await api.iniciarTrabalho(sessaoId, trabalho.id, etapa ?? undefined);
  } catch (erro) {
    tela.situacao.textContent = t("trabalho nao iniciou");
    tela.situacao.className = "execucao__situacao execucao__situacao--erro";
    tela.simbolo.textContent = "✖";
    tela.barra.hidden = true;
    tela.parar.disabled = true;
    mostrarDetalhe(tela, api.mensagemDeErro(erro));
    return;
  }

  tela.parar.disabled = false;
  tela.situacao.textContent = t("aguardando o agent");
  await esperarOFim(sessaoId, trabalho, tela, observador, anterior);
}

async function esperarOFim(
  sessaoId: string,
  trabalho: { id: string; nome: string },
  tela: Tela,
  observador: Observador,
  anterior: number,
): Promise<void> {
  const inicio = Date.now();
  const cronometro = window.setInterval(() => {
    if (!observador.ativo()) return window.clearInterval(cronometro);
    tela.tempo.textContent = t("decorrido", {
      tempo: comoTempo((Date.now() - inicio) / 1_000),
    });
  }, 500);

  // O trabalho já começou pelo menos uma vez? Enquanto não, um "parado" é o
  // Agent ainda não ter pegado o trabalho, e não o fim dele.
  let comecou = false;
  try {
    while (observador.ativo()) {
      await espera(INTERVALO_MS);
      if (!observador.ativo()) return;

      const execucao = await api.execucaoDoTrabalho(sessaoId, trabalho.id, anterior);
      if (!observador.ativo()) return;
      observador.registrar(execucao);
      if (execucao.executando) {
        comecou = true;
        mostrarSituacao(tela, execucao);
        continue;
      }
      // Instância nova no histórico é a prova de que esta execução terminou; o
      // `comecou` cobre o trabalho tão curto que nenhuma pergunta o pegou
      // rodando, mas que deixou a instância lá.
      if (execucao.instancia > anterior || comecou) {
        mostrarFim(tela, execucao);
        return;
      }
      tela.situacao.textContent = t("aguardando o agent");
    }
  } catch (erro) {
    if (!observador.ativo()) return;
    tela.barra.hidden = true;
    tela.simbolo.textContent = "⚠";
    tela.situacao.textContent = t("acompanhamento interrompido");
    tela.situacao.className = "execucao__situacao";
    tela.parar.disabled = true;
    mostrarDetalhe(tela, api.mensagemDeErro(erro));
  } finally {
    window.clearInterval(cronometro);
  }
}

/** O estado de quem ainda está rodando. */
function mostrarSituacao(tela: Tela, execucao: api.ExecucaoDoTrabalho): void {
  tela.situacao.className = "execucao__situacao";
  tela.situacao.textContent = t(ROTULO_DA_SITUACAO[execucao.situacao]);
  tela.detalhe.textContent = execucao.etapa ? t("na etapa", { etapa: execucao.etapa }) : "";
  // Começou a rodar: o que estava escrito aqui era de antes do disparo.
  mostrarDetalhe(tela, "");
}

/**
 * O texto longo do corpo: a mensagem do histórico ou o erro que apareceu.
 *
 * O rodapé não serve para isto. Lá o texto é uma linha só, cortada com
 * reticências, e um erro do Agent — que costuma vir com três frases dizendo
 * quem disparou, em que etapa parou e por quê — chegaria ao usuário pela
 * metade. Aqui ele quebra linha, rola e pode ser copiado.
 */
function mostrarDetalhe(tela: Tela, texto: string): void {
  tela.mensagem.textContent = texto;
  tela.mensagem.hidden = texto.length === 0;
}

/**
 * O fim: símbolo, veredito e a mensagem do Agent.
 *
 * A mensagem vem do histórico e não é enfeite — num trabalho que falhou é o
 * único lugar que diz **por que**, e é o texto que o SSMS mostra no mesmo caso.
 */
function mostrarFim(tela: Tela, execucao: api.ExecucaoDoTrabalho): void {
  const sucesso = execucao.resultado === "sucesso";
  tela.barra.hidden = true;
  tela.parar.disabled = true;
  tela.simbolo.textContent = sucesso ? "✔" : execucao.resultado === "falha" ? "✖" : "■";
  tela.simbolo.className = `execucao__simbolo execucao__simbolo--${sucesso ? "ok" : "fim"}`;
  tela.situacao.className = `execucao__situacao execucao__situacao--${
    sucesso ? "ok" : execucao.resultado === "falha" ? "erro" : ""
  }`;
  tela.situacao.textContent = t(ROTULO_DO_RESULTADO[execucao.resultado]);
  mostrarDetalhe(tela, execucao.mensagem);
  tela.detalhe.textContent =
    execucao.duracao === null ? "" : t("em tempo", { tempo: comoTempo(execucao.duracao) });
  tela.tempo.textContent = execucao.iniciouEm
    ? t("iniciada em", { momento: formatarDataHora(execucao.iniciouEm) })
    : "";
}

const ROTULO_DA_SITUACAO = {
  executando: "trabalho em execucao",
  aguardandoThread: "aguardando thread",
  entreTentativas: "entre tentativas",
  suspenso: "execucao suspensa",
  finalizando: "concluindo execucao",
  parado: "aguardando o agent",
  desconhecido: "situacao desconhecida",
} as const;

const ROTULO_DO_RESULTADO = {
  sucesso: "trabalho terminou ok",
  falha: "trabalho terminou com falha",
  cancelado: "trabalho foi cancelado",
  novaTentativa: "trabalho em nova tentativa",
  desconhecido: "trabalho terminou sem resultado",
} as const;

/**
 * Duração em segundos no formato do resto do aplicativo.
 *
 * É a mesma função do rodapé da consulta, de propósito: o tempo de um trabalho
 * se lê igual ao tempo de um `select`, e não há por que inventar um segundo
 * formato de duração dentro do mesmo programa.
 */
function comoTempo(segundos: number): string {
  return formatarDuracao(Math.max(0, Math.round(segundos)) * 1_000);
}

/** O rótulo do veredito, para quem precisa dele fora desta tela. */
export function rotuloDoResultado(resultado: api.ResultadoDoTrabalho): string {
  return t(ROTULO_DO_RESULTADO[resultado]);
}

function espera(ms: number): Promise<void> {
  return new Promise((resolver) => window.setTimeout(resolver, ms));
}

function anunciar(tela: Tela, tipo: "aviso" | "erro", texto: string): void {
  tela.status.className = tipo === "erro" ? "status status--erro" : "status";
  tela.status.textContent = texto;
}

interface Tela {
  caixa: HTMLElement;
  simbolo: HTMLElement;
  situacao: HTMLElement;
  detalhe: HTMLElement;
  tempo: HTMLElement;
  mensagem: HTMLElement;
  barra: HTMLElement;
  status: HTMLElement;
  parar: HTMLButtonElement;
  sair: HTMLButtonElement;
}

function montarTela(nome: string, etapa: string | null): Tela {
  const caixa = document.createElement("main");
  caixa.className = "dialogo dialogo-execucao";
  caixa.setAttribute("aria-label", t("execucao de", { trabalho: nome }));
  caixa.innerHTML = `
      <div class="execucao__corpo">
        <div class="execucao__linha" aria-live="polite">
          <span class="execucao__simbolo" data-elemento="simbolo">▷</span>
          <div class="execucao__texto">
            <p class="execucao__situacao" data-elemento="situacao"></p>
            <p class="execucao__detalhe" data-elemento="detalhe"></p>
          </div>
        </div>
        <div class="execucao__barra" data-elemento="barra">
          <div class="execucao__barra-pulso"></div>
        </div>
        <p class="execucao__tempo" data-elemento="tempo"></p>
        <p class="execucao__mensagem" data-elemento="mensagem" hidden></p>
      </div>
      <footer class="dialogo__rodape">
        <span class="status" data-elemento="status"></span>
        <div class="dialogo__botoes">
          <button data-elemento="parar" disabled>${t("parar")}</button>
          <button data-elemento="sair" class="botao-principal">${t("fechar")}</button>
        </div>
      </footer>`;

  const achar = <T extends HTMLElement>(elemento: string): T =>
    caixa.querySelector<T>(`[data-elemento="${elemento}"]`)!;

  const tela: Tela = {
    caixa,
    simbolo: achar("simbolo"),
    situacao: achar("situacao"),
    detalhe: achar("detalhe"),
    tempo: achar("tempo"),
    mensagem: achar("mensagem"),
    barra: achar("barra"),
    status: achar("status"),
    parar: achar<HTMLButtonElement>("parar"),
    sair: achar<HTMLButtonElement>("sair"),
  };
  tela.situacao.textContent = t("iniciando o trabalho");
  tela.detalhe.textContent = etapa ? t("a partir da etapa", { etapa }) : "";
  return tela;
}
