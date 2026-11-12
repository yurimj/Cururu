/**
 * Configuração dos botões de texto rápido da barra de consulta.
 *
 * A lista é global e não tem limite fixo. Alterações feitas em qualquer aba são
 * persistidas no armazenamento local do WebView e refletidas nas demais abas.
 */

import "./botoes-rapidos.css";
import { atalhoVisivel } from "../atalhos";
import { confirmar } from "../componentes/dialogo-simples";
import { fecharComEsc } from "../componentes/janela";
import { t } from "../idioma";

export interface ConfiguracaoDeBotaoRapido {
  rotulo: string;
  atalho: string;
  conteudo: string;
  executarAutomaticamente: boolean;
  /**
   * Executa o conteúdo direto, sem inserir nem selecionar nada no editor —
   * tem prioridade sobre `executarAutomaticamente`, que depende do texto
   * estar na tela para selecioná-lo.
   */
  naoInserirNaJanela: boolean;
  /**
   * As conexões em que o botão aparece, pelo `id` do perfil.
   *
   * **Vazio quer dizer "todas"** — e é por isso que o padrão é vazio: botão
   * criado antes desta versão, ou criado sem se pensar no assunto, continua
   * aparecendo em toda aba, como sempre apareceu. Guardar o `id`, e não o
   * nome, é o que faz o filtro sobreviver a renomear a conexão.
   *
   * O `id` de uma conexão excluída simplesmente deixa de casar com alguma
   * coisa; não se limpa a lista, porque a conexão pode voltar por importação.
   */
  conexoes: string[];
}

/** Uma conexão salva, como a lista do diálogo precisa vê-la. */
export interface ConexaoParaFiltro {
  id: string;
  nome: string;
  /** `#rrggbb` da bolinha de identificação; nulo em perfis antigos. */
  cor: string | null;
}

const CHAVE_ARMAZENAMENTO = "cururu.botoes-rapidos.v1";
const EVENTO_DE_ALTERACAO = "cururu:botoes-rapidos-alterados";
const TOTAL_INICIAL_DE_BOTOES = 10;

function configuracaoPadrao(indice: number): ConfiguracaoDeBotaoRapido {
  return {
    rotulo: String(indice + 1),
    // Ctrl+0 representa o décimo botão, depois de Ctrl+1…Ctrl+9.
    atalho: indice < 10 ? `Ctrl+${indice === 9 ? 0 : indice + 1}` : "",
    conteudo: indice === 0 ? "SELECT * FROM " : "",
    executarAutomaticamente: false,
    naoInserirNaJanela: false,
    conexoes: [],
  };
}

/**
 * O botão vale para esta conexão?
 *
 * Lista vazia vale para todas. É a regra que mantém intacto todo botão que já
 * existia quando o filtro foi criado.
 */
export function botaoValeParaConexao(
  configuracao: ConfiguracaoDeBotaoRapido,
  perfilId: string | null,
): boolean {
  if (configuracao.conexoes.length === 0) return true;
  return perfilId !== null && configuracao.conexoes.includes(perfilId);
}

function configuracoesPadrao(): ConfiguracaoDeBotaoRapido[] {
  return Array.from({ length: TOTAL_INICIAL_DE_BOTOES }, (_, indice) =>
    configuracaoPadrao(indice),
  );
}

function normalizarConfiguracoes(valor: unknown): ConfiguracaoDeBotaoRapido[] | null {
  if (!Array.isArray(valor)) return null;

  const atalhosUsados = new Set<string>();
  return valor.map((item, indice) => {
    const padrao = configuracaoPadrao(indice);
    if (!item || typeof item !== "object") return padrao;
    const parcial = item as Partial<ConfiguracaoDeBotaoRapido>;
    const candidatoRotulo = typeof parcial.rotulo === "string" ? parcial.rotulo.trim() : "";
    const rotulo =
      candidatoRotulo && Array.from(candidatoRotulo).length <= 8
        ? candidatoRotulo
        : padrao.rotulo;
    let atalho = typeof parcial.atalho === "string" ? parcial.atalho : padrao.atalho;
    if (atalho && atalhosUsados.has(atalho)) atalho = "";
    if (atalho) atalhosUsados.add(atalho);

    return {
      rotulo,
      atalho,
      conteudo: typeof parcial.conteudo === "string" ? parcial.conteudo : padrao.conteudo,
      executarAutomaticamente:
        typeof parcial.executarAutomaticamente === "boolean"
          ? parcial.executarAutomaticamente
          : false,
      naoInserirNaJanela:
        typeof parcial.naoInserirNaJanela === "boolean" ? parcial.naoInserirNaJanela : false,
      // Ausente é o botão salvo antes desta versão: vale para todas as
      // conexões, que é o que ele fazia até agora.
      conexoes: Array.isArray(parcial.conexoes)
        ? parcial.conexoes.filter((item): item is string => typeof item === "string")
        : [],
    };
  });
}

