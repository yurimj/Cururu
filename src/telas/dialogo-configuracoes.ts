/**
 * Tela de Configurações, aberta pela opção do mesmo nome no menu principal.
 *
 * Reúne o que antes eram três entradas soltas do menu — Teclas de Atalho,
 * Language e Tema —, cada uma com o seu jeito de aplicar a escolha: as duas
 * últimas valiam no clique do submenu, a primeira tinha tela própria com botão
 * de salvar. Aqui as três seguem a mesma regra, que é a de qualquer formulário
 * do Cururu: **o que se mexe é rascunho até o Salvar**, e sair pelo Esc, pelo ✕
 * ou pelo Cancelar não deixa nada gravado.
 *
 * A seção **Auto-Complete** guarda o que o editor oferece enquanto se digita
 * (requisito 43). O que se escolhe ali não muda pixel nenhum: viaja com o
 * pedido de sugestões, e por isso a gravação também é no Salvar — trocar de
 * opção no meio da digitação daria duas respostas diferentes para a mesma
 * palavra.
 *
 * A seção **Geral** guarda também quantos objetos a árvore lista por vez
 * (requisito 2). É um número digitado, e não uma escolha de lista, então o
 * Salvar o confere antes de gravar, como faz com os atalhos.
 */

import "./dialogo-configuracoes.css";

import {
  DEFINICOES_DE_ATALHOS,
  atalhosPadrao,
  atalhoVisivel,
  obterAtalhos,
  substituirAtalhos,
  textoDoEvento,
  type ConfiguracaoDeAtalhos,
  type DefinicaoDeAtalho,
  type IdDeAtalho,
} from "../atalhos";
import { fecharComEsc, tornarJanela } from "../componentes/janela";
import { IDIOMAS, definirIdioma, idiomaAtual, t, type Idioma } from "../idioma";
import {
  definirOpcoesDeAutocomplete,
  opcoesDeAutocomplete,
  type EscopoDeTabelas,
  type OpcoesDeAutocomplete,
} from "../opcoes-autocomplete";
import {
  OBJETOS_POR_VEZ_MAXIMO,
  OBJETOS_POR_VEZ_MINIMO,
  OBJETOS_POR_VEZ_PADRAO,
  definirOpcoesDaArvore,
  objetosPorVezValido,
  opcoesDaArvore,
} from "../opcoes-arvore";
import { TEMAS, definirTema, temaAtual, type Tema } from "../tema";

/** As seções da tela, na ordem em que aparecem na lista da esquerda. */
const SECOES = ["geral", "atalhos", "auto-complete"] as const;
type Secao = (typeof SECOES)[number];

/** O rótulo de cada seção, resolvido na hora de montar a lista. */
const ROTULO_DA_SECAO: Record<Secao, () => string> = {
  geral: () => t("geral"),
  atalhos: () => t("teclas de atalho"),
  "auto-complete": () => t("auto-complete"),
};

/** Resolve quando a tela é fechada, por qualquer saída. */
export function abrirDialogoDeConfiguracoes(secaoInicial: Secao = "geral"): Promise<void> {
  return new Promise((resolver) => new DialogoDeConfiguracoes(resolver).abrir(secaoInicial));
}

class DialogoDeConfiguracoes {
  private readonly fundo = document.createElement("div");
  private readonly caixa = document.createElement("section");
  private readonly corpo = document.createElement("div");
  private readonly situacao = document.createElement("span");

  /** Rascunho de cada escolha; nada disso vale antes do Salvar. */
  private atalhos: ConfiguracaoDeAtalhos = obterAtalhos();
  private idioma: Idioma = idiomaAtual();
  private tema: Tema = temaAtual();
  private readonly autocomplete: OpcoesDeAutocomplete = opcoesDeAutocomplete();
  /** O número digitado fica no próprio campo até o Salvar conferir. */
  private readonly campoObjetosPorVez = document.createElement("input");

  /** Um campo por par `id:posição`, para reescrever quem perdeu a tecla. */
  private readonly entradas = new Map<string, HTMLInputElement>();

  private readonly linhasDeAtalho = document.createElement("tbody");
  private pararJanela: () => void = () => {};
  private pararEsc: () => void = () => {};

  constructor(private readonly resolver: () => void) {}

