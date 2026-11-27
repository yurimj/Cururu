import * as api from "./api";
import { obterArquivosRecentes, registrarArquivoRecente } from "./arquivos-recentes";
import {
  IDIOMAS,
  definirIdioma,
  existeIdiomaEscolhido,
  idiomaAtual,
  idiomaValido,
  observarIdioma,
  t,
} from "./idioma";
import {
  eventoCorrespondeAoAtalho,
  obterAtalhos,
  substituirAtalhos,
  sufixoDoAtalho,
} from "./atalhos";
import { criarDivisor } from "./componentes/divisor";
import { comIndicadorDeAbertura } from "./componentes/indicador-de-abertura";
import { abrirMenuDeContextoNoElemento } from "./componentes/menu-contexto";
import { AreaDeAbas } from "./telas/abas";
import {
  ArvoreDeObjetos,
  type CategoriaSelecionada,
  type ContextoDeConexao,
  type LigacoesDaArvore,
} from "./telas/arvore-objetos";
import { abrirDialogoConexao, type ConexaoAberta } from "./telas/dialogo-conexao";
import { PainelMongo } from "./telas/painel-mongo";
import { abrirDialogoDeConfiguracoes } from "./telas/dialogo-configuracoes";
import { abrirDialogoDeAtualizacao } from "./telas/dialogo-atualizacao";
import { abrirDialogoSobre } from "./telas/dialogo-sobre";
import { abrirDialogoDeRecuperacao } from "./telas/dialogo-recuperacao";
import { abrirDialogoDeVersoes } from "./telas/dialogo-versoes";
import { PainelDeConsulta } from "./telas/painel-consulta";
import { montarBarraDeTitulo } from "./telas/barra-titulo";
import { PainelDeObjeto, type AbaDoObjeto } from "./telas/painel-objeto";
import {
  obterConfiguracoesDosBotoes,
  substituirConfiguracoesDosBotoes,
} from "./telas/botoes-rapidos";
import type { PerfilConexao } from "./tipos";

const aplicacao = document.querySelector<HTMLDivElement>("#aplicacao")!;

let painelDeConexoes: HTMLElement;
let areaDeTrabalho: HTMLElement;
let barraDeStatus: HTMLElement;
let botaoDeFiltro: HTMLButtonElement;
let botaoDeConsulta: HTMLButtonElement;
let abas: AreaDeAbas;

/** Um visualizador por tabela e sessão; as ações apenas trocam sua aba interna. */
const paineisDeObjeto = new Map<string, PainelDeObjeto>();

/** Categoria selecionada na árvore, sobre a qual o botão de filtro age. */
let categoriaSelecionada: CategoriaSelecionada | null = null;

/**
 * Banco de dados sob o ponteiro do mouse, quando há um (SQL Server).
 *
 * O atalho de nova consulta age sobre ele antes de agir sobre a seleção:
 * apontar para um banco e pedir uma consulta é o gesto direto, e ele não deve
 * exigir um clique antes.
 */
let bancoSobOMouse: ContextoDeConexao | null = null;

/** Conexão do nó selecionado, sobre a qual o botão de nova consulta age. */
let conexaoSelecionada: ContextoDeConexao | null = null;

/** Aba de consulta visível, sobre a qual agem Salvar/Carregar Consulta SQL. */
let consultaAtiva: {
  painel: PainelDeConsulta;
  contexto: ContextoDeConexao;
  /** Nome oferecido no "salvar como": o da aba, ou o do arquivo carregado. */
  nomeSugerido: string;
} | null = null;

/** Numeração das consultas novas: "Consulta 1", "Consulta 2"… */
let consultasCriadas = 0;

/** Contador que distingue as cópias de recuperação abertas na mesma execução. */
let copiasCriadas = 0;

/**
 * Arquivos que o Windows mandou abrir e que esperam uma conexão para virar aba
 * (requisitos 12 e 26).
 */
const arquivosEsperando: string[] = [];

async function iniciar(): Promise<void> {
  // Uma falha no diagnóstico não pode deixar a janela em branco: o resto da
  // aplicação continua utilizável, e o problema aparece como aviso.
  let situacao: api.SituacaoInstantClient;
  try {
    situacao = await api.diagnosticoInstantClient();
  } catch (erro) {
    situacao = {
      diretorio: null,
      problema: api.mensagemDeErro(erro),
      procuradoEm: [],
    };
  }

  // Antes de desenhar qualquer coisa: quem instalou escolhendo inglês precisa
  // ver a primeira tela em inglês, e não a barra de título em português por um
  // instante (requisito 31).
  await aplicarIdiomaDoInstalador();

  aplicacao.replaceChildren(montarBarraDeTitulo());
  if (situacao.problema) {
    aplicacao.append(montarAviso(situacao));
  }
  aplicacao.append(montarAreaPrincipal(), montarBarraDeStatus());
  ligarAtalhosDaJanela();
  mostrarBoasVindas();
  await recarregarConexoes();
  // Duplo clique num .sql, "Abrir com" (requisito 12): o arquivo pode ter vindo
  // na linha de comando desta janela, ou chegar depois, de outro processo.
  void receberArquivosDoWindows();

  // A procura por versão nova fica para o fim e não é esperada: a janela já
  // está utilizável, e uma rede lenta não pode atrasar a subida do aplicativo.
  void procurarAtualizacao(true);
}

