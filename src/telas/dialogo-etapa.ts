/**
 * Etapa de um trabalho do SQL Server Agent (requisito 36).
 *
 * É a página **Geral** da tela de etapa do SSMS — nome, tipo, banco e comando —
 * mais as duas ações que o SSMS guarda em *Avançado* e mostra na grade de
 * etapas: o que fazer ao terminar bem e o que fazer ao falhar. Deixá-las aqui é
 * o que dispensa uma segunda página para dois campos.
 *
 * O comando é o **mesmo editor da janela de consulta**, e não uma caixa de
 * texto: quem escreve o T-SQL de uma etapa espera o comentário em verde, a
 * palavra reservada destacada e o Tab indentando o bloco, como no resto do
 * Cururu. Pela mesma razão a tela é uma janela de verdade (`tornarJanela`) —
 * um comando de cem linhas não cabe numa caixa de tamanho fixo.
 */

import "./dialogo-trabalho.css";

import * as api from "../api";
import type { EtapaDeTrabalho } from "../api";
import { fecharComEsc, tornarJanela } from "../componentes/janela";
import { t, type Chave } from "../idioma";
import { EditorSql, type FonteDeSugestoes } from "./editor-sql";

/** `@on_success_action` e `@on_fail_action` do `sp_add_jobstep`. */
export const ACOES: ReadonlyArray<{ valor: number; chave: Chave }> = [
  { valor: 1, chave: "sair informando exito" },
  { valor: 2, chave: "sair informando falha" },
  { valor: 3, chave: "ir para a proxima etapa" },
];

/** Os três subsistemas oferecidos, com o rótulo que o SSMS usa. */
const TIPOS: ReadonlyArray<{ valor: EtapaDeTrabalho["tipo"]; rotulo: () => string }> = [
  { valor: "TSQL", rotulo: () => "Transact-SQL (T-SQL)" },
  { valor: "CmdExec", rotulo: () => t("etapa sistema operacional") },
  { valor: "PowerShell", rotulo: () => "PowerShell" },
];

/** Uma etapa nova. `ultima` decide o padrão do "ao ter êxito". */
export function etapaNova(ultima: boolean, banco: string | null): EtapaDeTrabalho {
  return {
    nome: "",
    tipo: "TSQL",
    banco,
    comando: "",
    // A última etapa sai informando êxito; as do meio seguem para a seguinte.
    // É o mesmo padrão do SSMS, e é o que evita um trabalho que termina no
    // meio sem ninguém entender por quê.
    aoTerExito: ultima ? 1 : 3,
    aoFalhar: 2,
  };
}

/**
 * Abre a tela e devolve a etapa montada, ou `null` ao cancelar.
 *
 * `sessaoId` é da conexão, e serve só ao autocomplete do comando: é a mesma
 * sessão que a tela de trabalho já usa para listar bancos e logins.
 */
export function abrirDialogoEtapa(
  atual: EtapaDeTrabalho,
  bancos: string[],
  sessaoId: string,
): Promise<EtapaDeTrabalho | null> {
  return new Promise((resolver) => new DialogoEtapa(atual, bancos, sessaoId, resolver).abrir());
}

