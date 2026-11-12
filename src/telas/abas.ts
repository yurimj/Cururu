/**
 * Área de abas da janela (requisito 11).
 *
 * Várias consultas SQL e fontes abertos ao mesmo tempo. Cada aba guarda a cor
 * da conexão a que pertence (requisito 26), e arrastar uma aba muda o lugar
 * dela na barra.
 */

import "./abas.css";
import { t } from "../idioma";

/** Quanto o ponteiro anda na horizontal antes de o clique na aba virar arrasto. */
const LIMIAR_DO_ARRASTO = 5;
/** Quanto a aba passa da marca antes de trocar de lugar: o tremor do mouse não desfaz a troca. */
const FOLGA_DA_TROCA = 6;
/** Teto da rolagem automática da barra, em pixels por quadro. */
const ROLAGEM_MAXIMA = 20;

/** Uma aba segura pelo ponteiro. */
interface Arrasto {
  botao: HTMLElement;
  ponteiro: number;
  /** Onde o botão foi apertado, para medir o limiar. */
  inicioX: number;
  /** Última posição do ponteiro: a rolagem automática reposiciona a aba sem evento novo. */
  ponteiroX: number;
  /** Distância entre a borda esquerda da aba e o ponto por onde ela foi segura. */
  pega: number;
  /** Quanto a aba está deslocada do lugar que ocupa na barra. */
  deslocamento: number;
  /** Pixels por quadro que a barra rola sozinha; zero quando a aba não força a borda. */
  rolagem: number;
  /** Falso até o ponteiro passar do limiar: até ali foi só um clique. */
  ativo: boolean;
}

export interface Aba {
  id: string;
  titulo: string;
  /** Cor da conexão, mostrada como faixa lateral na aba. */
  cor?: string | null;
  /** Nome da conexão, escrito na aba ao lado do título. */
  conexao?: string | null;
  dica?: string;
  conteudo: HTMLElement;
  /** Chamado quando a aba é fechada, para liberar o que ela segurava. */
  aoFechar?: () => void;
  /** Chamado quando a aba passa a ser a visível. */
  aoMostrar?: () => void;
}

export class AreaDeAbas {
  readonly elemento = document.createElement("div");
  private readonly barra = document.createElement("div");
  private readonly corpo = document.createElement("div");
  /**
   * Na mesma ordem da barra: é a ordem que o Ctrl+PageUp/PageDown percorre e a
   * que decide quem aparece quando a aba visível fecha.
   */
  private readonly abas = new Map<string, { aba: Aba; botao: HTMLElement }>();
  private atual: string | null = null;
  private arrasto: Arrasto | null = null;
  private quadroDeRolagem = 0;

  constructor(private readonly aoFicarVazia?: () => void) {
    this.elemento.className = "area-abas";
    this.barra.className = "area-abas__barra";
    this.barra.setAttribute("role", "tablist");
    this.corpo.className = "area-abas__corpo";
    this.elemento.append(this.barra, this.corpo);
    this.permitirArrasto();
  }

  get vazia(): boolean {
    return this.abas.size === 0;
  }

  /** A aba já está aberta? Evita abrir uma sessão nova para reabrir a mesma. */
  tem(id: string): boolean {
    return this.abas.has(id);
  }

  /** Fecha a aba visível; usado pelo atalho global Ctrl+W. */
  fecharAtual(): boolean {
    if (!this.atual) return false;
    this.fechar(this.atual);
    return true;
  }

  /** Troca para a aba anterior ou seguinte, dando a volta nas extremidades. */
  selecionarAdjacente(deslocamento: -1 | 1): boolean {
    if (!this.atual || this.abas.size < 2) return false;
    const ids = [...this.abas.keys()];
    const atual = ids.indexOf(this.atual);
    const destino = (atual + deslocamento + ids.length) % ids.length;
    this.selecionar(ids[destino]);
    return true;
  }