function carregarConfiguracoes(): ConfiguracaoDeBotaoRapido[] {
  try {
    const salvo = localStorage.getItem(CHAVE_ARMAZENAMENTO);
    if (!salvo) return configuracoesPadrao();
    return normalizarConfiguracoes(JSON.parse(salvo)) ?? configuracoesPadrao();
  } catch {
    return configuracoesPadrao();
  }
}

let configuracoes = carregarConfiguracoes();

function persistirConfiguracoes(): void {
  try {
    localStorage.setItem(CHAVE_ARMAZENAMENTO, JSON.stringify(configuracoes));
  } catch {
    // A configuração continua válida nesta execução se o WebView não puder persistir.
  }
  window.dispatchEvent(new Event(EVENTO_DE_ALTERACAO));
}

/** Devolve uma cópia para impedir alterações sem passar pelo salvamento. */
export function obterConfiguracoesDosBotoes(): ConfiguracaoDeBotaoRapido[] {
  return configuracoes.map((item) => ({ ...item }));
}

/** Substitui a lista inteira, usado pela restauração de backup. */
export function substituirConfiguracoesDosBotoes(valor: unknown): void {
  const novas = normalizarConfiguracoes(valor);
  if (!novas) throw new Error(t("backup sem lista de botoes"));
  configuracoes = novas;
  persistirConfiguracoes();
}

/** Observa alterações feitas pelo diálogo em qualquer aba aberta. */
export function observarConfiguracoesDosBotoes(aoAlterar: () => void): () => void {
  window.addEventListener(EVENTO_DE_ALTERACAO, aoAlterar);
  return () => window.removeEventListener(EVENTO_DE_ALTERACAO, aoAlterar);
}

/** Move um botão na lista; usado pelo arrastar e soltar da toolbar. */
export function moverBotaoRapido(origem: number, destino: number): void {
  if (
    origem < 0 ||
    origem >= configuracoes.length ||
    destino < 0 ||
    destino >= configuracoes.length ||
    origem === destino
  ) {
    return;
  }
  const [movido] = configuracoes.splice(origem, 1);
  configuracoes.splice(destino, 0, movido);
  persistirConfiguracoes();
}

/**
 * Traduz um evento para a mesma notação exibida no diálogo (Ctrl+Shift+S).
 * Letras e números vêm de `code`, para o atalho não mudar com Caps Lock.
 */
export function atalhoDoEvento(evento: KeyboardEvent): string | null {
  if (["Control", "Shift", "Alt", "Meta"].includes(evento.key)) return null;

  let tecla: string;
  if (/^Key[A-Z]$/.test(evento.code)) tecla = evento.code.slice(3);
  else if (/^Digit[0-9]$/.test(evento.code)) tecla = evento.code.slice(5);
  else if (/^Numpad[0-9]$/.test(evento.code)) tecla = `Num${evento.code.slice(6)}`;
  else if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(evento.key)) tecla = evento.key;
  else {
    const nomes: Record<string, string> = {
      " ": "Espaço",
      ArrowUp: "Seta acima",
      ArrowDown: "Seta abaixo",
      ArrowLeft: "Seta esquerda",
      ArrowRight: "Seta direita",
      Enter: "Enter",
      Tab: "Tab",
      Backspace: "Backspace",
      Delete: "Delete",
      Home: "Home",
      End: "End",
      PageUp: "Page Up",
      PageDown: "Page Down",
    };
    tecla = nomes[evento.key] ?? evento.key.toUpperCase();
  }

  if (!tecla) return null;
  const partes: string[] = [];
  if (evento.ctrlKey || evento.metaKey) partes.push("Ctrl");
  if (evento.altKey) partes.push("Alt");
  if (evento.shiftKey) partes.push("Shift");
  partes.push(tecla);
  return partes.join("+");
}

