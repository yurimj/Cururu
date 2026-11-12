/** Histórico local de arquivos abertos, do mais recente para o mais antigo. */
const CHAVE = "cururu.arquivos-recentes";
const LIMITE = 15;

function chaveDoCaminho(caminho: string): string {
  return caminho.replace(/\//g, "\\").toLowerCase();
}

export function obterArquivosRecentes(): string[] {
  try {
    const salvo: unknown = JSON.parse(localStorage.getItem(CHAVE) ?? "[]");
    if (!Array.isArray(salvo)) return [];
    const vistos = new Set<string>();
    return salvo.filter((caminho): caminho is string => {
      if (typeof caminho !== "string" || !caminho.trim()) return false;
      const chave = chaveDoCaminho(caminho);
      if (vistos.has(chave)) return false;
      vistos.add(chave);
      return true;
    }).slice(0, LIMITE);
  } catch {
    return [];
  }
}

export function registrarArquivoRecente(caminho: string): void {
  const chave = chaveDoCaminho(caminho);
  const recentes = [caminho, ...obterArquivosRecentes().filter(
    (existente) => chaveDoCaminho(existente) !== chave,
  )].slice(0, LIMITE);
  try {
    localStorage.setItem(CHAVE, JSON.stringify(recentes));
  } catch {
    // A falta de espaço para o histórico não impede a abertura do arquivo.
  }
}
