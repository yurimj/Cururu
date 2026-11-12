/**
 * Novo Trabalho e Propriedades do Trabalho do SQL Server Agent (requisito 36).
 *
 * Uma tela só para os dois: são os mesmos campos, e um segundo formulário só
 * para editar seria a mesma coisa escrita duas vezes, com duas chances de
 * divergir. Com `trabalhoId`, a tela nasce preenchida e grava por cima.
 *
 * É a tela de Novo Trabalho do SSMS, bem mais curta: das seis páginas dela ficam
 * três — **Geral**, **Etapas** e **Agendas**. Alertas, notificações e destinos
 * não entram; as duas primeiras dependem de operador cadastrado e de Database
 * Mail, e destinos só existe em administração multisservidor, que o Cururu não
 * faz.
 *
 * Os campos **Origem**, **Criado em**, **Última modificação** e **Última
 * execução** nunca são editáveis: quem os preenche é o servidor. No cadastro
 * eles saem vazios, como no SSMS — escondê-los num cadastro que os terá logo
 * depois deixaria a tela diferente daquela que o usuário já conhece; nas
 * propriedades é onde eles finalmente têm o que mostrar.
 */

import "./dialogo-trabalho.css";
import { comIndicadorDeAbertura } from "../componentes/indicador-de-abertura";

import * as api from "../api";
import type { AgendaDeTrabalho, EtapaDeTrabalho, NovoTrabalho } from "../api";
import { fecharComEsc, tornarJanela } from "../componentes/janela";
import { formatarDataHora } from "../formatacao";
import { t } from "../idioma";
import { abrirDialogoAgenda, agendaNova, resumoDaAgenda } from "./dialogo-agenda";
import { abrirDialogoEtapa, etapaNova, ACOES } from "./dialogo-etapa";

/** O que os campos do servidor mostram enquanto não há o que mostrar. */
const TRACO = "—";

/** Categoria proposta pelo SQL Server quando nenhuma é escolhida. */
const CATEGORIA_PADRAO = "[Uncategorized (Local)]";

/**
 * Abre a tela e devolve `true` quando algo foi gravado — que é quando a árvore
 * precisa se relistar.
 *
 * Sem `trabalhoId` é um cadastro novo; com ele, a mesma tela vira **Propriedades
 * do Trabalho** e grava por cima. É a mesma tela porque são os mesmos campos: um
 * segundo formulário só para editar seria a mesma coisa escrita duas vezes, com
 * duas chances de divergir.
 */
export function abrirDialogoTrabalho(
  sessaoId: string,
  trabalhoId?: string,
  nome?: string,
): Promise<boolean> {
  return new Promise((resolver) =>
    new DialogoTrabalho(sessaoId, trabalhoId ?? null, nome ?? "", resolver).abrir(),
  );
}