/**
 * O campo de atalho mostra a combinação traduzida e guarda a original.
 *
 * São coisas diferentes de propósito: o que se grava na configuração é sempre a
 * notação portuguesa, para o atalho continuar valendo depois de trocar o idioma
 * da tela; o que se lê ali é o nome da tecla no idioma de quem está olhando.
 */
function definirAtalhoDoCampo(campo: HTMLInputElement, atalho: string): void {
  campo.dataset.atalho = atalho;
  campo.value = atalhoVisivel(atalho);
}

function atalhoDoCampo(campo: HTMLInputElement): string {
  return campo.dataset.atalho ?? "";
}

/** Abre a janela para criar um botão novo. */
export function abrirNovoBotaoRapido(conexoes: ConexaoParaFiltro[] = []): void {
  abrirEditorDeBotao(null, conexoes);
}

/** Abre a janela de edição do botão indicado. */
export function abrirConfiguracaoDoBotao(
  indice: number,
  conexoes: ConexaoParaFiltro[] = [],
): void {
  abrirEditorDeBotao(indice, conexoes);
}

/**
 * A lista de marcação das conexões: "Todas" no topo, e uma linha por conexão.
 *
 * "Todas" não é uma conexão a mais — é a ausência de filtro, que no dado é a
 * lista vazia. Por isso as duas coisas se excluem na tela: marcar "Todas"
 * desmarca as específicas, marcar uma específica desmarca "Todas", e desmarcar
 * a última específica devolve o "Todas". Assim não existe estado inválido para
 * o Salvar recusar, e o que a tela mostra é sempre o que vai ser guardado.
 *
 * Devolve a função que lê a escolha na hora de salvar.
 */
function montarListaDeConexoes(
  lista: HTMLElement,
  conexoes: ConexaoParaFiltro[],
  escolhidas: string[],
): () => string[] {
  const todas = document.createElement("label");
  todas.className = "lista-de-marcacao__item lista-de-marcacao__item--todas";
  todas.innerHTML = `<input type="checkbox" data-conexao="" /><span>${t("todas as conexoes")}</span>`;
  const marcaTodas = todas.querySelector("input")!;
  marcaTodas.checked = escolhidas.length === 0;
  lista.append(todas);

  const especificas = conexoes.map((conexao) => {
    const item = document.createElement("label");
    item.className = "lista-de-marcacao__item";
    const marca = document.createElement("input");
    marca.type = "checkbox";
    marca.dataset.conexao = conexao.id;
    marca.checked = escolhidas.includes(conexao.id);
    const cor = document.createElement("i");
    cor.className = "lista-de-marcacao__cor";
    // Sem cor no perfil a bolinha some, em vez de virar um furo preto.
    cor.style.background = conexao.cor ?? "transparent";
    const nome = document.createElement("span");
    nome.textContent = conexao.nome;
    item.append(marca, cor, nome);
    lista.append(item);
    return marca;
  });

  const marcadas = (): HTMLInputElement[] => especificas.filter((marca) => marca.checked);

  marcaTodas.addEventListener("change", () => {
    // "Todas" só se desmarca escolhendo alguma: desmarcá-la sozinha deixaria a
    // tela dizendo "nenhuma", que não é um estado que exista.
    if (!marcaTodas.checked && marcadas().length === 0) marcaTodas.checked = true;
    if (marcaTodas.checked) for (const marca of especificas) marca.checked = false;
  });

  for (const marca of especificas) {
    marca.addEventListener("change", () => {
      marcaTodas.checked = marcadas().length === 0;
    });
  }

  return () => marcadas().map((marca) => marca.dataset.conexao ?? "");
}

