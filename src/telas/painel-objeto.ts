/**
 * Visualizador de tabela: dados, DDL e estrutura no mesmo documento.
 *
 * O grid de Dados usa o mesmo cursor e o mesmo fetch parcial das consultas.
 * DDL e Estrutura reutilizam os metadados que a árvore já consultava.
 *
 * No **SQL Server** o painel abre uma sessão só dele para os Dados, como uma aba
 * de consulta faz (requisito 7). Não é simetria: ali a conexão atende um pedido
 * de cada vez, e um grid deixado pela metade prenderia a sessão da árvore —
 * expandir um nó ficaria esperando o usuário rolar o grid até o fim. DDL e
 * Estrutura continuam na sessão da conexão: são leituras curtas de catálogo,
 * que acabam antes de a próxima começar.
 */

import "./painel-objeto.css";

import * as api from "../api";
import { t } from "../idioma";
import { formatarDuracao } from "../formatacao";
import { abrirMenuDeContexto } from "../componentes/menu-contexto";
import type { ContextoDeConexao } from "./arvore-objetos";
import { EditorSql } from "./editor-sql";
import { Grid } from "./grid";
import { copiarInsertParaClipboard } from "./script-insert";

const FETCH_PADRAO = 50;

export type AbaDoObjeto = "dados" | "ddl" | "estrutura";

/**
 * O rótulo de cada aba, lido na hora de montar.
 *
 * Função, e não constante de módulo: uma constante seria avaliada na importação
 * e ficaria presa ao idioma daquele instante, deixando o painel em português
 * depois de trocar para English.
 */
function rotuloDaAba(aba: AbaDoObjeto): string {
  if (aba === "ddl") return "DDL";
  return t(aba === "dados" ? "dados" : "estrutura");
}

/** Painel aberto por uma tabela ou view da árvore. */
export class PainelDeObjeto {
  readonly elemento = document.createElement("div");

  private readonly botoes = new Map<AbaDoObjeto, HTMLButtonElement>();
  private readonly paineis = new Map<AbaDoObjeto, HTMLElement>();
  private readonly corpos = new Map<AbaDoObjeto, HTMLElement>();
  private readonly rodapes = new Map<AbaDoObjeto, HTMLElement>();
  private readonly carregadas = new Set<AbaDoObjeto>();
  private readonly campoFetch = document.createElement("input");

  private abaAtual: AbaDoObjeto;
  private gridDados: Grid | null = null;
  private gridEstrutura: Grid | null = null;
  private editorDdl: EditorSql | null = null;
  private ddl = "";
  private cursorAtual: string | null = null;
  private encerrado = false;
  private versaoDados = 0;
  private versaoDdl = 0;
  private versaoEstrutura = 0;
  /** Sessão própria do grid no SQL Server; nula até a aba de Dados abrir. */
  private sessaoDoGrid: Promise<string> | null = null;

  constructor(
    private readonly contexto: ContextoDeConexao,
    private readonly nome: string,
    private readonly categoria: "tabelas" | "views",
    abaInicial: AbaDoObjeto,
    private readonly aoRelatar: (mensagem: string, erro?: boolean) => void,
  ) {
    this.elemento.className = "painel-objeto";
    this.abaAtual = abaInicial;
    this.elemento.append(this.montarAbas(), this.montarPaineis());
    this.selecionar(abaInicial);
  }

  /** O nome como se lê na tela: sem o `dbo.` do SQL Server (requisito 44). */
  private get visivel(): string {
    return api.nomeVisivel(this.nome);
  }

  /** Seleciona a seção pedida pelo menu de contexto e a carrega sob demanda. */
  selecionar(aba: AbaDoObjeto): void {
    this.abaAtual = aba;
    for (const [chave, botao] of this.botoes) {
      const ativa = chave === aba;
      botao.classList.toggle("painel-objeto__aba--ativa", ativa);
      botao.setAttribute("aria-selected", String(ativa));
    }
    for (const [chave, painel] of this.paineis) painel.hidden = chave !== aba;

    if (!this.carregadas.has(aba)) {
      this.carregadas.add(aba);
      void this.carregar(aba);
    }
    this.focar();
  }

  /** Ajusta o conteúdo quando a aba principal volta a ficar visível. */
  focar(): void {
    if (this.abaAtual === "dados") this.gridDados?.ajustar();
    if (this.abaAtual === "estrutura") this.gridEstrutura?.ajustar();
    if (this.abaAtual === "ddl") this.editorDdl?.focar();
  }