  abrir(aba: Aba): void {
    if (this.abas.has(aba.id)) {
      this.selecionar(aba.id);
      return;
    }

    const botao = document.createElement("div");
    botao.className = "aba";
    botao.setAttribute("role", "tab");
    botao.title = aba.dica ?? aba.titulo;
    botao.addEventListener("mousedown", (evento) => {
      // Botão do meio fecha, como em navegador e no SQL Developer.
      if (evento.button === 1) {
        evento.preventDefault();
        this.fechar(aba.id);
      } else if (evento.button === 0) {
        this.selecionar(aba.id);
      }
    });

    if (aba.cor) {
      const marca = document.createElement("span");
      marca.className = "aba__cor";
      marca.style.background = aba.cor;
      botao.append(marca);
    }

    // Símbolo de execução: some da tela quando parada e roda enquanto a consulta
    // da aba está executando (requisito 17). O rodapé já anuncia por voz, então
    // aqui o giro é só visual — é o que diz qual aba de fundo está ocupada.
    const carregando = document.createElement("span");
    carregando.className = "aba__carregando";
    carregando.setAttribute("aria-hidden", "true");
    botao.append(carregando);

    const titulo = document.createElement("span");
    titulo.className = "aba__titulo";
    titulo.textContent = aba.titulo;

    const fechar = document.createElement("button");
    fechar.className = "aba__fechar";
    fechar.textContent = "✕";
    fechar.title = t("fechar");
    fechar.setAttribute("aria-label", t("fechar aba nomeada", { aba: aba.titulo }));
    fechar.addEventListener("click", (evento) => {
      evento.stopPropagation();
      this.fechar(aba.id);
    });

    botao.append(titulo);
    // O nome da conexão fica escrito na aba, e não só na dica: é o que evita
    // executar em produção pensando que a aba era a de homologação.
    if (aba.conexao) {
      const conexao = document.createElement("span");
      conexao.className = "aba__conexao";
      conexao.textContent = aba.conexao;
      botao.append(conexao);
    }
    botao.append(fechar);
    this.barra.append(botao);

    aba.conteudo.classList.add("area-abas__painel");
    aba.conteudo.hidden = true;
    this.corpo.append(aba.conteudo);

    this.abas.set(aba.id, { aba, botao });
    this.selecionar(aba.id);
  }

  selecionar(id: string): void {
    const alvo = this.abas.get(id);
    if (!alvo) return;
    for (const [outroId, { aba, botao }] of this.abas) {
      const visivel = outroId === id;
      aba.conteudo.hidden = !visivel;
      botao.setAttribute("aria-selected", String(visivel));
      botao.classList.toggle("aba--ativa", visivel);
    }
    this.atual = id;
    alvo.aba.aoMostrar?.();
  }

  /**
   * Liga ou desliga o símbolo de execução na aba (requisito 17).
   *
   * Vale mesmo para a aba de fundo: é o que deixa ver, sem trocar de aba, que
   * uma consulta aberta noutra aba ainda está rodando. Aba já fechada é no-op.
   */
  definirCarregando(id: string, carregando: boolean): void {
    const alvo = this.abas.get(id);
    if (!alvo) return;
    alvo.botao.classList.toggle("aba--carregando", carregando);
  }

  fechar(id: string): void {
    const alvo = this.abas.get(id);
    if (!alvo) return;

    // Requisito 12: fechar não pergunta nada nem salva nada.
    alvo.aba.aoFechar?.();
    // Ctrl+W ou a conexão encerrada podem fechar a aba no meio do arrasto.
    if (this.arrasto?.botao === alvo.botao) this.encerrarArrasto();
    alvo.botao.remove();
    alvo.aba.conteudo.remove();
    this.abas.delete(id);

    if (this.atual === id) {
      this.atual = null;
      const proxima = this.abas.keys().next();
      if (!proxima.done) this.selecionar(proxima.value);
    }
    if (this.abas.size === 0) this.aoFicarVazia?.();
  }

  /** Fecha todas as abas de uma conexão que foi encerrada. */
  fecharDaConexao(sessaoId: string): void {
    for (const id of [...this.abas.keys()]) {
      if (id.startsWith(`${sessaoId}:`)) this.fechar(id);
    }
  }

