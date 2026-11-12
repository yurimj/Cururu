/**
 * Tela "Atualização do Cururu".
 *
 * Mostra o que mudou na versão publicada e deixa a decisão com o usuário: nada
 * é baixado nem instalado sem ele mandar. É o motivo de a tela existir em vez
 * de a atualização acontecer sozinha.
 *
 * O conteúdo do "o que mudou" é o corpo do release no GitHub, que por sua vez é
 * a seção daquela versão em `versoes.md` — a mesma lista que a tela de Versões
 * mostra. Uma origem só.
 */

import "./dialogo-atualizacao.css";

import * as api from "../api";
import { t } from "../idioma";
import { fecharComEsc } from "../componentes/janela";
import { formatarTamanho } from "../formatacao";
import { comMarcacaoInline, itensDaLista } from "../marcacao";

/**
 * Abre a tela de atualização.
 *
 * `atualizacao` nulo é o caso de já se estar na versão mais recente. A tela
 * ainda abre — quem clicou em "Procurar Atualizações" merece resposta —, só que
 * dizendo que não há nada a fazer.
 */
export function abrirDialogoDeAtualizacao(
  atualizacao: api.Atualizacao | null,
  versaoAtual: string,
): Promise<void> {
  return new Promise((resolver) => {
    const fundo = document.createElement("div");
    fundo.className = "dialogo-fundo";

    const caixa = document.createElement("section");
    caixa.className = "dialogo dialogo-atualizacao";
    caixa.setAttribute("role", "dialog");
    caixa.setAttribute("aria-modal", "true");
    caixa.setAttribute("aria-labelledby", "titulo-atualizacao");

    const barra = document.createElement("header");
    barra.className = "dialogo__barra";
    const titulo = document.createElement("h1");
    titulo.id = "titulo-atualizacao";
    titulo.textContent = t(atualizacao ? "atualizacao disponivel" : "atualizacao");
    const fechar = document.createElement("button");
    fechar.className = "dialogo__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar"));
    barra.append(titulo, fechar);

    const corpo = document.createElement("div");
    corpo.className = "atualizacao__corpo";

    const rodape = document.createElement("footer");
    rodape.className = "dialogo__rodape";
    const situacao = document.createElement("span");
    situacao.className = "status";
    const botoes = document.createElement("span");
    botoes.className = "dialogo__botoes";
    rodape.append(situacao, botoes);

    caixa.append(barra, corpo, rodape);
    fundo.append(caixa);

    let pararEsc = (): void => {};
    const encerrar = (): void => {
      pararEsc();
      fundo.remove();
      resolver();
    };
    // Durante o download o Fechar fica desabilitado; o Esc segue a mesma regra,
    // senão fecharia a tela por baixo de uma operação em andamento.
    pararEsc = fecharComEsc(
      fundo,
      encerrar,
      () => !fundo.querySelector<HTMLButtonElement>(".dialogo__fechar")?.disabled,
    );

    if (!atualizacao) {
      corpo.append(montarSemNovidade(versaoAtual));
      const confirmar = document.createElement("button");
      confirmar.textContent = t("fechar");
      confirmar.addEventListener("click", encerrar);
      botoes.append(confirmar);
      document.body.append(fundo);
      confirmar.focus();
      return;
    }

    corpo.append(montarCabecalho(atualizacao), montarNotas(atualizacao.notas));

    // A barra só aparece quando o download começa: antes disso ela seria uma
    // barra vazia ocupando espaço e sugerindo que algo já está acontecendo.
    const progresso = document.createElement("div");
    progresso.className = "atualizacao__progresso";
    progresso.hidden = true;
    const preenchimento = document.createElement("div");
    preenchimento.className = "atualizacao__progresso-preenchimento";
    progresso.append(preenchimento);
    corpo.append(progresso);

    const baixar = document.createElement("button");
    baixar.textContent = t("baixar e instalar");
    const agoraNao = document.createElement("button");
    agoraNao.textContent = t("agora nao");
    botoes.append(baixar, agoraNao);

    situacao.textContent = atualizacao.tamanho
      ? t("instalador de", { tamanho: formatarTamanho(atualizacao.tamanho) })
      : "";

    agoraNao.addEventListener("click", encerrar);
    baixar.addEventListener("click", () => {
      void baixarEInstalar(atualizacao, {
        baixar,
        agoraNao,
        fechar,
        situacao,
        progresso,
        preenchimento,
      });
    });

    document.body.append(fundo);
    baixar.focus();
  });
}

interface Controles {
  baixar: HTMLButtonElement;
  agoraNao: HTMLButtonElement;
  fechar: HTMLButtonElement;
  situacao: HTMLElement;
  progresso: HTMLElement;
  preenchimento: HTMLElement;
}

/**
 * Baixa o instalador, executa e fecha o Cururu.
 *
 * Fechar faz parte da operação, e não é um efeito colateral: o instalador
 * precisa substituir o `cururu.exe` que está rodando, e não conseguiria com o
 * arquivo em uso.
 */
