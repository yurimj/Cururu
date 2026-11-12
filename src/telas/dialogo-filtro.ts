/**
 * Diálogo de filtro de objetos (requisito 4).
 *
 * Inclusões e exclusões ficam em grupos separados: basta corresponder a uma
 * inclusão, enquanto qualquer correspondência numa exclusão remove o objeto.
 */

import "./dialogo-filtro.css";

import type { CriterioFiltro, Filtro, OperadorFiltro } from "../api";
import { t } from "../idioma";
import { fecharComEsc } from "../componentes/janela";

type Grupo = "inclusao" | "exclusao";

/**
 * Os operadores, resolvidos na hora de montar a lista.
 *
 * São função, e não constante: `LIKE`, `=` e `IN` são SQL e não se traduzem,
 * mas "começa com" e "termina com" são frase. Uma constante de módulo nasceria
 * no idioma da primeira carga e continuaria nele depois de trocar o idioma.
 */
const operadoresDeInclusao = (): ReadonlyArray<{ valor: OperadorFiltro; rotulo: string }> => [
  { valor: "like", rotulo: "LIKE" },
  { valor: "igual", rotulo: "=" },
  { valor: "em", rotulo: "IN" },
  { valor: "comeca", rotulo: t("operador comeca com") },
  { valor: "termina", rotulo: t("operador termina com") },
];

const operadoresDeExclusao = (): ReadonlyArray<{ valor: OperadorFiltro; rotulo: string }> => [
  { valor: "naoLike", rotulo: "LIKE" },
  { valor: "diferente", rotulo: "=" },
  { valor: "naoEm", rotulo: "IN" },
  { valor: "naoComeca", rotulo: t("operador comeca com") },
  { valor: "naoTermina", rotulo: t("operador termina com") },
];

const OPERADORES_NEGATIVOS = new Set<OperadorFiltro>([
  "naoLike",
  "diferente",
  "naoEm",
  "naoComeca",
  "naoTermina",
]);

/** Identidade e opções específicas da categoria filtrada. */
export interface OpcoesDoFiltro {
  /** Nome singular mostrado em cada linha: Tabela, View, Pacote… */
  alvo: string;
  /** Nome usado na prévia lógica: Table, View, Package… */
  alvoSql: string;
  simbolo: string;
  excluirGerados: boolean;
}

/** Abre o diálogo e devolve o filtro montado, ou `null` ao cancelar. */
export function abrirDialogoFiltro(
  titulo: string,
  atual: Filtro | null,
  opcoes: OpcoesDoFiltro,
): Promise<Filtro | null> {
  return new Promise((resolver) => {
    new DialogoFiltro(titulo, atual, opcoes, resolver).abrir();
  });
}

class DialogoFiltro {
  private readonly fundo = document.createElement("div");
  private readonly caixa = document.createElement("div");
  private readonly linhasInclusao = document.createElement("div");
  private readonly linhasExclusao = document.createElement("div");
  private readonly previa = document.createElement("code");
  private readonly excluirGerados = entrada("checkbox");
  private pararEsc: () => void = () => {};

  constructor(
    private readonly titulo: string,
    private readonly atual: Filtro | null,
    private readonly opcoes: OpcoesDoFiltro,
    private readonly resolver: (filtro: Filtro | null) => void,
  ) {}

  abrir(): void {
    this.fundo.className = "dialogo-fundo";
    this.caixa.className = "dialogo dialogo-filtro";
    this.caixa.setAttribute("role", "dialog");
    this.caixa.setAttribute("aria-modal", "true");
    this.caixa.setAttribute("aria-label", t("filtrar titulo", { alvo: this.titulo }));

    this.caixa.append(
      this.montarBarra(),
      this.montarCorpo(),
      this.montarOpcoes(),
      this.montarRodape(),
    );
    this.fundo.append(this.caixa);
    document.body.append(this.fundo);

    this.preencher();
    this.pararEsc = fecharComEsc(this.fundo, () => this.fechar(null));
    this.linhasInclusao.querySelector<HTMLInputElement>('input[type="text"]')?.focus();
  }

  private montarBarra(): HTMLElement {
    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.textContent = t("filtrar titulo", { alvo: this.titulo });
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar"));
    fechar.addEventListener("click", () => this.fechar(null));
    barra.append(titulo, fechar);
    return barra;
  }

  private montarCorpo(): HTMLElement {
    const corpo = document.createElement("div");
    corpo.className = "filtro__corpo";
    corpo.append(
      this.montarGrupo(
        "inclusao",
        t("incluir"),
        t("filtro ajuda inclusao", { alvo: this.opcoes.alvo }),
        t("criterio"),
      ),
      this.montarJuncao(),
      this.montarGrupo(
        "exclusao",
        t("excluir grupo"),
        t("filtro ajuda exclusao", { alvo: this.opcoes.alvo }),
        t("exclusao"),
      ),
      this.montarPrevia(),
    );
    return corpo;
  }