/**
 * Na primeira abertura, adota o idioma escolhido no instalador.
 *
 * Falhar aqui não impede nada: fora da janela do Tauri a ponte não existe, e
 * numa pasta portátil não há chave gravada. Nos dois casos vale o palpite pelo
 * idioma da máquina, que é o comportamento de sempre.
 */
async function aplicarIdiomaDoInstalador(): Promise<void> {
  if (existeIdiomaEscolhido()) return;
  try {
    const escolhido = idiomaValido(await api.idiomaDoInstalador());
    if (escolhido) definirIdioma(escolhido);
  } catch {
    // Sem instalador, sem registro ou sem ponte: segue o idioma da máquina.
  }
}

/**
 * Procura uma versão nova no repositório de releases e mostra o resultado.
 *
 * `silencioso` é o modo da subida: só abre a tela quando há novidade de
 * verdade. Abrir o Cururu sem internet — ou atrás de um proxy que barrou a
 * chamada — não pode virar um aviso de erro na cara de quem só quer trabalhar;
 * a falha fica no log, que é onde ela é procurada.
 */
async function procurarAtualizacao(silencioso = false): Promise<void> {
  if (!silencioso) escreverStatus(t("procurando atualizacoes"));

  let atualizacao: api.Atualizacao | null;
  try {
    atualizacao = await api.verificarAtualizacao();
  } catch (erro) {
    if (!silencioso) escreverStatus(api.mensagemDeErro(erro), true);
    return;
  }

  if (!silencioso) escreverStatus("");
  // Já estar atualizado só merece tela quando alguém perguntou.
  if (!atualizacao && silencioso) return;

  await abrirDialogoDeAtualizacao(atualizacao, atualizacao?.versaoAtual ?? (await numeroDaVersao()));
}

/** Só o número da versão em execução, sem a data de compilação junto. */
async function numeroDaVersao(): Promise<string> {
  try {
    return (await api.versaoEmExecucao()).split("·")[0].trim();
  } catch {
    return "";
  }
}

function montarAviso(situacao: api.SituacaoInstantClient): HTMLElement {
  const aviso = document.createElement("p");
  aviso.className = situacao.diretorio ? "aviso" : "aviso aviso--erro";
  aviso.textContent = situacao.problema ?? "";

  if (!situacao.diretorio && situacao.procuradoEm.length > 0) {
    const detalhe = document.createElement("ul");
    detalhe.className = "aviso__detalhe";
    for (const caminho of situacao.procuradoEm) {
      const item = document.createElement("li");
      item.textContent = caminho;
      detalhe.append(item);
    }
    aviso.append(detalhe);
  }
  return aviso;
}

function montarAreaPrincipal(): HTMLElement {
  const area = document.createElement("div");
  area.className = "area-principal";

  const lateral = document.createElement("aside");
  lateral.className = "painel-lateral";

  areaDeTrabalho = document.createElement("main");
  areaDeTrabalho.className = "area-trabalho";
  abas = new AreaDeAbas(mostrarBoasVindas);

  // O mínimo do painel acompanha o `min-width` da folha de estilo; abaixo disso
  // a árvore de objetos deixa de ser legível.
  const divisor = criarDivisor({
    orientacao: "vertical",
    redimensionado: lateral,
    vizinho: areaDeTrabalho,
    minimo: 180,
    minimoDoVizinho: 320,
  });

  lateral.append(montarTituloDaLateral(lateral, divisor), montarBarraDeFerramentas());

  painelDeConexoes = document.createElement("div");
  painelDeConexoes.className = "painel-lateral__conteudo";
  lateral.append(painelDeConexoes);

  area.append(lateral, divisor, areaDeTrabalho);
  return area;
}

/**
 * Título do painel de conexões, com o botão que minimiza e restaura.
 *
 * Minimizado sobra só a faixa com o botão: o painel para de ocupar largura mas
 * continua alcançável, sem precisar de um atalho escondido para trazê-lo de
 * volta.
 */
function montarTituloDaLateral(lateral: HTMLElement, divisor: HTMLElement): HTMLElement {
  const titulo = document.createElement("div");
  titulo.className = "painel-lateral__titulo";

  const alternar = document.createElement("button");
  alternar.className = "botao-simbolo painel-lateral__alternar";

  let minimizado = false;
  // A largura escolhida no arrasto fica guardada aqui: minimizar e restaurar
  // não pode devolver o painel ao tamanho padrão e descartar a escolha.
  let flexGuardado = "";

  const aplicar = (): void => {
    if (minimizado) {
      flexGuardado = lateral.style.flex;
      lateral.style.flex = "";
    } else {
      lateral.style.flex = flexGuardado;
    }
    lateral.classList.toggle("painel-lateral--minimizado", minimizado);
    // Sem painel para dimensionar, o divisor não tem o que fazer.
    divisor.hidden = minimizado;
    alternar.textContent = minimizado ? "»" : "«";
    alternar.title = t(minimizado ? "expandir conexoes" : "minimizar conexoes");
    alternar.setAttribute("aria-label", alternar.title);
    alternar.setAttribute("aria-expanded", String(!minimizado));
  };

  alternar.addEventListener("click", () => {
    minimizado = !minimizado;
    aplicar();
  });
  // Fica montado desde a abertura da janela, então precisa ser reescrito quando
  // o idioma muda — o que se monta na hora do clique não precisa.
  observarIdioma(aplicar);

  // O título deixou de ter a palavra "Conexões": no lugar dela fica o botão de
  // menu, e o de minimizar continua encostado na direita pela sobra do flex.
  titulo.append(botaoDeMenu(), alternar);
  return titulo;
}