  abrir(secaoInicial: Secao): void {
    this.fundo.className = "dialogo-fundo";
    this.caixa.className = "dialogo dialogo-configuracoes";
    this.caixa.setAttribute("role", "dialog");
    this.caixa.setAttribute("aria-modal", "true");
    this.caixa.setAttribute("aria-labelledby", "titulo-configuracoes");

    const conteudo = document.createElement("div");
    conteudo.className = "configuracoes__conteudo";
    this.corpo.className = "configuracoes__corpo";
    this.corpo.append(this.montarGeral(), this.montarAtalhos(), this.montarAutoComplete());
    conteudo.append(this.montarSecoes(), this.corpo);

    this.caixa.append(this.montarBarra(), conteudo, this.montarRodape());
    this.fundo.append(this.caixa);
    document.body.append(this.fundo);

    this.preencherAtalhos(this.atalhos);
    this.mostrarSecao(secaoInicial);
    // Move pela barra e muda de tamanho: a tabela de atalhos é longa, e quem
    // confere a lista inteira ganha em esticar a tela.
    this.pararJanela = tornarJanela(this.caixa, { redimensionavel: true, larguraMinima: 620 });
    this.pararEsc = fecharComEsc(this.fundo, () => this.fechar());
  }

  // --- Moldura -------------------------------------------------------------

  private montarBarra(): HTMLElement {
    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.id = "titulo-configuracoes";
    titulo.textContent = t("configuracoes");
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar"));
    fechar.addEventListener("click", () => this.fechar());
    barra.append(titulo, fechar);
    return barra;
  }

  private montarSecoes(): HTMLElement {
    const lista = document.createElement("nav");
    lista.className = "configuracoes__secoes";
    lista.setAttribute("role", "tablist");
    lista.setAttribute("aria-label", t("configuracoes"));
    for (const secao of SECOES) {
      const botao = document.createElement("button");
      botao.type = "button";
      botao.setAttribute("role", "tab");
      botao.dataset.secao = secao;
      botao.setAttribute("aria-selected", "false");
      botao.textContent = ROTULO_DA_SECAO[secao]();
      botao.addEventListener("click", () => this.mostrarSecao(secao));
      lista.append(botao);
    }
    return lista;
  }

  private montarRodape(): HTMLElement {
    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    this.situacao.className = "status";
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    const salvar = document.createElement("button");
    salvar.className = "botao-principal";
    salvar.textContent = t("salvar");
    salvar.addEventListener("click", () => this.salvar());
    const cancelar = document.createElement("button");
    cancelar.textContent = t("cancelar");
    cancelar.addEventListener("click", () => this.fechar());
    botoes.append(salvar, cancelar);
    rodape.append(this.situacao, botoes);
    return rodape;
  }

  private mostrarSecao(secao: Secao): void {
    for (const botao of this.caixa.querySelectorAll<HTMLButtonElement>("[data-secao]")) {
      botao.setAttribute("aria-selected", String(botao.dataset.secao === secao));
    }
    for (const painel of this.corpo.querySelectorAll<HTMLElement>("[data-painel]")) {
      painel.hidden = painel.dataset.painel !== secao;
    }
  }

  // --- Geral ---------------------------------------------------------------

  private montarGeral(): HTMLElement {
    const painel = document.createElement("section");
    painel.dataset.painel = "geral";

    const grade = document.createElement("div");
    grade.className = "configuracoes__geral";
    // "Language" é a única linha que não se traduz (requisito 31): quem abriu o
    // Cururu num idioma que não lê precisa de uma palavra que reconheça.
    grade.append(
      ...this.montarEscolha(
        "configuracoes-idioma",
        t("idioma"),
        IDIOMAS.map((idioma) => ({ valor: idioma.codigo, rotulo: idioma.nome })),
        this.idioma,
        (valor) => (this.idioma = valor as Idioma),
      ),
      ...this.montarEscolha(
        "configuracoes-tema",
        t("tema"),
        TEMAS.map((tema) => ({ valor: tema.codigo, rotulo: t(tema.chave) })),
        this.tema,
        (valor) => (this.tema = valor as Tema),
      ),
      ...this.montarObjetosPorVez(),
    );

    painel.append(grade);
    return painel;
  }