  private montarGrupo(
    grupo: Grupo,
    titulo: string,
    descricao: string,
    rotuloDoBotao: string,
  ): HTMLElement {
    const secao = document.createElement("section");
    secao.className = "filtro__grupo";

    const cabecalho = document.createElement("div");
    cabecalho.className = "filtro__grupo-cabecalho";
    const textos = document.createElement("div");
    const h2 = document.createElement("h2");
    h2.textContent = titulo;
    const explicacao = document.createElement("p");
    explicacao.textContent = descricao;
    textos.append(h2, explicacao);

    const acrescentar = document.createElement("button");
    acrescentar.className = "filtro__acrescentar";
    acrescentar.append(document.createTextNode("＋ "), document.createTextNode(rotuloDoBotao));
    acrescentar.addEventListener("click", () => this.acrescentarLinha(grupo));
    cabecalho.append(textos, acrescentar);

    const colunas = document.createElement("div");
    colunas.className = "filtro__colunas";
    for (const texto of [t("alvo"), t("operador"), t("valor"), ""]) {
      const coluna = document.createElement("span");
      coluna.textContent = texto;
      colunas.append(coluna);
    }

    const linhas = grupo === "inclusao" ? this.linhasInclusao : this.linhasExclusao;
    linhas.className = "filtro__linhas";
    linhas.dataset.grupo = grupo;
    secao.append(cabecalho, colunas, linhas);
    return secao;
  }

  private montarJuncao(): HTMLElement {
    const juncao = document.createElement("div");
    juncao.className = "filtro__juncao";
    const texto = document.createElement("span");
    texto.textContent = "AND NOT";
    juncao.append(texto);
    return juncao;
  }

  private montarPrevia(): HTMLElement {
    const bloco = document.createElement("div");
    bloco.className = "filtro__previa";
    const rotulo = document.createElement("span");
    rotulo.textContent = t("logica resultante");
    bloco.append(rotulo, this.previa);
    return bloco;
  }

  /** Só aparece onde USER_OBJECTS possui a coluna GENERATED. */
  private montarOpcoes(): HTMLElement {
    const opcoes = document.createElement("div");
    opcoes.className = "filtro__opcoes";
    if (this.opcoes.excluirGerados) {
      opcoes.append(caixaDeMarcacao(this.excluirGerados, t("excluir objetos gerados")));
    }
    return opcoes;
  }

  private montarRodape(): HTMLElement {
    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape filtro__rodape";

    const cancelar = document.createElement("button");
    cancelar.textContent = t("cancelar");
    cancelar.addEventListener("click", () => this.fechar(null));

    const aplicar = document.createElement("button");
    aplicar.className = "filtro__aplicar";
    aplicar.textContent = t("aplicar filtro botao");
    aplicar.addEventListener("click", () => this.fechar(this.montarFiltro()));

    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    botoes.append(cancelar, aplicar);
    rodape.append(botoes);
    return rodape;
  }

  private novaLinha(grupo: Grupo, criterio?: CriterioFiltro): HTMLElement {
    const linha = document.createElement("div");
    linha.className = "filtro__linha";
    linha.dataset.distingueMaiusculas = String(criterio?.distingueMaiusculas ?? false);

    const alvo = document.createElement("span");
    alvo.className = "filtro__alvo";
    alvo.title = t("alvo fixo", { alvo: this.opcoes.alvo });
    const simbolo = document.createElement("span");
    simbolo.className = "filtro__alvo-simbolo";
    simbolo.textContent = this.opcoes.simbolo;
    const nome = document.createElement("span");
    nome.textContent = this.opcoes.alvo;
    alvo.append(simbolo, nome);

    const itens = grupo === "inclusao" ? operadoresDeInclusao() : operadoresDeExclusao();
    const operador = seletor(itens, criterio?.operador ?? itens[0].valor);
    operador.dataset.papel = "operador";
    operador.setAttribute("aria-label", t("operador de", { grupo }));

    const valor = document.createElement("input");
    valor.type = "text";
    valor.dataset.papel = "valor";
    valor.value = criterio?.valor ?? "";
    valor.placeholder = t("use porcento como curinga");
    valor.setAttribute("aria-label", t("valor do criterio de", { grupo }));

    const remover = document.createElement("button");
    remover.className = "filtro__remover";
    remover.title = t("remover criterio");
    remover.setAttribute("aria-label", t("remover criterio"));
    remover.append(iconeLixeira());
    remover.addEventListener("click", () => {
      linha.remove();
      this.atualizarPrevia();
    });

    operador.addEventListener("change", () => this.atualizarPrevia());
    valor.addEventListener("input", () => this.atualizarPrevia());
    linha.append(alvo, operador, valor, remover);
    return linha;
  }