const molde = (edicao: boolean): string => {
  // O nome do trabalho entra depois, por `escreverTitulo`: aqui ele ainda não
  // chegou do servidor, e nome de trabalho não pode passar por `innerHTML`.
  const titulo = t(edicao ? "propriedades do trabalho" : "novo trabalho");
  return `
<div class="dialogo dialogo-trabalho" role="dialog" aria-modal="true"
     aria-label="${titulo}">
  <header class="dialogo__barra">
    <h1 data-elemento="titulo">${titulo}</h1>
    <button class="dialogo__fechar" data-acao="cancelar" title="${t("fechar")}"
            aria-label="${t("fechar")}">✕</button>
  </header>

  <div class="trabalho__conteudo">
    <nav class="trabalho__paginas" role="tablist" aria-label="${t("selecione uma pagina")}">
      <button role="tab" data-pagina="etapas" aria-selected="true">${t("etapas")}</button>
      <button role="tab" data-pagina="agendas" aria-selected="false">${t("agendas")}</button>
      <button role="tab" data-pagina="geral" aria-selected="false">${t("geral")}</button>
    </nav>

    <div class="trabalho__corpo">
      <section data-painel="etapas">
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
        <div class="trabalho__acoes">
          <button data-acao="nova-etapa">${t("nova")}</button>
          <button data-acao="editar-etapa">${t("editar")}</button>
          <button data-acao="excluir-etapa">${t("excluir")}</button>
          <button data-acao="subir-etapa" title="${t("mover para cima")}"
                  aria-label="${t("mover para cima")}">▲</button>
          <button data-acao="descer-etapa" title="${t("mover para baixo")}"
                  aria-label="${t("mover para baixo")}">▼</button>
        </div>
      </section>

      <section data-painel="agendas" hidden>
        <div class="trabalho__lista">
          <table class="trabalho__grade">
            <thead>
              <tr>
                <th>${t("nome")}</th>
                <th class="trabalho__ordem">${t("habilitada")}</th>
                <th>${t("descricao")}</th>
              </tr>
            </thead>
            <tbody data-elemento="agendas"></tbody>
          </table>
        </div>
        <div class="trabalho__acoes">
          <button data-acao="nova-agenda">${t("nova")}</button>
          <button data-acao="editar-agenda">${t("editar")}</button>
          <button data-acao="excluir-agenda">${t("excluir")}</button>
        </div>
      </section>

      <section data-painel="geral" hidden>
        <div class="grade">
          <label for="trabalho-nome">${t("nome")}</label>
          <input type="text" id="trabalho-nome" class="largura-total" data-campo="nome" />

          <label for="trabalho-proprietario">${t("proprietario")}</label>
          <select id="trabalho-proprietario" class="largura-total" data-campo="proprietario"></select>

          <label for="trabalho-categoria">${t("categoria")}</label>
          <select id="trabalho-categoria" class="largura-total" data-campo="categoria"></select>

          <label for="trabalho-descricao">${t("descricao")}</label>
          <textarea id="trabalho-descricao" class="largura-total trabalho__descricao"
                    data-campo="descricao"></textarea>

          <span></span>
          <label class="caixa-marcacao largura-total">
            <input type="checkbox" data-campo="habilitado" checked />
            ${t("habilitado")}
          </label>
        </div>

        <hr class="separador-painel" />

        <div class="grade trabalho__somente-leitura">
          <label>${t("origem")}</label>
          <output class="largura-total" data-elemento="origem">—</output>
          <label>${t("criado em")}</label>
          <output class="largura-total" data-elemento="criado-em">—</output>
          <label>${t("ultima modificacao")}</label>
          <output class="largura-total" data-elemento="modificado-em">—</output>
          <label>${t("ultima execucao")}</label>
          <output class="largura-total" data-elemento="ultima-execucao">—</output>
        </div>
        <p class="trabalho__nota" data-elemento="nota">${t("preenchido pelo servidor")}</p>
      </section>
    </div>
  </div>

  <footer class="dialogo__rodape">
    <span class="status" data-elemento="status"></span>
    <div class="dialogo__botoes">
      <button data-acao="confirmar" class="botao-principal">OK</button>
      <button data-acao="cancelar">${t("cancelar")}</button>
    </div>
  </footer>
</div>`;
};

class DialogoTrabalho {
  private readonly fundo = document.createElement("div");
  private caixa!: HTMLElement;
  private etapas: EtapaDeTrabalho[] = [];
  private agendas: AgendaDeTrabalho[] = [];
  private etapaSelecionada = -1;
  private agendaSelecionada = -1;
  /** Bancos oferecidos na etapa T-SQL; vazio até o servidor responder. */
  private bancos: string[] = [];
  private gravando = false;
  private pararJanela: () => void = () => {};
  private pararEsc: () => void = () => {};

  constructor(
    private readonly sessaoId: string,
    /** Nulo num cadastro novo; preenchido nas propriedades de um trabalho. */
    private readonly trabalhoId: string | null,
    /** Nome que a árvore já conhece, para o título nascer certo. */
    private readonly nome: string,
    private readonly resolver: (gravado: boolean) => void,
  ) {}

