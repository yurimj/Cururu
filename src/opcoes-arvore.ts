/**
 * Opções da árvore de objetos, na seção Geral das Configurações (requisito
 * 41).
 *
 * Mesmo padrão de `opcoes-autocomplete.ts`: a escolha fica guardada na
 * máquina e vale para as próximas aberturas. Ninguém precisa ser avisado da
 * troca — a árvore lê a opção na hora de listar cada página —, então não há
 * evento de alteração.
 *
 * Guardado como objeto pelo mesmo motivo de lá: a opção seguinte entra como
 * mais um campo, sem trocar a chave nem invalidar o que já foi escolhido.
 */

/** Quantos objetos cada categoria traz sem escolha guardada. */
export const OBJETOS_POR_VEZ_PADRAO = 200;

/** Menos que isso e o Listar mais… vira o trabalho principal da árvore. */
export const OBJETOS_POR_VEZ_MINIMO = 10;

/**
 * Há schemas com dezenas de milhares de tabelas: listar tudo de uma vez é
 * lento no banco e impraticável na tela. O teto deixa espaço para quem prefere
 * poucas páginas, sem que um zero a mais trave a árvore.
 */
export const OBJETOS_POR_VEZ_MAXIMO = 5000;

export interface OpcoesDaArvore {
  /** Quantos objetos cada categoria traz por vez; os demais vêm pelo Listar mais…. */
  objetosPorVez: number;
}

const CHAVE_ARMAZENAMENTO = "cururu.arvore.v1";

/** O número, quando é um inteiro dentro dos limites; `null` quando não é. */
export function objetosPorVezValido(valor: unknown): number | null {
  if (typeof valor !== "number" || !Number.isInteger(valor)) return null;
  return valor >= OBJETOS_POR_VEZ_MINIMO && valor <= OBJETOS_POR_VEZ_MAXIMO ? valor : null;
}

function normalizar(valor: unknown): OpcoesDaArvore {
  const recebido = valor && typeof valor === "object" ? (valor as Record<string, unknown>) : {};
  return {
    objetosPorVez: objetosPorVezValido(recebido.objetosPorVez) ?? OBJETOS_POR_VEZ_PADRAO,
  };
}

function carregar(): OpcoesDaArvore {
  try {
    const salvo = localStorage.getItem(CHAVE_ARMAZENAMENTO);
    return normalizar(salvo ? JSON.parse(salvo) : null);
  } catch {
    return normalizar(null);
  }
}

let atuais = carregar();

export function opcoesDaArvore(): OpcoesDaArvore {
  return { ...atuais };
}

export function definirOpcoesDaArvore(novas: OpcoesDaArvore): void {
  const normalizadas = normalizar(novas);
  if (normalizadas.objetosPorVez === atuais.objetosPorVez) return;
  atuais = normalizadas;
  localStorage.setItem(CHAVE_ARMAZENAMENTO, JSON.stringify(atuais));
}
