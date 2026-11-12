/**
 * Diálogo "Novo / Selecionar Conexão do Banco de Dados" — requisito 1.
 *
 * Espelha o do Oracle SQL Developer: lista de conexões salvas à esquerda,
 * formulário à direita, barra de status e a mesma fileira de botões.
 */

import "./dialogo-conexao.css";

import * as api from "../api";
import { comIndicadorDeAbertura } from "../componentes/indicador-de-abertura";
import { fecharComEsc } from "../componentes/janela";
import { t } from "../idioma";
import { formatarDuracao } from "../formatacao";
import { escolherConexaoImportavel } from "./dialogo-importacao";
import {
  detalhesDaConexao,
  perfilNovo,
  type Atribuicao,
  type AutenticacaoSql,
  type IdentificacaoBanco,
  type PerfilConexao,
  type TipoBanco,
  type TipoConexao,
} from "../tipos";

interface OpcaoCor {
  nome: string;
  valor: string;
  ambiente?: string;
}

/**
 * Paleta categórica: a ordem maximiza a diferença visual entre as primeiras
 * conexões. As cores de ambiente continuam na posição original da paleta,
 * mas são sugeridas automaticamente quando o nome indica PRD, HML ou DEV.
 */
const PALETA_CORES: readonly OpcaoCor[] = [
  { nome: "Azul", valor: "#3569D4" },
  { nome: "Laranja", valor: "#E97814", ambiente: "Homologação" },
  { nome: "Verde", valor: "#2F9B58", ambiente: "Desenvolvimento" },
  { nome: "Roxo", valor: "#9B4BB3" },
  { nome: "Turquesa", valor: "#1596A6" },
  { nome: "Rosa", valor: "#D64F87" },
  { nome: "Mostarda", valor: "#B28704" },
  { nome: "Vermelho coral", valor: "#D94B3D", ambiente: "Produção" },
  { nome: "Índigo", valor: "#5961C7" },
  { nome: "Marrom", valor: "#936748" },
  { nome: "Verde oliva", valor: "#758D24" },
  { nome: "Azul claro", valor: "#3E9DDB" },
];

const COR_PADRAO = PALETA_CORES[0].valor;

function moldeSenha(id: string, atributo: "data-campo" | "data-campo-sql"): string {
  return `<div class="campo-com-acoes largura-total">
    <input type="password" id="${id}" ${atributo}="senha" autocomplete="off" />
    <button type="button" data-acao="mostrar-senha" aria-controls="${id}">${t("mostrar senha")}</button>
    <button type="button" data-acao="copiar-senha">${t("copiar senha")}</button>
  </div>`;
}