  /**
   * Quantos objetos cada categoria da árvore traz por vez (requisito 2).
   *
   * Campo de número, e não lista: o valor certo depende do tamanho do schema
   * de cada um. A explicação vai na linha de baixo, sob o campo — o número
   * sozinho não diz o que conta.
   */
  private montarObjetosPorVez(): [HTMLLabelElement, HTMLInputElement, HTMLElement] {
    const campo = this.campoObjetosPorVez;
    campo.id = "configuracoes-objetos-por-vez";
    campo.type = "number";
    campo.min = String(OBJETOS_POR_VEZ_MINIMO);
    campo.max = String(OBJETOS_POR_VEZ_MAXIMO);
    campo.step = "1";
    campo.value = String(opcoesDaArvore().objetosPorVez);
    campo.addEventListener("input", () => this.limparSituacao());

    const rotulo = document.createElement("label");
    rotulo.htmlFor = campo.id;
    rotulo.textContent = t("objetos por vez");

    const ajuda = document.createElement("p");
    ajuda.id = "configuracoes-objetos-por-vez-ajuda";
    ajuda.className = "configuracoes__geral-ajuda";
    ajuda.textContent = t("objetos por vez ajuda", { padrao: OBJETOS_POR_VEZ_PADRAO });
    campo.setAttribute("aria-describedby", ajuda.id);
    return [rotulo, campo, ajuda];
  }

  /** Um par rótulo + lista da grade da seção Geral. */
  private montarEscolha(
    id: string,
    texto: string,
    opcoes: { valor: string; rotulo: string }[],
    escolhido: string,
    aoEscolher: (valor: string) => void,
  ): [HTMLLabelElement, HTMLSelectElement] {
    const rotulo = document.createElement("label");
    rotulo.htmlFor = id;
    rotulo.textContent = texto;
    const lista = document.createElement("select");
    lista.id = id;
    for (const opcao of opcoes) lista.append(new Option(opcao.rotulo, opcao.valor));
    lista.value = escolhido;
    lista.addEventListener("change", () => {
      aoEscolher(lista.value);
      this.limparSituacao();
    });
    return [rotulo, lista];
  }

  // --- Teclas de Atalho ----------------------------------------------------

  private montarAtalhos(): HTMLElement {
    const painel = document.createElement("section");
    painel.dataset.painel = "atalhos";
    painel.hidden = true;

    const linhaDeAjuda = document.createElement("div");
    linhaDeAjuda.className = "configuracoes__ajuda";
    const ajuda = document.createElement("p");
    ajuda.textContent = t("atalhos ajuda");
    const restaurar = document.createElement("button");
    restaurar.type = "button";
    restaurar.textContent = t("restaurar padrao");
    restaurar.addEventListener("click", () => {
      this.preencherAtalhos(atalhosPadrao());
      this.situacao.textContent = t("atalhos restaurados pendentes");
      this.situacao.classList.remove("status--erro");
    });
    linhaDeAjuda.append(ajuda, restaurar);

    const moldura = document.createElement("div");
    moldura.className = "atalhos__lista";
    const tabela = document.createElement("table");
    tabela.className = "atalhos__tabela";
    const cabecalho = document.createElement("thead");
    const linha = document.createElement("tr");
    for (const texto of [t("atalho"), t("atalho alternativo"), t("acao")]) {
      const celula = document.createElement("th");
      celula.textContent = texto;
      linha.append(celula);
    }
    cabecalho.append(linha);
    tabela.append(cabecalho, this.linhasDeAtalho);
    moldura.append(tabela);

    painel.append(linhaDeAjuda, moldura);
    return painel;
  }

  /**
   * O campo do rascunho que já usa a combinação, se algum além do que está
   * sendo editado — o dono atual, de quem a tecla vai trocar de lado.
   */
  private acharDono(
    atalho: string,
    id: IdDeAtalho,
    posicao: 0 | 1,
  ): { definicao: DefinicaoDeAtalho; posicao: 0 | 1 } | null {
    for (const definicao of DEFINICOES_DE_ATALHOS) {
      for (const pos of [0, 1] as const) {
        if (definicao.id === id && pos === posicao) continue;
        if (this.atalhos[definicao.id][pos] === atalho) return { definicao, posicao: pos };
      }
    }
    return null;
  }