/** Menu geral do aplicativo no título do painel de conexões. */
function botaoDeMenu(): HTMLButtonElement {
  const botao = document.createElement("button");
  botao.className = "botao-simbolo painel-lateral__menu";
  botao.append(hamburguer());
  observarIdioma(() => {
    botao.title = t("menu");
    botao.setAttribute("aria-label", t("menu"));
  });

  botao.addEventListener("click", (evento) => {
    evento.stopPropagation();
    const recentes = obterArquivosRecentes();
    abrirMenuDeContextoNoElemento(botao, [
      {
        rotulo: t("salvar consulta sql"),
        // Sem aba de consulta na tela não há o que salvar.
        desabilitado: consultaAtiva === null,
        acao: () => void salvarConsulta(),
      },
      {
        rotulo: t("salvar consulta sql como"),
        desabilitado: consultaAtiva === null,
        acao: () => void salvarConsulta(true),
      },
      {
        rotulo: t("abrir consulta sql"),
        // A aba aberta nasce amarrada a uma conexão (requisito 26).
        desabilitado: conexaoParaCarregar() === null,
        acao: () => void carregarConsulta(),
      },
      {
        rotulo: t("abrir recente"),
        desabilitado: recentes.length === 0,
        subitens: recentes.map((caminho) => ({
          rotulo: caminho,
          desabilitado: conexaoParaCarregar() === null,
          acao: () => {
            const contexto = conexaoParaCarregar();
            if (contexto) void abrirArquivo(contexto, caminho);
          },
        })),
      },
      {
        rotulo: t("recuperar consultas"),
        // Mesma exigência do Carregar: a consulta recuperada volta numa aba, e
        // aba sem conexão não existe (requisito 26). Se há cópia guardada só se
        // descobre indo ao disco, então quem avisa que não há é a própria tela.
        desabilitado: conexaoParaCarregar() === null,
        acao: () => void recuperarConsulta(),
      },
      { rotulo: t("salvar backup"), separadorAntes: true, acao: () => void salvarBackup() },
      { rotulo: t("carregar backup"), acao: () => void carregarBackup() },
      {
        // Teclas de Atalho e Tema deixaram o menu quando viraram seções desta
        // tela: duas portas para a mesma escolha são duas coisas a manter em
        // dia. Language é a exceção deliberada, logo abaixo.
        rotulo: t("configuracoes"),
        separadorAntes: true,
        acao: () => void abrirDialogoDeConfiguracoes(),
      },
      {
        // **Language fica nos dois lugares, de propósito.** Quem abriu o
        // Cururu num idioma que não lê não sabe o que quer dizer
        // "Configurações", "Settings" ou "Configuración" — e a porta de saída
        // não pode estar atrás de uma palavra que ele precisa entender para
        // achar. Aqui a palavra é sempre a mesma, e a troca vale no clique.
        rotulo: t("idioma"),
        subitens: IDIOMAS.map((idioma) => ({
          // O idioma em uso aparece marcado: o menu é a única tela onde dá
          // para conferir qual está valendo sem abrir formulário.
          rotulo: `${idioma.codigo === idiomaAtual() ? "• " : "  "}${idioma.nome}`,
          acao: () => definirIdioma(idioma.codigo),
        })),
      },
      {
        rotulo: t("versoes"),
        separadorAntes: true,
        acao: () => void abrirDialogoDeVersoes(),
      },
      { rotulo: t("procurar atualizacoes"), acao: () => void procurarAtualizacao() },
      { rotulo: t("sobre"), acao: () => void abrirDialogoSobre() },
    ]);
  });
  return botao;
}

/**
 * Atalhos da janela inteira para arquivos e abas.
 *
 * Ficam no documento, e não no editor, porque valem mesmo com o foco na árvore
 * ou no grid. O `preventDefault` é obrigatório: sem ele o WebView2 atende
 * primeiro e abre o "salvar página" e o "abrir arquivo" dele.
 */