  /** Libera cursor, sessão própria e CodeMirror quando a aba é fechada. */
  encerrar(): void {
    this.encerrado = true;
    this.fecharCursor();
    this.editorDdl?.destruir();
    // A sessão do grid é desta aba: some com ela. O `catch` é para a conexão
    // que já caiu — não há o que desligar, e nem a quem relatar.
    void this.sessaoDoGrid?.then(api.desconectar).catch(() => {});
    this.sessaoDoGrid = null;
  }

  private get sqlServer(): boolean {
    return this.contexto.perfil.tipoBanco === "sqlserver";
  }

  /** O banco do objeto (SQL Server); vazio no Oracle. */
  private get banco(): string | null {
    return this.sqlServer ? this.contexto.banco ?? null : null;
  }

  /**
   * Em que sessão o `select` roda.
   *
   * No Oracle é a da conexão, como sempre foi. No SQL Server é uma irmã, aberta
   * uma vez só e apontada para o banco do objeto — que é também o que dispensa
   * escrever o nome do banco na consulta.
   */
  private sessaoParaDados(): Promise<string> {
    if (!this.sqlServer) return Promise.resolve(this.contexto.sessaoId);
    this.sessaoDoGrid ??= api.conectarIrma(this.contexto.sessaoId, this.banco ?? undefined);
    return this.sessaoDoGrid;
  }

  private montarAbas(): HTMLElement {
    const barra = document.createElement("div");
    barra.className = "painel-objeto__abas";
    barra.setAttribute("role", "tablist");

    for (const chave of ["dados", "ddl", "estrutura"] as const) {
      const botao = document.createElement("button");
      botao.className = "painel-objeto__aba";
      botao.textContent = rotuloDaAba(chave);
      botao.setAttribute("role", "tab");
      botao.addEventListener("click", () => this.selecionar(chave));
      this.botoes.set(chave, botao);
      barra.append(botao);
    }
    return barra;
  }

  private montarPaineis(): HTMLElement {
    const area = document.createElement("div");
    area.className = "painel-objeto__paineis";
    area.append(
      this.montarPainel("dados", this.montarBarraDeDados()),
      this.montarPainel("ddl", this.montarBarraDeDdl()),
      this.montarPainel("estrutura", this.montarBarraDeEstrutura()),
    );
    return area;
  }

  private montarPainel(aba: AbaDoObjeto, barra: HTMLElement): HTMLElement {
    const painel = document.createElement("section");
    painel.className = "painel-objeto__painel";
    painel.setAttribute("role", "tabpanel");

    const corpo = document.createElement("div");
    corpo.className = "painel-objeto__corpo";
    const rodape = document.createElement("footer");
    rodape.className = "painel-objeto__rodape";
    painel.append(barra, corpo, rodape);

    this.paineis.set(aba, painel);
    this.corpos.set(aba, corpo);
    this.rodapes.set(aba, rodape);
    return painel;
  }

  private montarBarraDeDados(): HTMLElement {
    const barra = this.criarBarra();
    barra.append(this.criarBotao(t("atualizar"), () => void this.carregarDados()));

    const espaco = document.createElement("span");
    espaco.className = "painel-objeto__espaco";
    const rotulo = document.createElement("label");
    rotulo.textContent = t("linhas por vez");
    this.campoFetch.type = "number";
    this.campoFetch.value = String(FETCH_PADRAO);
    this.campoFetch.min = "1";
    this.campoFetch.max = "10000";
    this.campoFetch.title = t("ajuda linhas por vez");
    rotulo.append(this.campoFetch);
    barra.append(espaco, rotulo);
    return barra;
  }

  private montarBarraDeDdl(): HTMLElement {
    const barra = this.criarBarra();
    barra.append(
      this.criarBotao(t("copiar ddl"), () => void this.copiarDdl()),
      this.criarBotao(t("atualizar"), () => void this.carregarDdl()),
    );
    return barra;
  }

  private montarBarraDeEstrutura(): HTMLElement {
    const barra = this.criarBarra();
    barra.append(this.criarBotao(t("atualizar"), () => void this.carregarEstrutura()));
    return barra;
  }

  private criarBarra(): HTMLElement {
    const barra = document.createElement("div");
    barra.className = "painel-objeto__barra";
    return barra;
  }

  private criarBotao(rotulo: string, acao: () => void): HTMLButtonElement {
    const botao = document.createElement("button");
    botao.className = "botao-discreto";
    botao.textContent = rotulo;
    botao.addEventListener("click", acao);
    return botao;
  }

  private carregar(aba: AbaDoObjeto): Promise<void> {
    if (aba === "dados") return this.carregarDados();
    if (aba === "ddl") return this.carregarDdl();
    return this.carregarEstrutura();
  }