async function baixarEInstalar(
  atualizacao: api.Atualizacao,
  controles: Controles,
): Promise<void> {
  const { baixar, agoraNao, fechar, situacao, progresso, preenchimento } = controles;

  // Enquanto baixa, não há como voltar atrás pela metade: os botões saem do ar
  // para o usuário não disparar um segundo download por cima do primeiro.
  baixar.disabled = true;
  agoraNao.disabled = true;
  fechar.disabled = true;
  situacao.className = "status";
  situacao.textContent = t("baixando");
  progresso.hidden = false;
  preenchimento.style.width = "0%";

  try {
    await api.baixarAtualizacao(atualizacao.instalador, ({ baixados, total }) => {
      const referencia = total || atualizacao.tamanho;
      if (referencia > 0) {
        const porcento = Math.min(100, Math.round((baixados / referencia) * 100));
        preenchimento.style.width = `${porcento}%`;
        situacao.textContent = t("baixando porcento", {
          porcento,
          total: formatarTamanho(referencia),
        });
      } else {
        // Sem total anunciado não há porcentagem honesta a mostrar; resta dizer
        // quanto já veio.
        situacao.textContent = t("baixando quanto", { baixados: formatarTamanho(baixados) });
      }
    });

    preenchimento.style.width = "100%";
    situacao.textContent = t("iniciando o instalador");
    await api.instalarAtualizacao();

    situacao.textContent = t("vai fechar para instalar");
    await api.encerrarAplicativo();
  } catch (erro) {
    situacao.className = "status status--erro";
    situacao.textContent = api.mensagemDeErro(erro);
    progresso.hidden = true;
    baixar.disabled = false;
    agoraNao.disabled = false;
    fechar.disabled = false;

    // Falhou o caminho automático, resta o manual: a página do release tem o
    // instalador para baixar à mão.
    baixar.textContent = t("tentar de novo");
    if (atualizacao.pagina) mostrarLinkDaPagina(situacao, atualizacao.pagina);
  }
}

function mostrarLinkDaPagina(situacao: HTMLElement, pagina: string): void {
  const link = document.createElement("a");
  link.className = "atualizacao__link";
  link.href = pagina;
  link.textContent = t("abrir a pagina do release");
  link.title = pagina;
  link.addEventListener("click", (evento) => {
    evento.preventDefault();
    // A janela do Cururu não tem barra de endereço nem botão de voltar: o
    // endereço vai para o navegador do sistema.
    void api.abrirNoNavegador(pagina).catch(() => undefined);
  });
  situacao.append(" ", link);
}

function montarCabecalho(atualizacao: api.Atualizacao): HTMLElement {
  const cabecalho = document.createElement("div");
  cabecalho.className = "atualizacao__cabecalho";

  const versao = document.createElement("h2");
  versao.className = "atualizacao__versao";
  versao.textContent = `Cururu ${atualizacao.versao}`;
  cabecalho.append(versao);

  if (atualizacao.data) {
    const data = document.createElement("span");
    data.className = "atualizacao__data";
    data.textContent = atualizacao.data;
    cabecalho.append(data);
  }

  const atual = document.createElement("p");
  atual.className = "atualizacao__atual";
  atual.textContent = t("em execucao versao", { versao: atualizacao.versaoAtual });
  cabecalho.append(atual);

  return cabecalho;
}

function montarNotas(notas: string): HTMLElement {
  const bloco = document.createElement("div");
  bloco.className = "atualizacao__notas";

  const titulo = document.createElement("h3");
  titulo.className = "atualizacao__subtitulo";
  titulo.textContent = t("o que mudou");
  bloco.append(titulo);

  const itens = itensDaLista(notas);
  if (itens.length === 0) {
    const vazio = document.createElement("p");
    vazio.className = "atualizacao__vazio";
    // Um release pode vir sem lista; o texto cru ainda é melhor que nada.
    vazio.textContent = notas.trim() || t("release sem descricao");
    bloco.append(vazio);
    return bloco;
  }

  const lista = document.createElement("ul");
  lista.className = "atualizacao__itens";
  for (const texto of itens) {
    const item = document.createElement("li");
    item.append(comMarcacaoInline(texto));
    lista.append(item);
  }
  bloco.append(lista);
  return bloco;
}

function montarSemNovidade(versaoAtual: string): HTMLElement {
  const bloco = document.createElement("div");
  bloco.className = "atualizacao__sem-novidade";

  const titulo = document.createElement("p");
  titulo.className = "atualizacao__em-dia";
  titulo.textContent = t("cururu esta atualizado");

  const detalhe = document.createElement("p");
  detalhe.className = "atualizacao__atual";
  detalhe.textContent = versaoAtual
    ? t("versao mais recente", { versao: versaoAtual })
    : t("esta e a mais recente");

  bloco.append(titulo, detalhe);
  return bloco;
}
