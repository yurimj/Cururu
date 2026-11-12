/**
 * Opções do autocomplete (requisito 43), na seção Auto-Complete das
 * Configurações.
 *
 * Mesmo padrão de `tema.ts` e `atalhos.ts`: a escolha fica guardada na máquina,
 * vale para as próximas aberturas e é avisada a quem precisa reagir. O que
 * muda é o destino — estas opções não mexem em pixel nenhum, elas viajam até o
 * backend junto com o pedido de sugestões.
 *
 * Guardado como objeto, e não como valor solto, porque a seção nasceu para
 * crescer: a opção seguinte entra como mais um campo, sem trocar a chave nem
 * invalidar o que o usuário já escolheu.
 */

/** O que a opção **Tabelas** oferece ao completar um nome. */
export type EscopoDeTabelas =
  /** Toda tabela que a conexão enxergue. */
  | "todas"
  /** Só as que passam pelo filtro da categoria Tabelas daquela conexão. */
  | "filtradas";

export interface OpcoesDeAutocomplete {
  tabelas: EscopoDeTabelas;
}

/** Sem escolha guardada, o autocomplete oferece tudo — como sempre ofereceu. */
const PADRAO: OpcoesDeAutocomplete = { tabelas: "todas" };

const CHAVE_ARMAZENAMENTO = "cururu.autocomplete.v1";
const EVENTO_DE_ALTERACAO = "cururu:autocomplete-alterado";

function normalizar(valor: unknown): OpcoesDeAutocomplete {
  if (!valor || typeof valor !== "object") return { ...PADRAO };
  const recebido = valor as Record<string, unknown>;
  return {
    tabelas: recebido.tabelas === "filtradas" ? "filtradas" : "todas",
  };
}

function carregar(): OpcoesDeAutocomplete {
  try {
    const salvo = localStorage.getItem(CHAVE_ARMAZENAMENTO);
    return salvo ? normalizar(JSON.parse(salvo)) : { ...PADRAO };
  } catch {
    return { ...PADRAO };
  }
}

let atuais = carregar();

export function opcoesDeAutocomplete(): OpcoesDeAutocomplete {
  return { ...atuais };
}

export function definirOpcoesDeAutocomplete(novas: OpcoesDeAutocomplete): void {
  const normalizadas = normalizar(novas);
  if (normalizadas.tabelas === atuais.tabelas) return;
  atuais = normalizadas;
  localStorage.setItem(CHAVE_ARMAZENAMENTO, JSON.stringify(atuais));
  window.dispatchEvent(new CustomEvent(EVENTO_DE_ALTERACAO));
}

/**
 * Avisa só as trocas futuras, sem chamar de imediato — mesmo contrato de
 * `observarTema` e `observarAtalhos`.
 */
export function observarOpcoesDeAutocomplete(aoTrocar: () => void): () => void {
  window.addEventListener(EVENTO_DE_ALTERACAO, aoTrocar);
  return () => window.removeEventListener(EVENTO_DE_ALTERACAO, aoTrocar);
}