  private async carregarDados(): Promise<void> {
    const versao = ++this.versaoDados;
    this.fecharCursor();
    this.mostrarCarregando("dados", t("carregando dados"));
    try {
      const resultados = await api.executarSql(
        await this.sessaoParaDados(),
        `SELECT * FROM ${this.identificador()}`,
        false,
        [],
        this.tamanhoDoFetch,
      );
      const resultado = [...resultados].reverse().find((item) => item.consulta !== null);
      const consulta = resultado?.consulta;
      if (!resultado || !consulta) throw new Error(t("tabela sem grid"));
      if (this.encerrado || versao !== this.versaoDados) {
        if (consulta.temMais) void api.fecharCursor(consulta.cursorId);
        return;
      }

      this.cursorAtual = consulta.temMais ? consulta.cursorId : null;
      const grid = new Grid(consulta.colunas, {
        aoPedirMais: () => void this.trazerMaisLinhas(),
      });
      grid.acrescentar(consulta.linhas);
      grid.definirTemMais(consulta.temMais);
      grid.elemento.addEventListener("contextmenu", (evento) => this.menuDoGrid(evento, grid));
      this.gridDados = grid;
      this.corpo("dados").replaceChildren(grid.elemento);
      grid.ajustar();
      this.escreverRodape(
        "dados",
        `${t("n linhas", { n: consulta.linhas.length })}${consulta.temMais ? "+" : ""}` +
          ` — ${formatarDuracao(resultado.duracaoMs)}`,
      );
    } catch (erro) {
      if (this.encerrado || versao !== this.versaoDados) return;
      this.mostrarErro("dados", erro);
    }
  }

  private async trazerMaisLinhas(): Promise<void> {
    const cursor = this.cursorAtual;
    const grid = this.gridDados;
    if (!cursor || !grid) return;
    try {
      const bloco = await api.buscarMaisLinhas(cursor, this.tamanhoDoFetch);
      if (this.encerrado || cursor !== this.cursorAtual) return;
      grid.acrescentar(bloco.linhas);
      grid.definirTemMais(bloco.temMais);
      if (!bloco.temMais) this.cursorAtual = null;
      this.escreverRodape(
        "dados",
        `${t("n linhas", { n: grid.totalDeLinhas })}` +
          `${bloco.temMais ? "+" : ` — ${t("fim das linhas")}`}`,
      );
    } catch (erro) {
      this.aoRelatar(api.mensagemDeErro(erro), true);
      grid.definirTemMais(false);
    }
  }

  private async carregarDdl(): Promise<void> {
    const versao = ++this.versaoDdl;
    this.escreverRodape("ddl", t("carregando ddl"));
    if (!this.editorDdl) this.mostrarCarregando("ddl", t("carregando ddl"));
    try {
      const ddl = await api.fonteDoObjeto(
        this.contexto.sessaoId,
        this.categoria,
        this.nome,
        "especificacao",
        this.banco,
      );
      if (this.encerrado || versao !== this.versaoDdl) return;
      this.ddl = ddl;
      if (this.editorDdl) {
        this.editorDdl.substituirTexto(ddl);
      } else {
        this.editorDdl = new EditorSql({
          texto: ddl,
          somenteLeitura: true,
          sugestoes: {
            objetos: (prefixo) =>
              api.sugerirObjetos(this.contexto.sessaoId, prefixo, this.banco),
            ...(this.contexto.perfil.tipoBanco === "oracle" ? {
              relacoes: (prefixo: string, esquema: string | undefined, local: boolean) =>
                api.sugerirRelacoes(this.contexto.sessaoId, prefixo, esquema ?? null, local),
            } : {}),
            colunas: (objeto) => api.listarColunas(this.contexto.sessaoId, objeto, this.banco),
          },
        });
      }
      this.corpo("ddl").replaceChildren(this.editorDdl.elemento);
      this.escreverRodape(
        "ddl",
        `${t("ddl carregado")} — ${this.visivel} — ${this.contexto.perfil.nome}`,
      );
    } catch (erro) {
      if (this.encerrado || versao !== this.versaoDdl) return;
      this.mostrarErro("ddl", erro);
    }
  }

