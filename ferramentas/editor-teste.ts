/**
 * Banco de provas do editor SQL e da árvore de objetos, fora do Tauri.
 *
 * Serve para conferir comportamento de teclado e mouse — seleção em coluna com
 * Alt, cursores múltiplos, Tab no autocomplete e menu de Views — sem esperar os
 * ~6 minutos de um build release. Não entra no aplicativo: o `index.html` é a
 * única entrada do build e não referencia este arquivo.
 *
 * Abrir com `npm run dev` em /ferramentas/editor-teste.html.
 */

import { EditorSql } from "../src/telas/editor-sql";
import { ArvoreDeObjetos } from "../src/telas/arvore-objetos";
import { perfilNovo } from "../src/tipos";
import { abrirMenuDeContexto } from "../src/componentes/menu-contexto";

const palco = document.querySelector<HTMLDivElement>("#palco")!;
const estado = document.querySelector<HTMLDivElement>("#estado")!;

// Backend mínimo para provar o menu de uma View no navegador. As chamadas do
// editor continuam usando as sugestões de mentira logo abaixo.
(window as unknown as {
  __TAURI_INTERNALS__: { invoke: (comando: string, argumentos: Record<string, unknown>) => Promise<unknown> };
}).__TAURI_INTERNALS__ = {
  invoke: (comando, argumentos) => {
    if (comando === "ler_filtros") return Promise.resolve({});
    if (comando === "listar_objetos") {
      return Promise.resolve({
        objetos: argumentos.categoria === "views" ? [{ nome: "VW_CLIENTES", situacao: "VALID" }] : [],
        temMais: false,
      });
    }
    return Promise.resolve(null);
  },
};

const LINHAS = [
  "SELECT * FROM DEM_CLIENTES WHERE STATUS = 'PS'",
  "SELECT * FROM DEM_CLIENTES WHERE DOCUMENTO = ''",
  "SELECT * FROM DEM_CLIENTES_PAPEIS WHERE STATUS = 'PS'",
  "SELECT * FROM DEM_CLIENTES_PAPEIS ORDER BY DT_INC DESC",
  "SELECT * FROM DEM_CLIENTES_EMPRESAS WHERE STATUS = 'PS'",
  "SELECT * FROM DEM_CLIENTES_CONTAS WHERE STATUS = 'PS'",
  "SELECT * FROM DEM_CLIENTES_VENDAS WHERE STATUS = 'PS'",
  "SELECT * FROM DEM_CLIENTES_IMPOSTOS WHERE STATUS = 'PS'",
  "BEGIN PKG_FATURA.GERAR_BOLETO(1); END;",
  "-- PKG_IGNORADO.PROCEDURE_IGNORADA",
  "SELECT 'VIEW_IGNORADA' FROM dual;",
];

let ultimaReferencia = "—";

const chamadasDeSubprogramas: string[] = [];
(window as unknown as { __chamadasDeSubprogramas: string[] }).__chamadasDeSubprogramas =
  chamadasDeSubprogramas;

// O que o "banco" desta página diz que o Ctrl+clique abre. As perguntas ficam
// guardadas para a inspeção conferir que palavra reservada, texto entre aspas
// e comentário nunca vão ao banco.
const NAVEGAVEIS = new Set(["DEM_CLIENTES", "DEM_CLIENTES_PAPEIS", "PKG_FATURA.GERAR_BOLETO", "DUAL"]);
const perguntasDeReferencias: string[][] = [];
(window as unknown as { __perguntasDeReferencias: string[][] }).__perguntasDeReferencias =
  perguntasDeReferencias;

const editor = new EditorSql({
  texto: LINHAS.join("\n"),
  // Sugestões de mentira: aqui não há banco, e o que se testa é a interação.
  sugestoes: {
    objetos: (prefixo) =>
      Promise.resolve(
        ["EXEMPLO_CONTROLE", "EXEMPLO_ITEM", "EXEMPLO_STATUS"]
          .filter((nome) => nome.startsWith(prefixo.toUpperCase()))
          .map((nome) => ({ nome, tipo: "TABLE", dono: null })),
      ),
    colunas: () => Promise.resolve([]),
    subprogramas: (pacote) => {
      chamadasDeSubprogramas.push(pacote);
      return Promise.resolve(
        pacote.toUpperCase() === "DEM"
          ? [
              { nome: "ENVIAR_PEDIDO", especie: "PROCEDURE" },
              { nome: "STATUS_PEDIDO", especie: "FUNCTION" },
            ]
          : [],
      );
    },
  },
  aoCtrlClicar: (referencia) => {
    ultimaReferencia = referencia;
    relatar();
  },
  verificarReferencias: (referencias) => {
    perguntasDeReferencias.push(referencias);
    // Com espera, como a do banco: o sublinhado só pode vir depois da resposta.
    // `__esperaDoBanco` deixa a inspeção alongar a espera para ver o
    // indicador de carregamento.
    const espera = (window as unknown as { __esperaDoBanco?: number }).__esperaDoBanco ?? 150;
    return new Promise((responder) =>
      window.setTimeout(
        () => responder(referencias.filter((referencia) => NAVEGAVEIS.has(referencia))),
        espera,
      ),
    );
  },
  aoAbrirMenuDeReferencia: (evento, referencia) =>
    abrirMenuDeContexto(evento, [
      {
        rotulo: "Ir para definição",
        acao: () => {
          ultimaReferencia = referencia;
          relatar();
        },
      },
    ]),
});

palco.append(editor.elemento);
editor.focar();

const testarPackage = document.createElement("button");
testarPackage.textContent = "Testar autocomplete DEM.";
testarPackage.addEventListener("click", () => editor.inserir("\nDEM."));
estado.before(testarPackage);

const perfil = perfilNovo();
perfil.id = "teste";
perfil.nome = "Conexão de teste";
perfil.cor = "#4f8a5b";
const areaArvore = document.createElement("aside");
areaArvore.id = "arvore-teste";
areaArvore.append(
  new ArvoreDeObjetos(perfil, {
    conectar: () => Promise.resolve("sessao-teste"),
    aoNovaConsulta: () => {},
    aoConectar: () => {},
    aoAbrirFonte: async () => false,
    aoAbrirObjeto: (nome, categoria, aba) => {
      ultimaReferencia = `${categoria}:${nome}:${aba}`;
      relatar();
    },
    aoRelatar: () => {},
    aoSelecionarCategoria: () => {},
    aoSelecionarConexao: () => {},
    aoPairarBanco: () => {},
    aoEncerrarSessao: () => {},
  }).montar(),
);
palco.prepend(areaArvore);

// Expõe o editor para a inspeção automatizada poder ler a seleção de verdade,
// em vez de deduzi-la do que aparece pintado na tela.
(window as unknown as { editorDeProva: EditorSql }).editorDeProva = editor;

/**
 * Mostra na faixa de baixo quantos cursores e quantos retângulos de seleção o
 * CodeMirror está desenhando. É por aqui que a inspeção automatizada confere se
 * a seleção múltipla realmente vigora — sem depender de olhar a tela.
 */
function relatar(): void {
  const cursores = document.querySelectorAll(".cm-cursor").length;
  const selecoes = document.querySelectorAll(".cm-selectionBackground").length;
  estado.textContent = `cursores=${cursores} selecoes=${selecoes} referencia=${ultimaReferencia} packages=${chamadasDeSubprogramas.join(",") || "—"}`;
}

document.addEventListener("keyup", relatar);
document.addEventListener("mouseup", relatar);
setInterval(relatar, 250);
relatar();