function ligarAtalhosDaJanela(): void {
  // Recarregar a página destrói as abas em memória. Impede a ação nativa sem
  // interromper os handlers da árvore e do editor (onde F5 executa script).
  document.addEventListener("keydown", (evento) => {
    if (evento.key === "F5" ||
        ((evento.ctrlKey || evento.metaKey) && evento.key.toLowerCase() === "r" && !evento.altKey)) {
      evento.preventDefault();
    }
  });

  document.addEventListener("keydown", (evento) => {
    if (evento.ctrlKey && !evento.altKey && !evento.shiftKey && (evento.key === "PageUp" || evento.key === "PageDown")) {
      evento.preventDefault();
      if (evento.repeat || document.querySelector(".dialogo-fundo")) return;
      abas.selecionarAdjacente(evento.key === "PageUp" ? -1 : 1);
      return;
    }

    const acao = (["abrir-consulta", "salvar-consulta", "nova-consulta", "fechar-aba"] as const).find(
      (id) => eventoCorrespondeAoAtalho(evento, id),
    );
    if (!acao) return;
    // Barra o comportamento do WebView2 mesmo quando não há conexão ou aba:
    // Ctrl+T não pode abrir uma aba de navegador, nem Ctrl+W fechar a janela.
    evento.preventDefault();
    if (evento.repeat) return;
    // Com um diálogo aberto, a janela de trás não responde a atalho.
    if (document.querySelector(".dialogo-fundo")) return;

    switch (acao) {
      case "salvar-consulta":
        if (consultaAtiva) void salvarConsulta();
        break;
      case "abrir-consulta":
        if (conexaoParaCarregar()) void carregarConsulta();
        break;
      case "nova-consulta": {
        // Não conecta sozinho nem usa a conexão da aba: precisa estar aberta e
        // selecionada na árvore, exatamente como o botão Nova Consulta SQL.
        // O banco sob o mouse vem antes da seleção — apontar para um banco e
        // pedir a consulta abre nele, sem exigir um clique antes. Quem dispara
        // isto é a tecla configurada para "nova-consulta", seja ela qual for.
        const alvo = bancoSobOMouse ?? conexaoSelecionada;
        if (alvo) novaConsulta(alvo);
        break;
      }
      case "fechar-aba":
        abas.fecharAtual();
        break;
    }
  });
}

/**
 * Conexão que recebe uma consulta carregada do disco.
 *
 * A da árvore vem primeiro; na falta dela serve a da aba visível, senão abrir um
 * arquivo passaria a exigir voltar à árvore e clicar na conexão de novo.
 */
function conexaoParaCarregar(): ContextoDeConexao | null {
  if (conexaoSelecionada && conexaoSelecionada.perfil.tipoBanco !== "mongodb") return conexaoSelecionada;
  return consultaAtiva?.contexto ?? null;
}

/**
 * Requisito 12: salva o texto da aba visível em um arquivo `.sql`.
 *
 * O caminho só é perguntado quando a aba ainda não tem arquivo — porque nunca
 * foi salva nem veio de um. Depois disso o Salvar grava direto por cima, como
 * em qualquer editor; perguntar de novo a cada vez seria o incômodo. Quem quer
 * escolher outro lugar usa o **Salvar Como**, que é o `perguntarOnde`.
 */
async function salvarConsulta(perguntarOnde = false): Promise<void> {
  const ativa = consultaAtiva;
  if (!ativa) return;
  try {
    let caminho = perguntarOnde ? null : ativa.painel.arquivo;
    if (!caminho) {
      caminho = await api.escolherArquivoParaSalvar(
        // Salvando como, o diálogo abre já na pasta e no nome de agora.
        ativa.painel.arquivo ?? ativa.nomeSugerido,
        t("arquivo script sql"),
        "sql",
      );
      if (!caminho) return;
    }
    await api.gravarTexto(caminho, ativa.painel.texto);
    ativa.painel.amarrarAoArquivo(caminho);
    escreverStatus(t("consulta salva em", { caminho }));
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
  }
}

/** Requisito 12: abre um arquivo `.sql` numa aba nova. */
async function carregarConsulta(): Promise<void> {
  const contexto = conexaoParaCarregar();
  if (!contexto) return;
  let caminho: string | null;
  try {
    caminho = await api.escolherArquivoDeConsulta();
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
    return;
  }
  if (caminho) await abrirArquivo(contexto, caminho);
}

/** Lê um arquivo do disco e o abre numa aba nova da conexão (requisito 12). */
async function abrirArquivo(contexto: ContextoDeConexao, caminho: string): Promise<void> {
  const nome = nomeDoArquivo(caminho);
  try {
    const abriu = await comIndicadorDeAbertura(nome, async () => {
      const texto = await api.lerTexto(caminho);
      // Abrir o mesmo caminho de novo volta para a aba dele.
      return abrirAbaDeConsulta(contexto, `arquivo:${caminho}`, nome, texto, caminho);
    });
    if (abriu) {
      registrarArquivoRecente(caminho);
      escreverStatus(t("arquivo aberto em", { nome, conexao: contexto.perfil.nome }));
    }
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
  }
}

/**
 * Passa a ouvir os arquivos mandados pelo Windows e abre os que já chegaram.
 *
 * A escuta vem antes da primeira leitura: um arquivo que chegue entre as duas
 * é visto pelas duas, e a segunda só encontra a fila vazia.
 */
async function receberArquivosDoWindows(): Promise<void> {
  try {
    await api.aoReceberArquivos(() => void abrirArquivosRecebidos());
  } catch {
    // Fora da janela do Tauri não há Windows mandando arquivo nenhum.
    return;
  }
  await abrirArquivosRecebidos();
}

/**
 * Abre o que o Windows mandou. Sem conexão para amarrar a aba (requisito 26),
 * o arquivo espera, e a barra de status diz o que falta.
 */