  abrir(): void {
    this.fundo.className = "dialogo-fundo";
    this.fundo.innerHTML = molde(this.trabalhoId !== null);
    this.caixa = this.fundo.firstElementChild as HTMLElement;
    document.body.append(this.fundo);

    this.ligarEventos();
    this.escreverTitulo(this.nome);
    this.redesenharEtapas();
    this.redesenharAgendas();
    this.mostrarPagina(this.paginaInicial());
    // Move pela barra, mas não muda de tamanho: as três páginas cabem na caixa,
    // e o que precisava crescer — o comando da etapa — tem janela própria.
    this.pararJanela = tornarJanela(this.caixa);
    // Esc não pode fechar uma tela que já está gravando: o trabalho estaria
    // sendo criado no servidor, e a tela sumiria sem dizer o que aconteceu.
    this.pararEsc = fecharComEsc(this.fundo, () => this.fechar(false), () => !this.gravando);
    // Num cadastro o primeiro campo é o nome; nas propriedades, a tela abre nas
    // etapas, e o foco começa na lista de páginas.
    if (this.paginaInicial() === "geral") this.campo("nome").focus();
    else this.caixa.querySelector<HTMLButtonElement>('[data-pagina="etapas"]')?.focus();
    void comIndicadorDeAbertura(
      this.nome || t("novo trabalho"),
      () => this.carregarDados(),
    );
  }

  /**
   * Proprietário, categorias, servidor e bancos.
   *
   * Falhar aqui não fecha a tela: sem a lista de logins ainda se cadastra um
   * trabalho do próprio usuário, e é o servidor que dará a última palavra no OK.
   */
  private async carregarDados(): Promise<void> {
    try {
      const [dados, bancos] = await Promise.all([
        api.dadosDeTrabalho(this.sessaoId),
        api.listarBancos(this.sessaoId, true),
      ]);
      this.bancos = bancos;

      const proprietario = this.campo<HTMLSelectElement>("proprietario");
      for (const login of dados.logins) proprietario.append(new Option(login, login));
      proprietario.value = dados.loginAtual;

      const categoria = this.campo<HTMLSelectElement>("categoria");
      const categorias = dados.categorias.length ? dados.categorias : [CATEGORIA_PADRAO];
      for (const nome of categorias) categoria.append(new Option(nome, nome));
      categoria.value = categorias.includes(CATEGORIA_PADRAO) ? CATEGORIA_PADRAO : categorias[0];

      if (dados.servidor) this.elemento("origem").textContent = dados.servidor;
      if (this.trabalhoId) await this.preencherComOTrabalho(this.trabalhoId);
    } catch (erro) {
      this.relatar(api.mensagemDeErro(erro), true);
    }
  }

  /**
   * Preenche a tela com o trabalho que já existe.
   *
   * Vem depois das listas de proprietário e categoria porque escolher um valor
   * numa lista que ainda não tem opções não escolhe nada — e o trabalho ficaria
   * com o dono errado ao ser gravado.
   */
  private async preencherComOTrabalho(id: string): Promise<void> {
    const detalhe = await api.detalheDoTrabalho(this.sessaoId, id);
    this.campo("nome").value = detalhe.nome;
    this.campo<HTMLTextAreaElement>("descricao").value = detalhe.descricao;
    (this.campo("habilitado") as HTMLInputElement).checked = detalhe.habilitado;
    this.escolher("proprietario", detalhe.proprietario);
    this.escolher("categoria", detalhe.categoria);

    this.etapas = detalhe.etapas;
    this.agendas = detalhe.agendas;
    this.etapaSelecionada = this.etapas.length > 0 ? 0 : -1;
    this.agendaSelecionada = this.agendas.length > 0 ? 0 : -1;
    this.redesenharEtapas();
    this.redesenharAgendas();

    this.elemento("criado-em").textContent = formatarDataHora(detalhe.criadoEm) || TRACO;
    this.elemento("modificado-em").textContent = formatarDataHora(detalhe.modificadoEm) || TRACO;
    this.elemento("ultima-execucao").textContent =
      formatarDataHora(detalhe.ultimaExecucao) || t("nunca executado");
    this.elemento("nota").hidden = true;
    // O nome oficial é o do servidor: o da árvore pode estar velho.
    this.escreverTitulo(detalhe.nome);
  }

