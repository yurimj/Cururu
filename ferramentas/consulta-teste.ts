/** Banco de provas dos estados vazio, executando e com resultado da consulta. */

const resultado = {
  comando: "SELECT 1 AS VALOR FROM DUAL",
  linhaNoEditor: 1,
  duracaoMs: 1500,
  consulta: {
    cursorId: "cursor-teste",
    colunas: [{ nome: "VALOR", tipo: "NUMBER", numerica: true }],
    linhas: [["1"]],
    temMais: false,
  },
  linhasAfetadas: null,
  mensagem: "1 linha selecionada.",
  saidaDbms: [],
};

(window as unknown as {
  __TAURI_INTERNALS__: {
    invoke: (comando: string) => Promise<unknown>;
  };
}).__TAURI_INTERNALS__ = {
  invoke: (comando) => {
    if (comando === "analisar_binds") return Promise.resolve([]);
    if (comando === "executar_sql") {
      return new Promise((resolver) => window.setTimeout(() => resolver([resultado]), 1500));
    }
    if (comando === "sugerir_objetos" || comando === "listar_colunas") {
      return Promise.resolve([]);
    }
    return Promise.resolve(null);
  },
};

const [{ PainelDeConsulta }, { perfilNovo }] = await Promise.all([
  import("../src/telas/painel-consulta"),
  import("../src/tipos"),
]);

const perfil = perfilNovo();
perfil.nome = "Conexão de teste";
document.querySelector("#palco")!.append(
  new PainelDeConsulta(
    { perfil, sessaoId: "sessao-teste" },
    () => {},
    "SELECT 1 AS VALOR FROM DUAL",
  ).elemento,
);
