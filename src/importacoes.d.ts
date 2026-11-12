/**
 * Importações que o Vite resolve e o TypeScript não conhece sozinho.
 *
 * Com o sufixo `?raw`, o Vite entrega o conteúdo do arquivo como texto, embutido
 * no pacote. É assim que a tela de Versões lê `versoes.md` sem precisar de um
 * comando do backend nem de arquivo solto ao lado do executável.
 */
declare module "*?raw" {
  const conteudo: string;
  export default conteudo;
}
