/**
 * O pedacinho de markdown que as telas do Cururu leem.
 *
 * Duas telas mostram texto escrito em markdown: **Versões**, que lê
 * `versoes.md`, e **Atualização**, que lê o corpo do release publicado no
 * GitHub. As duas precisam exatamente das mesmas duas coisas — os itens da
 * lista e a marcação inline —, então isso mora aqui em vez de existir duas
 * vezes e divergir na terceira.
 *
 * Não é um interpretador de markdown: é só o que essas telas usam.
 */

/** As linhas de lista (`- item` ou `* item`), já sem o marcador. */
export function itensDaLista(texto: string): string[] {
  const itens: string[] = [];
  for (const linha of texto.split(/\r?\n/)) {
    const item = /^\s*[-*]\s+(.+)$/.exec(linha);
    if (item) itens.push(item[1].trim());
  }
  return itens;
}

/**
 * A marcação inline que os itens usam: `**negrito**`, `*itálico*` e crase para
 * nome de arquivo ou comando. O texto vai para o DOM como texto, nunca como
 * HTML — o arquivo de versões é nosso, mas o corpo do release vem da rede, e
 * escrever HTML a partir de texto é hábito ruim mesmo quando a origem é boa.
 */
export function comMarcacaoInline(texto: string): DocumentFragment {
  const fragmento = document.createDocumentFragment();
  const marcadores = /\*\*([^*]+)\*\*|\*([^*]+)\*|`([^`]+)`/g;
  let fim = 0;

  for (let achado = marcadores.exec(texto); achado; achado = marcadores.exec(texto)) {
    if (achado.index > fim) fragmento.append(texto.slice(fim, achado.index));
    const [inteiro, negrito, italico, codigo] = achado;
    const elemento = document.createElement(negrito ? "strong" : italico ? "em" : "code");
    elemento.textContent = negrito ?? italico ?? codigo;
    fragmento.append(elemento);
    fim = achado.index + inteiro.length;
  }

  if (fim < texto.length) fragmento.append(texto.slice(fim));
  return fragmento;
}
