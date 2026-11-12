/**
 * Agenda de um trabalho do SQL Server Agent (requisito 36).
 *
 * É a tela do SSMS com as mesmas quatro perguntas: que tipo de agenda, com que
 * periodicidade, em que horário do dia e durante quanto tempo. A resposta sai
 * nos números do `sp_add_jobschedule` — que é o vocabulário do próprio SQL
 * Server, e não uma invenção daqui (ver `AgendaDeTrabalho` em `api.ts`).
 */

import "./dialogo-trabalho.css";

import type { AgendaDeTrabalho } from "../api";
import { fecharComEsc } from "../componentes/janela";
import { t, type Chave } from "../idioma";
import { formatarData, formatarHora } from "../formatacao";

/** `@freq_type`: o tipo da agenda. */
export const FREQUENCIA = {
  umaVez: 1,
  diaria: 4,
  semanal: 8,
  mensal: 16,
  mensalRelativa: 32,
  aoIniciarAgente: 64,
  cpuOciosa: 128,
} as const;

/** `@freq_subday_type`: o horário dentro do dia. */
const HORARIO = { umaVez: 1, minutos: 4, horas: 8 } as const;

/** `@active_end_date` de uma agenda sem data de término. */
const SEM_TERMINO = 99991231;

/** `@freq_relative_interval`, na frequência mensal relativa. */
const ORDENS: ReadonlyArray<{ valor: number; chave: Chave }> = [
  { valor: 1, chave: "primeiro" },
  { valor: 2, chave: "segundo" },
  { valor: 4, chave: "terceiro" },
  { valor: 8, chave: "quarto" },
  { valor: 16, chave: "ultimo" },
];

/** `@freq_interval`, na frequência mensal relativa. */
const UNIDADES_RELATIVAS: ReadonlyArray<{ valor: number; chave: Chave }> = [
  { valor: 8, chave: "dia" },
  { valor: 9, chave: "dia util" },
  { valor: 10, chave: "dia de fim de semana" },
  { valor: 1, chave: "domingo" },
  { valor: 2, chave: "segunda-feira" },
  { valor: 3, chave: "terca-feira" },
  { valor: 4, chave: "quarta-feira" },
  { valor: 5, chave: "quinta-feira" },
  { valor: 6, chave: "sexta-feira" },
  { valor: 7, chave: "sabado" },
];

/**
 * Bits de `@freq_interval` na frequência semanal, na ordem em que o SSMS mostra
 * as caixas: a semana de trabalho primeiro, o domingo por último.
 */
const DIAS_DA_SEMANA: ReadonlyArray<{ bit: number; chave: Chave }> = [
  { bit: 2, chave: "segunda-feira" },
  { bit: 4, chave: "terca-feira" },
  { bit: 8, chave: "quarta-feira" },
  { bit: 16, chave: "quinta-feira" },
  { bit: 32, chave: "sexta-feira" },
  { bit: 64, chave: "sabado" },
  { bit: 1, chave: "domingo" },
];

/** Uma agenda nova, com os mesmos padrões que o SSMS propõe. */
export function agendaNova(): AgendaDeTrabalho {
  return {
    nome: "",
    habilitada: true,
    freqTipo: FREQUENCIA.semanal,
    // Segunda-feira, que é o que o SSMS marca ao abrir a tela.
    freqIntervalo: 2,
    freqSubtipo: HORARIO.umaVez,
    freqSubintervalo: 0,
    freqIntervaloRelativo: 0,
    freqFator: 1,
    inicioData: numeroDeHoje(),
    fimData: SEM_TERMINO,
    inicioHora: 0,
    fimHora: 235959,
  };
}

/** Abre a tela e devolve a agenda montada, ou `null` ao cancelar. */
export function abrirDialogoAgenda(atual: AgendaDeTrabalho): Promise<AgendaDeTrabalho | null> {
  return new Promise((resolver) => new DialogoAgenda(atual, resolver).abrir());
}

// --- Resumo em texto -------------------------------------------------------

/**
 * A agenda escrita por extenso — a mesma frase que o SSMS mostra no campo
 * **Resumo** e na coluna Descrição da lista de agendas.
 *
 * Vale a pena existir porque os números do `sp_add_jobschedule` não se leem:
 * `freqTipo: 32, freqIntervaloRelativo: 16, freqIntervalo: 9` é "no último dia
 * útil do mês", e ninguém confere isso olhando a lista.
 */