async function abrirArquivosRecebidos(): Promise<void> {
  let caminhos: string[];
  try {
    caminhos = await api.arquivosRecebidos();
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
    return;
  }
  if (caminhos.length === 0) return;
  for (const caminho of caminhos) {
    if (!arquivosEsperando.includes(caminho)) arquivosEsperando.push(caminho);
  }

  const contexto = conexaoParaCarregar();
  if (contexto) {
    abrirArquivosEsperando(contexto);
  } else if (arquivosEsperando.length === 1) {
    escreverStatus(
      t("selecione conexao para abrir", { nome: nomeDoArquivo(arquivosEsperando[0]) }),
    );
  } else {
    escreverStatus(t("selecione conexao para abrir n", { n: arquivosEsperando.length }));
  }
}

/**
 * Abre na conexão os arquivos que esperavam uma. Devolve se havia algum: ao
 * conectar, eles tomam o lugar da aba vazia do requisito 3.
 */
function abrirArquivosEsperando(contexto: ContextoDeConexao): boolean {
  if (arquivosEsperando.length === 0) return false;
  const caminhos = arquivosEsperando.splice(0);
  // Um de cada vez, para as abas ficarem na ordem em que os arquivos vieram.
  void (async () => {
    for (const caminho of caminhos) await abrirArquivo(contexto, caminho);
  })();
  return true;
}

/** Requisito 12: traz de volta uma cópia de recuperação, numa aba nova. */
async function recuperarConsulta(): Promise<void> {
  const contexto = conexaoParaCarregar();
  if (!contexto) return;

  const escolha = await abrirDialogoDeRecuperacao();
  if (!escolha) return;

  const { texto, entrada } = escolha;
  // A aba recuperada nasce sem origem mesmo quando a cópia veio de um arquivo:
  // ela é uma segunda via, e gravar por cima do arquivo original tem de
  // continuar sendo um ato deliberado, pelo Salvar.
  const abriu = await comIndicadorDeAbertura(entrada.titulo, () =>
    abrirAbaDeConsulta(contexto, `recuperada:${entrada.id}`, entrada.titulo, texto),
  );
  if (abriu) {
    escreverStatus(t("consulta recuperada", { titulo: entrada.titulo, quando: entrada.quando }));
  }
}

async function salvarBackup(): Promise<void> {
  try {
    const data = new Date().toISOString().slice(0, 10);
    const caminho = await api.escolherArquivoParaSalvar(
      `cururu-backup-${data}.json`,
      t("arquivo backup do cururu"),
      "json",
    );
    if (!caminho) return;
    await api.exportarBackup(caminho, obterConfiguracoesDosBotoes(), obterAtalhos());
    escreverStatus(t("backup salvo em", { caminho }));
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
  }
}

async function carregarBackup(): Promise<void> {
  try {
    const caminho = await api.escolherArquivoDeBackup();
    if (!caminho) return;
    if (
      !window.confirm(t("carregar backup substitui"))
    ) {
      return;
    }
    const conteudo = await api.importarBackup(caminho);
    substituirConfiguracoesDosBotoes(conteudo.botoes);
    // Backups anteriores não têm este campo; nesse caso preservamos a configuração atual.
    substituirAtalhos(Object.keys(conteudo.atalhos).length > 0 ? conteudo.atalhos : obterAtalhos());
    await recarregarConexoes();
    escreverStatus(t("backup carregado"));
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
  }
}

/** A barra logo abaixo do título do painel. */
function montarBarraDeFerramentas(): HTMLElement {
  const barra = document.createElement("div");
  barra.className = "barra-ferramentas";

  const nova = document.createElement("button");
  nova.className = "botao-simbolo";
  observarIdioma(() => {
    nova.title = t("nova conexao");
    nova.setAttribute("aria-label", t("nova conexao"));
  });
  nova.append(plugueMachoDeConexao());
  nova.addEventListener("click", () => void novaConexaoClicada());

  botaoDeFiltro = document.createElement("button");
  botaoDeFiltro.className = "botao-simbolo";
  botaoDeFiltro.setAttribute("aria-label", t("filtrar"));
  botaoDeFiltro.append(funil());
  botaoDeFiltro.disabled = true;
  botaoDeFiltro.addEventListener("click", () => void categoriaSelecionada?.filtrar());

  botaoDeConsulta = document.createElement("button");
  botaoDeConsulta.className = "botao-simbolo";
  botaoDeConsulta.append(folhaDeConsulta());

  // As duas dicas dependem do que está selecionado, então quem as escreve é o
  // mesmo código que responde à seleção — e ele também roda na troca de idioma.
  observarIdioma(() => {
    botaoDeFiltro.setAttribute("aria-label", t("filtrar"));
    ligacoes.aoSelecionarCategoria(categoriaSelecionada);
    ligacoes.aoSelecionarConexao(conexaoSelecionada);
  });
  botaoDeConsulta.disabled = true;
  botaoDeConsulta.addEventListener("click", () => {
    if (conexaoSelecionada) novaConsulta(conexaoSelecionada);
  });

  barra.append(nova, botaoDeFiltro, botaoDeConsulta);
  return barra;
}