  /**
   * Escolhe um valor na lista, acrescentando-o se ele não estiver lá.
   *
   * Acontece de verdade: o dono de um trabalho pode ser um login que a conta
   * atual não enxerga, e uma lista que não o tem gravaria o trabalho no nome de
   * outra pessoa sem ninguém perceber.
   */
  private escolher(campo: string, valor: string): void {
    if (!valor) return;
    const lista = this.campo<HTMLSelectElement>(campo);
    if (![...lista.options].some((opcao) => opcao.value === valor)) {
      lista.append(new Option(valor, valor));
    }
    lista.value = valor;
  }

  private campo<T extends HTMLElement = HTMLInputElement>(nome: string): T {
    const elemento = this.caixa.querySelector<T>(`[data-campo="${nome}"]`);
    if (!elemento) throw new Error(`campo ausente na tela de trabalho: ${nome}`);
    return elemento;
  }

  private elemento<T extends HTMLElement = HTMLElement>(nome: string): T {
    const achado = this.caixa.querySelector<T>(`[data-elemento="${nome}"]`);
    if (!achado) throw new Error(`elemento ausente na tela de trabalho: ${nome}`);
    return achado;
  }

  private botao(acao: string): HTMLButtonElement {
    const achado = this.caixa.querySelector<HTMLButtonElement>(`[data-acao="${acao}"]`);
    if (!achado) throw new Error(`botão ausente na tela de trabalho: ${acao}`);
    return achado;
  }

  private ligarEventos(): void {
    for (const aba of this.caixa.querySelectorAll<HTMLButtonElement>("[data-pagina]")) {
      aba.addEventListener("click", () => this.mostrarPagina(aba.dataset.pagina ?? "geral"));
    }
    for (const botao of this.caixa.querySelectorAll<HTMLButtonElement>('[data-acao="cancelar"]')) {
      botao.addEventListener("click", () => this.fechar(false));
    }
    this.botao("confirmar").addEventListener("click", () => void this.gravar());

    this.botao("nova-etapa").addEventListener("click", () => void this.novaEtapa());
    this.botao("editar-etapa").addEventListener("click", () => void this.editarEtapa());
    this.botao("excluir-etapa").addEventListener("click", () => this.excluirEtapa());
    this.botao("subir-etapa").addEventListener("click", () => this.moverEtapa(-1));
    this.botao("descer-etapa").addEventListener("click", () => this.moverEtapa(1));

    this.botao("nova-agenda").addEventListener("click", () => void this.novaAgenda());
    this.botao("editar-agenda").addEventListener("click", () => void this.editarAgenda());
    this.botao("excluir-agenda").addEventListener("click", () => this.excluirAgenda());
  }

  /**
   * Por onde a tela abre.
   *
   * Nas **propriedades**, pelas etapas: quem abre um trabalho que já existe vai
   * ver o que ele faz, e o Geral só tem nome, dono e categoria — que raramente
   * mudam. Num **cadastro**, pelo Geral: sem nome não há o que gravar.
   */
  private paginaInicial(): string {
    return this.trabalhoId ? "etapas" : "geral";
  }

  /**
   * O título, com o nome do trabalho entre aspas.
   *
   * Por `textContent`, e não pelo molde: o nome vem do servidor, e um `<` no
   * meio dele viraria marcação. Num cadastro não há nome, e o título é o de
   * sempre.
   */
  private escreverTitulo(nome: string): void {
    if (!this.trabalhoId || !nome) return;
    const titulo = t("propriedades do job", { nome });
    this.elemento("titulo").textContent = titulo;
    this.caixa.setAttribute("aria-label", titulo);
  }