export function resumoDaAgenda(agenda: AgendaDeTrabalho): string {
  if (agenda.freqTipo === FREQUENCIA.aoIniciarAgente) return t("agenda ao iniciar agente");
  if (agenda.freqTipo === FREQUENCIA.cpuOciosa) return t("agenda cpu ociosa");
  if (agenda.freqTipo === FREQUENCIA.umaVez) {
    return t("agenda uma vez", {
      data: formatarData(textoDeData(agenda.inicioData)),
      hora: formatarHora(textoDeHora(agenda.inicioHora)),
    });
  }
  return `${periodicidade(agenda)}${horarioDoDia(agenda)} ${vigencia(agenda)}`;
}

function periodicidade(agenda: AgendaDeTrabalho): string {
  const n = Math.max(1, agenda.freqFator);
  switch (agenda.freqTipo) {
    case FREQUENCIA.diaria:
      return agenda.freqIntervalo <= 1
        ? t("agenda todo dia")
        : t("agenda a cada dias", { n: agenda.freqIntervalo });
    case FREQUENCIA.semanal: {
      const dias = DIAS_DA_SEMANA.filter(({ bit }) => (agenda.freqIntervalo & bit) !== 0)
        .map(({ chave }) => t(chave))
        .join(", ");
      const escolhidos = dias || t("nenhum dia");
      return n === 1
        ? t("agenda toda semana", { dias: escolhidos })
        : t("agenda a cada semanas", { n, dias: escolhidos });
    }
    case FREQUENCIA.mensal:
      return n === 1
        ? t("agenda todo mes no dia", { dia: agenda.freqIntervalo })
        : t("agenda a cada meses no dia", { n, dia: agenda.freqIntervalo });
    case FREQUENCIA.mensalRelativa: {
      const ordem = t(
        ORDENS.find(({ valor }) => valor === agenda.freqIntervaloRelativo)?.chave ?? "primeiro",
      );
      const unidade = t(
        UNIDADES_RELATIVAS.find(({ valor }) => valor === agenda.freqIntervalo)?.chave ?? "dia",
      );
      return n === 1
        ? t("agenda todo mes relativa", { ordem, unidade })
        : t("agenda a cada meses relativa", { n, ordem, unidade });
    }
    default:
      return "";
  }
}

function horarioDoDia(agenda: AgendaDeTrabalho): string {
  const inicio = formatarHora(textoDeHora(agenda.inicioHora));
  if (agenda.freqSubtipo === HORARIO.umaVez) return t("agenda as horas", { hora: inicio });

  const fim = formatarHora(textoDeHora(agenda.fimHora));
  const chave = agenda.freqSubtipo === HORARIO.horas ? "agenda a cada horas" : "agenda a cada minutos";
  return t(chave, { n: agenda.freqSubintervalo, inicio, fim });
}

function vigencia(agenda: AgendaDeTrabalho): string {
  const inicio = formatarData(textoDeData(agenda.inicioData));
  return agenda.fimData >= SEM_TERMINO
    ? t("agenda vigencia", { inicio })
    : t("agenda vigencia ate", { inicio, fim: formatarData(textoDeData(agenda.fimData)) });
}

// --- Conversão dos números do Agent ----------------------------------------

/** `20260827` → `2026-08-27`, que é o formato do campo de data do HTML. */
function textoDeData(numero: number): string {
  if (numero <= 0) return textoDeData(numeroDeHoje());
  const texto = String(numero).padStart(8, "0");
  return `${texto.slice(0, 4)}-${texto.slice(4, 6)}-${texto.slice(6, 8)}`;
}

function numeroDeData(texto: string): number {
  const numero = Number(texto.replaceAll("-", ""));
  return Number.isFinite(numero) && numero > 0 ? numero : numeroDeHoje();
}

/** `31105` → `03:11:05`. */
function textoDeHora(numero: number): string {
  const texto = String(Math.max(0, numero)).padStart(6, "0");
  return `${texto.slice(0, 2)}:${texto.slice(2, 4)}:${texto.slice(4, 6)}`;
}