  private async carregarEstrutura(): Promise<void> {
    const versao = ++this.versaoEstrutura;
    this.mostrarCarregando("estrutura", t("carregando estrutura"));
    try {
      const colunas = await api.listarColunas(this.contexto.sessaoId, this.nome, this.banco);
      if (this.encerrado || versao !== this.versaoEstrutura) return;
      const grid = new Grid([
        { nome: t("nome da coluna"), tipo: "VARCHAR2", numerica: false },
        { nome: t("tipo"), tipo: "VARCHAR2", numerica: false },
        { nome: t("aceita nulo"), tipo: "VARCHAR2", numerica: false },
      ]);
      grid.acrescentar(
        colunas.map((coluna) => [
          coluna.nome,
          coluna.tipo,
          coluna.aceitaNulo ? t("sim") : t("nao"),
        ]),
      );
      grid.elemento.addEventListener("contextmenu", (evento) => this.menuDoGrid(evento, grid));
      this.gridEstrutura = grid;
      this.corpo("estrutura").replaceChildren(grid.elemento);
      grid.ajustar();
      this.escreverRodape(
        "estrutura",
        `${t("n colunas", { n: colunas.length })} — ${this.visivel} —` +
          ` ${this.contexto.perfil.nome}`,
      );
    } catch (erro) {
      if (this.encerrado || versao !== this.versaoEstrutura) return;
      this.mostrarErro("estrutura", erro);
    }
  }

  private async copiarDdl(): Promise<void> {
    if (!this.ddl) return;
    try {
      await navigator.clipboard.writeText(this.ddl);
      this.aoRelatar(t("ddl copiado", { objeto: this.visivel }));
    } catch (erro) {
      this.aoRelatar(api.mensagemDeErro(erro), true);
    }
  }

  private menuDoGrid(evento: MouseEvent, grid: Grid): void {
    abrirMenuDeContexto(evento, [
      { rotulo: t("copiar"), acao: () => void grid.copiar(false) },
      { rotulo: t("copiar com cabecalho"), acao: () => void grid.copiar(true) },
      {
        rotulo: t("copiar para teams"),
        acao: () => void grid.copiarParaTeams(),
      },
      { rotulo: t("selecionar tudo"), acao: () => grid.selecionarTudo() },
      // Requisito 39: aqui a tabela é conhecida sem análise de texto nenhuma —
      // a consulta deste grid é `SELECT * FROM` ela mesma. Na aba de Estrutura
      // não há o que inserir: aquele grid mostra o dicionário, e não a tabela.
      ...(grid === this.gridDados
        ? [
            {
              rotulo: t("gerar script de insert"),
              separadorAntes: true,
              acao: () =>
                void copiarInsertParaClipboard(
                  grid.paraScriptDeInsert(),
                  this.identificador(),
                  this.sqlServer,
                  this.aoRelatar,
                ),
            },
          ]
        : []),
    ]);
  }

  private mostrarCarregando(aba: AbaDoObjeto, texto: string): void {
    const mensagem = document.createElement("p");
    mensagem.className = "painel-objeto__mensagem";
    mensagem.textContent = texto;
    this.corpo(aba).replaceChildren(mensagem);
    this.escreverRodape(aba, texto);
  }

  private mostrarErro(aba: AbaDoObjeto, erro: unknown): void {
    const texto = api.mensagemDeErro(erro);
    const mensagem = document.createElement("p");
    mensagem.className = "painel-objeto__mensagem painel-objeto__mensagem--erro";
    mensagem.textContent = texto;
    this.corpo(aba).replaceChildren(mensagem);
    this.escreverRodape(aba, texto, true);
    this.aoRelatar(texto, true);
  }

  private corpo(aba: AbaDoObjeto): HTMLElement {
    const corpo = this.corpos.get(aba);
    if (!corpo) throw new Error(`corpo da aba ${aba} não foi montado`);
    return corpo;
  }

  private escreverRodape(aba: AbaDoObjeto, texto: string, erro = false): void {
    const rodape = this.rodapes.get(aba);
    if (!rodape) return;
    rodape.textContent = texto;
    rodape.classList.toggle("painel-objeto__rodape--erro", erro);
  }

  private fecharCursor(): void {
    if (!this.cursorAtual) return;
    void api.fecharCursor(this.cursorAtual);
    this.cursorAtual = null;
  }

  /** O nome do objeto citado como o banco dele exige. */
  private identificador(): string {
    return this.sqlServer ? identificadorSqlServer(this.nome) : identificadorOracle(this.nome);
  }

  private get tamanhoDoFetch(): number {
    const valor = Number(this.campoFetch.value);
    return Number.isFinite(valor) && valor > 0 ? Math.min(valor, 10000) : FETCH_PADRAO;
  }
}

/** Cita inclusive nomes comuns, preservando também tabelas criadas entre aspas. */
function identificadorOracle(nome: string): string {
  return nome
    .split(".")
    .map((parte) => `"${parte.replace(/"/g, '""')}"`)
    .join(".");
}

/** `dbo.cliente` vira `[dbo].[cliente]`, como o SQL Server exige. */
function identificadorSqlServer(nome: string): string {
  return nome
    .split(".")
    .map((parte) => `[${parte.replace(/]/g, "]]")}]`)
    .join(".");
}
