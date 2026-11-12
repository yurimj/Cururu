import "./painel-mongo.css";

import { autocompletion, startCompletion, type CompletionContext } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { json } from "@codemirror/lang-json";
import { defaultHighlightStyle, syntaxHighlighting, syntaxTree } from "@codemirror/language";
import { EditorView, keymap } from "@codemirror/view";
import * as api from "../api";
import { t } from "../idioma";
import type { ContextoDeConexao } from "./arvore-objetos";

const FILTRO_INICIAL = "{}";

export class PainelMongo {
  readonly elemento = document.createElement("section");
  private readonly banco = document.createElement("select");
  private readonly colecao = document.createElement("select");
  private readonly limite = document.createElement("select");
  private readonly consultar = document.createElement("button");
  private readonly estado = document.createElement("div");
  private readonly resultados = document.createElement("div");
  private readonly editor: EditorView;
  private campos: string[] = [];
  private geracao = 0;
  private consultando = false;
  private encerrado = false;

  constructor(private readonly contexto: ContextoDeConexao) {
    this.elemento.className = "painel-mongo";
    const barra = document.createElement("div");
    barra.className = "painel-mongo__barra";
    this.preencher(this.banco, [], t("escolher banco mongodb"));
    this.preencher(this.colecao, [], t("escolher colecao mongodb"));
    for (const limite of [25, 50, 100]) this.limite.add(new Option(String(limite), String(limite)));
    this.limite.value = "50";
    this.consultar.textContent = t("consultar mongodb");
    this.consultar.title = `${t("consultar mongodb")} (Ctrl+Enter)`;
    const modo = document.createElement("span");
    modo.textContent = t("somente leitura mongodb");
    modo.className = "painel-mongo__modo";
    barra.append(
      rotular(t("banco de dados"), this.banco),
      rotular(t("colecao mongodb"), this.colecao),
      rotular(t("limite mongodb"), this.limite),
      this.consultar,
      modo,
    );

    const legenda = document.createElement("p");
    legenda.className = "painel-mongo__legenda";
    legenda.textContent = t("ajuda filtro mongodb");
    const montagem = document.createElement("div");
    montagem.className = "painel-mongo__editor";
    this.editor = new EditorView({
      parent: montagem,
      doc: FILTRO_INICIAL,
      selection: { anchor: 1 },
      extensions: [
        json(),
        history(),
        syntaxHighlighting(defaultHighlightStyle),
        EditorView.contentAttributes.of({ "aria-label": t("filtro mongodb") }),
        autocompletion({ override: [(contexto) => this.completar(contexto)] }),
        keymap.of([
          { key: "Ctrl-Enter", run: () => { void this.executar(); return true; } },
          ...defaultKeymap,
          ...historyKeymap,
        ]),
      ],
    });
    this.estado.className = "painel-mongo__estado";
    this.estado.setAttribute("role", "status");
    this.resultados.className = "painel-mongo__resultados";
    this.elemento.append(barra, legenda, montagem, this.estado, this.resultados);

    this.banco.addEventListener("change", () => void this.carregarColecoes());
    this.colecao.addEventListener("change", () => this.selecionarColecao());
    this.consultar.addEventListener("click", () => void this.executar());
    this.atualizarBotoes();
    void this.carregarBancos();
  }

  focar(): void {
    if (!this.banco.disabled && !this.banco.value) this.banco.focus();
    else this.editor.focus();
  }

  encerrar(): void {
    this.encerrado = true;
    this.editor.destroy();
  }

  private async carregarBancos(): Promise<void> {
    this.status(t("carregando"));
    try {
      const bancos = await api.listarBancosMongo(this.contexto.sessaoId);
      if (this.encerrado) return;
      this.preencher(this.banco, bancos, t("escolher banco mongodb"), this.contexto.banco);
      this.status(bancos.length ? "" : t("nenhum banco acessivel"));
      if (this.banco.value) await this.carregarColecoes(this.contexto.colecao);
    } catch (erro) {
      this.status(api.mensagemDeErro(erro), true);
    }
  }

  private async carregarColecoes(inicial?: string): Promise<void> {
    this.preencher(this.colecao, [], t("escolher colecao mongodb"));
    const geracao = this.selecionarColecao();
    const banco = this.banco.value;
    if (!banco) return;
    this.status(t("carregando"));
    try {
      const colecoes = await api.listarColecoesMongo(this.contexto.sessaoId, banco);
      if (!this.vigente(geracao)) return;
      this.preencher(this.colecao, colecoes, t("escolher colecao mongodb"), inicial);
      this.selecionarColecao();
      if (!colecoes.length) this.status(t("nenhuma colecao"));
      if (inicial && this.colecao.value === inicial) await this.executar();
    } catch (erro) {
      if (this.vigente(geracao)) this.status(api.mensagemDeErro(erro), true);
    }
  }