  private mostrarPagina(pagina: string): void {
    for (const aba of this.caixa.querySelectorAll<HTMLButtonElement>("[data-pagina]")) {
      aba.setAttribute("aria-selected", String(aba.dataset.pagina === pagina));
    }
    for (const painel of this.caixa.querySelectorAll<HTMLElement>("[data-painel]")) {
      painel.hidden = painel.dataset.painel !== pagina;
    }
  }

  // --- Etapas --------------------------------------------------------------

  private async novaEtapa(): Promise<void> {
    const etapa = await abrirDialogoEtapa(
      etapaNova(true, this.bancos[0] ?? null),
      this.bancos,
      this.sessaoId,
    );
    if (!etapa) return;
    // A etapa que era a última deixa de ser: quem sai informando êxito no meio
    // do trabalho interrompe tudo o que vem depois sem erro nenhum registrado.
    const anterior = this.etapas.at(-1);
    if (anterior && anterior.aoTerExito === 1) anterior.aoTerExito = 3;
    this.etapas.push(etapa);
    this.etapaSelecionada = this.etapas.length - 1;
    this.redesenharEtapas();
  }

  private async editarEtapa(): Promise<void> {
    const atual = this.etapas[this.etapaSelecionada];
    if (!atual) return;
    const etapa = await abrirDialogoEtapa(atual, this.bancos, this.sessaoId);
    if (!etapa) return;
    this.etapas[this.etapaSelecionada] = etapa;
    this.redesenharEtapas();
  }

  private excluirEtapa(): void {
    if (this.etapaSelecionada < 0) return;
    this.etapas.splice(this.etapaSelecionada, 1);
    this.etapaSelecionada = Math.min(this.etapaSelecionada, this.etapas.length - 1);
    this.redesenharEtapas();
  }

  private moverEtapa(passo: number): void {
    const de = this.etapaSelecionada;
    const para = de + passo;
    if (de < 0 || para < 0 || para >= this.etapas.length) return;
    [this.etapas[de], this.etapas[para]] = [this.etapas[para], this.etapas[de]];
    this.etapaSelecionada = para;
    this.redesenharEtapas();
  }

  private redesenharEtapas(): void {
    const corpo = this.elemento("etapas");
    corpo.replaceChildren();
    for (const [ordem, etapa] of this.etapas.entries()) {
      corpo.append(
        this.linha(ordem === this.etapaSelecionada, [String(ordem + 1), etapa.nome, etapa.tipo, rotuloDaAcao(etapa.aoTerExito), rotuloDaAcao(etapa.aoFalhar)], () => {
          this.etapaSelecionada = ordem;
          this.redesenharEtapas();
        }, () => void this.editarEtapa()),
      );
    }
    if (this.etapas.length === 0) corpo.append(this.linhaVazia(5, t("nenhuma etapa")));

    const nada = this.etapaSelecionada < 0;
    this.botao("editar-etapa").disabled = nada;
    this.botao("excluir-etapa").disabled = nada;
    this.botao("subir-etapa").disabled = nada || this.etapaSelecionada === 0;
    this.botao("descer-etapa").disabled = nada || this.etapaSelecionada === this.etapas.length - 1;
  }

  // --- Agendas -------------------------------------------------------------

  private async novaAgenda(): Promise<void> {
    const agenda = await abrirDialogoAgenda(agendaNova());
    if (!agenda) return;
    this.agendas.push(agenda);
    this.agendaSelecionada = this.agendas.length - 1;
    this.redesenharAgendas();
  }

  private async editarAgenda(): Promise<void> {
    const atual = this.agendas[this.agendaSelecionada];
    if (!atual) return;
    const agenda = await abrirDialogoAgenda(atual);
    if (!agenda) return;
    this.agendas[this.agendaSelecionada] = agenda;
    this.redesenharAgendas();
  }

  private excluirAgenda(): void {
    if (this.agendaSelecionada < 0) return;
    this.agendas.splice(this.agendaSelecionada, 1);
    this.agendaSelecionada = Math.min(this.agendaSelecionada, this.agendas.length - 1);
    this.redesenharAgendas();
  }

