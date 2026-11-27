import "./alertas.css";

import * as api from "../api";
import { t } from "../idioma";

interface Verificacao {
  regra: api.AlertaSalvo;
  ultimoAviso?: { mensagem: string; instante: number };
  temporizador?: number;
  executando?: boolean;
}

/** Vive com a sessão da árvore, inclusive quando o nó está recolhido. */
export class MonitorDeAlertas {
  private readonly verificacoes = new Map<string, Verificacao>();
  private encerrado = false;
  private readonly inicializacao: Promise<void>;

  constructor(
    private readonly sessaoId: string,
    private readonly conexao: string,
  ) {
    this.inicializacao = this.lerRegras();
  }

  private async lerRegras(): Promise<void> {
    try {
      for (const regra of await api.listarAlertas(this.sessaoId)) this.atualizar(regra);
    } catch {
      // O diálogo tenta ler de novo ao abrir e mostra a falha ali.
    }
  }

  async pronto(): Promise<void> {
    await this.inicializacao;
  }

  atualizar(regra: api.AlertaSalvo): void {
    if (this.encerrado) return;
    const anterior = this.verificacoes.get(regra.id);
    this.remover(regra.id);
    const verificacao: Verificacao = {
      regra,
      ultimoAviso: anterior?.regra.consulta === regra.consulta &&
        anterior.regra.banco === regra.banco && anterior.regra.contagemLegada === regra.contagemLegada
        ? anterior.ultimoAviso : undefined,
    };
    this.verificacoes.set(regra.id, verificacao);
    this.agendar(verificacao);
  }

  remover(id: string): void {
    window.clearTimeout(this.verificacoes.get(id)?.temporizador);
    this.verificacoes.delete(id);
  }

  async executar(id: string): Promise<void> {
    await this.pronto();
    const verificacao = this.verificacoes.get(id);
    if (!verificacao || this.encerrado || verificacao.executando) return;
    window.clearTimeout(verificacao.temporizador);
    await this.verificar(verificacao, true);
  }

  encerrar(): void {
    this.encerrado = true;
    for (const id of this.verificacoes.keys()) this.remover(id);
  }

  private agendar(verificacao: Verificacao): void {
    if (!verificacao.regra.ativo) return;
    verificacao.temporizador = window.setTimeout(
      () => void this.verificar(verificacao), verificacao.regra.minutos * 60_000,
    );
  }

  private async verificar(verificacao: Verificacao, manual = false): Promise<void> {
    if (verificacao.executando) return;
    verificacao.executando = true;
    const { regra } = verificacao;
    const vigente = () => this.verificacoes.get(regra.id) === verificacao;
    try {
      const resultado = await api.verificarAlerta(this.sessaoId, regra.id, manual);
      if (!vigente() || resultado === null) return;
      const { mensagem } = resultado;
      if (mensagem === null) {
        verificacao.ultimoAviso = undefined;
        return;
      }

      const agora = performance.now();
      const chave = regra.contagemLegada ? "" : mensagem;
      const ultimo = verificacao.ultimoAviso;
      if (ultimo?.mensagem === chave &&
          (regra.repetirApos === null || agora - ultimo.instante < regra.repetirApos * 60_000)) return;

      verificacao.ultimoAviso = { mensagem: chave, instante: agora };
      const texto = regra.contagemLegada ? t("alerta disparado", { valor: mensagem }) : mensagem;
      const nota = mostrarAlerta(regra, this.conexao, texto);
      if (regra.temTeams) {
        const enviado = await api.notificarAlertaNoTeams(this.sessaoId, regra.id, mensagem, manual).catch(() => false);
        if (!enviado && vigente()) nota.textContent = t("alerta teams falhou");
      }
    } catch {
      // A próxima verificação tenta novamente. Falhas de rede ficam no log.
    } finally {
      verificacao.executando = false;
      if (vigente()) this.agendar(verificacao);
    }
  }
}

function mostrarAlerta(regra: api.AlertaSalvo, conexao: string, texto: string): HTMLElement {
  let avisos = document.querySelector(".avisos-de-alerta");
  if (!avisos) {
    avisos = document.createElement("div");
    avisos.className = "avisos-de-alerta";
    avisos.setAttribute("aria-live", "polite");
    document.body.append(avisos);
  }
  const aviso = document.createElement("article");
  const cabecalho = document.createElement("header");
  const titulo = document.createElement("strong");
  titulo.textContent = `${t("alertas")} · ${regra.nome}`;
  const fechar = document.createElement("button");
  fechar.type = "button";
  fechar.className = "dialogo__fechar";
  fechar.textContent = "×";
  fechar.setAttribute("aria-label", t("fechar"));
  fechar.addEventListener("click", () => aviso.remove());
  cabecalho.append(titulo, fechar);
  const destino = document.createElement("div");
  destino.textContent = regra.banco ? `${conexao} · ${regra.banco}` : conexao;
  const mensagem = document.createElement("div");
  mensagem.className = "alertas__mensagem";
  mensagem.textContent = texto;
  const nota = document.createElement("div");
  nota.className = "alertas__erro";
  aviso.append(cabecalho, destino, mensagem, nota);
  avisos.prepend(aviso);
  return nota;
}