  /**
   * Arrastar uma aba muda o lugar dela na barra, como nos navegadores.
   *
   * São eventos de ponteiro, e não o arrastar e soltar do HTML, que no Windows
   * o Tauri não deixa chegar à página (premissas.md). A captura fica na barra,
   * e não na aba, porque a aba troca de lugar no meio do arrasto — e o elemento
   * tirado do documento perde a captura.
   */
  private permitirArrasto(): void {
    this.barra.addEventListener("pointerdown", (evento) => {
      if (evento.button !== 0 || !evento.isPrimary || this.abas.size < 2) return;
      const alvo = evento.target as Element;
      // Apertar o ✕ e escorregar é fechar com a mira errada, não arrastar.
      if (alvo.closest(".aba__fechar")) return;
      const botao = alvo.closest<HTMLElement>(".aba");
      if (!botao) return;
      // Um arrasto que não viu o botão ser solto não pode prender o seguinte.
      this.encerrarArrasto();
      // Sem `preventDefault`: ele cancelaria o `mousedown` que seleciona a aba.
      this.barra.setPointerCapture(evento.pointerId);
      this.arrasto = {
        botao,
        ponteiro: evento.pointerId,
        inicioX: evento.clientX,
        ponteiroX: evento.clientX,
        pega: this.xNaBarra(evento.clientX) - botao.offsetLeft,
        deslocamento: 0,
        rolagem: 0,
        ativo: false,
      };
    });

    this.barra.addEventListener("pointermove", (evento) => {
      const arrasto = this.arrasto;
      if (arrasto?.ponteiro !== evento.pointerId) return;
      // Botão solto sem o `pointerup` chegar aqui: sem isto, a aba seguiria o
      // ponteiro que só passa por cima da barra.
      if ((evento.buttons & 1) === 0) {
        this.encerrarArrasto();
        return;
      }
      arrasto.ponteiroX = evento.clientX;
      if (!arrasto.ativo) {
        if (Math.abs(evento.clientX - arrasto.inicioX) < LIMIAR_DO_ARRASTO) return;
        arrasto.ativo = true;
        arrasto.botao.classList.add("aba--arrastando");
      }
      this.acompanharPonteiro();
    });

    const soltar = (evento: PointerEvent): void => {
      if (this.arrasto?.ponteiro === evento.pointerId) this.encerrarArrasto();
    };
    this.barra.addEventListener("pointerup", soltar);
    this.barra.addEventListener("pointercancel", soltar);
    this.barra.addEventListener("lostpointercapture", soltar);
  }

  /** Posição do ponteiro no conteúdo da barra, que rola quando as abas não cabem. */
  private xNaBarra(clientX: number): number {
    const caixa = this.barra.getBoundingClientRect();
    return clientX - caixa.left - this.barra.clientLeft + this.barra.scrollLeft;
  }