function montarBarraDeStatus(): HTMLElement {
  barraDeStatus = document.createElement("footer");
  barraDeStatus.className = "barra-status";
  return barraDeStatus;
}

function escreverStatus(texto: string, erro = false): void {
  barraDeStatus.textContent = texto;
  barraDeStatus.className = erro ? "barra-status barra-status--erro" : "barra-status";
}

function mostrarBoasVindas(): void {
  const vazio = document.createElement("div");
  vazio.className = "vazio";
  const imagem = document.createElement("img");
  imagem.src = "/cururu.svg";
  imagem.alt = "";
  const texto = document.createElement("p");
  observarIdioma(() => { texto.textContent = t("selecione uma conexao para comecar"); });
  vazio.append(imagem, texto);
  areaDeTrabalho.replaceChildren(vazio);
}

/** Garante que a área de abas está na tela, no lugar das boas-vindas. */
function mostrarAbas(): void {
  if (!areaDeTrabalho.contains(abas.elemento)) {
    areaDeTrabalho.replaceChildren(abas.elemento);
  }
}

// --- Abas de consulta e de fonte -------------------------------------------

/** Requisito 3: nova aba de consulta SQL vazia. */
function novaConsulta(contexto: ContextoDeConexao): void {
  consultasCriadas += 1;
  if (contexto.perfil.tipoBanco === "mongodb") {
    abrirAbaMongo(contexto);
    return;
  }
  const titulo = t("consulta n", { n: consultasCriadas });
  void comIndicadorDeAbertura(titulo, () =>
    abrirAbaDeConsulta(contexto, `consulta-${consultasCriadas}`, titulo, ""),
  );
}

function abrirAbaMongo(contexto: ContextoDeConexao): void {
  const id = `${contexto.sessaoId}:mongo:${consultasCriadas}`;
  const painel = new PainelMongo(contexto);
  mostrarAbas();
  abas.abrir({
    id,
    titulo: contexto.colecao ?? t("consulta n", { n: consultasCriadas }),
    cor: contexto.perfil.cor,
    conexao: contexto.perfil.nome,
    conteudo: painel.elemento,
    aoMostrar: () => {
      consultaAtiva = null;
      painel.focar();
    },
    aoFechar: () => painel.encerrar(),
  });
}

/**
 * Fonte de um objeto, aberto no mesmo painel da consulta: já vem com destaque
 * de sintaxe e com o botão de compilar (requisito 13).
 */
function mostrarFonte(titulo: string, texto: string, contexto: ContextoDeConexao): Promise<boolean> {
  return abrirAbaDeConsulta(contexto, `fonte-${titulo}`, titulo, texto);
}

/** Abre uma tabela ou view uma vez e alterna entre Dados, DDL e Estrutura. */
function mostrarObjeto(
  nome: string,
  categoria: "tabelas" | "views",
  abaInicial: AbaDoObjeto,
  contexto: ContextoDeConexao,
): void {
  // O banco entra no identificador: duas tabelas de mesmo nome em bancos
  // diferentes do SQL Server são dois objetos, e uma aba só mostraria a
  // primeira que fosse aberta.
  const id = `${contexto.sessaoId}:objeto:${contexto.banco ?? ""}:${categoria}:${nome}`;
  const existente = paineisDeObjeto.get(id);
  if (existente) {
    existente.selecionar(abaInicial);
    abas.selecionar(id);
    return;
  }

  mostrarAbas();
  const painel = new PainelDeObjeto(contexto, nome, categoria, abaInicial, escreverStatus);
  paineisDeObjeto.set(id, painel);
  // A aba mostra o nome sem o `dbo.` (requisito 44); a dica leva o nome
  // qualificado, que é o que identifica o objeto de verdade.
  const visivel = api.nomeVisivel(nome);
  abas.abrir({
    id,
    titulo: visivel,
    cor: contexto.perfil.cor,
    conexao: contexto.perfil.nome,
    dica: `${nome} — ${contexto.banco ? `${contexto.banco} — ` : ""}${contexto.perfil.nome}`,
    conteudo: painel.elemento,
    aoMostrar: () => {
      // Salvar Consulta SQL não age sobre visualizadores de tabela.
      consultaAtiva = null;
      painel.focar();
    },
    aoFechar: () => {
      paineisDeObjeto.delete(id);
      painel.encerrar();
    },
  });
}