function numeroDeHora(texto: string): number {
  // O campo de hora do HTML omite os segundos quando eles são zero.
  const [hora = "0", minuto = "0", segundo = "0"] = texto.split(":");
  return Number(hora) * 10000 + Number(minuto) * 100 + Number(segundo);
}

function numeroDeHoje(): number {
  const hoje = new Date();
  return hoje.getFullYear() * 10000 + (hoje.getMonth() + 1) * 100 + hoje.getDate();
}

// --- A tela ----------------------------------------------------------------

const molde = (): string => `
<div class="dialogo dialogo-agenda" role="dialog" aria-modal="true" aria-label="${t("agenda")}">
  <header class="dialogo__barra">
    <h1>${t("agenda")}</h1>
    <button class="dialogo__fechar" data-acao="cancelar" title="${t("fechar")}"
            aria-label="${t("fechar")}">✕</button>
  </header>

  <div class="trabalho__corpo">
    <div class="grade">
      <label for="agenda-nome">${t("nome")}</label>
      <input type="text" id="agenda-nome" class="largura-total" data-campo="nome" />

      <label for="agenda-tipo">${t("tipo de agenda")}</label>
      <select id="agenda-tipo" data-campo="tipo">
        <option value="recorrente">${t("recorrente")}</option>
        <option value="umaVez">${t("uma vez")}</option>
        <option value="${FREQUENCIA.aoIniciarAgente}">${t("ao iniciar o agente")}</option>
        <option value="${FREQUENCIA.cpuOciosa}">${t("quando a cpu ficar ociosa")}</option>
      </select>
      <label class="caixa-marcacao">
        <input type="checkbox" data-campo="habilitada" />
        ${t("habilitada")}
      </label>
    </div>

    <fieldset data-bloco="umaVez" hidden>
      <legend>${t("uma vez")}</legend>
      <div class="linha-campos">
        <label for="agenda-data-unica">${t("data")}</label>
        <input type="date" id="agenda-data-unica" data-campo="dataUnica" />
        <label for="agenda-hora-unica">${t("hora")}</label>
        <input type="time" id="agenda-hora-unica" step="1" data-campo="horaUnica" />
      </div>
    </fieldset>

    <fieldset data-bloco="frequencia">
      <legend>${t("frequencia")}</legend>
      <div class="linha-campos">
        <label for="agenda-ocorre">${t("ocorre")}</label>
        <select id="agenda-ocorre" data-campo="ocorre">
          <option value="${FREQUENCIA.diaria}">${t("diariamente")}</option>
          <option value="${FREQUENCIA.semanal}">${t("semanalmente")}</option>
          <option value="${FREQUENCIA.mensal}">${t("mensalmente")}</option>
        </select>
      </div>

      <div class="linha-campos" data-bloco="diaria">
        <label for="agenda-dias">${t("recorre a cada")}</label>
        <input type="number" id="agenda-dias" min="1" max="99" data-campo="dias" />
        <span>${t("dias minusculo")}</span>
      </div>

      <div data-bloco="semanal">
        <div class="linha-campos">
          <label for="agenda-semanas">${t("recorre a cada")}</label>
          <input type="number" id="agenda-semanas" min="1" max="99" data-campo="semanas" />
          <span>${t("semanas minusculo")}</span>
        </div>
        <div class="agenda__dias" data-elemento="dias-da-semana" role="group"
             aria-label="${t("dias da semana")}"></div>
      </div>

      <div data-bloco="mensal">
        <div class="linha-campos">
          <span class="opcao-radio">
            <input type="radio" id="agenda-mensal-dia" name="agenda-mensal" value="dia" />
            <label for="agenda-mensal-dia">${t("dia")}</label>
          </span>
          <input type="number" min="1" max="31" data-campo="diaDoMes" />
          <span>${t("de cada")}</span>
          <input type="number" min="1" max="99" data-campo="mesesPorDia" />
          <span>${t("meses minusculo")}</span>
        </div>
        <div class="linha-campos">
          <span class="opcao-radio">
            <input type="radio" id="agenda-mensal-relativa" name="agenda-mensal" value="relativa" />
            <label for="agenda-mensal-relativa">${t("artigo do mes relativo")}</label>
          </span>
          <select data-campo="ordem"></select>
          <select data-campo="unidadeRelativa"></select>
          <span>${t("de cada")}</span>
          <input type="number" min="1" max="99" data-campo="mesesPorRelativa" />
          <span>${t("meses minusculo")}</span>
        </div>
      </div>
    </fieldset>

    <fieldset data-bloco="horario">
      <legend>${t("frequencia diaria")}</legend>
      <div class="linha-campos">
        <span class="opcao-radio">
          <input type="radio" id="agenda-horario-unico" name="agenda-horario" value="unico" />
          <label for="agenda-horario-unico">${t("ocorre uma vez as")}</label>
        </span>
        <input type="time" step="1" data-campo="horaDoDia" />
      </div>
      <div class="linha-campos">
        <span class="opcao-radio">
          <input type="radio" id="agenda-horario-intervalo" name="agenda-horario" value="intervalo" />
          <label for="agenda-horario-intervalo">${t("ocorre a cada")}</label>
        </span>
        <input type="number" min="1" max="99" data-campo="intervalo" />
        <select data-campo="unidadeDoIntervalo">
          <option value="${HORARIO.horas}">${t("horas minusculo")}</option>
          <option value="${HORARIO.minutos}">${t("minutos minusculo")}</option>
        </select>
        <label for="agenda-inicio-dia">${t("comecando as")}</label>
        <input type="time" id="agenda-inicio-dia" step="1" data-campo="inicioDoDia" />
        <label for="agenda-fim-dia">${t("terminando as")}</label>
        <input type="time" id="agenda-fim-dia" step="1" data-campo="fimDoDia" />
      </div>
    </fieldset>

    <fieldset data-bloco="duracao">
      <legend>${t("duracao")}</legend>
      <div class="linha-campos">
        <label for="agenda-inicio">${t("data de inicio")}</label>
        <input type="date" id="agenda-inicio" data-campo="inicio" />
        <span class="opcao-radio">
          <input type="radio" id="agenda-com-fim" name="agenda-fim" value="com" />
          <label for="agenda-com-fim">${t("data de termino")}</label>
        </span>
        <input type="date" data-campo="fim" />
      </div>
      <div class="linha-campos">
        <span class="opcao-radio">
          <input type="radio" id="agenda-sem-fim" name="agenda-fim" value="sem" />
          <label for="agenda-sem-fim">${t("sem data de termino")}</label>
        </span>
      </div>
    </fieldset>

    <div class="agenda__resumo">
      <span>${t("resumo")}</span>
      <p data-elemento="resumo"></p>
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

class DialogoAgenda {
  private readonly fundo = document.createElement("div");
  private caixa!: HTMLElement;
  private pararEsc: () => void = () => {};

  constructor(
    private readonly atual: AgendaDeTrabalho,
    private readonly resolver: (agenda: AgendaDeTrabalho | null) => void,
  ) {}

  abrir(): void {
    this.fundo.className = "dialogo-fundo";
    this.fundo.innerHTML = molde();
    this.caixa = this.fundo.firstElementChild as HTMLElement;
    document.body.append(this.fundo);

    this.montarListas();
    this.preencher();
    this.ligarEventos();
    this.aplicarVisibilidade();
    this.pararEsc = fecharComEsc(this.fundo, () => this.fechar(null));
    this.campo("nome").focus();
  }

  private campo<T extends HTMLElement = HTMLInputElement>(nome: string): T {
    const elemento = this.caixa.querySelector<T>(`[data-campo="${nome}"]`);
    if (!elemento) throw new Error(`campo ausente na tela de agenda: ${nome}`);
    return elemento;
  }

  private elemento<T extends HTMLElement = HTMLElement>(nome: string): T {
    const achado = this.caixa.querySelector<T>(`[data-elemento="${nome}"]`);
    if (!achado) throw new Error(`elemento ausente na tela de agenda: ${nome}`);
    return achado;
  }

  private bloco(nome: string): HTMLElement[] {
    return [...this.caixa.querySelectorAll<HTMLElement>(`[data-bloco="${nome}"]`)];
  }

  private montarListas(): void {
    const ordem = this.campo<HTMLSelectElement>("ordem");
    for (const { valor, chave } of ORDENS) {
      ordem.append(new Option(t(chave), String(valor)));
    }
    const unidade = this.campo<HTMLSelectElement>("unidadeRelativa");
    for (const { valor, chave } of UNIDADES_RELATIVAS) {
      unidade.append(new Option(t(chave), String(valor)));
    }

    const dias = this.elemento("dias-da-semana");
    for (const { bit, chave } of DIAS_DA_SEMANA) {
      const rotulo = document.createElement("label");
      rotulo.className = "caixa-marcacao";
      const marca = document.createElement("input");
      marca.type = "checkbox";
      marca.dataset.dia = String(bit);
      rotulo.append(marca, document.createTextNode(` ${t(chave)}`));
      dias.append(rotulo);
    }
  }

  private preencher(): void {
    const a = this.atual;
    this.campo("nome").value = a.nome;
    (this.campo("habilitada") as HTMLInputElement).checked = a.habilitada;

    const tipo = this.campo<HTMLSelectElement>("tipo");
    if (a.freqTipo === FREQUENCIA.aoIniciarAgente || a.freqTipo === FREQUENCIA.cpuOciosa) {
      tipo.value = String(a.freqTipo);
    } else {
      tipo.value = a.freqTipo === FREQUENCIA.umaVez ? "umaVez" : "recorrente";
    }

    this.campo("dataUnica").value = textoDeData(a.inicioData);
    this.campo("horaUnica").value = textoDeHora(a.inicioHora);

    // A lista "Ocorre" tem três opções, e a mensal relativa é uma variante da
    // mensal: as duas caem em "Mensalmente", separadas pelo par de rádios.
    const ocorre = this.campo<HTMLSelectElement>("ocorre");
    ocorre.value = String(
      a.freqTipo === FREQUENCIA.mensalRelativa ? FREQUENCIA.mensal : recorrenciaValida(a.freqTipo),
    );

    this.campo("dias").value = String(
      a.freqTipo === FREQUENCIA.diaria ? Math.max(1, a.freqIntervalo) : 1,
    );
    this.campo("semanas").value = String(Math.max(1, a.freqFator));
    for (const marca of this.marcasDosDias()) {
      marca.checked =
        a.freqTipo === FREQUENCIA.semanal && (a.freqIntervalo & Number(marca.dataset.dia)) !== 0;
    }

    const relativa = a.freqTipo === FREQUENCIA.mensalRelativa;
    this.campo("diaDoMes").value = String(a.freqTipo === FREQUENCIA.mensal ? a.freqIntervalo : 1);
    this.campo("mesesPorDia").value = String(Math.max(1, a.freqFator));
    this.campo("mesesPorRelativa").value = String(Math.max(1, a.freqFator));
    this.campo<HTMLSelectElement>("ordem").value = String(a.freqIntervaloRelativo || 1);
    this.campo<HTMLSelectElement>("unidadeRelativa").value = String(relativa ? a.freqIntervalo : 8);
    this.radio("agenda-mensal", relativa ? "relativa" : "dia");

    const intervalo = a.freqSubtipo !== HORARIO.umaVez;
    this.radio("agenda-horario", intervalo ? "intervalo" : "unico");
    this.campo("horaDoDia").value = textoDeHora(intervalo ? 0 : a.inicioHora);
    this.campo("intervalo").value = String(Math.max(1, a.freqSubintervalo));
    this.campo<HTMLSelectElement>("unidadeDoIntervalo").value = String(
      a.freqSubtipo === HORARIO.minutos ? HORARIO.minutos : HORARIO.horas,
    );
    this.campo("inicioDoDia").value = textoDeHora(intervalo ? a.inicioHora : 0);
    this.campo("fimDoDia").value = textoDeHora(intervalo ? a.fimHora : 235959);

    this.campo("inicio").value = textoDeData(a.inicioData);
    this.campo("fim").value = textoDeData(a.fimData >= SEM_TERMINO ? a.inicioData : a.fimData);
    this.radio("agenda-fim", a.fimData >= SEM_TERMINO ? "sem" : "com");
  }

  private marcasDosDias(): HTMLInputElement[] {
    return [...this.caixa.querySelectorAll<HTMLInputElement>("[data-dia]")];
  }

  private radio(grupo: string, valor: string): void {
    for (const opcao of this.caixa.querySelectorAll<HTMLInputElement>(`input[name="${grupo}"]`)) {
      opcao.checked = opcao.value === valor;
    }
  }

  private valorDoRadio(grupo: string): string {
    return (
      this.caixa.querySelector<HTMLInputElement>(`input[name="${grupo}"]:checked`)?.value ?? ""
    );
  }

  private ligarEventos(): void {
    for (const botao of this.caixa.querySelectorAll<HTMLButtonElement>('[data-acao="cancelar"]')) {
      botao.addEventListener("click", () => this.fechar(null));
    }
    this.caixa
      .querySelector<HTMLButtonElement>('[data-acao="confirmar"]')
      ?.addEventListener("click", () => this.confirmar());

    // Qualquer mexida repinta o resumo e reavalia o que fica visível: são as
    // duas coisas que o SSMS também atualiza a cada clique.
    this.caixa.addEventListener("change", () => this.aplicarVisibilidade());
    this.caixa.addEventListener("input", () => this.atualizarResumo());
  }

  /** Mostra só os campos que o tipo escolhido usa. */
  private aplicarVisibilidade(): void {
    const tipo = this.campo<HTMLSelectElement>("tipo").value;
    const recorrente = tipo === "recorrente";
    const umaVez = tipo === "umaVez";

    for (const bloco of this.bloco("umaVez")) bloco.hidden = !umaVez;
    for (const nome of ["frequencia", "horario", "duracao"]) {
      for (const bloco of this.bloco(nome)) bloco.hidden = !recorrente;
    }

    const ocorre = Number(this.campo<HTMLSelectElement>("ocorre").value);
    for (const bloco of this.bloco("diaria")) bloco.hidden = ocorre !== FREQUENCIA.diaria;
    for (const bloco of this.bloco("semanal")) bloco.hidden = ocorre !== FREQUENCIA.semanal;
    for (const bloco of this.bloco("mensal")) bloco.hidden = ocorre !== FREQUENCIA.mensal;

    const relativa = this.valorDoRadio("agenda-mensal") === "relativa";
    this.campo("diaDoMes").disabled = relativa;
    this.campo("mesesPorDia").disabled = relativa;
    this.campo<HTMLSelectElement>("ordem").disabled = !relativa;
    this.campo<HTMLSelectElement>("unidadeRelativa").disabled = !relativa;
    this.campo("mesesPorRelativa").disabled = !relativa;

    const porIntervalo = this.valorDoRadio("agenda-horario") === "intervalo";
    this.campo("horaDoDia").disabled = porIntervalo;
    for (const nome of ["intervalo", "unidadeDoIntervalo", "inicioDoDia", "fimDoDia"]) {
      this.campo<HTMLInputElement | HTMLSelectElement>(nome).disabled = !porIntervalo;
    }

    this.campo("fim").disabled = this.valorDoRadio("agenda-fim") !== "com";
    this.atualizarResumo();
  }

  private atualizarResumo(): void {
    this.elemento("resumo").textContent = resumoDaAgenda(this.montar());
  }

  /** Lê a tela e devolve os números do `sp_add_jobschedule`. */
  private montar(): AgendaDeTrabalho {
    const tipo = this.campo<HTMLSelectElement>("tipo").value;
    const base: AgendaDeTrabalho = {
      nome: this.campo("nome").value.trim(),
      habilitada: (this.campo("habilitada") as HTMLInputElement).checked,
      freqTipo: FREQUENCIA.diaria,
      freqIntervalo: 1,
      freqSubtipo: 0,
      freqSubintervalo: 0,
      freqIntervaloRelativo: 0,
      freqFator: 0,
      inicioData: numeroDeHoje(),
      fimData: SEM_TERMINO,
      inicioHora: 0,
      fimHora: 235959,
    };

    if (tipo === String(FREQUENCIA.aoIniciarAgente) || tipo === String(FREQUENCIA.cpuOciosa)) {
      // As duas não têm periodicidade nem horário: quem decide quando rodar é o
      // próprio Agent.
      return { ...base, freqTipo: Number(tipo), freqIntervalo: 0 };
    }

    if (tipo === "umaVez") {
      return {
        ...base,
        freqTipo: FREQUENCIA.umaVez,
        freqIntervalo: 0,
        inicioData: numeroDeData(this.campo("dataUnica").value),
        inicioHora: numeroDeHora(this.campo("horaUnica").value),
      };
    }

    const agenda: AgendaDeTrabalho = {
      ...base,
      inicioData: numeroDeData(this.campo("inicio").value),
      fimData:
        this.valorDoRadio("agenda-fim") === "com"
          ? numeroDeData(this.campo("fim").value)
          : SEM_TERMINO,
    };

    switch (Number(this.campo<HTMLSelectElement>("ocorre").value)) {
      case FREQUENCIA.diaria:
        agenda.freqTipo = FREQUENCIA.diaria;
        agenda.freqIntervalo = numero(this.campo("dias"), 1);
        break;
      case FREQUENCIA.semanal:
        agenda.freqTipo = FREQUENCIA.semanal;
        agenda.freqIntervalo = this.marcasDosDias()
          .filter((marca) => marca.checked)
          .reduce((soma, marca) => soma + Number(marca.dataset.dia), 0);
        agenda.freqFator = numero(this.campo("semanas"), 1);
        break;
      default:
        if (this.valorDoRadio("agenda-mensal") === "relativa") {
          agenda.freqTipo = FREQUENCIA.mensalRelativa;
          agenda.freqIntervaloRelativo = Number(this.campo<HTMLSelectElement>("ordem").value);
          agenda.freqIntervalo = Number(this.campo<HTMLSelectElement>("unidadeRelativa").value);
          agenda.freqFator = numero(this.campo("mesesPorRelativa"), 1);
        } else {
          agenda.freqTipo = FREQUENCIA.mensal;
          agenda.freqIntervalo = numero(this.campo("diaDoMes"), 1);
          agenda.freqFator = numero(this.campo("mesesPorDia"), 1);
        }
    }

    if (this.valorDoRadio("agenda-horario") === "intervalo") {
      agenda.freqSubtipo = Number(this.campo<HTMLSelectElement>("unidadeDoIntervalo").value);
      agenda.freqSubintervalo = numero(this.campo("intervalo"), 1);
      agenda.inicioHora = numeroDeHora(this.campo("inicioDoDia").value);
      agenda.fimHora = numeroDeHora(this.campo("fimDoDia").value);
    } else {
      agenda.freqSubtipo = HORARIO.umaVez;
      agenda.freqSubintervalo = 0;
      agenda.inicioHora = numeroDeHora(this.campo("horaDoDia").value);
      agenda.fimHora = 235959;
    }
    return agenda;
  }

  private confirmar(): void {
    const agenda = this.montar();
    const problema = this.problema(agenda);
    if (problema) {
      this.elemento("status").textContent = problema;
      this.elemento("status").className = "status status--erro";
      return;
    }
    this.fechar(agenda);
  }

  /**
   * O que o servidor recusaria, recusado aqui com o nome do campo.
   *
   * Uma agenda semanal sem dia nenhum marcado é o caso que mais aparece: o
   * `sp_add_jobschedule` a aceita e o trabalho simplesmente nunca roda.
   */
  private problema(agenda: AgendaDeTrabalho): string | null {
    if (!agenda.nome) return t("informe o nome da agenda");
    if (agenda.freqTipo === FREQUENCIA.semanal && agenda.freqIntervalo === 0) {
      return t("marque ao menos um dia");
    }
    if (agenda.fimData < agenda.inicioData) return t("termino antes do inicio");
    // A janela do dia só existe quando há repetição dentro dele. Nas outras, a
    // hora final é sempre 23:59:59 e comparar as duas recusaria uma agenda de
    // uma vez marcada justamente para 23:59:59.
    const repeteNoDia =
      agenda.freqSubtipo === HORARIO.minutos || agenda.freqSubtipo === HORARIO.horas;
    if (repeteNoDia && agenda.fimHora <= agenda.inicioHora) {
      return t("hora final antes da inicial");
    }
    return null;
  }

  private fechar(agenda: AgendaDeTrabalho | null): void {
    this.pararEsc();
    this.fundo.remove();
    this.resolver(agenda);
  }
}

function recorrenciaValida(freqTipo: number): number {
  return freqTipo === FREQUENCIA.diaria || freqTipo === FREQUENCIA.semanal
    ? freqTipo
    : FREQUENCIA.mensal;
}

function numero(campo: HTMLInputElement, minimo: number): number {
  const valor = Number.parseInt(campo.value, 10);
  return Number.isFinite(valor) && valor >= minimo ? valor : minimo;
}