  /**
   * Leva a aba arrastada para baixo do ponteiro e a troca de lugar com cada
   * vizinha que ela ultrapassa.
   *
   * As medidas saem de `offsetLeft`, que ignora `transform`: a própria aba e as
   * vizinhas ainda deslizando são medidas no lugar que ocupam na barra.
   */
  private acompanharPonteiro(): void {
    const arrasto = this.arrasto;
    if (!arrasto) return;
    const { botao } = arrasto;
    const irmas = [...this.barra.children] as HTMLElement[];
    const primeira = irmas[0];
    const ultima = irmas[irmas.length - 1];
    const largura = botao.offsetWidth;
    const visivelDe = this.barra.scrollLeft;
    const visivelAte = visivelDe + this.barra.clientWidth;

    // A aba não sai da parte visível da barra nem passa das abas das pontas.
    const pedida = this.xNaBarra(arrasto.ponteiroX) - arrasto.pega;
    const minimo = Math.max(primeira.offsetLeft, visivelDe);
    const fim = Math.min(ultima.offsetLeft + ultima.offsetWidth, visivelAte);
    const maximo = Math.max(minimo, fim - largura);
    const esquerda = Math.min(Math.max(pedida, minimo), maximo);

    // A aba e uma vizinha trocam de lugar quando o centro da aba cruza a marca
    // entre os dois lugares possíveis: o centro que a aba teria logo antes da
    // vizinha e o que teria logo depois. O meio da vizinha não serviria de
    // marca: presa na ponta da barra, uma aba larga não leva o centro até o meio
    // de uma primeira aba mais estreita, e nunca sairia do segundo lugar.
    const vao = parseFloat(getComputedStyle(this.barra).columnGap) || 0;
    const centro = esquerda + largura / 2;
    const indice = irmas.indexOf(botao);
    const seguinte =
      irmas.find((irma, i) => {
        if (i === indice) return false;
        const depois = i > indice;
        // Onde a vizinha começaria sem a aba arrastada antes dela.
        const inicio = depois ? irma.offsetLeft - largura - vao : irma.offsetLeft;
        const marca = inicio + (irma.offsetWidth + vao + largura) / 2;
        return centro < marca + (depois ? FOLGA_DA_TROCA : -FOLGA_DA_TROCA);
      }) ?? null;
    if (seguinte !== botao.nextElementSibling) {
      const antes = new Map(irmas.map((irma) => [irma, irma.offsetLeft] as const));
      this.barra.insertBefore(botao, seguinte);
      this.seguirOrdemDaBarra();
      for (const irma of irmas) {
        const distancia = antes.get(irma)! - irma.offsetLeft;
        if (irma !== botao && distancia !== 0) deslizar(irma, distancia);
      }
    }

    arrasto.deslocamento = esquerda - botao.offsetLeft;
    botao.style.transform = `translateX(${arrasto.deslocamento}px)`;

    // Aba forçando a borda rola a barra, mais depressa quanto mais o ponteiro
    // passa dela, até mostrar as abas que estavam escondidas.
    const excesso =
      pedida < visivelDe ? pedida - visivelDe : Math.max(0, pedida + largura - visivelAte);
    const velocidade = Math.min(Math.max(2, Math.abs(excesso) / 4), ROLAGEM_MAXIMA);
    arrasto.rolagem = excesso === 0 ? 0 : Math.sign(excesso) * velocidade;
    if (arrasto.rolagem !== 0 && this.quadroDeRolagem === 0) {
      this.quadroDeRolagem = requestAnimationFrame(this.rolarSozinha);
    }
  }

  private readonly rolarSozinha = (): void => {
    this.quadroDeRolagem = 0;
    const rolagem = this.arrasto?.rolagem ?? 0;
    if (rolagem === 0) return;
    const antes = this.barra.scrollLeft;
    this.barra.scrollLeft += rolagem;
    // Na ponta a rolagem não anda, e o laço para até o ponteiro se mexer.
    if (this.barra.scrollLeft !== antes) this.acompanharPonteiro();
  };

  /** Refaz o mapa na ordem da barra, logo depois de a aba arrastada trocar de lugar. */
  private seguirOrdemDaBarra(): void {
    const posicao = new Map(
      [...this.barra.children].map((elemento, indice) => [elemento, indice] as const),
    );
    const ordenadas = [...this.abas].sort(
      ([, a], [, b]) => (posicao.get(a.botao) ?? 0) - (posicao.get(b.botao) ?? 0),
    );
    this.abas.clear();
    for (const [id, entrada] of ordenadas) this.abas.set(id, entrada);
  }

  /** Deixa a aba no lugar que ela ocupa na barra. */
  private encerrarArrasto(): void {
    const arrasto = this.arrasto;
    if (!arrasto) return;
    this.arrasto = null;
    cancelAnimationFrame(this.quadroDeRolagem);
    this.quadroDeRolagem = 0;
    if (this.barra.hasPointerCapture(arrasto.ponteiro)) {
      this.barra.releasePointerCapture(arrasto.ponteiro);
    }
    if (!arrasto.ativo) return;

    const { botao, deslocamento } = arrasto;
    botao.classList.remove("aba--arrastando");
    botao.style.transform = "";
    // Do ponto onde foi solta, a aba desliza até o lugar em vez de saltar.
    if (deslocamento !== 0) deslizar(botao, deslocamento);
  }
}

/**
 * Faz o elemento sair de `distancia` pixels do lugar dele e deslizar até lá.
 *
 * Soma ao que já estiver deslizando, em vez de substituir: uma vizinha que
 * troca de lugar duas vezes seguidas não salta no meio do caminho.
 */
function deslizar(elemento: HTMLElement, distancia: number): void {
  elemento.animate([{ transform: `translateX(${distancia}px)` }, { transform: "none" }], {
    duration: 120,
    easing: "ease-out",
    composite: "add",
  });
}