async function abrirAbaDeConsulta(
  contexto: ContextoDeConexao,
  chave: string,
  titulo: string,
  texto: string,
  origem: string | null = null,
): Promise<boolean> {
  mostrarAbas();
  // O id da aba começa com a sessão de metadados da conexão, para todas as abas
  // dela fecharem juntas quando a conexão é encerrada — e cada `aoFechar`
  // encerra a sessão própria da aba.
  const idAba = `${contexto.sessaoId}:${chave}`;
  // Aba já aberta (mesmo arquivo, mesmo fonte): volta para ela sem abrir outra
  // sessão à toa.
  if (abas.tem(idAba)) {
    abas.selecionar(idAba);
    return true;
  }

  // Cada aba executa na própria sessão (requisito 7), aberta aqui ao abrir a
  // aba. Se falhar, a aba não abre — não haveria onde executar.
  let sessaoExecucao: string;
  try {
    sessaoExecucao = await api.conectarIrma(contexto.sessaoId, contexto.banco);
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
    return false;
  }

  copiasCriadas += 1;
  const painel = new PainelDeConsulta(
    contexto,
    sessaoExecucao,
    escreverStatus,
    texto,
    {
      // Identificador só com o que vira nome de arquivo sem sustos. O instante da
      // abertura separa as abas de execuções diferentes, que convivem na pasta.
      id: `aba-${Date.now().toString(36)}-${copiasCriadas}`,
      titulo,
      arquivo: origem,
    },
    (referencia, banco) => abrirReferencia(referencia, contexto, banco),
    (executando) => abas.definirCarregando(idAba, executando),
  );
  const nomeSugerido = nomeDeArquivoSugerido(titulo);

  abas.abrir({
    id: idAba,
    titulo,
    cor: contexto.perfil.cor,
    conexao: contexto.perfil.nome,
    dica: `${titulo} — ${contexto.perfil.nome}`,
    conteudo: painel.elemento,
    aoMostrar: () => {
      consultaAtiva = { painel, contexto, nomeSugerido };
      painel.focar();
    },
    aoFechar: () => {
      // Só limpa se quem fechou foi a aba visível: fechar uma aba de fundo não
      // pode desarmar o Salvar da que está na tela.
      if (consultaAtiva?.painel === painel) consultaAtiva = null;
      painel.encerrar();
    },
  });
  return true;
}

/**
 * Resolve no banco e abre o destino de um Ctrl+clique no editor.
 *
 * `banco` é o da aba, e só o SQL Server o usa: é nele que o nome escrito na
 * consulta é resolvido, e o destino pode estar em outro — `Vendas.dbo.cliente`
 * numa aba de outro banco abre lá.
 */
async function abrirReferencia(
  referencia: string,
  contexto: ContextoDeConexao,
  banco: string | null = null,
): Promise<void> {
  try {
    await comIndicadorDeAbertura(referencia, async () => {
      const destino = await api.resolverReferencia(contexto.sessaoId, referencia, banco);
      if (!destino) {
        escreverStatus(t("referencia nao encontrada", { referencia }), true);
        return;
      }

      const noDestino: ContextoDeConexao = {
        ...contexto,
        banco: destino.banco ?? banco ?? contexto.banco,
      };

      if (destino.categoria === "tabelas" || destino.categoria === "views") {
        mostrarObjeto(destino.nome, destino.categoria, "estrutura", noDestino);
        return;
      }

      const texto = await api.fonteDoObjeto(
        contexto.sessaoId,
        destino.categoria,
        destino.nome,
        destino.parte,
        noDestino.banco ?? null,
      );
      const sufixo =
        destino.categoria === "pacotes"
          ? destino.parte === "corpo"
            ? " (Body)"
            : " (Spec)"
          : "";
      await mostrarFonte(`${destino.nome}${sufixo}`, texto, contexto);
    });
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
  }
}

