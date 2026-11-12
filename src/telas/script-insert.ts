/**
 * Gerador de script de INSERT a partir do grid (requisito 39).
 *
 * O script é montado com o que o grid tem — que é **texto de tela**, e não o
 * valor cru do banco. Número chega `1234,56` e data chega `27/08/2026`
 * (requisito 21), então cada valor é desformatado antes de virar literal. O que
 * não tem como ser reconstruído daí sai como `NULL` e é relatado: conteúdo
 * binário, que o grid mostra como descrição de tamanho, e valor que o driver
 * não converteu.
 *
 * A montagem é função pura sobre texto — nada aqui fala com banco, e por isso
 * se confere chamando com casos de teste. Só a última função sai disso: é ela
 * que escreve no clipboard e relata, e existe para os dois lugares que
 * oferecem o item — o grid da consulta e a aba de Dados do painel de objeto —
 * usarem as mesmas recusas e os mesmos avisos.
 */

import { mensagemDeErro, type ColunaResultado, type Linha } from "../api";
import { t } from "../idioma";

/** Acima disso o Oracle recusa o literal de texto, e o script não roda. */
const LIMITE_DO_LITERAL = 4000;

/** O texto que o grid mostra para data e hora (requisito 21), nos dois bancos. */
const DATA_HORA = /^(\d{2})\/(\d{2})\/(\d{4})(?: (\d{2}):(\d{2}):(\d{2}))?$/;

/** Número como o grid mostra: vírgula decimal e sem separador de milhar. */
const NUMERO = /^-?\d+(?:,\d+)?(?:[eE][+-]?\d+)?$/;

/** O driver não soube converter o valor; o grid mostra isto na célula. */
const NAO_CONVERTIDO = "(valor não convertido)";

export interface PedidoDeInsert {
  /**
   * A tabela como o comando a escreveu — é esse nome que resolve na sessão de
   * quem vai rodar o script.
   */
  tabela: string;
  colunas: ColunaResultado[];
  linhas: Linha[];
  /** `TO_DATE` no Oracle, `CONVERT` no SQL Server. */
  sqlServer: boolean;
}

export interface ScriptDeInsert {
  texto: string;
  /** Quantos comandos saíram — um por linha escolhida. */
  quantidade: number;
  /** Colunas que saíram como `NULL` porque o grid não tem o valor. */
  colunasNulas: string[];
  /** Colunas cujo literal passa do limite que o Oracle aceita. */
  colunasLongas: string[];
}

/**
 * O nome da primeira coluna que não é nome de coluna, ou `null` se todas são.
 *
 * `SELECT COUNT(*) FROM cliente` lê uma tabela só e ainda assim não gera
 * INSERT: `COUNT(*)` não é lugar onde se escreva valor. Recusar aqui, antes de
 * montar o script, é o que evita entregar ao clipboard um comando que o banco
 * vai rejeitar.
 */