const molde = (): string => `
<div class="dialogo dialogo-etapa" role="dialog" aria-modal="true" aria-label="${t("etapa")}">
  <header class="dialogo__barra">
    <h1>${t("etapa")}</h1>
    <button class="dialogo__fechar" data-acao="cancelar" title="${t("fechar")}"
            aria-label="${t("fechar")}">✕</button>
  </header>

  <div class="trabalho__corpo">
    <div class="grade">
      <label for="etapa-nome">${t("nome da etapa")}</label>
      <input type="text" id="etapa-nome" class="largura-total" data-campo="nome" />

      <label for="etapa-tipo">${t("tipo")}</label>
      <select id="etapa-tipo" class="largura-total" data-campo="tipo"></select>

      <label for="etapa-banco" data-elemento="rotulo-banco">${t("banco de dados singular")}</label>
      <select id="etapa-banco" class="largura-total" data-campo="banco"></select>
    </div>

    <span class="trabalho__rotulo-bloco">${t("comando")}</span>
    <div class="etapa__comando" data-elemento="comando"></div>

    <div class="grade">
      <label for="etapa-exito">${t("ao ter exito")}</label>
      <select id="etapa-exito" class="largura-total" data-campo="aoTerExito"></select>

      <label for="etapa-falha">${t("ao falhar")}</label>
      <select id="etapa-falha" class="largura-total" data-campo="aoFalhar"></select>
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

class DialogoEtapa {
  private readonly fundo = document.createElement("div");
  private caixa!: HTMLElement;
  private editor!: EditorSql;
  private pararJanela: () => void = () => {};
  private pararEsc: () => void = () => {};

  constructor(
    private readonly atual: EtapaDeTrabalho,
    private readonly bancos: string[],
    private readonly sessaoId: string,
    private readonly resolver: (etapa: EtapaDeTrabalho | null) => void,
  ) {}

  abrir(): void {
    this.fundo.className = "dialogo-fundo";
    this.fundo.innerHTML = molde();
    this.caixa = this.fundo.firstElementChild as HTMLElement;
    document.body.append(this.fundo);

    this.montarListas();
    this.montarEditor();
    this.preencher();
    this.ligarEventos();
    this.aplicarVisibilidade();
    this.pararJanela = tornarJanela(this.caixa, {
      redimensionavel: true,
      maximizavel: true,
      larguraMinima: 480,
      alturaMinima: 420,
    });
    this.pararEsc = fecharComEsc(this.fundo, () => this.fechar(null));
    this.campo("nome").focus();
  }

  private campo<T extends HTMLElement = HTMLInputElement>(nome: string): T {
    const elemento = this.caixa.querySelector<T>(`[data-campo="${nome}"]`);
    if (!elemento) throw new Error(`campo ausente na tela de etapa: ${nome}`);
    return elemento;
  }

  private elemento<T extends HTMLElement = HTMLElement>(nome: string): T {
    const achado = this.caixa.querySelector<T>(`[data-elemento="${nome}"]`);
    if (!achado) throw new Error(`elemento ausente na tela de etapa: ${nome}`);
    return achado;
  }

  private montarListas(): void {
    const tipo = this.campo<HTMLSelectElement>("tipo");
    for (const { valor, rotulo } of TIPOS) tipo.append(new Option(rotulo(), valor));

    const banco = this.campo<HTMLSelectElement>("banco");
    for (const nome of this.bancos) banco.append(new Option(nome, nome));
    // Um banco que já estava na etapa mas não veio na lista — outro servidor,
    // permissão retirada — não pode sumir do formulário sem aviso.
    if (this.atual.banco && !this.bancos.includes(this.atual.banco)) {
      banco.append(new Option(this.atual.banco, this.atual.banco));
    }

    for (const nome of ["aoTerExito", "aoFalhar"]) {
      const lista = this.campo<HTMLSelectElement>(nome);
      for (const { valor, chave } of ACOES) lista.append(new Option(t(chave), String(valor)));
    }
  }

  /** O editor entra no lugar da caixa de texto, já com o comando da etapa. */
  private montarEditor(): void {
    this.editor = new EditorSql({
      texto: this.atual.comando,
      rotulo: t("comando"),
      sugestoes: this.fonteDeSugestoes(),
    });
    this.elemento("comando").append(this.editor.elemento);
  }

  /**
   * Autocomplete do comando, no banco escolhido na própria tela.
   *
   * Só no T-SQL: num CmdExec ou num PowerShell não há objeto de banco nenhum a
   * sugerir, e perguntar ao servidor por um banco que a etapa nem usa seria
   * consulta jogada fora.
   */
  private fonteDeSugestoes(): FonteDeSugestoes {
    const banco = (): string | null =>
      this.campo<HTMLSelectElement>("tipo").value === "TSQL"
        ? this.campo<HTMLSelectElement>("banco").value || null
        : null;
    return {
      objetos: async (prefixo, escopo) => {
        const alvo = escopo?.banco ?? banco();
        return alvo
          ? api.sugerirObjetos(this.sessaoId, prefixo, alvo, escopo?.esquema ?? null)
          : [];
      },
      colunas: async (objeto, bancoDoNome) => {
        const alvo = bancoDoNome ?? banco();
        return alvo ? api.listarColunas(this.sessaoId, objeto, alvo) : [];
      },
      // A etapa roda num banco só, mas o nome de três partes alcança os
      // outros — e para escrevê-lo é preciso ver a lista.
      bancos: async () => (banco() ? this.bancos : []),
    };
  }

  private preencher(): void {
    this.campo("nome").value = this.atual.nome;
    this.campo<HTMLSelectElement>("tipo").value = this.atual.tipo;
    this.campo<HTMLSelectElement>("banco").value = this.atual.banco ?? this.bancos[0] ?? "";
    this.campo<HTMLSelectElement>("aoTerExito").value = String(this.atual.aoTerExito);
    this.campo<HTMLSelectElement>("aoFalhar").value = String(this.atual.aoFalhar);
  }

  private ligarEventos(): void {
    for (const botao of this.caixa.querySelectorAll<HTMLButtonElement>('[data-acao="cancelar"]')) {
      botao.addEventListener("click", () => this.fechar(null));
    }
    this.caixa
      .querySelector<HTMLButtonElement>('[data-acao="confirmar"]')
      ?.addEventListener("click", () => this.confirmar());
    this.campo<HTMLSelectElement>("tipo").addEventListener("change", () =>
      this.aplicarVisibilidade(),
    );
  }

  /** O banco só existe no T-SQL: nos outros a procedure recusa o parâmetro. */
  private aplicarVisibilidade(): void {
    const tsql = this.campo<HTMLSelectElement>("tipo").value === "TSQL";
    this.campo<HTMLSelectElement>("banco").hidden = !tsql;
    this.elemento("rotulo-banco").hidden = !tsql;
  }

  private confirmar(): void {
    const tipo = this.campo<HTMLSelectElement>("tipo").value as EtapaDeTrabalho["tipo"];
    const etapa: EtapaDeTrabalho = {
      nome: this.campo("nome").value.trim(),
      tipo,
      banco: tipo === "TSQL" ? this.campo<HTMLSelectElement>("banco").value || null : null,
      comando: this.editor.texto,
      aoTerExito: Number(this.campo<HTMLSelectElement>("aoTerExito").value),
      aoFalhar: Number(this.campo<HTMLSelectElement>("aoFalhar").value),
    };

    const problema = !etapa.nome
      ? t("informe o nome da etapa")
      : !etapa.comando.trim()
        ? t("informe o comando da etapa")
        : null;
    if (problema) {
      const status = this.elemento("status");
      status.textContent = problema;
      status.className = "status status--erro";
      return;
    }
    this.fechar(etapa);
  }

  private fechar(etapa: EtapaDeTrabalho | null): void {
    this.pararEsc();
    this.editor.destruir();
    this.pararJanela();
    this.fundo.remove();
    this.resolver(etapa);
  }
}