  private selecionarColecao(): number {
    const geracao = ++this.geracao;
    this.campos = [];
    this.consultando = false;
    this.resultados.replaceChildren();
    this.status("");
    this.atualizarBotoes();
    if (this.colecao.value) {
      void api.camposMongo(this.contexto.sessaoId, this.banco.value, this.colecao.value)
        .then((campos) => {
          if (!this.vigente(geracao)) return;
          this.campos = campos;
          if (campos.length && this.editor.hasFocus && this.editor.state.doc.toString() !== FILTRO_INICIAL) {
            startCompletion(this.editor);
          }
        })
        .catch(() => {}); // A amostra pode falhar e a consulta filtrada ainda funcionar.
    }
    return geracao;
  }

  private async executar(): Promise<void> {
    if (!this.colecao.value || this.consultando) return;
    const geracao = this.geracao;
    this.consultando = true;
    this.atualizarBotoes();
    this.resultados.replaceChildren();
    this.status(t("carregando"));
    try {
      const documentos = await api.consultarMongo(
        this.contexto.sessaoId, this.banco.value, this.colecao.value,
        this.editor.state.doc.toString(), Number(this.limite.value),
      );
      if (!this.vigente(geracao)) return;
      this.resultados.replaceChildren(...documentos.map((documento, indice) =>
        ramo(t("documento mongodb n", { n: indice + 1 }), documento),
      ));
      this.status(documentos.length
        ? t("documentos mongodb", { quantidade: documentos.length })
        : t("nenhum documento mongodb"));
    } catch (erro) {
      if (this.vigente(geracao)) this.status(api.mensagemDeErro(erro), true);
    } finally {
      if (this.vigente(geracao)) {
        this.consultando = false;
        this.atualizarBotoes();
      }
    }
  }

  private completar(contexto: CompletionContext) {
    if (!this.campos.length) return null;
    const parcial = contexto.matchBefore(/"[^"\\\r\n,:{}\[\]]*$/);
    const inicio = parcial?.from ?? contexto.pos;
    const antes = contexto.state.sliceDoc(0, inicio).trimEnd();
    if ((!parcial && !contexto.explicit) || !/[{,]$/.test(antes)) return null;
    let no = syntaxTree(contexto.state).resolveInner(antes.length - 1, 1);
    while (no.parent && !["Object", "Array"].includes(no.name)) no = no.parent;
    if (no.name !== "Object") return null;
    const depois = contexto.state.sliceDoc(contexto.pos);
    const fechamento = parcial ? /^[^"\\\r\n,:{}\[\]]*"/.exec(depois)?.[0] ?? "" : "";
    const temSeparador = /^\s*:/.test(depois.slice(fechamento.length));
    return {
      from: inicio,
      to: contexto.pos + fechamento.length,
      options: this.campos.map((campo) => ({
        label: JSON.stringify(campo),
        apply: JSON.stringify(campo) + (temSeparador ? "" : ": "),
        type: "property",
      })),
    };
  }

  private vigente(geracao: number): boolean {
    return !this.encerrado && this.geracao === geracao;
  }

  private preencher(select: HTMLSelectElement, nomes: string[], dica: string, selecionado?: string): void {
    select.replaceChildren(new Option(dica, ""), ...nomes.map((nome) => new Option(nome, nome)));
    select.disabled = nomes.length === 0;
    if (selecionado && nomes.includes(selecionado)) select.value = selecionado;
  }

  private atualizarBotoes(): void {
    this.consultar.disabled = !this.colecao.value || this.consultando;
  }

  private status(texto: string, erro = false): void {
    this.estado.textContent = texto;
    this.estado.classList.toggle("painel-mongo__estado--erro", erro);
  }
}

function rotular(texto: string, controle: HTMLElement): HTMLLabelElement {
  const rotulo = document.createElement("label");
  rotulo.append(texto, controle);
  return rotulo;
}

// Extended JSON mantém tipos do BSON, inclusive inteiros de 64 bits. A árvore
// mostra esses valores na forma canônica e abre objetos e arrays sob demanda.
const ESCALARES_BSON = new Set([
  "$oid", "$date", "$numberInt", "$numberLong", "$numberDouble", "$numberDecimal",
  "$binary", "$regularExpression", "$timestamp", "$minKey", "$maxKey", "$symbol",
  "$undefined", "$dbPointer",
]);

function ramo(nome: string, valor: api.ValorJson): HTMLElement {
  const composto = valor !== null && typeof valor === "object";
  if (composto) {
    const chaves = Object.keys(valor);
    if (Array.isArray(valor) || chaves.length !== 1 || !ESCALARES_BSON.has(chaves[0])) {
      const detalhes = document.createElement("details");
      const resumo = document.createElement("summary");
      resumo.textContent = `${nome}  ${Array.isArray(valor) ? `[${valor.length}]` : "{…}"}`;
      detalhes.append(resumo);
      detalhes.addEventListener("toggle", () => {
        if (detalhes.open && detalhes.childElementCount === 1) {
          const filhos = document.createElement("div");
          filhos.className = "painel-mongo__campos";
          for (const [campo, dado] of Object.entries(valor)) filhos.append(ramo(campo, dado));
          detalhes.append(filhos);
        }
      });
      return detalhes;
    }
  }
  const linha = document.createElement("div");
  const dado = document.createElement("code");
  dado.textContent = JSON.stringify(valor);
  linha.append(`${nome}: `, dado);
  return linha;
}
