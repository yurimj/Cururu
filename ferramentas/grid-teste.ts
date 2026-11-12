/**
 * Banco de provas do grid de resultados, fora do Tauri.
 *
 * O grid só aparece no aplicativo depois de uma consulta ao banco, e no
 * navegador não há banco nenhum. Este arquivo o monta com linhas de mentira,
 * para conferir largura de coluna, arrasto da divisa, ordenação e seleção sem
 * esperar os minutos de um build release. Não entra no aplicativo: o
 * `index.html` é a única entrada do build e não referencia este arquivo.
 *
 * Abrir com `npm run dev` em /ferramentas/grid-teste.html.
 */

import { Grid } from "../src/telas/grid";

const palco = document.querySelector<HTMLDivElement>("#palco")!;
const estado = document.querySelector<HTMLDivElement>("#estado")!;

const MENSAGENS = [
  "Exemplo: nenhum registro pendente para o período informado",
  "Exemplo: valor de 1.234,56 divergente do total do documento",
  "Exemplo: tempo de resposta acima do limite aceitável",
  "Exemplo: falha ao consultar um serviço de demonstração",
];

const grid = new Grid(
  [
    { nome: "ID", tipo: "NUMBER", numerica: true },
    { nome: "MSG", tipo: "VARCHAR2", numerica: false },
    { nome: "DT_INC", tipo: "TIMESTAMP", numerica: false },
  ],
  { aoAbrirValor: (coluna, valor) => window.alert(`${coluna}\n\n${valor}`) },
);

grid.acrescentar(
  Array.from({ length: 300 }, (_, indice) => [
    String(5822368 - indice * 7),
    indice % 37 === 0 ? null : MENSAGENS[indice % MENSAGENS.length],
    `${String((indice % 28) + 1).padStart(2, "0")}/07/2026 09:23:14`,
  ]),
);

palco.append(grid.elemento);
grid.ajustar();

/** Mostra a largura de cada coluna, para a inspeção não depender de medir pixel na tela. */
function relatar(): void {
  const estilo = getComputedStyle(grid.elemento);
  const larguras = [0, 1, 2].map((c) => estilo.getPropertyValue(`--coluna-${c}`).trim());
  estado.textContent = `larguras: ${larguras.join("  ")}`;
}

document.addEventListener("pointerup", relatar);
setInterval(relatar, 250);
relatar();
