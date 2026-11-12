import { startCompletion, currentCompletions, closeCompletion, completionStatus } from "@codemirror/autocomplete";
import type { EditorView } from "@codemirror/view";
import { EditorSql } from "../src/telas/editor-sql";
import type { Sugestao } from "../src/api";

const resultado = document.querySelector<HTMLPreElement>("#resultado")!;
const recipiente = document.querySelector<HTMLDivElement>("#editor")!;
const mensagens: string[] = [];
const pausa = (ms: number) => new Promise<void>((resolver) => setTimeout(resolver, ms));
const cliente: Sugestao = { nome: "DEM_CAD_CLIENTE", tipo: "TABLE", dono: null };
const novo: Sugestao = { nome: "DEM_CAD_NOVO", tipo: "VIEW", dono: "OUTRO" };
const conferir = (condicao: boolean, mensagem: string) => { if (!condicao) throw new Error(mensagem); };
async function esperar(condicao: () => boolean, mensagem: string) {
  const prazo = performance.now() + 2000;
  while (!condicao() && performance.now() < prazo) await pausa(10);
  conferir(condicao(), mensagem);
}
function aprovado(texto: string) { mensagens.push(`OK — ${texto}`); resultado.textContent = mensagens.join("\n"); }

async function verificar() {
  const remotas: { prefixo: string; resolver: (r: Sugestao[]) => void }[] = [];
  const locais: { prefixo: string; esquema?: string }[] = [];
  let indice: Sugestao[] = [cliente];
  const editor = new EditorSql({
    texto: "SELECT * FROM DE",
    sugestoes: {
      objetos: async () => [], colunas: async () => [],
      relacoes: (prefixo, esquema, local) => {
        if (local) {
          locais.push({ prefixo, esquema });
          return Promise.resolve(indice.filter((r) => r.nome.startsWith(prefixo.toUpperCase())));
        }
        return new Promise((resolver) => remotas.push({ prefixo, resolver }));
      },
    },
  });
  recipiente.replaceChildren(editor.elemento);
  const visao = (editor as unknown as { visao: EditorView }).visao;
  const nomes = () => currentCompletions(visao.state).map((c) => c.label);
  const escrever = (texto: string) => {
    visao.dispatch({ changes: { from: 0, to: visao.state.doc.length, insert: texto }, selection: { anchor: texto.length } });
    visao.focus(); startCompletion(visao);
  };
  const inicio = performance.now();
  escrever("SELECT * FROM DE");
  await esperar(() => nomes().includes(cliente.nome) && remotas.length === 1, "fonte local não apareceu com remoto pendente");
  aprovado(`Sugestão local visível em ${Math.round(performance.now() - inicio)} ms, com banco ainda pendente.`);
  escrever("SELECT * FROM DEM_CAD_");
  await esperar(() => locais.some((r) => r.prefixo === "DEM_CAD_") && nomes().includes(cliente.nome), "novo prefixo ficou preso à consulta antiga");
  conferir(remotas.length === 1, "consulta remota não respeitou a fila");
  aprovado("DEM_CAD_ respondeu enquanto a consulta de DE continuava pendente.");
  remotas[0].resolver([{ nome: "DEM_ANTIGO", tipo: "TABLE", dono: null }]);
  await esperar(() => remotas.length === 2, "consulta do prefixo atual não começou");
  remotas[1].resolver([cliente, novo]);
  await esperar(() => nomes().includes(novo.nome), "complemento remoto não entrou no menu aberto");
  conferir(nomes().filter((n) => n === cliente.nome).length === 1, "objeto repetido entre as fontes");
  conferir(!nomes().includes("DEM_ANTIGO"), "resposta antiga apareceu no prefixo novo");
  aprovado("Resposta remota complementou o menu sem duplicatas nem resultado obsoleto.");
  escrever("SELECT * FROM OUTRO.DEM_CAD_");
  await esperar(() => locais.some((r) => r.esquema === "OUTRO"), "schema não chegou à fonte local");
  await esperar(() => remotas.length === 3, "busca remota qualificada não começou");
  closeCompletion(visao);
  remotas[2].resolver([novo]);
  await pausa(150);
  conferir(completionStatus(visao.state) === null, "resposta tardia reabriu menu fechado");
  aprovado("Schema repassado corretamente; resposta tardia respeitou o fechamento do menu.");
  const chamadas = locais.length;
  escrever("SELECT 'DEM_CAD_");
  await pausa(150);
  conferir(locais.length === chamadas, "autocomplete de tabelas ativou dentro de literal");
  escrever("-- SELECT * FROM DEM_CAD_");
  await pausa(150);
  conferir(locais.length === chamadas, "autocomplete de tabelas ativou dentro de comentário");
  aprovado("Literais e comentários não consultam o índice de tabelas.");
  indice = [];
  escrever("SELECT * FROM ZZ");
  await esperar(() => remotas.length === 4, "busca sem índice não começou");
  remotas[3].resolver([]);
  await pausa(100);
  indice = [{ nome: "ZZ_NOVA", tipo: "TABLE", dono: null }];
  escrever("SELECT * FROM ZZ_");
  await esperar(() => nomes().includes("ZZ_NOVA"), "vazio antigo impediu ler índice atualizado");
  aprovado("Índice atualizado aparece mesmo depois de resposta vazia anterior.");
  resultado.textContent += "\nTODAS AS VERIFICAÇÕES PASSARAM";
}
void verificar().catch((erro) => { resultado.textContent += `\nFALHA: ${erro.message}`; console.error(erro); });
