/**
 * Exportação do grid (requisito 16).
 *
 * Aqui mora só o CSV, que é texto. A planilha do Excel é montada do lado
 * Rust, em `src-tauri/src/xlsx.rs`: um `.xlsx` é um zip, e binário não
 * atravessa bem a ponte.
 */

import type { Linha } from "../api";

/**
 * CSV no dialeto que o Excel em português espera: separador `;` e marca de
 * ordem de bytes no início, para ele reconhecer o UTF-8 e não estragar os
 * acentos.
 */
export function paraCsv(colunas: string[], linhas: Linha[]): string {
  const partes = [colunas.map(campoCsv).join(";")];
  for (const linha of linhas) {
    partes.push(linha.map((valor) => campoCsv(valor ?? "")).join(";"));
  }
  return `﻿${partes.join("\r\n")}\r\n`;
}

function campoCsv(valor: string): string {
  if (/[";\r\n]/.test(valor)) {
    return `"${valor.replace(/"/g, '""')}"`;
  }
  return valor;
}
