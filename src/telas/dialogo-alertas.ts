import * as api from "../api";
import { confirmar } from "../componentes/dialogo-simples";
import { fecharComEsc } from "../componentes/janela";
import { t } from "../idioma";
import type { ContextoDeConexao } from "./arvore-objetos";
import type { MonitorDeAlertas } from "./monitor-de-alertas";

const ERROS_DE_ALERTA = [
  "alerta banco obrigatorio",
  "alerta campos invalidos",
  "alerta repeticao invalida",
  "alerta consulta invalida",
  "alerta webhook invalido",
  "alerta inexistente",
] as const;

export function abrirDialogoDeAlertas(
  { sessaoId, perfil }: ContextoDeConexao,
  monitor: MonitorDeAlertas,
  regraInicial: api.AlertaSalvo | null,
  aoAlterar: () => void,
): void {
  const anterior = document.activeElement;
  const fundo = document.createElement("div");
  fundo.className = "dialogo-fundo";
  fundo.innerHTML = `
    <form class="dialogo dialogo-alertas" role="dialog" aria-modal="true" aria-labelledby="titulo-alertas">
      <header class="dialogo__barra">
        <h1 id="titulo-alertas"></h1>
        <button type="button" class="dialogo__fechar" aria-label="${t("fechar")}">×</button>
      </header>
      <fieldset class="alertas__campos" disabled>
        <label>${t("nome")}<input type="text" data-campo="nome" required></label>
        <div class="alertas__linha">
          <label>${t("alerta intervalo")}
            <input type="number" data-campo="minutos" min="1" max="1440" required>
          </label>
          <label>${t("alerta repetir apos")}
            <input type="number" data-campo="repetir" min="1" max="1440"
                   aria-describedby="ajuda-repeticao-alerta">
          </label>
          <label data-campo="rotulo-banco">${t("banco de dados")}<select data-campo="banco"></select></label>
        </div>
        <p id="ajuda-repeticao-alerta" class="alertas__ajuda">${t("alerta repetir ajuda")}</p>
        <label>${t("alerta consulta")}
          <textarea data-campo="consulta" rows="6" spellcheck="false" required></textarea>
        </label>
        <p class="alertas__ajuda">${t("alerta criterio")}</p>
        <p class="alertas__ajuda" data-campo="aviso-legado" hidden>${t("alerta legado ajuda")}</p>
        <label class="alertas__marca"><input type="checkbox" data-campo="ativo"> ${t("alerta ativo")}</label>
        <label class="alertas__marca"><input type="checkbox" data-campo="teams"> ${t("alerta enviar teams")}</label>
        <div data-campo="painel-teams" hidden>
          <label>${t("alerta url teams")}
            <input type="text" data-campo="webhook" autocomplete="off" spellcheck="false"
                   aria-describedby="aviso-url-alerta">
          </label>
          <p id="aviso-url-alerta" class="alertas__ajuda"><strong>${t("alerta webhook segredo")}</strong></p>
          <p class="alertas__ajuda">${t("alerta webhook ajuda")}</p>
        </div>
        <p class="alertas__ajuda">${t("alerta conexao aberta")}</p>
      </fieldset>
      <footer class="dialogo__rodape">
        <output class="status" aria-live="polite"></output>
        <button type="button" data-campo="excluir" disabled>${t("excluir")}</button>
        <button type="submit" class="botao-principal" disabled>${t("salvar")}</button>
      </footer>
    </form>`;
  const campo = <T extends HTMLElement>(nome: string): T =>
    fundo.querySelector<T>(`[data-campo="${nome}"]`)!;
  const form = fundo.querySelector("form")!;
  const campos = fundo.querySelector("fieldset")!;
  const fechar = fundo.querySelector<HTMLButtonElement>(".dialogo__fechar")!;
  const salvar = fundo.querySelector<HTMLButtonElement>('button[type="submit"]')!;
  const status = fundo.querySelector("output")!;
  const excluir = campo<HTMLButtonElement>("excluir");
  const nome = campo<HTMLInputElement>("nome");
  const minutos = campo<HTMLInputElement>("minutos");
  const repetir = campo<HTMLInputElement>("repetir");
  const banco = campo<HTMLSelectElement>("banco");
  const consulta = campo<HTMLTextAreaElement>("consulta");
  const ativo = campo<HTMLInputElement>("ativo");
  const teams = campo<HTMLInputElement>("teams");
  const webhook = campo<HTMLInputElement>("webhook");
  const painelTeams = campo("painel-teams");
  const avisoLegado = campo("aviso-legado");
  const sqlServer = perfil.tipoBanco === "sqlserver";
  const atual = regraInicial;
  let webhookSalvo = "";
  let ocupado = false;
  let carregado = false;
  fundo.querySelector("h1")!.textContent = t("alertas titulo", { conexao: perfil.nome });
  campo("rotulo-banco").hidden = !sqlServer;
  banco.required = sqlServer;

  const encerrar = () => {
    if (ocupado) return;
    pararEsc();
    fundo.remove();
    if (anterior instanceof HTMLElement) anterior.focus();
  };
  const pararEsc = fecharComEsc(fundo, encerrar, () => !ocupado);
  fechar.addEventListener("click", encerrar);
  document.body.append(fundo);
  fechar.focus();

  const informar = (mensagem = "", erro = false) => {
    status.textContent = mensagem;
    status.classList.toggle("alertas__erro", erro);
  };
  const informarErro = (erro: unknown) => {
    const mensagem = api.mensagemDeErro(erro);
    const chave = ERROS_DE_ALERTA.find((chave) => chave === mensagem);
    informar(chave ? t(chave) : mensagem, true);
  };
  const atualizarBotoes = () => {
    campos.disabled = ocupado || !carregado;
    salvar.disabled = campos.disabled;
    excluir.disabled = campos.disabled || !atual;
    fechar.disabled = ocupado;
  };
  const aplicarTeams = () => {
    painelTeams.hidden = !teams.checked;
    webhook.required = teams.checked;
  };

  nome.value = atual?.nome ?? "";
  minutos.value = String(atual?.minutos ?? 5);
  repetir.value = atual?.repetirApos?.toString() ?? "";
  consulta.value = atual?.consulta ?? "";
  ativo.checked = atual?.ativo ?? true;
  teams.checked = atual?.temTeams ?? false;
  avisoLegado.hidden = !atual?.contagemLegada;
  aplicarTeams();

  const aplicarBancos = (bancos: string[]) => {
    const alvo = atual?.banco;
    const opcoes = alvo && !bancos.includes(alvo) ? [alvo, ...bancos] : bancos;
    banco.replaceChildren(new Option("—", ""), ...opcoes.map((nome) => new Option(nome, nome)));
    banco.value = alvo ?? "";
  };
  teams.addEventListener("change", aplicarTeams);
  form.addEventListener("submit", async (evento) => {
    evento.preventDefault();
    if (ocupado) return;
    ocupado = true;
    atualizarBotoes();
    informar();
    try {
      const regra = await api.salvarAlerta(sessaoId, {
        id: atual?.id ?? "",
        nome: nome.value,
        minutos: minutos.valueAsNumber,
        repetirApos: repetir.value ? repetir.valueAsNumber : null,
        banco: sqlServer ? banco.value : null,
        consulta: consulta.value,
        ativo: ativo.checked,
      }, teams.checked, webhook.value.trim() === webhookSalvo ? "" : webhook.value);
      monitor.atualizar(regra);
    } catch (erro) {
      informarErro(erro);
      return;
    } finally {
      ocupado = false;
      atualizarBotoes();
    }
    encerrar();
    aoAlterar();
  });
  excluir.addEventListener("click", async () => {
    const regra = atual;
    if (!regra || ocupado) return;
    if (!await confirmar({
      titulo: t("alertas"), mensagem: t("alerta excluir pergunta", { nome: regra.nome }),
    })) return;
    ocupado = true;
    atualizarBotoes();
    informar();
    try {
      await api.excluirAlerta(sessaoId, regra.id);
      monitor.remover(regra.id);
      ocupado = false;
      encerrar();
      aoAlterar();
    } catch (erro) {
      informarErro(erro);
    } finally {
      ocupado = false;
      atualizarBotoes();
    }
  });

  void (async () => {
    const [bancos, url] = await Promise.allSettled([
      sqlServer ? api.listarBancos(sessaoId, true) : Promise.resolve([]),
      atual?.temTeams ? api.lerWebhookAlerta(sessaoId, atual.id) : Promise.resolve(null),
      monitor.pronto(),
    ]);
    if (!fundo.isConnected) return;
    aplicarBancos(bancos.status === "fulfilled" ? bancos.value : []);
    if (bancos.status === "rejected") informarErro(bancos.reason);
    if (url.status === "fulfilled") webhook.value = webhookSalvo = url.value ?? "";
    else informarErro(url.reason);
    carregado = true;
    atualizarBotoes();
    nome.focus();
  })();
}