export function colunaSemIdentificador(colunas: ColunaResultado[]): string | null {
  for (const coluna of colunas) {
    if (!/^[A-Za-z_][\w$#]*$/.test(coluna.nome)) return coluna.nome;
  }
  return null;
}

export function gerarScriptDeInsert(pedido: PedidoDeInsert): ScriptDeInsert {
  const { tabela, colunas, linhas, sqlServer } = pedido;
  const nulas = new Set<string>();
  const longas = new Set<string>();
  const nomes = colunas.map((coluna) => coluna.nome).join(", ");

  const comandos = linhas.map((linha) => {
    const valores = colunas.map((coluna, indice) => {
      const literal = literalDoValor(linha[indice] ?? null, coluna, sqlServer);
      if (literal === null) {
        nulas.add(coluna.nome);
        return "NULL";
      }
      if (literal.length > LIMITE_DO_LITERAL) longas.add(coluna.nome);
      return literal;
    });
    return `INSERT INTO ${tabela} (${nomes}) VALUES (${valores.join(", ")});`;
  });

  return {
    texto: comandos.length === 0 ? "" : `${comandos.join("\r\n")}\r\n`,
    quantidade: comandos.length,
    colunasNulas: [...nulas],
    colunasLongas: [...longas],
  };
}

/**
 * O literal de um valor, ou `null` quando o grid não tem como reproduzi-lo — e
 * aí quem chama põe `NULL` e relata a coluna.
 *
 * `NULL` do banco também devolve `null`, mas sem relatar: ali o `NULL` do
 * script é o valor certo, e não uma perda.
 */
function literalDoValor(
  valor: string | null,
  coluna: ColunaResultado,
  sqlServer: boolean,
): string | null {
  if (valor === null) return "NULL";
  if (eBinaria(coluna.tipo) || valor === NAO_CONVERTIDO) return null;

  if (coluna.numerica || coluna.tipo.toLowerCase() === "bit") {
    const numero = valor.replace(",", ".");
    // Um número que não se reconhece é `Infinity`, `NaN` ou coisa que o driver
    // escreveu à sua maneira: escrever aquilo cru quebraria o comando.
    return NUMERO.test(valor) ? numero : null;
  }

  if (eDataHora(coluna.tipo)) {
    const literal = literalDeDataHora(valor, coluna.tipo, sqlServer);
    if (literal) return literal;
    // Data que não casa com o formato do grid vira texto entre aspas: o banco
    // ainda pode aceitá-la, e recusar aqui seria pior.
  }

  return `'${valor.replace(/'/g, "''")}'`;
}

/**
 * `27/08/2026 15:11:05` → `TO_DATE('27/08/2026 15:11:05','DD/MM/YYYY
 * HH24:MI:SS')` no Oracle; `CONVERT(datetime, '2026-08-27 15:11:05', 120)` no
 * SQL Server.
 *
 * A conversão é explícita nos dois porque o literal solto depende de
 * configuração da sessão — `NLS_DATE_FORMAT` num, `DATEFORMAT` no outro —, e um
 * script que só roda na sessão de quem o gerou não serve para nada.
 */
function literalDeDataHora(valor: string, tipo: string, sqlServer: boolean): string | null {
  const partes = DATA_HORA.exec(valor);
  if (!partes) return null;
  const [, dia, mes, ano, hora, minuto, segundo] = partes;
  const temHora = hora !== undefined;

  if (!sqlServer) {
    const formato = temHora ? "DD/MM/YYYY HH24:MI:SS" : "DD/MM/YYYY";
    // Numa coluna `TIMESTAMP`, `TO_TIMESTAMP` diz no próprio script o que a
    // coluna é. O segundo é o quanto o grid mostra: fração de segundo não
    // chega até aqui.
    const funcao = tipo.toUpperCase().startsWith("TIMESTAMP") ? "TO_TIMESTAMP" : "TO_DATE";
    return `${funcao}('${valor}','${formato}')`;
  }
  // Estilo 120 é `yyyy-mm-dd hh:mi:ss`; 23 é só a data. Os dois são
  // determinados, e é isso que se quer de um script que vai rodar em outro
  // servidor.
  return temHora
    ? `CONVERT(datetime, '${ano}-${mes}-${dia} ${hora}:${minuto}:${segundo}', 120)`
    : `CONVERT(date, '${ano}-${mes}-${dia}', 23)`;
}

/**
 * Põe no clipboard o script das linhas escolhidas e conta no rodapé o que
 * houve: quantos comandos saíram, o que se perdeu no caminho, e por que não
 * saiu nada quando é o caso.
 *
 * `relatar` é o rodapé de quem chamou — a assinatura é a mesma nos dois
 * painéis.
 */
export async function copiarInsertParaClipboard(
  dados: { colunas: ColunaResultado[]; linhas: Linha[] },
  tabela: string,
  sqlServer: boolean,
  relatar: (mensagem: string, erro?: boolean) => void,
): Promise<void> {
  const { colunas, linhas } = dados;
  if (linhas.length === 0) {
    relatar(t("insert sem linhas"), true);
    return;
  }
  const calculada = colunaSemIdentificador(colunas);
  if (calculada) {
    relatar(t("insert coluna calculada", { coluna: calculada }), true);
    return;
  }

  const script = gerarScriptDeInsert({ tabela, colunas, linhas, sqlServer });
  try {
    await navigator.clipboard.writeText(script.texto);
  } catch (erro) {
    relatar(mensagemDeErro(erro), true);
    return;
  }

  // O aviso vale vermelho mesmo tendo dado certo: o script está no clipboard,
  // mas o que ele insere não é o que estava no banco — e quem for colar
  // precisa saber disso antes, não depois.
  const avisos: string[] = [];
  if (script.colunasNulas.length > 0) {
    avisos.push(t("insert colunas nulas", { colunas: script.colunasNulas.join(", ") }));
  }
  if (script.colunasLongas.length > 0) {
    avisos.push(t("insert colunas longas", { colunas: script.colunasLongas.join(", ") }));
  }
  relatar(
    [t("insert copiado", { quantidade: script.quantidade, tabela }), ...avisos].join(" — "),
    avisos.length > 0,
  );
}

/**
 * Data e hora nos dois bancos: `DATE` e `TIMESTAMP(6)` do Oracle, `date`,
 * `datetime`, `datetime2`, `datetimeoffset` e `smalldatetime` do SQL Server —
 * todos menos o último começam com uma das duas palavras.
 *
 * `time` fica de fora de propósito: o grid mostra `15:11:05`, e esse texto entre
 * aspas é o que a coluna aceita.
 */
function eDataHora(tipo: string): boolean {
  const nome = tipo.toUpperCase();
  return (
    nome.startsWith("DATE") || nome.startsWith("TIMESTAMP") || nome === "SMALLDATETIME"
  );
}

/**
 * Conteúdo binário, nos dois bancos.
 *
 * O grid mostra `(binário, 320 bytes)` no lugar do conteúdo, porque não há como
 * mostrá-lo numa célula; escrever aquela descrição como literal poria texto
 * onde havia bytes.
 */
function eBinaria(tipo: string): boolean {
  const nome = tipo.toUpperCase();
  return (
    nome.startsWith("RAW") ||
    nome === "LONG RAW" ||
    nome === "BLOB" ||
    nome === "BFILE" ||
    nome === "BINARY" ||
    nome === "VARBINARY" ||
    nome === "IMAGE"
  );
}