  /**
   * Uma célula com o campo de uma posição do par. Backspace/Delete puro deixa o
   * campo vazio — uma ação pode ficar sem tecla, porque o usuário limpou ou
   * porque outra ação tomou a combinação dela.
   */
  private criarCampo(definicao: DefinicaoDeAtalho, posicao: 0 | 1): HTMLTableCellElement {
    const celula = document.createElement("td");
    const entrada = document.createElement("input");
    entrada.className = "atalhos__entrada";
    entrada.readOnly = true;
    entrada.value = atalhoVisivel(this.atalhos[definicao.id][posicao]);
    entrada.setAttribute(
      "aria-label",
      posicao === 1 ? `${t(definicao.acao)} — ${t("atalho alternativo")}` : t(definicao.acao),
    );
    entrada.addEventListener("keydown", (evento) => {
      // Esc não é combinação gravável: ele fecha a janela, aqui como em
      // qualquer outra tela do Cururu. Deixá-lo seguir sem `preventDefault` nem
      // `stopPropagation` é o que permite ao `fecharComEsc` recebê-lo mesmo com
      // o foco dentro do campo.
      if (evento.key === "Escape") return;
      evento.preventDefault();
      evento.stopPropagation();
      // Só Backspace/Delete puro limpa; com modificador (Ctrl+Backspace,
      // Alt+Delete…) é uma combinação a gravar, não um gesto de apagar.
      const apagar =
        evento.key === "Backspace" || evento.key === "Delete"
          ? !evento.ctrlKey && !evento.altKey && !evento.metaKey
          : false;
      if (apagar) {
        this.atalhos[definicao.id][posicao] = "";
        entrada.value = "";
        this.limparSituacao();
        return;
      }
      const novo = textoDoEvento(evento);
      if (!novo) return;
      const anterior = this.atalhos[definicao.id][posicao];
      const dono = this.acharDono(novo, definicao.id, posicao);
      // A tecla muda de dono: quem a tinha fica com a combinação que estava
      // neste campo — ou sem tecla, se o campo estava vazio.
      this.atalhos[definicao.id][posicao] = novo;
      entrada.value = atalhoVisivel(novo);
      if (dono) {
        this.atalhos[dono.definicao.id][dono.posicao] = anterior;
        const campoDoDono = this.entradas.get(`${dono.definicao.id}:${dono.posicao}`);
        if (campoDoDono) campoDoDono.value = atalhoVisivel(anterior);
        this.situacao.textContent = anterior
          ? t("atalho trocado", {
              atalho: atalhoVisivel(novo),
              acao: t(dono.definicao.acao),
              tecla: atalhoVisivel(anterior),
            })
          : t("atalho removido", {
              atalho: atalhoVisivel(novo),
              acao: t(dono.definicao.acao),
            });
      } else {
        this.situacao.textContent = "";
      }
      this.situacao.classList.remove("status--erro");
    });
    celula.append(entrada);
    this.entradas.set(`${definicao.id}:${posicao}`, entrada);
    return celula;
  }

  private preencherAtalhos(valores: ConfiguracaoDeAtalhos): void {
    this.atalhos = Object.fromEntries(
      Object.entries(valores).map(([id, par]) => [id, [...par]]),
    ) as ConfiguracaoDeAtalhos;
    this.entradas.clear();
    this.linhasDeAtalho.replaceChildren();
    for (const definicao of DEFINICOES_DE_ATALHOS) {
      const linha = document.createElement("tr");
      const acao = document.createElement("td");
      acao.textContent = t(definicao.acao);
      linha.append(this.criarCampo(definicao, 0), this.criarCampo(definicao, 1), acao);
      this.linhasDeAtalho.append(linha);
    }
  }

  // --- Auto-Complete -------------------------------------------------------

  private montarAutoComplete(): HTMLElement {
    const painel = document.createElement("section");
    painel.dataset.painel = "auto-complete";
    painel.hidden = true;

    painel.append(
      this.montarGrupoDeEscolha({
        nome: "autocomplete-tabelas",
        titulo: t("autocomplete tabelas"),
        ajuda: t("autocomplete tabelas ajuda"),
        nota: t("autocomplete tabelas nota"),
        escolhido: this.autocomplete.tabelas,
        opcoes: [
          {
            valor: "todas",
            rotulo: t("autocomplete tabelas todas"),
            ajuda: t("autocomplete tabelas todas ajuda"),
          },
          {
            valor: "filtradas",
            rotulo: t("autocomplete tabelas filtradas"),
            ajuda: t("autocomplete tabelas filtradas ajuda"),
          },
        ],
        aoEscolher: (valor) => (this.autocomplete.tabelas = valor as EscopoDeTabelas),
      }),
    );
    return painel;
  }

