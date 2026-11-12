/**
 * Formata durações, tamanhos, datas e horas para a interface (requisito 21).
 *
 * Data e hora vêm do SQL Server como `yyyy-MM-dd HH:mm:ss` e são **hora do
 * servidor**, sem fuso. Por isso os campos são lidos um a um em vez de se passar
 * o texto a `new Date`: interpretar aquilo como UTC deslocaria o horário de quem
 * lê, e num campo de última execução isso é a diferença entre "rodou de
 * madrugada" e "rodou de manhã".
 */

import { idiomaAtual } from "./idioma";

/** `2026-08-27 15:11:05` no formato do idioma da tela, com data e hora. */
export function formatarDataHora(momento: string | null | undefined): string {
  const data = interpretar(momento);
  if (!data) return momento ?? "";
  return new Intl.DateTimeFormat(idiomaAtual(), {
    dateStyle: "short",
    timeStyle: "medium",
  }).format(data);
}

/** Só a data: `27/08/2026` em português. */
export function formatarData(momento: string | null | undefined): string {
  const data = interpretar(momento);
  if (!data) return momento ?? "";
  return new Intl.DateTimeFormat(idiomaAtual(), { dateStyle: "short" }).format(data);
}

/**
 * Só a hora: `15:11:05`.
 *
 * Aceita também `HH:mm:ss` sozinho, que é como as agendas do SQL Server Agent
 * guardam horário. Sem isso o inglês mostraria `15:11:05` em vez de
 * `3:11:05 PM` — o texto passaria por formatado sem ter sido.
 */
export function formatarHora(momento: string | null | undefined): string {
  const soHora = /^(\d{2}):(\d{2})(?::(\d{2}))?$/.test(momento ?? "");
  const data = interpretar(soHora ? `2000-01-01 ${momento}` : momento);
  if (!data) return momento ?? "";
  return new Intl.DateTimeFormat(idiomaAtual(), { timeStyle: "medium" }).format(data);
}

/** Aceita `yyyy-MM-dd`, com ou sem a hora atrás. */
function interpretar(momento: string | null | undefined): Date | null {
  const partes = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2}))?)?/.exec(
    momento ?? "",
  );
  if (!partes) return null;
  return new Date(
    Number(partes[1]),
    Number(partes[2]) - 1,
    Number(partes[3]),
    Number(partes[4] ?? 0),
    Number(partes[5] ?? 0),
    Number(partes[6] ?? 0),
  );
}

/** Formata durações para a interface sem expor milissegundos como unidade. */
export function formatarDuracao(duracaoMs: number): string {
  const totalMs = Math.max(0, Math.round(duracaoMs));
  if (totalMs < 60_000) return formatarSegundos(totalMs);

  const minutos = Math.floor(totalMs / 60_000);
  const restante = totalMs % 60_000;
  // Passada a hora, o minuto deixa de ser legível: "187 min" obriga quem lê a
  // dividir de cabeça para descobrir que são mais de três horas. Um trabalho do
  // Agent (requisito 36) chega lá sem esforço.
  if (minutos >= 60) return `${Math.floor(minutos / 60)} h ${minutos % 60} min`;
  return restante === 0 ? `${minutos} min` : `${minutos} min ${formatarSegundos(restante)}`;
}

/** Tamanho de arquivo em unidade legível: `112,4 MB`, com vírgula decimal. */
export function formatarTamanho(bytes: number): string {
  if (bytes < 1024) return `${Math.max(0, Math.round(bytes))} bytes`;

  const unidades = ["KB", "MB", "GB"];
  let valor = bytes / 1024;
  let unidade = 0;
  while (valor >= 1024 && unidade < unidades.length - 1) {
    valor /= 1024;
    unidade += 1;
  }
  return `${valor.toFixed(1).replace(".", ",")} ${unidades[unidade]}`;
}

function formatarSegundos(duracaoMs: number): string {
  const inteiros = Math.floor(duracaoMs / 1_000);
  const fracao = duracaoMs % 1_000;
  if (fracao === 0) return `${inteiros} s`;

  const decimais = String(fracao).padStart(3, "0").replace(/0+$/, "");
  return `${inteiros},${decimais} s`;
}