function normalizarNome(nome: string): string {
  return nome
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

function corReservadaParaAmbiente(nome: string): string | null {
  const normalizado = normalizarNome(nome);
  const contem = (expressoes: string): boolean =>
    new RegExp(`(^|[\\s_.-])(${expressoes})([\\s_.-]|$)`).test(normalizado);

  if (contem("prd|prod|producao|production")) return "#D94B3D";
  if (contem("hml|hom|homol|homologacao")) return "#E97814";
  if (contem("dev|desenv|desenvolvimento|development")) return "#2F9B58";
  return null;
}

/** Evita preto, branco, cinzas e tons tão claros/escuros que somem na UI. */
function corAdequadaParaIdentificacao(cor: string | null): boolean {
  if (!cor || !/^#[0-9a-f]{6}$/i.test(cor)) return false;
  const canais = [1, 3, 5].map((inicio) => Number.parseInt(cor.slice(inicio, inicio + 2), 16));
  const maior = Math.max(...canais);
  const menor = Math.min(...canais);
  return maior + menor >= 143 && maior + menor <= 367 && maior - menor >= 50;
}

const molde = (): string => `
<div class="dialogo dialogo-conexao" role="dialog" aria-modal="true" aria-labelledby="titulo-dialogo">
  <header class="dialogo__barra">
    <h1 id="titulo-dialogo">${t("novo ou selecionar conexao")}</h1>
    <button class="dialogo__fechar" data-acao="cancelar" title="${t("fechar")}" aria-label="${t("fechar")}">✕</button>
  </header>

  <div class="dialogo__corpo">
    <section class="lista-conexoes">
      <table>
        <thead>
          <tr>
            <th>${t("nome da conexao")}</th>
          </tr>
        </thead>
        <tbody data-elemento="lista"></tbody>
      </table>
    </section>

    <section class="formulario">
      <div class="linha">
        <label for="campo-nome">${t("nome")}</label>
        <input type="text" id="campo-nome" class="campo-nome" data-campo="nome" />
        <span class="seletor-cor">
          <input type="color" id="campo-cor" data-campo="cor" required aria-required="true"
                 title="${t("escolher cor personalizada")}" />
          <label for="campo-cor">${t("cor obrigatoria")}</label>
        </span>
      </div>

      <div class="linha linha-paleta">
        <label id="rotulo-paleta-cores">${t("cores sugeridas")}</label>
        <span class="paleta-cores" data-elemento="paleta-cores" role="group"
              aria-labelledby="rotulo-paleta-cores"></span>
        <small>${t("dica das cores")}</small>
      </div>

      <div class="linha">
        <label for="campo-tipo-banco">${t("tipo de banco")}</label>
        <select id="campo-tipo-banco" data-campo="tipoBanco" style="flex:0 0 auto">
          <option value="oracle">Oracle</option>
          <option value="sqlserver">SQL Server</option>
          <option value="mongodb">MongoDB</option>
        </select>
      </div>

      <div data-banco="oracle">
        <div class="abas" data-grupo="abas-usuario" role="tablist">
          <button role="tab" data-aba="usuario" aria-selected="true">${t("informacoes do usuario")}</button>
          <button role="tab" data-aba="proxy" aria-selected="false">${t("usuario proxy")}</button>
        </div>

        <div class="painel-aba" data-painel="usuario">
          <div class="grade">
            <label for="campo-autenticacao">${t("tipo de autenticacao")}</label>
            <select id="campo-autenticacao" class="largura-total" style="max-width:190px" disabled>
              <option>${t("padrao")}</option>
            </select>

            <label for="campo-usuario">${t("nome do usuario")}</label>
            <input type="text" id="campo-usuario" data-campo="usuario" />
            <label for="campo-atribuicao">${t("atribuicao")}</label>
            <select id="campo-atribuicao" data-campo="atribuicao">
              <option value="padrao">${t("padrao minusculo")}</option>
              <option value="sysdba">SYSDBA</option>
              <option value="sysoper">SYSOPER</option>
            </select>

            <label for="campo-senha">${t("senha")}</label>
            ${moldeSenha("campo-senha", "data-campo")}
            <label class="caixa-marcacao" style="grid-column: 2 / -1">
              <input type="checkbox" data-campo="salvarSenha" />
              ${t("salvar senha")}
            </label>
          </div>
        </div>

        <div class="painel-aba" data-painel="proxy" hidden>
          <label class="caixa-marcacao">
            <input type="checkbox" data-campo="usarProxy" />
            ${t("usar proxy")}
          </label>
          <div class="grade">
            <label for="campo-usuario-proxy">${t("nome do usuario proxy")}</label>
            <input type="text" id="campo-usuario-proxy" class="largura-total" data-campo="usuarioProxy" />
            <label for="campo-senha-proxy">${t("senha proxy")}</label>
            <input type="password" id="campo-senha-proxy" class="largura-total" data-campo="senhaProxy" />
          </div>
        </div>
      </div>

      <div class="linha" data-banco="oracle">
        <label for="campo-tipo-conexao">${t("tipo de conexao")}</label>
        <select id="campo-tipo-conexao" data-campo="tipoConexao" style="flex:0 0 auto">
          <option value="basico">${t("basico")}</option>
          <option value="tns">TNS</option>
          <option value="avancado">${t("avancado")}</option>
        </select>
      </div>

      <div data-banco="oracle">
        <div class="abas" data-grupo="abas-detalhes" role="tablist">
          <button role="tab" data-aba="detalhes" aria-selected="true">${t("detalhes")}</button>
          <button role="tab" data-aba="avancado" aria-selected="false">${t("avancado")}</button>
        </div>

        <div class="painel-aba" data-painel="detalhes">
          <div class="grade" data-bloco="basico">
            <label for="campo-host">${t("nome do host")}</label>
            <input type="text" id="campo-host" class="largura-total" data-campo="host" />

            <label for="campo-porta">${t("porta")}</label>
            <input type="number" id="campo-porta" style="max-width:110px" data-campo="porta" min="1" max="65535" />
            <span></span><span></span>

            <span class="opcao-radio">
              <input type="radio" id="opcao-sid" name="identificacao" value="sid" />
              <label for="opcao-sid">SID</label>
            </span>
            <input type="text" class="largura-total" data-campo="sid" />

            <span class="opcao-radio">
              <input type="radio" id="opcao-servico" name="identificacao" value="servico" />
              <label for="opcao-servico">${t("nome do servico")}</label>
            </span>
            <input type="text" class="largura-total" data-campo="servico" />
          </div>

          <div class="grade" data-bloco="tns" hidden>
            <label for="campo-alias-tns">${t("alias da rede")}</label>
            <input type="text" id="campo-alias-tns" class="largura-total" data-campo="aliasTns"
                   placeholder="${t("alias tns exemplo")}" />
          </div>

          <div class="grade" data-bloco="avancado" hidden>
            <label for="campo-url">${t("url personalizada")}</label>
            <input type="text" id="campo-url" class="largura-total" data-campo="urlPersonalizada"
                   placeholder="//host:porta/servico" />
          </div>
        </div>

        <div class="painel-aba" data-painel="avancado" hidden>
          <label class="caixa-marcacao">
            <input type="checkbox" data-campo="autoCommit" />
            Auto-commit
          </label>
          <p style="margin:0;color:var(--cor-texto-suave)">
            ${t("auto commit explicacao")}
          </p>
        </div>
      </div>

      <div class="painel-aba painel-aba--solto" data-banco="sqlserver mongodb" hidden>
        <div class="grade">
          <label for="campo-servidor" data-elemento="rotulo-servidor">${t("nome do servidor")}</label>
          <input type="text" id="campo-servidor" class="largura-total" data-campo="servidor"
                 spellcheck="false" autocomplete="off" placeholder="${t("servidor exemplo")}" />

          <label for="campo-autenticacao-sql" data-so-sql>${t("autenticacao")}</label>
          <select id="campo-autenticacao-sql" class="largura-total" data-campo="autenticacaoSql" data-so-sql>
            <option value="sqlserver">${t("autenticacao do sql server")}</option>
            <option value="windows">${t("autenticacao do windows")}</option>
          </select>

          <label for="campo-usuario-sql" data-elemento="rotulo-usuario-sql">${t("logon")}</label>
          <input type="text" id="campo-usuario-sql" class="largura-total" data-campo-sql="usuario"
                 spellcheck="false" autocomplete="off" />

          <label for="campo-senha-sql">${t("senha")}</label>
          ${moldeSenha("campo-senha-sql", "data-campo-sql")}

          <label class="caixa-marcacao" style="grid-column: 2 / -1">
            <input type="checkbox" data-campo-sql="salvarSenha" />
            ${t("lembrar senha")}
          </label>
        </div>

        <hr class="separador-painel" data-so-sql />

        <label class="caixa-marcacao" data-so-sql>
          <input type="checkbox" data-campo-sql="autoCommit" />
          Auto-commit
        </label>
        <p data-so-sql style="margin:0;color:var(--cor-texto-suave)">
          ${t("auto commit explicacao sqlserver")}
        </p>
      </div>
    </section>
  </div>

  <footer class="dialogo__rodape">
    <span class="status">${t("status")} <span data-elemento="status"></span></span>
    <span class="dialogo__botoes">
      <button data-acao="importar">${t("importar")}</button>
      <button data-acao="salvar">${t("salvar")}</button>
      <button data-acao="excluir" disabled>${t("excluir")}</button>
      <button data-acao="testar">${t("testar")}</button>
      <button data-acao="conectar">${t("conectar")}</button>
      <button data-acao="cancelar">${t("cancelar")}</button>
    </span>
  </footer>
</div>
`;

/**
 * O que falta para uma conexão de SQL Server, ou `null` se não falta nada.
 *
 * Na autenticação do Windows o usuário é **opcional**: em branco vale a
 * identidade de quem está logado, que é o caminho normal do SSMS. Exigi-lo aqui
 * impediria justamente o uso mais comum.
 */
function validarSqlServer(perfil: PerfilConexao): string | null {
  if (!perfil.servidor) return t("informe o servidor");
  if (perfil.autenticacaoSql === "sqlserver" && !perfil.usuario) return t("informe o logon");
  return null;
}

/** Campos do formulário ligados diretamente a uma propriedade do perfil. */
type CampoSimples = Extract<
  keyof PerfilConexao,
  | "nome"
  | "tipoBanco"
  | "servidor"
  | "autenticacaoSql"
  | "usuario"
  | "senha"
  | "salvarSenha"
  | "atribuicao"
  | "tipoConexao"
  | "host"
  | "porta"
  | "sid"
  | "servico"
  | "aliasTns"
  | "urlPersonalizada"
  | "usarProxy"
  | "usuarioProxy"
  | "senhaProxy"
  | "autoCommit"
>;

/**
 * Campos que o bloco do SQL Server repete do bloco do Oracle.
 *
 * O perfil guarda **um** par de credenciais: "Logon" no SSMS e "Nome do
 * Usuário" no SQL Developer são o mesmo dado, e duplicá-lo faria o mesmo
 * segredo ser gravado duas vezes no DPAPI. O que se duplica é só o controle na
 * tela, porque cada bloco tem o seu layout — daí o `data-campo-sql`, que evita
 * dois elementos disputarem o mesmo `data-campo`.
 */
type CampoSql = Extract<
  keyof PerfilConexao,
  "usuario" | "senha" | "salvarSenha" | "autoCommit"
>;

/** O que o diálogo devolve quando a conexão é aberta. */
export interface ConexaoAberta {
  sessaoId: string;
  perfil: PerfilConexao;
}

/**
 * Abre o diálogo. Resolve com a conexão aberta, ou `null` se o usuário
 * cancelou.
 *
 * `perfilInicial` já deixa o formulário preenchido — é o caminho de quando a
 * conexão foi salva sem senha e o usuário precisa informá-la para conectar.
 */
export function abrirDialogoConexao(
  perfilInicial?: PerfilConexao,
): Promise<ConexaoAberta | null> {
  return new Promise((resolver) => {
    void new DialogoConexao(resolver, perfilInicial).abrir();
  });
}

class DialogoConexao {
  private readonly fundo = document.createElement("div");
  private readonly raiz: HTMLElement;
  private perfis: PerfilConexao[] = [];
  private atual: PerfilConexao = perfilNovo();
  private corEscolhidaManualmente = false;
  private ocupado = false;
  private senhaSalvaExibida = false;
  private pararEsc: () => void = () => {};

  constructor(
    private readonly resolver: (conexao: ConexaoAberta | null) => void,
    perfilInicial?: PerfilConexao,
  ) {
    this.fundo.className = "dialogo-fundo";
    this.fundo.innerHTML = molde();
    this.raiz = this.fundo.firstElementChild as HTMLElement;
    if (perfilInicial) {
      this.atual = { ...perfilInicial };
      this.corEscolhidaManualmente = Boolean(perfilInicial.cor);
    }
  }

  async abrir(): Promise<void> {
    document.body.append(this.fundo);
    this.desenharPaletaCores();
    this.ligarEventos();
    this.escreverFormulario();
    await comIndicadorDeAbertura(this.atual.nome || t("nova conexao"), () => this.carregarPerfis());

    if (this.atual.id) {
      if (!this.atual.cor) this.aplicarCorAutomatica();
      this.marcarSelecionado(this.atual.id);
      // Só falta a senha: é nela que o cursor deve cair.
      this.campoDeSenhaVisivel().focus();
      return;
    }

    // Sem perfil indicado, a primeira conexão salva já abre marcada: é a que o
    // usuário quase sempre vai abrir.
    const primeira = this.perfis[0];
    if (primeira) {
      this.selecionarPerfil(primeira);
      this.campoDeSenhaVisivel().focus();
      return;
    }

    if (!this.atual.cor) this.aplicarCorAutomatica();
    this.elemento<HTMLInputElement>('[data-campo="nome"]').focus();
  }

  // --- Consultas ao DOM ---------------------------------------------------

  private elemento<T extends Element>(seletor: string): T {
    const achado = this.raiz.querySelector<T>(seletor);
    if (!achado) throw new Error(`elemento não encontrado no diálogo: ${seletor}`);
    return achado;
  }

  private campo<T extends Element>(nome: CampoSimples): T {
    return this.elemento<T>(`[data-campo="${nome}"]`);
  }

  private campoSql<T extends Element>(nome: CampoSql): T {
    return this.elemento<T>(`[data-campo-sql="${nome}"]`);
  }

  /**
   * O banco cujo bloco está em cena.
   *
   * É o do **perfil**, e não o valor solto do `<select>`: os dois só divergem
   * dentro do próprio evento de troca, que resolve a diferença na hora
   * (ver `ligarEventos`). Com o perfil mandando, nunca se lê um bloco pelo
   * rótulo do outro.
   */
  private get bancoEscolhido(): TipoBanco {
    return this.atual.tipoBanco;
  }

  /**
   * O campo de senha do bloco visível.
   *
   * Existe porque os dois blocos têm o seu, e quem chama quer o do banco
   * escolhido — é para lá que o cursor precisa ir.
   */
  private campoDeSenhaVisivel(): HTMLInputElement {
    return this.bancoEscolhido === "oracle"
      ? this.campo<HTMLInputElement>("senha")
      : this.campoSql<HTMLInputElement>("senha");
  }

  private todos<T extends Element>(seletor: string): T[] {
    return Array.from(this.raiz.querySelectorAll<T>(seletor));
  }

  // --- Eventos -------------------------------------------------------------

  private ligarEventos(): void {
    this.pararEsc = fecharComEsc(this.fundo, () => this.fechar(null));

    for (const botao of this.todos<HTMLButtonElement>("[data-acao]")) {
      botao.addEventListener("click", () => this.executarAcao(botao.dataset.acao ?? ""));
    }

    for (const grupo of this.todos<HTMLElement>("[data-grupo]")) {
      for (const aba of Array.from(grupo.querySelectorAll<HTMLButtonElement>("[data-aba]"))) {
        aba.addEventListener("click", () => this.selecionarAba(grupo, aba.dataset.aba ?? ""));
      }
    }

    this.campo<HTMLSelectElement>("tipoBanco").addEventListener("change", () => {
      // O bloco que está saindo de cena é lido **antes** da troca: quem digitou
      // o logon e trocou o tipo por engano não pode perder o que digitou. Só
      // depois o perfil muda de banco e a tela é redesenhada — é o
      // `escreverFormulario` que espelha usuário e senha no outro bloco.
      const escolhido = this.campo<HTMLSelectElement>("tipoBanco").value as TipoBanco;
      this.lerFormulario();
      this.atual.tipoBanco = escolhido;
      // Cada banco nasce com o padrão da ferramenta de referência dele: o SQL
      // Developer abre com auto-commit desligado, o SSMS com ele ligado. A
      // diferença não é gosto — no SQL Server o auto-commit desligado abre
      // transação implícita em todo comando, `select` inclusive, e é isso que
      // impede trocar de banco na barra da aba enquanto ela estiver aberta.
      // O tipo de banco só se escolhe em conexão nova, então não há escolha
      // gravada a atropelar aqui.
      this.atual.autoCommit = escolhido === "sqlserver";
      this.escreverFormulario();
    });

    this.campo<HTMLSelectElement>("autenticacaoSql").addEventListener("change", () => {
      this.lerFormulario();
      this.aplicarAutenticacaoSql();
    });

    const servidor = this.campo<HTMLInputElement>("servidor");
    servidor.addEventListener("paste", (evento) => {
      const texto = evento.clipboardData?.getData("text/plain");
      if (texto && this.separarCredenciaisMongo(texto)) evento.preventDefault();
    });
    servidor.addEventListener("change", () => this.separarCredenciaisMongo(servidor.value));

    this.campo<HTMLSelectElement>("tipoConexao").addEventListener("change", () => {
      this.lerFormulario();
      this.aplicarTipoDeConexao();
    });

    for (const opcao of this.todos<HTMLInputElement>('input[name="identificacao"]')) {
      opcao.addEventListener("change", () => {
        this.atual.identificacao = opcao.value as IdentificacaoBanco;
        this.aplicarIdentificacao();
      });
    }

    this.campo<HTMLInputElement>("nome").addEventListener("input", (evento) => {
      this.atual.nome = (evento.target as HTMLInputElement).value.trim();
      if (!this.corEscolhidaManualmente) this.aplicarCorAutomatica();
    });

    this.elemento<HTMLInputElement>('[data-campo="cor"]').addEventListener("input", (evento) => {
      this.definirCor((evento.target as HTMLInputElement).value, true);
    });

    for (const marcacao of [
      this.campo<HTMLInputElement>("salvarSenha"),
      this.campoSql<HTMLInputElement>("salvarSenha"),
    ]) {
      marcacao.addEventListener("change", () => this.atualizarIndicacaoDeSenhaSalva());
    }
    for (const campo of this.todos<HTMLInputElement>('[data-campo="senha"], [data-campo-sql="senha"]')) {
      campo.addEventListener("input", () => {
        this.senhaSalvaExibida = false;
        this.atualizarIndicacaoDeSenhaSalva();
      });
    }
  }

  private executarAcao(acao: string): void {
    switch (acao) {
      case "cancelar":
        this.fechar(null);
        break;
      case "salvar":
        void this.salvar();
        break;
      case "excluir":
        void this.excluir();
        break;
      case "importar":
        void this.importar();
        break;
      case "testar":
        void this.testar();
        break;
      case "conectar":
        void this.conectar();
        break;
      case "mostrar-senha":
        void this.acessarSenha(false);
        break;
      case "copiar-senha":
        void this.acessarSenha(true);
        break;
    }
  }

  private selecionarAba(grupo: HTMLElement, aba: string): void {
    for (const botao of Array.from(grupo.querySelectorAll<HTMLButtonElement>("[data-aba]"))) {
      botao.setAttribute("aria-selected", String(botao.dataset.aba === aba));
    }
    const painelDoGrupo = grupo.parentElement;
    if (!painelDoGrupo) return;
    for (const painel of Array.from(painelDoGrupo.querySelectorAll<HTMLElement>("[data-painel]"))) {
      painel.hidden = painel.dataset.painel !== aba;
    }
  }

  // --- Lista de conexões salvas -------------------------------------------

  private async carregarPerfis(): Promise<void> {
    try {
      this.perfis = await api.listarPerfis();
      this.desenharLista();
    } catch (erro) {
      this.escreverStatus(
        t("falha ao ler conexoes", { erro: api.mensagemDeErro(erro) }),
        "erro",
      );
    }
  }

  private desenharLista(): void {
    const corpo = this.elemento<HTMLTableSectionElement>('[data-elemento="lista"]');
    corpo.replaceChildren();

    if (this.perfis.length === 0) {
      const linha = corpo.insertRow();
      const celula = linha.insertCell();
      celula.className = "lista-conexoes__vazia";
      celula.textContent = t("nenhuma conexao salva");
    }

    for (const perfil of this.perfis) {
      const linha = corpo.insertRow();
      linha.dataset.id = perfil.id;
      linha.addEventListener("click", () => this.selecionarPerfil(perfil));
      linha.addEventListener("dblclick", () => void this.conectar());

      const celulaNome = linha.insertCell();
      if (perfil.cor) {
        const marca = document.createElement("span");
        marca.className = "marca-cor";
        marca.style.background = perfil.cor;
        celulaNome.append(marca);
      }
      celulaNome.append(perfil.nome);
      // A lista tem só a coluna do nome; os detalhes ficam na dica do mouse.
      celulaNome.title = `${perfil.nome}\n${detalhesDaConexao(perfil)}`;
    }

    corpo.append(this.linhaNovaConexao());
  }

  /** Linha logo abaixo da última conexão: o atalho para cadastrar outra. */
  private linhaNovaConexao(): HTMLTableRowElement {
    const linha = document.createElement("tr");
    const celula = linha.insertCell();
    celula.className = "lista-conexoes__celula-nova";

    const botao = document.createElement("button");
    botao.type = "button";
    botao.className = "lista-conexoes__nova";
    botao.textContent = t("nova conexao lista");
    botao.title = t("cadastrar nova conexao");
    botao.setAttribute("aria-pressed", String(!this.atual.id));
    botao.addEventListener("click", () => this.novaConexao());

    celula.append(botao);
    return linha;
  }

  /** Limpa o formulário para cadastrar uma conexão do zero. */
  private novaConexao(): void {
    // Trocar o perfil no meio de um Conectar/Salvar faria a operação terminar
    // sobre outro perfil.
    if (this.ocupado) return;

    this.atual = perfilNovo();
    this.corEscolhidaManualmente = false;
    this.aplicarCorAutomatica();
    this.marcarSelecionado(null);
    this.escreverFormulario();
    this.escreverStatus("");
    this.campo<HTMLInputElement>("nome").focus();
  }

  private selecionarPerfil(perfil: PerfilConexao): void {
    this.atual = { ...perfil };
    this.corEscolhidaManualmente = Boolean(perfil.cor);
    if (!this.atual.cor) this.aplicarCorAutomatica();
    this.marcarSelecionado(perfil.id);
    this.escreverFormulario();
    this.escreverStatus("");
  }

  private marcarSelecionado(id: string | null): void {
    for (const linha of this.todos<HTMLTableRowElement>("[data-id]")) {
      linha.setAttribute("aria-selected", String(linha.dataset.id === id));
    }
    // Sem conexão marcada, o destaque fica no atalho de cadastro.
    this.raiz
      .querySelector(".lista-conexoes__nova")
      ?.setAttribute("aria-pressed", String(!id));
  }

  // --- Formulário ----------------------------------------------------------

  /** Copia o perfil atual para os controles da tela. */
  private escreverFormulario(): void {
    const perfil = this.atual;
    this.senhaSalvaExibida = false;

    this.campo<HTMLInputElement>("nome").value = perfil.nome;
    this.campo<HTMLSelectElement>("tipoBanco").value = perfil.tipoBanco;

    // Usuário, senha e "salvar senha" são um dado só do perfil, mostrado nos
    // dois blocos: quem troca de banco no meio do cadastro não redigita nada.
    for (const usuario of [
      this.campo<HTMLInputElement>("usuario"),
      this.campoSql<HTMLInputElement>("usuario"),
    ]) {
      usuario.value = perfil.usuario;
    }
    for (const senha of [
      this.campo<HTMLInputElement>("senha"),
      this.campoSql<HTMLInputElement>("senha"),
    ]) {
      senha.type = "password";
      senha.value = perfil.senha ?? "";
    }
    for (const salvar of [
      this.campo<HTMLInputElement>("salvarSenha"),
      this.campoSql<HTMLInputElement>("salvarSenha"),
    ]) {
      salvar.checked = perfil.salvarSenha;
    }
    for (const commit of [
      this.campo<HTMLInputElement>("autoCommit"),
      this.campoSql<HTMLInputElement>("autoCommit"),
    ]) {
      commit.checked = perfil.autoCommit;
    }

    this.campo<HTMLInputElement>("servidor").value = perfil.servidor;
    this.campo<HTMLSelectElement>("autenticacaoSql").value = perfil.autenticacaoSql;
    this.atualizarIndicacaoDeSenhaSalva();
    this.campo<HTMLSelectElement>("atribuicao").value = perfil.atribuicao;
    this.campo<HTMLSelectElement>("tipoConexao").value = perfil.tipoConexao;
    this.campo<HTMLInputElement>("host").value = perfil.host;
    this.campo<HTMLInputElement>("porta").value = String(perfil.porta);
    this.campo<HTMLInputElement>("sid").value = perfil.sid;
    this.campo<HTMLInputElement>("servico").value = perfil.servico;
    this.campo<HTMLInputElement>("aliasTns").value = perfil.aliasTns;
    this.campo<HTMLInputElement>("urlPersonalizada").value = perfil.urlPersonalizada;
    this.campo<HTMLInputElement>("usarProxy").checked = perfil.usarProxy;
    this.campo<HTMLInputElement>("usuarioProxy").value = perfil.usuarioProxy;
    this.campo<HTMLInputElement>("senhaProxy").value = perfil.senhaProxy ?? "";

    this.elemento<HTMLInputElement>('[data-campo="cor"]').value = perfil.cor ?? COR_PADRAO;
    this.atualizarSelecaoDaPaleta();
    for (const opcao of this.todos<HTMLInputElement>('input[name="identificacao"]')) {
      opcao.checked = opcao.value === perfil.identificacao;
    }

    this.aplicarTipoDeBanco();
    this.aplicarTipoDeConexao();
    this.aplicarIdentificacao();
    this.atualizarDisponibilidadeAcoes();
  }

  /** Excluir só faz sentido para um perfil que já existe no disco. */
  private atualizarDisponibilidadeAcoes(): void {
    const excluir = this.elemento<HTMLButtonElement>('[data-acao="excluir"]');
    excluir.disabled = this.ocupado || !this.atual.id;

    // O tipo de banco só se escolhe **na criação**. Trocá-lo numa conexão já
    // salva não é editar um campo: é trocar a conexão por outra, com outros
    // dados, outro driver e outro servidor — e deixaria para trás o que a
    // conexão tinha do banco anterior. Quem precisa do outro banco cadastra
    // uma conexão nova.
    const tipoBanco = this.campo<HTMLSelectElement>("tipoBanco");
    tipoBanco.disabled = Boolean(this.atual.id);
    tipoBanco.title = this.atual.id ? t("tipo de banco so em conexao nova") : "";
  }

  private desenharPaletaCores(): void {
    const paleta = this.elemento<HTMLElement>('[data-elemento="paleta-cores"]');
    for (const opcao of PALETA_CORES) {
      const botao = document.createElement("button");
      botao.type = "button";
      botao.className = "paleta-cores__opcao";
      botao.dataset.cor = opcao.valor;
      botao.style.setProperty("--amostra-cor", opcao.valor);
      const uso = opcao.ambiente ? ` — reservada para ${opcao.ambiente}` : "";
      botao.title = `${opcao.nome}${uso}`;
      botao.setAttribute("aria-label", `${opcao.nome}${uso}`);
      botao.setAttribute("aria-pressed", "false");
      botao.addEventListener("click", () => this.definirCor(opcao.valor, true));
      paleta.append(botao);
    }
  }

  private definirCor(cor: string, manual: boolean): void {
    this.atual.cor = cor.toUpperCase();
    this.corEscolhidaManualmente = manual;
    this.elemento<HTMLInputElement>('[data-campo="cor"]').value = cor.toLowerCase();
    this.atualizarSelecaoDaPaleta();
  }

  private atualizarSelecaoDaPaleta(): void {
    const atual = this.atual.cor?.toUpperCase() ?? null;
    for (const botao of this.todos<HTMLButtonElement>(".paleta-cores__opcao")) {
      const selecionada = botao.dataset.cor?.toUpperCase() === atual;
      botao.setAttribute("aria-pressed", String(selecionada));
    }
  }

  private aplicarCorAutomatica(): void {
    const reservada = corReservadaParaAmbiente(this.atual.nome);
    this.definirCor(reservada ?? this.primeiraCorDisponivel(), false);
  }

  private primeiraCorDisponivel(): string {
    const usos = new Map(PALETA_CORES.map(({ valor }) => [valor, 0]));
    for (const perfil of this.perfis) {
      if (perfil.id === this.atual.id || !perfil.cor) continue;
      const cor = perfil.cor.toUpperCase();
      if (usos.has(cor)) usos.set(cor, (usos.get(cor) ?? 0) + 1);
    }

    return PALETA_CORES.reduce((melhor, opcao) =>
      (usos.get(opcao.valor) ?? 0) < (usos.get(melhor.valor) ?? 0) ? opcao : melhor,
    ).valor;
  }

  /**
   * Os asteriscos indicam que há uma senha guardada, sem preencher o campo.
   */
  private atualizarIndicacaoDeSenhaSalva(): void {
    const manter =
      this.bancoEscolhido === "oracle"
        ? this.campo<HTMLInputElement>("salvarSenha").checked
        : this.campoSql<HTMLInputElement>("salvarSenha").checked;
    const campo = this.campoDeSenhaVisivel();
    const acoes = campo.parentElement!;
    const mostrar = acoes.querySelector<HTMLButtonElement>('[data-acao="mostrar-senha"]')!;
    const copiar = acoes.querySelector<HTMLButtonElement>('[data-acao="copiar-senha"]')!;
    const visivel = campo.type === "text";
    const disponivel = Boolean(campo.value || this.atual.temSenhaSalva);
    campo.placeholder = this.atual.temSenhaSalva && manter && !visivel ? "************" : "";
    mostrar.textContent = t(visivel ? "ocultar senha" : "mostrar senha");
    mostrar.disabled = this.ocupado || (!disponivel && !visivel);
    copiar.disabled = this.ocupado || !disponivel;
  }

  private async acessarSenha(copiar: boolean): Promise<void> {
    const campo = this.campoDeSenhaVisivel();
    const perfil = this.atual;
    if (!copiar && campo.type === "text") {
      campo.type = "password";
      if (this.senhaSalvaExibida) campo.value = "";
      this.senhaSalvaExibida = false;
      this.atualizarIndicacaoDeSenhaSalva();
      return;
    }
    await this.comBloqueio(async () => {
      const digitada = campo.value;
      const senha = digitada || (perfil.temSenhaSalva ? await api.lerSenhaSalva(perfil.id) : null);
      // O usuário pode fechar o diálogo, trocar de perfil ou editar o campo
      // enquanto a leitura do DPAPI está em curso.
      if (!this.fundo.isConnected || this.atual !== perfil || campo.value !== digitada) return;
      if (!senha) {
        this.escreverStatus(t("senha indisponivel"), "erro");
        return;
      }
      if (copiar) {
        await navigator.clipboard.writeText(senha);
        if (this.atual === perfil) this.escreverStatus(t("senha copiada"), "sucesso");
      } else {
        this.senhaSalvaExibida = !digitada;
        campo.value = senha;
        campo.type = "text";
      }
    });
  }

  private senhaDigitada(): string | null {
    return this.senhaSalvaExibida ? null : this.campoDeSenhaVisivel().value || null;
  }

  /** Copia os controles da tela para o perfil atual. */
  private lerFormulario(): void {
    const perfil = this.atual;

    perfil.nome = this.campo<HTMLInputElement>("nome").value.trim();

    // Só o bloco visível é lido. O outro continua na tela com o layout do banco
    // que não está escolhido, e ler dali gravaria no perfil um campo que o
    // usuário não vê — o host do Oracle numa conexão de SQL Server, por exemplo.
    if (perfil.tipoBanco !== "oracle") {
      this.lerBlocoSqlServer(perfil);
    } else {
      this.lerBlocoOracle(perfil);
    }
  }

  /** Move as credenciais coladas para os campos próprios antes de ler o perfil. */
  private separarCredenciaisMongo(texto: string): boolean {
    if (this.bancoEscolhido !== "mongodb") return false;

    // Mantém hosts, banco e opções como vieram, inclusive listas de hosts.
    const partes = /^(mongodb(?:\+srv)?:\/\/)([^:@/?#]+)(?::([^@/?#]*))?@([^@/?#]+)([/?#].*)?$/.exec(texto.trim());
    if (!partes) return false;
    const [, esquema, usuarioCodificado, senhaCodificada, hosts, restante = ""] = partes;
    let usuario: string;
    let senha: string | undefined;
    try {
      usuario = decodeURIComponent(usuarioCodificado);
      senha = senhaCodificada === undefined ? undefined : decodeURIComponent(senhaCodificada);
    } catch {
      return false;
    }
    if (/[\r\n]/.test(usuario) || (senha !== undefined && /[\r\n]/.test(senha))) return false;

    this.campo<HTMLInputElement>("servidor").value = esquema + hosts + restante;
    this.campoSql<HTMLInputElement>("usuario").value = usuario;
    if (senha !== undefined) {
      const campo = this.campoDeSenhaVisivel();
      campo.type = "password";
      campo.value = senha;
      this.senhaSalvaExibida = false;
      this.atualizarIndicacaoDeSenhaSalva();
    }
    return true;
  }

  private lerBlocoSqlServer(perfil: PerfilConexao): void {
    this.separarCredenciaisMongo(this.campo<HTMLInputElement>("servidor").value);
    perfil.servidor = this.campo<HTMLInputElement>("servidor").value.trim();
    perfil.autenticacaoSql = this.campo<HTMLSelectElement>("autenticacaoSql")
      .value as AutenticacaoSql;
    const usuario = this.campoSql<HTMLInputElement>("usuario").value;
    perfil.usuario = perfil.tipoBanco === "mongodb" ? usuario : usuario.trim();
    perfil.senha = this.senhaDigitada();
    perfil.salvarSenha = this.campoSql<HTMLInputElement>("salvarSenha").checked;
    perfil.autoCommit = this.campoSql<HTMLInputElement>("autoCommit").checked;
  }

  private lerBlocoOracle(perfil: PerfilConexao): void {
    perfil.usuario = this.campo<HTMLInputElement>("usuario").value.trim();
    perfil.senha = this.senhaDigitada();
    perfil.salvarSenha = this.campo<HTMLInputElement>("salvarSenha").checked;
    perfil.atribuicao = this.campo<HTMLSelectElement>("atribuicao").value as Atribuicao;
    perfil.tipoConexao = this.campo<HTMLSelectElement>("tipoConexao").value as TipoConexao;
    perfil.host = this.campo<HTMLInputElement>("host").value.trim();
    perfil.porta = Number(this.campo<HTMLInputElement>("porta").value) || 1521;
    perfil.sid = this.campo<HTMLInputElement>("sid").value.trim();
    perfil.servico = this.campo<HTMLInputElement>("servico").value.trim();
    perfil.aliasTns = this.campo<HTMLInputElement>("aliasTns").value.trim();
    perfil.urlPersonalizada = this.campo<HTMLInputElement>("urlPersonalizada").value.trim();
    perfil.usarProxy = this.campo<HTMLInputElement>("usarProxy").checked;
    perfil.usuarioProxy = this.campo<HTMLInputElement>("usuarioProxy").value.trim();
    perfil.senhaProxy = this.campo<HTMLInputElement>("senhaProxy").value || null;
    perfil.autoCommit = this.campo<HTMLInputElement>("autoCommit").checked;

    const marcado = this.todos<HTMLInputElement>('input[name="identificacao"]').find((o) => o.checked);
    if (marcado) perfil.identificacao = marcado.value as IdentificacaoBanco;
  }

  /**
   * Mostra os campos do banco escolhido e esconde os do outro.
   *
   * Os dois layouts ficam montados desde a abertura e o que muda é só o
   * `hidden`: trocar de banco não recria elemento nenhum, e o que está oculto
   * não custa layout nem pintura.
   */
  private aplicarTipoDeBanco(): void {
    const banco = this.atual.tipoBanco;
    for (const bloco of this.todos<HTMLElement>("[data-banco]")) {
      bloco.hidden = !bloco.dataset.banco?.split(" ").includes(banco);
    }
    for (const campo of this.todos<HTMLElement>("[data-so-sql]")) {
      campo.hidden = banco === "mongodb";
    }
    this.elemento<HTMLElement>('[data-elemento="rotulo-servidor"]').textContent =
      t(banco === "mongodb" ? "uri mongodb" : "nome do servidor");
    this.campo<HTMLInputElement>("servidor").placeholder =
      banco === "mongodb" ? "mongodb://localhost:27017" : t("servidor exemplo");
    this.aplicarAutenticacaoSql();
    this.atualizarIndicacaoDeSenhaSalva();
  }

  /**
   * Ajusta o rótulo da credencial do SQL Server ao modo de autenticação.
   *
   * No SSMS os dois modos pedem o mesmo par usuário/senha e só o rótulo muda —
   * "Logon" na autenticação do SQL Server, "Nome de usuário" na do Windows. Na
   * do Windows o campo continua editável de propósito: em branco vale a
   * identidade de quem está conectado, e preenchido vale outra credencial do
   * domínio.
   */
  private aplicarAutenticacaoSql(): void {
    const mongo = this.bancoEscolhido === "mongodb";
    const windows = this.campo<HTMLSelectElement>("autenticacaoSql").value === "windows";
    this.elemento<HTMLElement>('[data-elemento="rotulo-usuario-sql"]').textContent = mongo || windows
      ? t("nome de usuario")
      : t("logon");
    this.campoSql<HTMLInputElement>("usuario").placeholder = mongo ? t("opcional") : windows
      ? t("usuario do windows exemplo")
      : "";
  }

  /** Mostra o bloco de campos correspondente ao Tipo de Conexão. */
  private aplicarTipoDeConexao(): void {
    const tipo = this.campo<HTMLSelectElement>("tipoConexao").value;
    for (const bloco of this.todos<HTMLElement>("[data-bloco]")) {
      bloco.hidden = bloco.dataset.bloco !== tipo;
    }
  }

  /** SID e Nome do Serviço são excludentes: só o escolhido fica editável. */
  private aplicarIdentificacao(): void {
    const porSid = this.atual.identificacao === "sid";
    this.campo<HTMLInputElement>("sid").disabled = !porSid;
    this.campo<HTMLInputElement>("servico").disabled = porSid;
  }

  // --- Ações ---------------------------------------------------------------

  private validar(exigirCor: boolean): string | null {
    const perfil = this.atual;
    if (!perfil.nome) return t("informe o nome da conexao");
    if (exigirCor && !perfil.cor) return t("escolha uma cor");
    if (exigirCor && !corAdequadaParaIdentificacao(perfil.cor)) {
      return t("cor recusada");
    }

    if (perfil.tipoBanco === "mongodb") {
      const endereco = /^mongodb(?:\+srv)?:\/\/([^/?]+)/.exec(perfil.servidor)?.[1];
      if (!endereco || endereco.includes("@")) return t("uri mongodb invalida");
      if (perfil.senha && !perfil.usuario) return t("usuario mongodb necessario");
      return null;
    }
    if (perfil.tipoBanco === "sqlserver") return validarSqlServer(perfil);

    if (!perfil.usuario) return t("informe o usuario");

    if (perfil.tipoConexao === "basico") {
      if (!perfil.host) return t("informe o host");
      if (perfil.porta < 1 || perfil.porta > 65535) return t("porta fora da faixa");
      if (perfil.identificacao === "sid" && !perfil.sid) return t("informe o sid");
      if (perfil.identificacao === "servico" && !perfil.servico) return t("informe o servico");
    }
    if (perfil.tipoConexao === "tns" && !perfil.aliasTns) return t("informe o alias");
    if (perfil.tipoConexao === "avancado" && !perfil.urlPersonalizada) {
      return t("informe a url");
    }
    return null;
  }

  /**
   * Traz conexões já existentes na máquina (requisito 25).
   *
   * A senha nunca vem junto: o SQL Developer guarda a dele cifrada com chave
   * própria, e o tnsnames.ora não tem senha.
   */
  private async importar(): Promise<void> {
    await this.comBloqueio(async () => {
      const encontradas = await api.procurarConexoesImportaveis();
      if (encontradas.length === 0) {
        this.escreverStatus(
          t("nenhuma conexao importavel"),
        );
        return;
      }
      const escolhida = await escolherConexaoImportavel(encontradas);
      if (!escolhida) return;

      this.atual = { ...perfilNovo(), ...escolhida.perfil, id: "" };
      this.corEscolhidaManualmente = Boolean(this.atual.cor);
      if (!this.atual.cor) this.aplicarCorAutomatica();
      this.marcarSelecionado(null);
      this.escreverFormulario();
      this.escreverStatus(
        `Importada de ${escolhida.origem}. Informe a senha e clique em Salvar.`,
      );
      this.campoDeSenhaVisivel().focus();
    });
  }

  private async salvar(): Promise<void> {
    this.lerFormulario();
    const problema = this.validar(true);
    if (problema) {
      this.escreverStatus(problema, "erro");
      return;
    }
    await this.comBloqueio(async () => {
      const salvo = await api.salvarPerfil(this.atual);
      this.atual = { ...salvo };
      this.corEscolhidaManualmente = true;
      // O backend confirma `temSenhaSalva` sem devolver o segredo. Reescrever
      // limpa a senha digitada da tela e deixa somente o placeholder visual.
      this.escreverFormulario();
      await this.carregarPerfis();
      this.marcarSelecionado(salvo.id);
      this.escreverStatus(`Conexão "${salvo.nome}" salva.`, "sucesso");
    });
  }

  private async excluir(): Promise<void> {
    if (!this.atual.id) return;
    const id = this.atual.id;
    const nomeSalvo = this.perfis.find((perfil) => perfil.id === id)?.nome ?? this.atual.nome;
    if (!window.confirm(`Excluir a conexão "${nomeSalvo}"?`)) return;

    await this.comBloqueio(async () => {
      await api.excluirPerfil(id);
      this.atual = perfilNovo();
      this.corEscolhidaManualmente = false;
      await this.carregarPerfis();
      this.aplicarCorAutomatica();
      this.marcarSelecionado(null);
      this.escreverFormulario();
      this.escreverStatus(t("conexao excluida", { conexao: nomeSalvo }), "sucesso");
      this.campo<HTMLInputElement>("nome").focus();
    });
  }

  private async testar(): Promise<void> {
    this.lerFormulario();
    const problema = this.validar(false);
    if (problema) {
      this.escreverStatus(problema, "erro");
      return;
    }
    this.escreverStatus(t("testando"));
    await this.comBloqueio(async () => {
      const resultado = await api.testarConexao(this.atual);
      if (resultado.sucesso) {
        const versao = resultado.versaoBanco ? ` — ${resultado.versaoBanco}` : "";
        this.escreverStatus(
          `Com êxito em ${formatarDuracao(resultado.duracaoMs)}${versao}`,
          "sucesso",
        );
      } else {
        this.escreverStatus(resultado.mensagem, "erro");
      }
    });
  }

  private async conectar(): Promise<void> {
    this.lerFormulario();
    const problema = this.validar(true);
    if (problema) {
      this.escreverStatus(problema, "erro");
      return;
    }
    this.escreverStatus(t("conectando"));
    await this.comBloqueio(async () => {
      const sessaoId = await api.conectar(this.atual);
      try {
        const salvo = await api.salvarPerfil(this.atual);
        this.atual = { ...salvo };
        this.fechar({ sessaoId, perfil: this.atual });
      } catch (erro) {
        // Se o perfil não puder ser salvo, o diálogo permanece aberto e a
        // sessão recém-criada é encerrada para não ficar órfã.
        try {
          await api.desconectar(sessaoId);
        } catch {
          // Mantém o erro original do salvamento para exibição no status.
        }
        throw erro;
      }
    });
  }

  /** Desabilita os botões enquanto uma operação de banco está em curso. */
  private async comBloqueio(operacao: () => Promise<void>): Promise<void> {
    if (this.ocupado) return;
    this.ocupado = true;
    this.atualizarIndicacaoDeSenhaSalva();
    const botoes = this.todos<HTMLButtonElement>(".dialogo__botoes button");
    for (const botao of botoes) botao.disabled = true;
    try {
      await operacao();
    } catch (erro) {
      this.escreverStatus(api.mensagemDeErro(erro), "erro");
    } finally {
      for (const botao of botoes) botao.disabled = false;
      this.ocupado = false;
      this.atualizarDisponibilidadeAcoes();
      this.atualizarIndicacaoDeSenhaSalva();
    }
  }

  private escreverStatus(texto: string, tipo?: "erro" | "sucesso"): void {
    const alvo = this.elemento<HTMLElement>('[data-elemento="status"]');
    alvo.textContent = texto;
    const faixa = this.elemento<HTMLElement>(".status");
    faixa.className = tipo ? `status status--${tipo}` : "status";
  }

  private fechar(resultado: ConexaoAberta | null): void {
    this.pararEsc();
    this.fundo.remove();
    this.resolver(resultado);
  }
}