function abrirEditorDeBotao(indice: number | null, conexoes: ConexaoParaFiltro[]): void {
  const ehNovo = indice === null;
  const atual = ehNovo ? configuracaoPadrao(configuracoes.length) : configuracoes[indice];
  if (!atual) return;

  const fundo = document.createElement("div");
  fundo.className = "dialogo-fundo";
  fundo.innerHTML = `
    <section class="dialogo dialogo-botao-rapido" role="dialog" aria-modal="true"
             aria-labelledby="titulo-botao-rapido">
      <header class="dialogo__barra">
        <h1 id="titulo-botao-rapido">${ehNovo ? t("novo botao") : t("configurar botao", { n: indice + 1 })}</h1>
        <button class="dialogo__fechar" data-acao="cancelar" title="${t("fechar")}"
                aria-label="${t("fechar")}">✕</button>
      </header>
      <div class="dialogo-botao-rapido__corpo">
        <label>
          <span>${t("label do botao")}</span>
          <input type="text" maxlength="8" data-campo="rotulo" />
        </label>
        <label>
          <span>${t("tecla de atalho do botao")}</span>
          <input type="text" readonly data-campo="atalho"
                 aria-describedby="ajuda-atalho" />
        </label>
        <p id="ajuda-atalho" class="dialogo-botao-rapido__ajuda">
          ${t("ajuda atalho do botao")}
        </p>
        <div class="dialogo-botao-rapido__conexoes">
          <span>${t("conexao")}</span>
          <div class="lista-de-marcacao" role="group" aria-label="${t("conexao")}"
               data-campo="conexoes"></div>
        </div>
        <p class="dialogo-botao-rapido__ajuda">${t("ajuda conexoes do botao")}</p>
        <label>
          <span>${t("posicao")}</span>
          <input type="number" min="1" max="${configuracoes.length + (ehNovo ? 1 : 0)}"
                 data-campo="posicao" />
        </label>
        <label class="dialogo-botao-rapido__conteudo">
          <span>${t("conteudo")}</span>
          <textarea rows="7" spellcheck="false" data-campo="conteudo"></textarea>
        </label>
        <label class="dialogo-botao-rapido__opcao">
          <span></span>
          <span><input type="checkbox" data-campo="executar-automaticamente" /> ${t("executar automaticamente")}</span>
        </label>
        <label class="dialogo-botao-rapido__opcao">
          <span></span>
          <span><input type="checkbox" data-campo="nao-inserir-na-janela" /> ${t("nao inserir na janela")}</span>
        </label>
        <p class="dialogo-botao-rapido__erro" role="alert" data-elemento="erro"></p>
      </div>
      <footer class="dialogo__rodape">
        ${ehNovo ? "" : `<button class="botao-excluir" data-acao="excluir">${t("excluir")}</button>`}
        <span class="status"></span>
        <span class="dialogo__botoes">
          <button data-acao="salvar">${t("salvar")}</button>
          <button data-acao="cancelar">${t("cancelar")}</button>
        </span>
      </footer>
    </section>`;

  const rotulo = fundo.querySelector<HTMLInputElement>('[data-campo="rotulo"]')!;
  const atalho = fundo.querySelector<HTMLInputElement>('[data-campo="atalho"]')!;
  const posicao = fundo.querySelector<HTMLInputElement>('[data-campo="posicao"]')!;
  const conteudo = fundo.querySelector<HTMLTextAreaElement>('[data-campo="conteudo"]')!;
  const executarAutomaticamente = fundo.querySelector<HTMLInputElement>(
    '[data-campo="executar-automaticamente"]',
  )!;
  const naoInserirNaJanela = fundo.querySelector<HTMLInputElement>(
    '[data-campo="nao-inserir-na-janela"]',
  )!;
  const erro = fundo.querySelector<HTMLElement>('[data-elemento="erro"]')!;
  const listaDeConexoes = fundo.querySelector<HTMLElement>('[data-campo="conexoes"]')!;
  const conexoesEscolhidas = montarListaDeConexoes(listaDeConexoes, conexoes, atual.conexoes);
  rotulo.value = atual.rotulo;
  definirAtalhoDoCampo(atalho, atual.atalho);
  posicao.value = String(ehNovo ? configuracoes.length + 1 : indice + 1);
  conteudo.value = atual.conteudo;
  executarAutomaticamente.checked = atual.executarAutomaticamente;
  naoInserirNaJanela.checked = atual.naoInserirNaJanela;

  // "Não inserir" executa direto, sem depender do texto estar selecionado no
  // editor: enquanto marcado, "Executar automaticamente" não tem efeito.
  const atualizarDependenciaDeExecucao = (): void => {
    executarAutomaticamente.disabled = naoInserirNaJanela.checked;
  };
  atualizarDependenciaDeExecucao();
  naoInserirNaJanela.addEventListener("change", atualizarDependenciaDeExecucao);

  let pararEsc = (): void => {};
  const fechar = (): void => {
    pararEsc();
    fundo.remove();
  };
  const escreverErro = (mensagem = ""): void => {
    erro.textContent = mensagem;
  };

  atalho.addEventListener("keydown", (evento) => {
    // Esc não é combinação gravável: ele fecha a janela, aqui como em qualquer
    // outra tela do Cururu. Deixá-lo seguir sem `preventDefault` nem
    // `stopPropagation` é o que permite ao `fecharComEsc` recebê-lo com o foco
    // dentro do campo.
    if (evento.key === "Escape") return;
    evento.preventDefault();
    evento.stopPropagation();
    if (evento.key === "Backspace" || evento.key === "Delete") {
      definirAtalhoDoCampo(atalho, "");
      escreverErro();
      return;
    }
    const novo = atalhoDoEvento(evento);
    if (!novo) return;
    const temModificador = evento.ctrlKey || evento.metaKey || evento.altKey || evento.shiftKey;
    const teclaDeFuncao = /^F(?:[1-9]|1[0-9]|2[0-4])$/.test(evento.key);
    if (!temModificador && !teclaDeFuncao) {
      escreverErro(t("use modificador no atalho"));
      return;
    }
    definirAtalhoDoCampo(atalho, novo);
    escreverErro();
  });

  const salvar = (): void => {
    const novoRotulo = rotulo.value.trim();
    if (!novoRotulo) {
      escreverErro(t("informe um label"));
      rotulo.focus();
      return;
    }
    if (Array.from(novoRotulo).length > 8) {
      escreverErro(t("label no maximo oito"));
      rotulo.focus();
      return;
    }
    const repetido = configuracoes.findIndex(
      (item, outroIndice) =>
        outroIndice !== indice &&
        Boolean(atalhoDoCampo(atalho)) &&
        item.atalho === atalhoDoCampo(atalho),
    );
    if (repetido >= 0) {
      escreverErro(t("atalho ja pertence", { n: repetido + 1 }));
      atalho.focus();
      return;
    }

    const limite = configuracoes.length + (ehNovo ? 1 : 0);
    const novaPosicao = Number(posicao.value);
    if (!Number.isInteger(novaPosicao) || novaPosicao < 1 || novaPosicao > limite) {
      escreverErro(t("posicao entre", { limite }));
      posicao.focus();
      return;
    }

    const novaConfiguracao = {
      rotulo: novoRotulo,
      atalho: atalhoDoCampo(atalho),
      conteudo: conteudo.value,
      executarAutomaticamente: executarAutomaticamente.checked,
      naoInserirNaJanela: naoInserirNaJanela.checked,
      conexoes: conexoesEscolhidas(),
    };
    const origem = ehNovo ? configuracoes.length : indice;
    if (ehNovo) configuracoes.push(novaConfiguracao);
    else configuracoes[indice] = novaConfiguracao;

    const [movido] = configuracoes.splice(origem, 1);
    configuracoes.splice(novaPosicao - 1, 0, movido);
    persistirConfiguracoes();
    fechar();
  };

  const excluir = (): void => {
    if (indice === null) return;
    void confirmar({
      titulo: t("excluir botao titulo"),
      mensagem: t("excluir botao mensagem", { rotulo: atual.rotulo }),
    }).then((confirmado) => {
      if (!confirmado) return;
      configuracoes.splice(indice, 1);
      persistirConfiguracoes();
      fechar();
    });
  };

  fundo.addEventListener("click", (evento) => {
    const acao = (evento.target as HTMLElement).closest<HTMLElement>("[data-acao]")?.dataset
      .acao;
    if (acao === "cancelar") fechar();
    else if (acao === "salvar") salvar();
    else if (acao === "excluir") excluir();
  });
  fundo.addEventListener("keydown", (evento) => {
    if (evento.key === "Enter" && evento.ctrlKey) salvar();
  });
  pararEsc = fecharComEsc(fundo, fechar);

  document.body.append(fundo);
  rotulo.select();
}