  /**
   * Um grupo de opções exclusivas, cada uma com a sua explicação embaixo.
   *
   * Escolha assim não cabe numa lista suspensa: o que separa "todas" de
   * "apenas as filtradas" não é o rótulo, é a consequência — e a consequência
   * precisa estar à vista na hora de escolher, não escondida atrás de um clique
   * que abre a lista.
   */
  private montarGrupoDeEscolha(opcoes: {
    nome: string;
    titulo: string;
    ajuda: string;
    nota?: string;
    escolhido: string;
    opcoes: { valor: string; rotulo: string; ajuda: string }[];
    aoEscolher: (valor: string) => void;
  }): HTMLElement {
    const grupo = document.createElement("fieldset");
    grupo.className = "configuracoes__grupo";

    const titulo = document.createElement("legend");
    titulo.textContent = opcoes.titulo;
    const ajuda = document.createElement("p");
    ajuda.className = "configuracoes__grupo-ajuda";
    ajuda.textContent = opcoes.ajuda;
    grupo.append(titulo, ajuda);

    for (const item of opcoes.opcoes) {
      const linha = document.createElement("label");
      linha.className = "configuracoes__opcao";

      const marca = document.createElement("input");
      marca.type = "radio";
      marca.name = opcoes.nome;
      marca.value = item.valor;
      marca.checked = item.valor === opcoes.escolhido;
      marca.addEventListener("change", () => {
        if (!marca.checked) return;
        opcoes.aoEscolher(item.valor);
        this.limparSituacao();
      });

      const texto = document.createElement("span");
      texto.className = "configuracoes__opcao-texto";
      const rotulo = document.createElement("span");
      rotulo.className = "configuracoes__opcao-rotulo";
      rotulo.textContent = item.rotulo;
      const detalhe = document.createElement("span");
      detalhe.className = "configuracoes__opcao-ajuda";
      detalhe.textContent = item.ajuda;
      texto.append(rotulo, detalhe);

      linha.append(marca, texto);
      grupo.append(linha);
    }

    if (opcoes.nota) {
      const nota = document.createElement("p");
      nota.className = "configuracoes__nota";
      nota.textContent = opcoes.nota;
      grupo.append(nota);
    }
    return grupo;
  }

  // --- Saídas --------------------------------------------------------------

  private limparSituacao(): void {
    this.situacao.textContent = "";
    this.situacao.classList.remove("status--erro");
  }

  /**
   * Grava as escolhas. A validação é dos atalhos e do número de objetos da
   * árvore: idioma e tema saem de listas fechadas, onde não há valor inválido
   * a recusar.
   */
  private salvar(): void {
    const usados = new Set<string>();
    for (const definicao of DEFINICOES_DE_ATALHOS) {
      for (const valor of this.atalhos[definicao.id]) {
        if (!valor) continue; // "" é o atalho alternativo em aberto.
        if (usados.has(valor)) {
          // A mensagem fala de uma tecla, e a tecla está na outra seção: sem
          // trazer a lista à frente, o erro apareceria no rodapé sem nada por
          // perto que o explicasse.
          this.mostrarSecao("atalhos");
          this.situacao.textContent = t("atalho repetido", { atalho: valor });
          this.situacao.classList.add("status--erro");
          return;
        }
        usados.add(valor);
      }
    }
    const objetosPorVez = objetosPorVezValido(Number(this.campoObjetosPorVez.value));
    if (objetosPorVez === null) {
      this.mostrarSecao("geral");
      this.situacao.textContent = t("objetos por vez invalido", {
        minimo: OBJETOS_POR_VEZ_MINIMO,
        maximo: OBJETOS_POR_VEZ_MAXIMO,
      });
      this.situacao.classList.add("status--erro");
      this.campoObjetosPorVez.focus();
      return;
    }
    substituirAtalhos(this.atalhos);
    definirIdioma(this.idioma);
    definirTema(this.tema);
    definirOpcoesDeAutocomplete(this.autocomplete);
    definirOpcoesDaArvore({ objetosPorVez });
    this.fechar();
  }

  private fechar(): void {
    this.pararEsc();
    this.pararJanela();
    this.fundo.remove();
    this.resolver();
  }
}