  private redesenharAgendas(): void {
    const corpo = this.elemento("agendas");
    corpo.replaceChildren();
    for (const [ordem, agenda] of this.agendas.entries()) {
      corpo.append(
        this.linha(
          ordem === this.agendaSelecionada,
          [agenda.nome, agenda.habilitada ? t("sim") : t("nao"), resumoDaAgenda(agenda)],
          () => {
            this.agendaSelecionada = ordem;
            this.redesenharAgendas();
          },
          () => void this.editarAgenda(),
        ),
      );
    }
    if (this.agendas.length === 0) corpo.append(this.linhaVazia(3, t("nenhuma agenda")));

    const nada = this.agendaSelecionada < 0;
    this.botao("editar-agenda").disabled = nada;
    this.botao("excluir-agenda").disabled = nada;
  }

  // --- Grades --------------------------------------------------------------

  private linha(
    selecionada: boolean,
    celulas: string[],
    aoSelecionar: () => void,
    aoAbrir: () => void,
  ): HTMLTableRowElement {
    const linha = document.createElement("tr");
    if (selecionada) linha.className = "trabalho__linha--selecionada";
    for (const texto of celulas) {
      const celula = document.createElement("td");
      celula.textContent = texto;
      celula.title = texto;
      linha.append(celula);
    }
    linha.addEventListener("click", aoSelecionar);
    linha.addEventListener("dblclick", aoAbrir);
    return linha;
  }

  private linhaVazia(colunas: number, texto: string): HTMLTableRowElement {
    const linha = document.createElement("tr");
    const celula = document.createElement("td");
    celula.colSpan = colunas;
    celula.className = "trabalho__vazio";
    celula.textContent = texto;
    linha.append(celula);
    return linha;
  }

  // --- Gravação ------------------------------------------------------------

  private montar(): NovoTrabalho {
    return {
      nome: this.campo("nome").value.trim(),
      proprietario: this.campo<HTMLSelectElement>("proprietario").value || null,
      categoria: this.campo<HTMLSelectElement>("categoria").value || null,
      descricao: this.campo<HTMLTextAreaElement>("descricao").value.trim(),
      habilitado: (this.campo("habilitado") as HTMLInputElement).checked,
      etapas: this.etapas,
      agendas: this.agendas,
    };
  }

  private async gravar(): Promise<void> {
    if (this.gravando) return;
    const trabalho = this.montar();
    if (!trabalho.nome) {
      this.mostrarPagina("geral");
      this.campo("nome").focus();
      this.relatar(t("informe o nome do trabalho"), true);
      return;
    }
    // Um trabalho sem etapa é aceito pelo servidor e nunca faz nada. Barrar aqui
    // é mais honesto do que criar um trabalho que só decepciona na primeira
    // execução.
    if (trabalho.etapas.length === 0) {
      this.mostrarPagina("etapas");
      this.relatar(t("informe ao menos uma etapa"), true);
      return;
    }

    this.gravando = true;
    this.botao("confirmar").disabled = true;
    this.relatar(t(this.trabalhoId ? "gravando o trabalho" : "criando o trabalho"));
    try {
      if (this.trabalhoId) {
        await api.atualizarTrabalho(this.sessaoId, this.trabalhoId, trabalho);
      } else {
        await api.criarTrabalho(this.sessaoId, trabalho);
      }
      this.fechar(true);
    } catch (erro) {
      this.relatar(api.mensagemDeErro(erro), true);
      this.gravando = false;
      this.botao("confirmar").disabled = false;
    }
  }

  private relatar(mensagem: string, erro = false): void {
    const status = this.elemento("status");
    status.textContent = mensagem;
    status.className = erro ? "status status--erro" : "status";
  }

  private fechar(criado: boolean): void {
    this.pararEsc();
    this.pararJanela();
    this.fundo.remove();
    this.resolver(criado);
  }
}

function rotuloDaAcao(valor: number): string {
  const acao = ACOES.find((opcao) => opcao.valor === valor);
  return acao ? t(acao.chave) : String(valor);
}