  private preencher(): void {
    this.excluirGerados.checked = this.atual?.excluirGerados ?? false;
    const inclusoes = (this.atual?.criterios ?? []).filter(
      (criterio) => !OPERADORES_NEGATIVOS.has(criterio.operador),
    );
    const exclusoes = (this.atual?.criterios ?? []).filter((criterio) =>
      OPERADORES_NEGATIVOS.has(criterio.operador),
    );

    this.linhasInclusao.replaceChildren(
      ...inclusoes.map((criterio) => this.novaLinha("inclusao", criterio)),
    );
    this.linhasExclusao.replaceChildren(
      ...exclusoes.map((criterio) => this.novaLinha("exclusao", criterio)),
    );
    if (inclusoes.length === 0) this.linhasInclusao.append(this.novaLinha("inclusao"));
    if (exclusoes.length === 0) this.linhasExclusao.append(this.novaLinha("exclusao"));
    this.atualizarPrevia();
  }

  private acrescentarLinha(grupo: Grupo): void {
    const linha = this.novaLinha(grupo);
    const destino = grupo === "inclusao" ? this.linhasInclusao : this.linhasExclusao;
    destino.append(linha);
    linha.querySelector<HTMLInputElement>('[data-papel="valor"]')?.focus();
    this.atualizarPrevia();
  }

  /** Lê um grupo. Linhas cujo valor está em branco não viram critério. */
  private lerGrupo(grupo: Grupo): CriterioFiltro[] {
    const linhas = grupo === "inclusao" ? this.linhasInclusao : this.linhasExclusao;
    return Array.from(linhas.children)
      .map((linha) => ({
        campo: "nome" as const,
        operador: papel<HTMLSelectElement>(linha, "operador").value as OperadorFiltro,
        valor: papel<HTMLInputElement>(linha, "valor").value.trim(),
        distingueMaiusculas: (linha as HTMLElement).dataset.distingueMaiusculas === "true",
      }))
      .filter((criterio) => criterio.valor !== "");
  }

  private montarFiltro(): Filtro {
    return {
      criterios: [...this.lerGrupo("inclusao"), ...this.lerGrupo("exclusao")],
      // Campo legado mantido no formato salvo; a nova tela define a composição.
      corresponderTodos: false,
      excluirGerados: this.excluirGerados.checked,
      incluirSinonimos: false,
    };
  }

  private atualizarPrevia(): void {
    const inclusoes = this.expressoesDe(this.linhasInclusao, false);
    const exclusoes = this.expressoesDe(this.linhasExclusao, true);
    const grupos: string[] = [];
    if (inclusoes.length > 0) grupos.push(`(${inclusoes.join(" OR ")})`);
    if (exclusoes.length > 0) {
      const grupo = `(${exclusoes.join(" OR ")})`;
      grupos.push(inclusoes.length > 0 ? `AND NOT ${grupo}` : `NOT ${grupo}`);
    }
    this.previa.textContent = grupos.join(" ") || t("nenhum criterio");
  }

  private expressoesDe(linhas: HTMLElement, exclusao: boolean): string[] {
    return Array.from(linhas.children).flatMap((linha) => {
      const valor = papel<HTMLInputElement>(linha, "valor").value.trim();
      if (!valor) return [];
      const operador = papel<HTMLSelectElement>(linha, "operador");
      const itens = exclusao ? operadoresDeExclusao() : operadoresDeInclusao();
      const rotulo = itens.find((item) => item.valor === operador.value)?.rotulo ?? operador.value;
      return [`${this.opcoes.alvoSql} ${rotulo} ${valor}`];
    });
  }

  private fechar(filtro: Filtro | null): void {
    this.pararEsc();
    this.fundo.remove();
    this.resolver(filtro);
  }
}

function papel<T extends Element>(linha: Element, nome: string): T {
  const achado = linha.querySelector<T>(`[data-papel="${nome}"]`);
  if (!achado) throw new Error(`campo ${nome} não encontrado na linha do filtro`);
  return achado;
}

function seletor<T extends string>(
  itens: ReadonlyArray<{ valor: T; rotulo: string }>,
  selecionado: T,
): HTMLSelectElement {
  const campo = document.createElement("select");
  for (const item of itens) {
    const opcao = document.createElement("option");
    opcao.value = item.valor;
    opcao.textContent = item.rotulo;
    campo.append(opcao);
  }
  campo.value = itens.some((item) => item.valor === selecionado) ? selecionado : itens[0].valor;
  return campo;
}

function entrada(tipo: "checkbox"): HTMLInputElement {
  const campo = document.createElement("input");
  campo.type = tipo;
  return campo;
}

function caixaDeMarcacao(campo: HTMLInputElement, texto: string): HTMLElement {
  const rotulo = document.createElement("label");
  rotulo.className = "caixa-marcacao";
  rotulo.append(campo, document.createTextNode(texto));
  return rotulo;
}

function iconeLixeira(): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.8");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  const tampa = document.createElementNS("http://www.w3.org/2000/svg", "path");
  tampa.setAttribute("d", "M3 6h18M8 6V4h8v2");
  const corpo = document.createElementNS("http://www.w3.org/2000/svg", "path");
  corpo.setAttribute("d", "M19 6l-1 14H6L5 6m4 4v6m6-6v6");
  svg.append(tampa, corpo);
  return svg;
}