/** Nome oferecido no "salvar como", a partir do título da aba. */
function nomeDeArquivoSugerido(titulo: string): string {
  // O título vira nome de arquivo, e "PKG_X (Body)" ou um objeto com ponto no
  // nome não podem produzir um caminho inválido.
  const limpo = titulo.replace(/[\\/:*?"<>|]/g, "_").trim();
  return limpo.toLowerCase().endsWith(".sql") ? limpo : `${limpo}.sql`;
}

/** Último trecho de um caminho do Windows: o nome do arquivo. */
function nomeDoArquivo(caminho: string): string {
  return caminho.split(/[\\/]/).pop() || caminho;
}

// --- Conexões ---------------------------------------------------------------

const ligacoes: LigacoesDaArvore = {
  conectar: conectarPerfil,
  aoNovaConsulta: novaConsulta,
  aoConectar: (contexto) => {
    // Quem mandou abrir um arquivo quer ver o arquivo: ele toma o lugar da aba
    // vazia que conectar abriria (requisitos 3 e 12).
    if (contexto.perfil.tipoBanco === "mongodb" || !abrirArquivosEsperando(contexto)) novaConsulta(contexto);
  },
  aoAbrirFonte: mostrarFonte,
  aoAbrirObjeto: mostrarObjeto,
  aoRelatar: escreverStatus,
  aoSelecionarCategoria: (selecao) => {
    categoriaSelecionada = selecao;
    botaoDeFiltro.disabled = selecao === null;
    botaoDeFiltro.title = selecao
      ? t("filtrar categoria", { rotulo: selecao.rotulo })
      : t("filtrar sem categoria");
  },
  aoSelecionarConexao: (contexto) => {
    conexaoSelecionada = contexto;
    botaoDeConsulta.disabled = contexto === null;
    botaoDeConsulta.title = contexto
      ? t(contexto.perfil.tipoBanco === "mongodb" ? "nova consulta mongodb em" : "nova consulta sql em", {
          conexao: contexto.perfil.nome,
          atalho: sufixoDoAtalho("nova-consulta"),
        })
      : t("nova consulta sql sem conexao", { atalho: sufixoDoAtalho("nova-consulta") });
    // Arquivo que esperava conexão abre na primeira que for selecionada.
    if (contexto && contexto.perfil.tipoBanco !== "mongodb") abrirArquivosEsperando(contexto);
  },
  aoPairarBanco: (contexto) => {
    bancoSobOMouse = contexto;
  },
  aoEncerrarSessao: (sessaoId) => {
    abas.fecharDaConexao(sessaoId);
    if (conexaoSelecionada?.sessaoId === sessaoId) {
      ligacoes.aoSelecionarConexao(null);
    }
    // O ponteiro pode ter ficado sobre um banco de uma conexão que acabou de
    // ser encerrada; o atalho de nova consulta não pode continuar mirando nela.
    if (bancoSobOMouse?.sessaoId === sessaoId) bancoSobOMouse = null;
  },
};

let arvoresDeConexoes: ArvoreDeObjetos[] = [];
observarIdioma(() => {
  for (const arvore of arvoresDeConexoes) arvore.traduzirAlertas();
});

/**
 * Lista as conexões salvas, **sem conectar**. A conexão só é aberta quando o
 * usuário expande o nó.
 */
async function recarregarConexoes(aberta?: ConexaoAberta): Promise<void> {
  let perfis: PerfilConexao[];
  try {
    perfis = await api.listarPerfis();
  } catch (erro) {
    escreverStatus(api.mensagemDeErro(erro), true);
    return;
  }

  for (const arvore of arvoresDeConexoes) arvore.encerrarAlertas();
  arvoresDeConexoes = [];
  painelDeConexoes.replaceChildren();
  if (perfis.length === 0) {
    const vazio = document.createElement("p");
    vazio.className = "painel-lateral__vazio";
    vazio.textContent = t("nenhuma conexao salva");
    painelDeConexoes.append(vazio);
    return;
  }

  for (const perfil of perfis) {
    const sessaoInicial = aberta?.perfil.id === perfil.id ? aberta.sessaoId : undefined;
    const arvore = new ArvoreDeObjetos(perfil, ligacoes);
    arvoresDeConexoes.push(arvore);
    painelDeConexoes.append(arvore.montar(sessaoInicial));
  }
}

/**
 * Abre a conexão de um perfil.
 *
 * Sem senha guardada não há como conectar sozinho: o diálogo de conexão é
 * aberto já preenchido, com o cursor na senha.
 */
async function conectarPerfil(perfil: PerfilConexao): Promise<string> {
  if (perfil.temSenhaSalva || (perfil.tipoBanco === "mongodb" && !perfil.usuario)) {
    return api.conectar(perfil);
  }
  const aberta = await abrirDialogoConexao(perfil);
  if (!aberta) throw new Error(t("conexao cancelada"));
  return aberta.sessaoId;
}

async function novaConexaoClicada(): Promise<void> {
  const aberta = await abrirDialogoConexao();
  // Mesmo cancelando pode ter havido "Salvar": a lista é relida de todo jeito.
  await recarregarConexoes(aberta?.perfil.tipoBanco === "mongodb" ? aberta : undefined);
  if (aberta) escreverStatus(t("conectado em", { conexao: aberta.perfil.nome }));
}

// --- Ícones -----------------------------------------------------------------

function svg(caminho: string, preencher: string): SVGSVGElement {
  const elemento = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  elemento.setAttribute("viewBox", "0 0 16 16");
  elemento.setAttribute("aria-hidden", "true");
  const forma = document.createElementNS("http://www.w3.org/2000/svg", "path");
  forma.setAttribute("d", caminho);
  forma.setAttribute("fill", preencher);
  elemento.append(forma);
  return elemento;
}

/** Plugue macho visto de lado, com os dois pinos expostos para a direita. */
function plugueMachoDeConexao(): SVGSVGElement {
  const elemento = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  elemento.setAttribute("viewBox", "0 0 16 16");
  elemento.setAttribute("aria-hidden", "true");

  const forma = document.createElementNS("http://www.w3.org/2000/svg", "path");
  forma.setAttribute("d", "M1 8h3M4 5h6v6H4V5zm6 1.5h4M10 9.5h4");
  forma.setAttribute("fill", "none");
  forma.setAttribute("stroke", "currentColor");
  forma.setAttribute("stroke-width", "1.5");
  forma.setAttribute("stroke-linecap", "round");
  forma.setAttribute("stroke-linejoin", "round");
  elemento.append(forma);
  return elemento;
}

function funil(): SVGSVGElement {
  return svg("M1.5 2h13l-5.2 6.2V14L6.7 12.4V8.2L1.5 2z", "currentColor");
}

function hamburguer(): SVGSVGElement {
  return svg("M2 3h12v2H2V3zm0 4h12v2H2V7zm0 4h12v2H2v-2z", "currentColor");
}

/** Folha de consulta com um "+", no espírito do ícone do SQL Developer. */
function folhaDeConsulta(): SVGSVGElement {
  return svg(
    "M3 1h6l4 4v4h-2V6H8V3H5v10h3v2H3V1z" + "M11.5 10.5h1.6v2h2v1.6h-2v2h-1.6v-2h-2v-1.6h2v-2z",
    "currentColor",
  );
}

void iniciar();
