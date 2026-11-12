/**
 * Idioma da interface (requisito 31).
 *
 * A regra de língua do projeto continua valendo para **código, comentário e
 * documentação**: tudo em português. O que passa a ser traduzível é o texto que
 * o usuário lê na tela, e só ele.
 *
 * Termo técnico de uso corrente não se traduz em idioma nenhum — `commit`,
 * `select`, `grid`, `fetch`, `script`, `log`, `DDL`, `bind`. Em português eles
 * já estavam no original; em inglês e espanhol continuam iguais. É a mesma
 * regra de premissas.md, aplicada às três línguas.
 *
 * O texto sai daqui por uma chave. A chave é o texto em português, em minúsculas
 * e sem acento, porque assim o lugar de uso continua legível — `t("nova consulta
 * sql")` diz o que devolve sem precisar consultar o catálogo.
 */

/** Os três idiomas oferecidos. */
export type Idioma = "en" | "es" | "pt-BR";

/** Como cada idioma se chama — no próprio idioma, como manda o costume. */
export const IDIOMAS: { codigo: Idioma; nome: string }[] = [
  { codigo: "en", nome: "English" },
  { codigo: "es", nome: "Español" },
  { codigo: "pt-BR", nome: "PT-BR" },
];

const CHAVE_ARMAZENAMENTO = "cururu.idioma";
const EVENTO_DE_ALTERACAO = "cururu:idioma-alterado";

/**
 * Idioma de partida quando não há escolha guardada: o da máquina, quando for um
 * dos três; **inglês** quando não for.
 *
 * A regra é a mesma do instalador (requisito 31): português vira português,
 * espanhol vira espanhol, e o resto do mundo recebe inglês. Cair em português
 * por padrão, como era antes, entregava a um alemão ou a um francês uma tela
 * que ele não lê — e, pior, com o nome do idioma escrito nela também em
 * português. O inglês é a escolha que mais gente consegue ler para achar o
 * caminho até a opção **Language**.
 */
function idiomaDoSistema(): Idioma {
  const preferido = navigator.language?.toLowerCase() ?? "";
  if (preferido.startsWith("pt")) return "pt-BR";
  if (preferido.startsWith("es")) return "es";
  return "en";
}

function lerGuardado(): Idioma {
  const salvo = localStorage.getItem(CHAVE_ARMAZENAMENTO);
  if (salvo === "en" || salvo === "es" || salvo === "pt-BR") return salvo;
  return idiomaDoSistema();
}

let atual: Idioma = lerGuardado();

export function idiomaAtual(): Idioma {
  return atual;
}

/**
 * Já existe escolha feita **por quem usa**, em vez do palpite do sistema?
 *
 * É o que separa a primeira abertura das demais: só na primeira o idioma
 * escolhido no instalador (requisito 31) tem direito de mandar. Depois que
 * alguém mexeu no menu Language, é essa escolha que vale, e o instalador não
 * volta a opinar.
 */
export function existeIdiomaEscolhido(): boolean {
  return localStorage.getItem(CHAVE_ARMAZENAMENTO) !== null;
}

/** Aceita o código vindo do backend, ignorando o que não for um dos três. */
export function idiomaValido(codigo: string | null): Idioma | null {
  return codigo === "en" || codigo === "es" || codigo === "pt-BR" ? codigo : null;
}

/**
 * Troca o idioma e avisa quem estiver ouvindo.
 *
 * Quase toda a tela é montada na hora em que aparece — menu, diálogo, dica de
 * botão —, então esses já nascem no idioma novo sem ninguém precisar fazer nada.
 * O que fica montado desde a abertura da janela se atualiza pelo `observarIdioma`.
 */
export function definirIdioma(novo: Idioma): void {
  if (novo === atual) return;
  atual = novo;
  localStorage.setItem(CHAVE_ARMAZENAMENTO, novo);
  document.documentElement.lang = novo;
  window.dispatchEvent(new CustomEvent(EVENTO_DE_ALTERACAO));
}

/**
 * Chama a função agora e a cada troca de idioma; devolve como parar de ouvir.
 *
 * Chamar de imediato é o que permite escrever o rótulo uma vez só, no lugar em
 * que ele é criado, em vez de repetir o texto na montagem e no observador.
 */
export function observarIdioma(aoTrocar: () => void): () => void {
  aoTrocar();
  window.addEventListener(EVENTO_DE_ALTERACAO, aoTrocar);
  return () => window.removeEventListener(EVENTO_DE_ALTERACAO, aoTrocar);
}

/**
 * O texto da chave no idioma atual.
 *
 * `valores` substitui `{nome}` no texto. Interpolar aqui, e não concatenar no
 * lugar de uso, é o que deixa cada idioma escolher a ordem das palavras.
 */
export function t(chave: Chave, valores?: Record<string, string | number>): string {
  const texto = TEXTOS[chave]?.[atual] ?? TEXTOS[chave]?.["pt-BR"] ?? chave;
  if (!valores) return texto;
  return texto.replace(/\{(\w+)\}/g, (inteiro, nome: string) =>
    nome in valores ? String(valores[nome]) : inteiro,
  );
}

export type Chave = keyof typeof TEXTOS;

/**
 * O catálogo.
 *
 * `Record<Idioma, string>` não é decoração: é o que faz o TypeScript recusar a
 * compilação quando uma chave nova entra sem as três traduções. Sem isso, a
 * falta só apareceria na tela de quem usa o idioma esquecido.
 */
const TEXTOS = {
  "alertas": { "pt-BR": "Alertas", en: "Alerts", es: "Alertas" },
  "alertas titulo": {
    "pt-BR": "Alertas — {conexao}", en: "Alerts — {conexao}", es: "Alertas — {conexao}",
  },
  "novo alerta": { "pt-BR": "Novo alerta", en: "New alert", es: "Nueva alerta" },
  "logs do alerta": { "pt-BR": "Logs", en: "Logs", es: "Registros" },
  "executar alerta": { "pt-BR": "Executar", en: "Run", es: "Ejecutar" },
  "logs do alerta titulo": {
    "pt-BR": "Logs de Execução — {nome}",
    en: "Execution Logs — {nome}",
    es: "Registros de ejecución — {nome}",
  },
  "logs do alerta vazios": {
    "pt-BR": "Ainda não há registros para este alerta.",
    en: "There are no logs for this alert yet.",
    es: "Todavía no hay registros de esta alerta.",
  },
  "logs do alerta ordem": {
    "pt-BR": "Registros mais recentes primeiro.",
    en: "Newest entries first.",
    es: "Los registros más recientes aparecen primero.",
  },
  "alerta ativo": {
    "pt-BR": "Alerta ativo", en: "Alert enabled", es: "Alerta activa",
  },
  "alerta intervalo": {
    "pt-BR": "Intervalo (minutos)", en: "Interval (minutes)", es: "Intervalo (minutos)",
  },
  "alerta repetir apos": {
    "pt-BR": "Repetir após (minutos)", en: "Repeat after (minutes)", es: "Repetir tras (minutos)",
  },
  "alerta repetir ajuda": {
    "pt-BR": "Deixe em branco para avisar só quando o texto mudar ou voltar a aparecer.",
    en: "Leave blank to notify only when the text changes or reappears.",
    es: "Déjalo en blanco para avisar solo cuando el texto cambie o vuelva a aparecer.",
  },
  "alerta consulta": { "pt-BR": "Consulta SQL", en: "SQL query", es: "Consulta SQL" },
  "alerta criterio": {
    "pt-BR": "Use um SELECT de uma coluna. O texto da primeira linha é a mensagem do alerta.",
    en: "Use a SELECT with one column. The text in the first row is the alert message.",
    es: "Usa un SELECT de una columna. El texto de la primera fila es el mensaje de la alerta.",
  },
  "alerta legado ajuda": {
    "pt-BR": "Esta regra ainda usa a condição anterior (> 1). Altere a consulta para usar o texto retornado.",
    en: "This rule still uses the previous condition (> 1). Change the query to use its returned text.",
    es: "Esta regla aún usa la condición anterior (> 1). Cambia la consulta para usar el texto devuelto.",
  },
  "alerta conexao aberta": {
    "pt-BR": "Monitorado enquanto a conexão estiver aberta no Cururu.",
    en: "Monitored while the connection is open in Cururu.",
    es: "Se supervisa mientras la conexión esté abierta en Cururu.",
  },
  "alerta enviar teams": {
    "pt-BR": "Também enviar ao Teams", en: "Also send to Teams", es: "Enviar también a Teams",
  },
  "alerta url teams": {
    "pt-BR": "URL do fluxo do Teams", en: "Teams workflow URL", es: "URL del flujo de Teams",
  },
  "alerta webhook segredo": {
    "pt-BR": "Não compartilhe esta URL. Quem tiver o endereço pode acionar o fluxo.",
    en: "Do not share this URL. Anyone with the link can trigger the workflow.",
    es: "No compartas esta URL. Cualquiera que tenga el enlace puede activar el flujo.",
  },
  "alerta pausado": { "pt-BR": "pausado", en: "paused", es: "en pausa" },
  "alerta webhook ajuda": {
    "pt-BR": "No Power Automate, use um webhook do Teams e a ação Postar uma mensagem para mim mesmo.",
    en: "In Power Automate, use a Teams webhook and the Post a message to myself action.",
    es: "En Power Automate, usa un webhook de Teams y la acción Publicar un mensaje para mí.",
  },
  "alerta excluir pergunta": {
    "pt-BR": "Excluir o alerta “{nome}”?", en: "Delete the alert “{nome}”?", es: "¿Eliminar la alerta «{nome}»?",
  },
  "alerta disparado": {
    "pt-BR": "A consulta retornou {valor}. O limite é maior que 1.",
    en: "The query returned {valor}. The threshold is greater than 1.",
    es: "La consulta devolvió {valor}. El umbral es mayor que 1.",
  },
  "alerta teams falhou": {
    "pt-BR": "Falha ao enviar ao Teams.", en: "Could not send to Teams.", es: "No se pudo enviar a Teams.",
  },
  "alerta banco obrigatorio": {
    "pt-BR": "Escolha o banco de dados.", en: "Choose a database.", es: "Elige una base de datos.",
  },
  "alerta campos invalidos": {
    "pt-BR": "Informe um nome e um intervalo de 1 a 1.440 minutos.",
    en: "Enter a name and an interval of 1 to 1,440 minutes.",
    es: "Introduce un nombre y un intervalo de 1 a 1.440 minutos.",
  },
  "alerta repeticao invalida": {
    "pt-BR": "Informe uma repetição de 1 a 1.440 minutos ou deixe em branco.",
    en: "Enter a repeat delay of 1 to 1,440 minutes or leave it blank.",
    es: "Introduce un plazo de repetición de 1 a 1.440 minutos o déjalo en blanco.",
  },
  "alerta consulta invalida": {
    "pt-BR": "Informe uma única instrução SELECT.",
    en: "Enter a single SELECT statement.",
    es: "Introduce una única instrucción SELECT.",
  },
  "alerta webhook invalido": {
    "pt-BR": "Informe a URL HTTPS assinada do webhook do Power Automate.",
    en: "Enter the signed HTTPS URL of the Power Automate webhook.",
    es: "Introduce la URL HTTPS firmada del webhook de Power Automate.",
  },
  "alerta inexistente": {
    "pt-BR": "O alerta foi excluído.", en: "The alert was deleted.", es: "La alerta fue eliminada.",
  },
  // --- Barra de título da janela ---------------------------------------------
  "minimizar janela": {
    "pt-BR": "Minimizar",
    en: "Minimize",
    es: "Minimizar",
  },
  "maximizar janela": {
    "pt-BR": "Maximizar",
    en: "Maximize",
    es: "Maximizar",
  },
  "restaurar janela": {
    "pt-BR": "Restaurar",
    en: "Restore",
    es: "Restaurar",
  },

  // --- Painel de conexões ---------------------------------------------------
  "menu": { "pt-BR": "Menu", en: "Menu", es: "Menú" },
  "expandir conexoes": {
    "pt-BR": "Expandir conexões",
    en: "Expand connections",
    es: "Expandir conexiones",
  },
  "minimizar conexoes": {
    "pt-BR": "Minimizar conexões",
    en: "Collapse connections",
    es: "Minimizar conexiones",
  },
  "nenhuma conexao salva": {
    "pt-BR": "Nenhuma conexão salva.",
    en: "No saved connections.",
    es: "Ninguna conexión guardada.",
  },
  "selecione uma conexao para comecar": {
    "pt-BR": "Selecione uma conexão à esquerda para começar.",
    en: "Select a connection on the left to start.",
    es: "Seleccione una conexión a la izquierda para empezar.",
  },

  // --- Menu geral -----------------------------------------------------------
  "salvar consulta sql": {
    "pt-BR": "Salvar Consulta SQL (Ctrl+S)",
    en: "Save SQL query (Ctrl+S)",
    es: "Guardar consulta SQL (Ctrl+S)",
  },
  "salvar consulta sql como": {
    "pt-BR": "Salvar Consulta SQL Como…",
    en: "Save SQL query as…",
    es: "Guardar consulta SQL como…",
  },
  "abrir recente": { "pt-BR": "Abrir Recente", en: "Open Recent", es: "Abrir recientes" },
  "abrir consulta sql": {
    "pt-BR": "Abrir Consulta SQL (Ctrl+O)",
    en: "Open SQL query (Ctrl+O)",
    es: "Abrir consulta SQL (Ctrl+O)",
  },
  "recuperar consultas": {
    "pt-BR": "Recuperar Consultas…",
    en: "Recover queries…",
    es: "Recuperar consultas…",
  },
  "salvar backup": {
    "pt-BR": "Salvar backup Cururu",
    en: "Save Cururu backup",
    es: "Guardar copia de seguridad Cururu",
  },
  "carregar backup": {
    "pt-BR": "Carregar backup Cururu",
    en: "Load Cururu backup",
    es: "Cargar copia de seguridad Cururu",
  },
  "teclas de atalho": {
    "pt-BR": "Teclas de Atalho",
    en: "Keyboard shortcuts",
    es: "Teclas de acceso rápido",
  },
  "atalho": { "pt-BR": "Atalho", en: "Shortcut", es: "Atajo" },
  "atalho alternativo": {
    "pt-BR": "Atalho alternativo",
    en: "Alternative shortcut",
    es: "Atajo alternativo",
  },
  "acao": { "pt-BR": "Ação", en: "Action", es: "Acción" },
  "restaurar padrao": {
    "pt-BR": "Restaurar padrão",
    en: "Restore defaults",
    es: "Restaurar valores predeterminados",
  },
  "atalhos ajuda": {
    "pt-BR":
      "Selecione um campo e pressione a nova combinação de teclas. Uma combinação já usada por outra ação troca de dono.",
    en: "Select a field and press the new key combination. A combination already used by another action changes owner.",
    es: "Seleccione un campo y presione la nueva combinación de teclas. Una combinación ya usada por otra acción cambia de dueño.",
  },
  "atalho repetido": {
    "pt-BR": "O atalho {atalho} está repetido.",
    en: "The {atalho} shortcut is duplicated.",
    es: "El atalho {atalho} está repetido.",
  },
  "atalho trocado": {
    "pt-BR": '"{atalho}" estava em "{acao}", que agora usa "{tecla}".',
    en: '"{atalho}" belonged to "{acao}", which now uses "{tecla}".',
    es: '"{atalho}" pertenecía a "{acao}", que ahora usa "{tecla}".',
  },
  "atalho removido": {
    "pt-BR": '"{atalho}" foi removido de "{acao}".',
    en: '"{atalho}" was removed from "{acao}".',
    es: '"{atalho}" fue quitado de "{acao}".',
  },
  "atalhos restaurados pendentes": {
    "pt-BR": "Padrões preparados; clique em Salvar para aplicar.",
    en: "Defaults prepared; click Save to apply.",
    es: "Valores predeterminados preparados; haga clic en Guardar para aplicarlos.",
  },
  "atalho abrir consulta": {
    "pt-BR": "Abrir um arquivo .sql numa aba nova",
    en: "Open a .sql file in a new tab",
    es: "Abrir un archivo .sql en una pestaña nueva",
  },
  "atalho salvar consulta": {
    "pt-BR": "Salvar a consulta da aba visível",
    en: "Save the query in the visible tab",
    es: "Guardar la consulta de la pestaña visible",
  },
  "atalho nova consulta": {
    "pt-BR": "Abrir uma nova consulta na conexão selecionada",
    en: "Open a new query on the selected connection",
    es: "Abrir una nueva consulta en la conexión seleccionada",
  },
  "atalho fechar aba": {
    "pt-BR": "Fechar a aba visível",
    en: "Close the visible tab",
    es: "Cerrar la pestaña visible",
  },
  "atalho executar": { "pt-BR": "Executar", en: "Run", es: "Ejecutar" },
  "atalho executar script": {
    "pt-BR": "Executar como script",
    en: "Run as script",
    es: "Ejecutar como script",
  },
  "atalho compilar": { "pt-BR": "Compilar", en: "Compile", es: "Compilar" },
  "atalho duplicar linha": {
    "pt-BR": "Duplicar a linha atual ou a seleção",
    en: "Duplicate the current line or selection",
    es: "Duplicar la línea actual o la selección",
  },
  "atalho dividir linhas": {
    "pt-BR": "Dividir linhas longas nas quebras visuais",
    en: "Split long lines at visual wraps",
    es: "Dividir líneas largas en los saltos visuales",
  },
  "atalho juntar linhas": {
    "pt-BR": "Juntar as linhas selecionadas",
    en: "Join selected lines",
    es: "Unir las líneas seleccionadas",
  },
  "atalho excluir linha": {
    "pt-BR": "Excluir a linha atual",
    en: "Delete the current line",
    es: "Eliminar la línea actual",
  },
  "atalho minusculas": {
    "pt-BR": "Converter a seleção para minúsculas",
    en: "Convert selection to lowercase",
    es: "Convertir la selección a minúsculas",
  },
  "atalho maiusculas": {
    "pt-BR": "Converter a seleção para MAIÚSCULAS",
    en: "Convert selection to UPPERCASE",
    es: "Convertir la selección a MAYÚSCULAS",
  },
  "atalho mover linha acima": { "pt-BR": "Mover a linha para cima", en: "Move line up", es: "Mover la línea hacia arriba" },
  "atalho mover linha abaixo": { "pt-BR": "Mover a linha para baixo", en: "Move line down", es: "Mover la línea hacia abajo" },
  "atalho apagar palavra anterior": { "pt-BR": "Apagar a palavra anterior", en: "Delete previous word", es: "Borrar la palabra anterior" },
  "atalho apagar palavra seguinte": { "pt-BR": "Apagar a próxima palavra", en: "Delete next word", es: "Borrar la palabra siguiente" },
  "atalho apagar inicio linha": { "pt-BR": "Apagar até o começo da linha", en: "Delete to line start", es: "Borrar hasta el inicio de la línea" },
  "atalho apagar fim linha": { "pt-BR": "Apagar até o final da linha", en: "Delete to line end", es: "Borrar hasta el final de la línea" },
  "atalho comentar linhas": { "pt-BR": "Comentar ou descomentar linhas com --", en: "Toggle line comments with --", es: "Comentar o descomentar líneas con --" },
  "atalho comentar bloco": { "pt-BR": "Comentar ou descomentar a seleção com /* … */", en: "Toggle block comment with /* … */", es: "Comentar o descomentar la selección con /* … */" },
  "atalho selecao coluna acima": { "pt-BR": "Estender a seleção em coluna para cima", en: "Extend column selection upward", es: "Extender la selección en columna hacia arriba" },
  "atalho selecao coluna abaixo": { "pt-BR": "Estender a seleção em coluna para baixo", en: "Extend column selection downward", es: "Extender la selección en columna hacia abajo" },
  "atalho reduzir indentacao": { "pt-BR": "Reduzir a indentação", en: "Decrease indentation", es: "Reducir la sangría" },
  "atalho aumentar indentacao": { "pt-BR": "Aumentar a indentação", en: "Increase indentation", es: "Aumentar la sangría" },
  // A única chave que não se traduz: quem está preso num idioma que não lê
  // precisa achar a saída, e "Language" é a palavra que ele vai procurar.
  "idioma": { "pt-BR": "Language", en: "Language", es: "Language" },
  "tema": { "pt-BR": "Tema", en: "Theme", es: "Tema" },
  "tema claro": { "pt-BR": "Claro", en: "Light", es: "Claro" },
  "tema escuro": { "pt-BR": "Escuro", en: "Dark", es: "Oscuro" },
  "objetos por vez": {
    "pt-BR": "Objetos listados por vez",
    en: "Objects listed at a time",
    es: "Objetos listados por vez",
  },
  "objetos por vez ajuda": {
    "pt-BR":
      "Quantos objetos cada categoria da árvore traz de uma vez; os demais aparecem pelo Listar mais. Padrão: {padrao}.",
    en: "How many objects each tree category brings at once; the rest appear through List more. Default: {padrao}.",
    es: "Cuántos objetos trae cada categoría del árbol de una vez; los demás aparecen con Listar más. Predeterminado: {padrao}.",
  },
  "objetos por vez invalido": {
    "pt-BR": "Objetos listados por vez: informe um número inteiro de {minimo} a {maximo}.",
    en: "Objects listed at a time: enter a whole number from {minimo} to {maximo}.",
    es: "Objetos listados por vez: indique un número entero de {minimo} a {maximo}.",
  },
  "configuracoes": {
    "pt-BR": "Configurações",
    en: "Settings",
    es: "Configuración",
  },
  // Termo técnico de uso corrente: fica no original nos três idiomas.
  "auto-complete": { "pt-BR": "Auto-Complete", en: "Auto-Complete", es: "Auto-Complete" },
  "autocomplete tabelas": { "pt-BR": "Tabelas", en: "Tables", es: "Tablas" },
  "autocomplete tabelas ajuda": {
    "pt-BR": "Quais tabelas o editor oferece enquanto você digita.",
    en: "Which tables the editor offers as you type.",
    es: "Qué tablas ofrece el editor mientras usted escribe.",
  },
  "autocomplete tabelas todas": { "pt-BR": "Todas", en: "All", es: "Todas" },
  "autocomplete tabelas todas ajuda": {
    "pt-BR":
      "Qualquer tabela que a conexão enxergue pode ser sugerida, inclusive as de outros donos alcançadas por sinônimo.",
    en: "Any table the connection can see may be suggested, including other owners' tables reached through a synonym.",
    es: "Cualquier tabla que la conexión vea puede sugerirse, incluidas las de otros dueños alcanzadas por sinónimo.",
  },
  "autocomplete tabelas filtradas": {
    "pt-BR": "Apenas as do filtro da conexão",
    en: "Only those in the connection filter",
    es: "Solo las del filtro de la conexión",
  },
  "autocomplete tabelas filtradas ajuda": {
    "pt-BR":
      "Só as tabelas que passam pelo filtro da categoria Tabelas — o mesmo que recorta a árvore de objetos. Num schema com dezenas de milhares de tabelas, é o que faz a lista caber no que você realmente usa. Sem filtro configurado não há o que recortar, e as duas opções dão no mesmo.",
    en: "Only tables that pass the Tables category filter — the same one that narrows the object tree. In a schema with tens of thousands of tables, this is what keeps the list down to what you actually use. With no filter set there is nothing to narrow, and both options do the same.",
    es: "Solo las tablas que pasan por el filtro de la categoría Tablas — el mismo que recorta el árbol de objetos. En un esquema con decenas de miles de tablas, es lo que hace que la lista quepa en lo que usted realmente usa. Sin filtro configurado no hay nada que recortar, y las dos opciones dan lo mismo.",
  },
  "autocomplete tabelas nota": {
    "pt-BR":
      "Vale só para tabelas: pacotes, procedures, funções, views e sequências continuam sendo sugeridos por inteiro. Funciona no Oracle e no SQL Server, cada um com o filtro da sua conexão.",
    en: "Applies to tables only: packages, procedures, functions, views and sequences are still suggested in full. It works on both Oracle and SQL Server, each with its own connection filter.",
    es: "Vale solo para tablas: paquetes, procedures, funciones, vistas y secuencias se siguen sugiriendo por completo. Funciona en Oracle y en SQL Server, cada uno con el filtro de su conexión.",
  },
  "versoes": { "pt-BR": "Versões", en: "Versions", es: "Versiones" },
  "procurar atualizacoes": {
    "pt-BR": "Procurar Atualizações…",
    en: "Check for updates…",
    es: "Buscar actualizaciones…",
  },
  "sobre": { "pt-BR": "Sobre", en: "About", es: "Acerca de" },

  // --- Barra de ferramentas -------------------------------------------------
  "nova conexao": { "pt-BR": "Nova conexão", en: "New connection", es: "Nueva conexión" },
  "filtrar": { "pt-BR": "Filtrar", en: "Filter", es: "Filtrar" },
  "filtrar sem categoria": {
    "pt-BR": "Filtrar (selecione uma categoria na árvore)",
    en: "Filter (select a category in the tree)",
    es: "Filtrar (seleccione una categoría en el árbol)",
  },
  "filtrar categoria": { "pt-BR": "Filtrar {rotulo}", en: "Filter {rotulo}", es: "Filtrar {rotulo}" },
  // `{atalho}` já chega formatado como " (Ctrl+T)", ou vazio quando a ação está
  // sem atalho. Vem da configuração vigente (requisito 32), e não de um texto
  // fixo: quem trocou a tecla precisa ler a que trocou.
  "nova consulta sql": {
    "pt-BR": "Nova Consulta SQL{atalho}",
    en: "New SQL query{atalho}",
    es: "Nueva consulta SQL{atalho}",
  },
  "nova consulta sql sem conexao": {
    "pt-BR": "Nova Consulta SQL{atalho} — selecione uma conexão aberta",
    en: "New SQL query{atalho} — select an open connection",
    es: "Nueva consulta SQL{atalho} — seleccione una conexión abierta",
  },
  "nova consulta sql em": {
    "pt-BR": "Nova Consulta SQL em {conexao}{atalho}",
    en: "New SQL query on {conexao}{atalho}",
    es: "Nueva consulta SQL en {conexao}{atalho}",
  },

  // --- Mensagens da barra de status ----------------------------------------
  "procurando atualizacoes": {
    "pt-BR": "Procurando atualizações…",
    en: "Checking for updates…",
    es: "Buscando actualizaciones…",
  },
  "consulta salva em": {
    "pt-BR": "Consulta salva em {caminho}.",
    en: "Query saved to {caminho}.",
    es: "Consulta guardada en {caminho}.",
  },
  "arquivo aberto em": {
    "pt-BR": "{nome} aberta em {conexao}.",
    en: "{nome} opened on {conexao}.",
    es: "{nome} abierta en {conexao}.",
  },
  "selecione conexao para abrir": {
    "pt-BR": "Selecione uma conexão para abrir {nome}.",
    en: "Select a connection to open {nome}.",
    es: "Seleccione una conexión para abrir {nome}.",
  },
  "selecione conexao para abrir n": {
    "pt-BR": "Selecione uma conexão para abrir os {n} arquivos.",
    en: "Select a connection to open the {n} files.",
    es: "Seleccione una conexión para abrir los {n} archivos.",
  },
  "consulta recuperada": {
    "pt-BR": "{titulo} recuperada de {quando}.",
    en: "{titulo} recovered from {quando}.",
    es: "{titulo} recuperada de {quando}.",
  },
  "backup salvo em": {
    "pt-BR": "Backup salvo em {caminho}.",
    en: "Backup saved to {caminho}.",
    es: "Copia de seguridad guardada en {caminho}.",
  },
  "backup carregado": {
    "pt-BR": "Backup carregado com sucesso.",
    en: "Backup loaded.",
    es: "Copia de seguridad cargada.",
  },
  "conectado em": {
    "pt-BR": "Conectado em {conexao}.",
    en: "Connected to {conexao}.",
    es: "Conectado a {conexao}.",
  },

  // --- Árvore de objetos ----------------------------------------------------
  "clique para conectar": {
    "pt-BR": "{conexao} — clique para conectar",
    en: "{conexao} — click to connect",
    es: "{conexao} — haga clic para conectar",
  },
  "conectado": { "pt-BR": "{conexao} — conectado", en: "{conexao} — connected", es: "{conexao} — conectado" },
  "atualizar": { "pt-BR": "Atualizar", en: "Refresh", es: "Actualizar" },
  "conectar": { "pt-BR": "Conectar", en: "Connect", es: "Conectar" },
  "desconectar": { "pt-BR": "Desconectar", en: "Disconnect", es: "Desconectar" },
  "abrir": { "pt-BR": "Abrir", en: "Open", es: "Abrir" },
  "exibir dados": { "pt-BR": "Exibir Dados", en: "Show data", es: "Mostrar datos" },
  "exibir ddl": { "pt-BR": "Exibir DDL", en: "Show DDL", es: "Mostrar DDL" },
  "exibir estrutura": {
    "pt-BR": "Exibir Estrutura",
    en: "Show structure",
    es: "Mostrar estructura",
  },
  "editar comentario": {
    "pt-BR": "Editar Comentário…",
    en: "Edit comment…",
    es: "Editar comentario…",
  },
  "comentario da view": {
    "pt-BR": "Comentário — {view}",
    en: "Comment — {view}",
    es: "Comentario — {view}",
  },
  "comentario": { "pt-BR": "Comentário", en: "Comment", es: "Comentario" },
  "caracteres restantes": {
    "pt-BR": "Restam {n} caracteres",
    en: "{n} characters left",
    es: "Quedan {n} caracteres",
  },
  "comentario gravado": {
    "pt-BR": "Comentário de {view} gravado.",
    en: "Comment on {view} saved.",
    es: "Comentario de {view} guardado.",
  },
  "comentario apagado": {
    "pt-BR": "Comentário de {view} apagado.",
    en: "Comment on {view} removed.",
    es: "Comentario de {view} borrado.",
  },
  "aplicar filtro": { "pt-BR": "Aplicar filtro…", en: "Apply filter…", es: "Aplicar filtro…" },
  "limpar filtro": { "pt-BR": "Limpar filtro", en: "Clear filter", es: "Limpiar filtro" },
  "filtrado": { "pt-BR": "(Filtrado)", en: "(Filtered)", es: "(Filtrado)" },
  "filtro ativo": {
    "pt-BR": "Filtro ativo: {filtro}",
    en: "Active filter: {filtro}",
    es: "Filtro activo: {filtro}",
  },
  "falha ao ler filtros": {
    "pt-BR": "Não foi possível ler os filtros salvos: {erro}",
    en: "The saved filters could not be read: {erro}",
    es: "No se pudieron leer los filtros guardados: {erro}",
  },
  "filtro aplicado sem guardar": {
    "pt-BR": "Filtro aplicado, mas não foi possível guardá-lo: {erro}",
    en: "Filter applied, but it could not be saved: {erro}",
    es: "Filtro aplicado, pero no se pudo guardar: {erro}",
  },
  "listar mais": { "pt-BR": "Listar mais {n}…", en: "List {n} more…", es: "Listar {n} más…" },
  "ja listados": { "pt-BR": "Já listados: {n}", en: "Listed so far: {n}", es: "Ya listados: {n}" },
  "carregando": { "pt-BR": "carregando…", en: "loading…", es: "cargando…" },
  "carregando abertura": {
    "pt-BR": "Carregando {nome}…",
    en: "Loading {nome}…",
    es: "Cargando {nome}…",
  },
  "vazio": { "pt-BR": "(vazio)", en: "(empty)", es: "(vacío)" },
  "especificacao": { "pt-BR": "Especificação", en: "Specification", es: "Especificación" },
  "conexao encerrada": {
    "pt-BR": "Conexão {conexao} encerrada.",
    en: "Connection {conexao} closed.",
    es: "Conexión {conexao} cerrada.",
  },
  "conexao nao esta aberta": {
    "pt-BR": "a conexão não está aberta",
    en: "the connection is not open",
    es: "la conexión no está abierta",
  },

  // --- Alvo singular de cada categoria, como a tela de filtro o nomeia -------
  "alvo tabela": { "pt-BR": "Tabela", en: "Table", es: "Tabla" },
  "alvo view": { "pt-BR": "View", en: "View", es: "View" },
  "alvo pacote": { "pt-BR": "Pacote", en: "Package", es: "Paquete" },
  "alvo procedure": { "pt-BR": "Procedure", en: "Procedure", es: "Procedure" },
  "alvo funcao": { "pt-BR": "Função", en: "Function", es: "Función" },
  "alvo job": { "pt-BR": "Job", en: "Job", es: "Job" },

  // --- Resumo do filtro na dica do nó (requisito 4) --------------------------
  "resumo do filtro ou": { "pt-BR": "OU", en: "OR", es: "O" },
  "resumo do filtro e": { "pt-BR": "E", en: "AND", es: "Y" },
  "resumo do filtro nao": { "pt-BR": "NÃO", en: "NOT", es: "NO" },
  "resumo sem objetos gerados": {
    "pt-BR": "sem objetos gerados",
    en: "without generated objects",
    es: "sin objetos generados",
  },
  "resumo com sinonimos": {
    "pt-BR": "com sinônimos",
    en: "with synonyms",
    es: "con sinónimos",
  },

  // --- Visualizador de objeto: dados, DDL e estrutura ------------------------
  "dados": { "pt-BR": "Dados", en: "Data", es: "Datos" },
  "estrutura": { "pt-BR": "Estrutura", en: "Structure", es: "Estructura" },
  "copiar ddl": { "pt-BR": "Copiar DDL", en: "Copy DDL", es: "Copiar DDL" },
  "carregando dados": {
    "pt-BR": "Carregando dados…",
    en: "Loading data…",
    es: "Cargando datos…",
  },
  "carregando ddl": { "pt-BR": "Carregando DDL…", en: "Loading DDL…", es: "Cargando DDL…" },
  "carregando estrutura": {
    "pt-BR": "Carregando estrutura…",
    en: "Loading structure…",
    es: "Cargando estructura…",
  },
  "n linhas": { "pt-BR": "{n} linha(s)", en: "{n} row(s)", es: "{n} fila(s)" },
  "n colunas": { "pt-BR": "{n} coluna(s)", en: "{n} column(s)", es: "{n} columna(s)" },
  "fim das linhas": { "pt-BR": "fim", en: "end", es: "fin" },
  "ddl carregado": { "pt-BR": "DDL carregado", en: "DDL loaded", es: "DDL cargado" },
  "nome da coluna": {
    "pt-BR": "Nome da coluna",
    en: "Column name",
    es: "Nombre de la columna",
  },
  "aceita nulo": { "pt-BR": "Nulo?", en: "Nullable?", es: "¿Nulo?" },
  "tabela sem grid": {
    "pt-BR": "a tabela não devolveu um resultado em grid",
    en: "the table returned no grid result",
    es: "la tabla no devolvió un resultado en grid",
  },

  // --- Abas e consulta ------------------------------------------------------
  "consulta n": { "pt-BR": "Consulta {n}", en: "Query {n}", es: "Consulta {n}" },
  "referencia nao encontrada": {
    "pt-BR": "Objeto não encontrado: {referencia}.",
    en: "Object not found: {referencia}.",
    es: "Objeto no encontrado: {referencia}.",
  },
  "ir para definicao": {
    "pt-BR": "Ir para definição",
    en: "Go to definition",
    es: "Ir a la definición",
  },
  "saida dbms": { "pt-BR": "Saída DBMS", en: "DBMS output", es: "Salida DBMS" },
  "mensagens e log": { "pt-BR": "Mensagens - Log", en: "Messages - Log", es: "Mensajes - Log" },
  "historico sql": { "pt-BR": "Histórico SQL", en: "SQL history", es: "Historial SQL" },

  // Barra da consulta
  "executar": { "pt-BR": "Executar (Ctrl+Enter)", en: "Run (Ctrl+Enter)", es: "Ejecutar (Ctrl+Enter)" },
  "executar automaticamente": {
    "pt-BR": "Executar automaticamente",
    en: "Run automatically",
    es: "Ejecutar automáticamente",
  },
  "nao inserir na janela": {
    "pt-BR": "Não inserir SELECT na janela de consulta",
    en: "Don't insert the SELECT in the query window",
    es: "No insertar el SELECT en la ventana de consulta",
  },
  "executar como script": {
    "pt-BR": "Executar como script (F5)",
    en: "Run as script (F5)",
    es: "Ejecutar como script (F5)",
  },
  "parar a execucao": {
    "pt-BR": "Parar a execução",
    en: "Stop execution",
    es: "Detener la ejecución",
  },
  "compilar": {
    "pt-BR": "Compilar (Ctrl+Shift+C)",
    en: "Compile (Ctrl+Shift+C)",
    es: "Compilar (Ctrl+Shift+C)",
  },
  conexao: { "pt-BR": "Conexão", en: "Connection", es: "Conexión" },
  "todas as conexoes": { "pt-BR": "Todas", en: "All", es: "Todas" },
  "ajuda conexoes do botao": {
    "pt-BR": "O botão aparece só nas conexões marcadas.",
    en: "The button only appears on the selected connections.",
    es: "El botón solo aparece en las conexiones marcadas.",
  },
  "textos rapidos": { "pt-BR": "Textos rápidos", en: "Quick texts", es: "Textos rápidos" },
  "adicionar botao rapido": {
    "pt-BR": "Adicionar botão rápido",
    en: "Add quick button",
    es: "Añadir botón rápido",
  },
  "inserir no editor": {
    "pt-BR": "Inserir no editor",
    en: "Insert into the editor",
    es: "Insertar en el editor",
  },
  "sem conteudo definido": {
    "pt-BR": "Sem conteúdo definido",
    en: "No content defined",
    es: "Sin contenido definido",
  },
  "excluir botao titulo": {
    "pt-BR": "Excluir botão",
    en: "Delete button",
    es: "Eliminar botón",
  },
  "excluir botao mensagem": {
    "pt-BR": "Excluir o botão “{rotulo}”? Esta ação não pode ser desfeita.",
    en: "Delete the “{rotulo}” button? This action can't be undone.",
    es: "¿Eliminar el botón “{rotulo}”? Esta acción no se puede deshacer.",
  },
  "tamanho do buffer": {
    "pt-BR": "Tamanho do Buffer:",
    en: "Buffer size:",
    es: "Tamaño del búfer:",
  },
  "limpar": { "pt-BR": "Limpar", en: "Clear", es: "Limpiar" },
  "limpar mensagens": {
    "pt-BR": "Limpar as mensagens desta aba",
    en: "Clear this tab's messages",
    es: "Limpiar los mensajes de esta pestaña",
  },
  "linhas por vez": { "pt-BR": "Linhas por vez:", en: "Rows at a time:", es: "Filas por vez:" },
  "linhas por vez explicacao": {
    "pt-BR": "Quantas linhas o banco entrega por vez. Aumente para trazer mais de uma só vez.",
    en: "How many rows the database delivers at a time. Raise it to fetch more in one go.",
    es: "Cuántas filas entrega la base por vez. Auméntelo para traer más de una sola vez.",
  },
  "copiar": { "pt-BR": "Copiar", en: "Copy", es: "Copiar" },
  "copiar com cabecalho": {
    "pt-BR": "Copiar com cabeçalho",
    en: "Copy with header",
    es: "Copiar con encabezado",
  },
  "copiar para teams": {
    "pt-BR": "Copiar com cabeçalho (para Teams)",
    en: "Copy with header (for Teams)",
    es: "Copiar con encabezado (para Teams)",
  },
  "selecionar tudo": { "pt-BR": "Selecionar tudo", en: "Select all", es: "Seleccionar todo" },
  "exportar para csv": {
    "pt-BR": "Exportar para CSV…",
    en: "Export to CSV…",
    es: "Exportar a CSV…",
  },
  "exportar para excel": {
    "pt-BR": "Exportar para Excel…",
    en: "Export to Excel…",
    es: "Exportar a Excel…",
  },
  "gerar script de insert": {
    "pt-BR": "Gerar script de INSERT",
    en: "Generate INSERT script",
    es: "Generar script de INSERT",
  },
  "insert copiado": {
    "pt-BR": "{quantidade} INSERT no clipboard, para {tabela}",
    en: "{quantidade} INSERT(s) on the clipboard, for {tabela}",
    es: "{quantidade} INSERT en el portapapeles, para {tabela}",
  },
  "insert sem linhas": {
    "pt-BR": "Não há linha para gerar INSERT.",
    en: "There is no row to generate an INSERT from.",
    es: "No hay fila para generar INSERT.",
  },
  "insert coluna calculada": {
    "pt-BR": "{coluna} não é coluna da tabela: o INSERT precisa de nome de coluna, não de expressão.",
    en: "{coluna} is not a table column: an INSERT needs column names, not expressions.",
    es: "{coluna} no es columna de la tabla: el INSERT necesita nombre de columna, no expresiones.",
  },
  "insert colunas nulas": {
    "pt-BR": "{colunas} saiu como NULL: o grid mostra a descrição do binário, não o conteúdo",
    en: "{colunas} came out as NULL: the grid shows a description of the binary, not the content",
    es: "{colunas} salió como NULL: el grid muestra la descripción del binario, no el contenido",
  },
  "insert colunas longas": {
    "pt-BR": "{colunas} passa de 4000 caracteres, e o Oracle recusa literal desse tamanho",
    en: "{colunas} exceeds 4000 characters, and Oracle rejects a literal that long",
    es: "{colunas} supera los 4000 caracteres, y Oracle rechaza un literal de ese tamaño",
  },
  "nada para executar": {
    "pt-BR": "Não há nada para executar.",
    en: "There is nothing to run.",
    es: "No hay nada que ejecutar.",
  },
  "executando": { "pt-BR": "Executando…", en: "Running…", es: "Ejecutando…" },
  "interrupcao enviada": {
    "pt-BR": "Interrupção enviada ao banco.",
    en: "Interrupt sent to the database.",
    es: "Interrupción enviada a la base.",
  },
  "conexao restabelecida": {
    "pt-BR":
      "A conexão havia caído e foi restabelecida — esta é outra sessão do banco, e o que estava sem commit se perdeu.",
    en: "The connection had dropped and was reestablished — this is a new database session, and anything left uncommitted was lost.",
    es: "La conexión se había caído y fue restablecida — esta es otra sesión de la base, y lo que estaba sin commit se perdió.",
  },
  "conexao restabelecida execute outra vez": {
    "pt-BR": "A conexão havia caído e foi restabelecida. Execute o comando outra vez.",
    en: "The connection had dropped and was reestablished. Run the command again.",
    es: "La conexión se había caído y fue restablecida. Ejecute el comando otra vez.",
  },
  "commit efetuado": { "pt-BR": "Commit efetuado.", en: "Commit done.", es: "Commit efectuado." },
  "transacao aberta": {
    "pt-BR": "Commit — há transação aberta nesta aba",
    en: "Commit — this tab has an open transaction",
    es: "Commit — hay transacción abierta en esta pestaña",
  },
  "rollback efetuado": {
    "pt-BR": "Rollback efetuado.",
    en: "Rollback done.",
    es: "Rollback efectuado.",
  },
  "auto commit ligado": {
    "pt-BR": "Auto-commit ligado.",
    en: "Auto-commit on.",
    es: "Auto-commit activado.",
  },
  "auto commit desligado": {
    "pt-BR": "Auto-commit desligado.",
    en: "Auto-commit off.",
    es: "Auto-commit desactivado.",
  },
  "sem fonte para compilar": {
    "pt-BR": "Não há fonte para compilar.",
    en: "There is no source to compile.",
    es: "No hay fuente para compilar.",
  },
  "compilado com sucesso": {
    "pt-BR": "Compilado com sucesso.",
    en: "Compiled successfully.",
    es: "Compilado correctamente.",
  },

  // --- Diálogo de conexão ---------------------------------------------------
  "novo ou selecionar conexao": {
    "pt-BR": "Novo / Selecionar Conexão do Banco de Dados",
    en: "New / Select Database Connection",
    es: "Nueva / Seleccionar Conexión de Base de Datos",
  },
  "fechar": { "pt-BR": "Fechar", en: "Close", es: "Cerrar" },
  "nome da conexao": { "pt-BR": "Nome da Conexão", en: "Connection name", es: "Nombre de la conexión" },
  "nome": { "pt-BR": "Nome", en: "Name", es: "Nombre" },
  "cor obrigatoria": { "pt-BR": "Cor *", en: "Color *", es: "Color *" },
  "escolher cor personalizada": {
    "pt-BR": "Escolher uma cor personalizada",
    en: "Pick a custom color",
    es: "Elegir un color personalizado",
  },
  "cores sugeridas": { "pt-BR": "Cores sugeridas", en: "Suggested colors", es: "Colores sugeridos" },
  "dica das cores": {
    "pt-BR": "PRD: coral · HML: laranja · DEV: verde",
    en: "PRD: coral · STG: orange · DEV: green",
    es: "PRD: coral · HML: naranja · DEV: verde",
  },
  "tipo de banco": {
    "pt-BR": "Tipo de Banco de Dados",
    en: "Database type",
    es: "Tipo de base de datos",
  },

  "tipo de banco so em conexao nova": {
    "pt-BR": "O tipo de banco só pode ser escolhido ao cadastrar a conexão. Para o outro banco, crie uma conexão nova.",
    en: "The database type can only be chosen when the connection is created. For the other database, create a new connection.",
    es: "El tipo de base de datos solo se elige al registrar la conexión. Para la otra base, cree una conexión nueva.",
  },

  // Conexão com SQL Server: os rótulos são os do SSMS, em cada idioma.
  "nome do servidor": {
    "pt-BR": "Nome do Servidor",
    en: "Server name",
    es: "Nombre del servidor",
  },
  // A barra invertida vai dobrada: num literal de TypeScript `\i` vale `i`,
  // e o exemplo aparecia como "servidorinstância" — justamente escondendo o
  // separador que ele existe para ensinar.
  "servidor exemplo": {
    "pt-BR": "servidor, servidor\\instância ou servidor,1433",
    en: "server, server\\instance or server,1433",
    es: "servidor, servidor\\instancia o servidor,1433",
  },
  "autenticacao": {
    "pt-BR": "Autenticação",
    en: "Authentication",
    es: "Autenticación",
  },
  "autenticacao do sql server": {
    "pt-BR": "Autenticação do SQL Server",
    en: "SQL Server Authentication",
    es: "Autenticación de SQL Server",
  },
  "autenticacao do windows": {
    "pt-BR": "Autenticação do Windows",
    en: "Windows Authentication",
    es: "Autenticación de Windows",
  },
  "logon": { "pt-BR": "Logon", en: "Login", es: "Inicio de sesión" },
  "nome de usuario": {
    "pt-BR": "Nome de usuário",
    en: "User name",
    es: "Nombre de usuario",
  },
  "usuario do windows exemplo": {
    "pt-BR": "em branco: o usuário do Windows conectado",
    en: "blank: the signed-in Windows user",
    es: "en blanco: el usuario de Windows conectado",
  },
  "lembrar senha": {
    "pt-BR": "Lembrar senha",
    en: "Remember password",
    es: "Recordar contraseña",
  },
  "auto commit explicacao sqlserver": {
    "pt-BR":
      "No SQL Server cada comando é confirmado sozinho por padrão. Desligado aqui, o Cururu abre a transação e nada é confirmado até você usar commit.",
    en: "In SQL Server each statement commits on its own by default. Turned off here, Cururu opens the transaction and nothing is committed until you commit.",
    es: "En SQL Server cada comando se confirma solo de forma predeterminada. Apagado aquí, Cururu abre la transacción y nada se confirma hasta que uses commit.",
  },
  "informacoes do usuario": {
    "pt-BR": "Informações do Usuário",
    en: "User information",
    es: "Información del usuario",
  },
  "usuario proxy": { "pt-BR": "Usuário Proxy", en: "Proxy user", es: "Usuario proxy" },
  "tipo de autenticacao": {
    "pt-BR": "Tipo de Autenticação",
    en: "Authentication type",
    es: "Tipo de autenticación",
  },
  "padrao": { "pt-BR": "Padrão", en: "Default", es: "Predeterminado" },
  "padrao minusculo": { "pt-BR": "padrão", en: "default", es: "predeterminado" },
  "nome do usuario": { "pt-BR": "Nome do Usuário", en: "Username", es: "Nombre de usuario" },
  "atribuicao": { "pt-BR": "Atribuição", en: "Role", es: "Rol" },
  "senha": { "pt-BR": "Senha", en: "Password", es: "Contraseña" },
  "salvar senha": { "pt-BR": "Salvar Senha", en: "Save password", es: "Guardar contraseña" },
  "opcional": { "pt-BR": "Opcional", en: "Optional", es: "Opcional" },
  "uri mongodb": { "pt-BR": "URI do MongoDB", en: "MongoDB URI", es: "URI de MongoDB" },
  "uri mongodb invalida": {
    "pt-BR": "Informe uma URI válida iniciada por mongodb:// ou mongodb+srv://.",
    en: "Enter a valid URI starting with mongodb:// or mongodb+srv://.",
    es: "Indique una URI válida que empiece por mongodb:// o mongodb+srv://.",
  },
  "usuario mongodb necessario": {
    "pt-BR": "Informe o usuário da senha do MongoDB.",
    en: "Enter the username for the MongoDB password.",
    es: "Indique el usuario de la contraseña de MongoDB.",
  },
  "nova consulta mongodb": {
    "pt-BR": "Nova consulta MongoDB{atalho}",
    en: "New MongoDB query{atalho}",
    es: "Nueva consulta MongoDB{atalho}",
  },
  "nova consulta mongodb em": {
    "pt-BR": "Nova consulta MongoDB em {conexao}{atalho}",
    en: "New MongoDB query in {conexao}{atalho}",
    es: "Nueva consulta MongoDB en {conexao}{atalho}",
  },
  "colecao mongodb": { "pt-BR": "Coleção", en: "Collection", es: "Colección" },
  "nenhuma colecao": {
    "pt-BR": "Nenhuma coleção acessível",
    en: "No accessible collections",
    es: "No hay colecciones accesibles",
  },
  "escolher banco mongodb": {
    "pt-BR": "Selecione um banco",
    en: "Select a database",
    es: "Seleccione una base de datos",
  },
  "escolher colecao mongodb": {
    "pt-BR": "Selecione uma coleção",
    en: "Select a collection",
    es: "Seleccione una colección",
  },
  "filtro mongodb": { "pt-BR": "Filtro Extended JSON", en: "Extended JSON filter", es: "Filtro Extended JSON" },
  "ajuda filtro mongodb": {
    "pt-BR": "Use {} para listar documentos ou informe um filtro, como { \"status\": \"ativo\" }.",
    en: "Use {} to list documents or enter a filter, such as { \"status\": \"active\" }.",
    es: "Use {} para listar documentos o introduzca un filtro, como { \"estado\": \"activo\" }.",
  },
  "consultar mongodb": { "pt-BR": "Consultar", en: "Run query", es: "Consultar" },
  "limite mongodb": { "pt-BR": "Limite", en: "Limit", es: "Límite" },
  "somente leitura mongodb": { "pt-BR": "Somente leitura", en: "Read only", es: "Solo lectura" },
  "documentos mongodb": {
    "pt-BR": "{quantidade} documento(s) exibido(s)",
    en: "{quantidade} document(s) displayed",
    es: "{quantidade} documento(s) mostrado(s)",
  },
  "nenhum documento mongodb": {
    "pt-BR": "Nenhum documento encontrado.",
    en: "No documents found.",
    es: "No se encontraron documentos.",
  },
  "documento mongodb n": { "pt-BR": "Documento {n}", en: "Document {n}", es: "Documento {n}" },
  "mostrar senha": { "pt-BR": "Mostrar senha", en: "Show password", es: "Mostrar contraseña" },
  "ocultar senha": { "pt-BR": "Ocultar senha", en: "Hide password", es: "Ocultar contraseña" },
  "copiar senha": { "pt-BR": "Copiar senha", en: "Copy password", es: "Copiar contraseña" },
  "senha copiada": { "pt-BR": "Senha copiada.", en: "Password copied.", es: "Contraseña copiada." },
  "senha indisponivel": {
    "pt-BR": "A senha salva não está disponível.",
    en: "The saved password is unavailable.",
    es: "La contraseña guardada no está disponible.",
  },
  "usar proxy": {
    "pt-BR": "Usar autenticação por usuário proxy",
    en: "Use proxy user authentication",
    es: "Usar autenticación por usuario proxy",
  },
  "nome do usuario proxy": {
    "pt-BR": "Nome do Usuário Proxy",
    en: "Proxy username",
    es: "Nombre del usuario proxy",
  },
  "senha proxy": { "pt-BR": "Senha Proxy", en: "Proxy password", es: "Contraseña proxy" },
  "tipo de conexao": { "pt-BR": "Tipo de Conexão", en: "Connection type", es: "Tipo de conexión" },
  "basico": { "pt-BR": "Básico", en: "Basic", es: "Básico" },
  "avancado": { "pt-BR": "Avançado", en: "Advanced", es: "Avanzado" },
  "detalhes": { "pt-BR": "Detalhes", en: "Details", es: "Detalles" },
  "nome do host": { "pt-BR": "Nome do Host", en: "Host name", es: "Nombre del host" },
  "porta": { "pt-BR": "Porta", en: "Port", es: "Puerto" },
  "nome do servico": { "pt-BR": "Nome do Serviço", en: "Service name", es: "Nombre del servicio" },
  "alias da rede": { "pt-BR": "Alias da Rede", en: "Network alias", es: "Alias de red" },
  "alias tns exemplo": {
    "pt-BR": "nome definido no tnsnames.ora",
    en: "name defined in tnsnames.ora",
    es: "nombre definido en tnsnames.ora",
  },
  "url personalizada": { "pt-BR": "URL Personalizada", en: "Custom URL", es: "URL personalizada" },
  "auto commit explicacao": {
    "pt-BR":
      "No Oracle o auto-commit é comportamento do cliente, não do servidor. Desligado, nenhum commit é feito automaticamente após um update.",
    en: "In Oracle, auto-commit is client behavior, not server behavior. Turned off, no commit is issued automatically after an update.",
    es: "En Oracle, el auto-commit es comportamiento del cliente, no del servidor. Apagado, no se hace ningún commit automáticamente tras un update.",
  },
  "status": { "pt-BR": "Status:", en: "Status:", es: "Estado:" },
  "importar": { "pt-BR": "Importar…", en: "Import…", es: "Importar…" },
  "salvar": { "pt-BR": "Salvar", en: "Save", es: "Guardar" },
  "excluir": { "pt-BR": "Excluir", en: "Delete", es: "Eliminar" },
  "testar": { "pt-BR": "Testar", en: "Test", es: "Probar" },
  "cancelar": { "pt-BR": "Cancelar", en: "Cancel", es: "Cancelar" },
  "nova conexao lista": { "pt-BR": "+ Nova Conexão", en: "+ New connection", es: "+ Nueva conexión" },
  "cadastrar nova conexao": {
    "pt-BR": "Cadastrar uma nova conexão",
    en: "Register a new connection",
    es: "Registrar una nueva conexión",
  },
  "conectando": { "pt-BR": "Conectando…", en: "Connecting…", es: "Conectando…" },
  "testando": { "pt-BR": "Testando…", en: "Testing…", es: "Probando…" },

  // Validação do diálogo de conexão
  "informe o nome da conexao": {
    "pt-BR": "Informe o nome da conexão.",
    en: "Enter the connection name.",
    es: "Indique el nombre de la conexión.",
  },
  "informe o usuario": {
    "pt-BR": "Informe o nome do usuário.",
    en: "Enter the username.",
    es: "Indique el nombre de usuario.",
  },
  "informe o host": {
    "pt-BR": "Informe o nome do host.",
    en: "Enter the host name.",
    es: "Indique el nombre del host.",
  },
  "informe o servico": {
    "pt-BR": "Informe o nome do serviço.",
    en: "Enter the service name.",
    es: "Indique el nombre del servicio.",
  },
  "informe o sid": { "pt-BR": "Informe o SID.", en: "Enter the SID.", es: "Indique el SID." },
  "informe o servidor": {
    "pt-BR": "Informe o nome do servidor.",
    en: "Enter the server name.",
    es: "Indique el nombre del servidor.",
  },
  "informe o logon": {
    "pt-BR": "Informe o logon.",
    en: "Enter the login.",
    es: "Indique el inicio de sesión.",
  },
  "informe o alias": {
    "pt-BR": "Informe o alias da rede.",
    en: "Enter the network alias.",
    es: "Indique el alias de red.",
  },
  "informe a url": {
    "pt-BR": "Informe a URL personalizada.",
    en: "Enter the custom URL.",
    es: "Indique la URL personalizada.",
  },
  "porta fora da faixa": {
    "pt-BR": "A porta deve ficar entre 1 e 65535.",
    en: "The port must be between 1 and 65535.",
    es: "El puerto debe estar entre 1 y 65535.",
  },
  "escolha uma cor": {
    "pt-BR": "Escolha uma cor para identificar a conexão.",
    en: "Pick a color to identify the connection.",
    es: "Elija un color para identificar la conexión.",
  },
  "cor recusada": {
    "pt-BR":
      "Escolha uma cor média e bem definida; preto, branco, cinza e tons muito claros ou escuros não são aceitos.",
    en: "Pick a mid-tone, well-defined color; black, white, gray and very light or dark shades are not accepted.",
    es: "Elija un color medio y bien definido; negro, blanco, gris y tonos muy claros u oscuros no se aceptan.",
  },
  "nenhuma conexao importavel": {
    "pt-BR": "Nenhuma conexão encontrada no SQL Developer nem em tnsnames.ora.",
    en: "No connections found in SQL Developer or tnsnames.ora.",
    es: "Ninguna conexión encontrada en SQL Developer ni en tnsnames.ora.",
  },

  "banco de dados": {
    "pt-BR": "Banco de Dados",
    en: "Databases",
    es: "Bases de datos",
  },
  "nenhum banco acessivel": {
    "pt-BR": "Nenhum banco acessível",
    en: "No accessible database",
    es: "Ninguna base accesible",
  },
  "banco em uso": {
    "pt-BR": "Banco em uso nesta aba",
    en: "Database in use on this tab",
    es: "Base en uso en esta pestaña",
  },
  "compilar so no oracle": {
    "pt-BR": "Compilar é PL/SQL: não existe em conexão SQL Server.",
    en: "Compile is PL/SQL: it does not exist on a SQL Server connection.",
    es: "Compilar es PL/SQL: no existe en una conexión SQL Server.",
  },
  "banco trocado": {
    "pt-BR": "Esta aba passou a executar em {banco}.",
    en: "This tab now runs on {banco}.",
    es: "Esta pestaña pasó a ejecutar en {banco}.",
  },
  "tabelas": { "pt-BR": "Tabelas", en: "Tables", es: "Tablas" },
  "views": { "pt-BR": "Views", en: "Views", es: "Views" },
  "pacotes": { "pt-BR": "Pacotes", en: "Packages", es: "Paquetes" },
  "procedures": { "pt-BR": "Procedures", en: "Procedures", es: "Procedures" },
  "funcoes": { "pt-BR": "Funções", en: "Functions", es: "Funciones" },
  "programador": { "pt-BR": "Programador", en: "Scheduler", es: "Programador" },

  // --- Categorias de um banco do SQL Server ---------------------------------
  // "Stored Procedures" e "Functions" ficam em inglês nas três línguas porque é
  // assim que o SSMS as escreve, inclusive na versão em português — e a árvore
  // do Cururu é lida ao lado da dele.
  "stored procedures": {
    "pt-BR": "Stored Procedures",
    en: "Stored Procedures",
    es: "Stored Procedures",
  },
  "functions": { "pt-BR": "Functions", en: "Functions", es: "Functions" },
  "funcoes com valor de tabela": {
    "pt-BR": "Funções com Valor de Tabela",
    en: "Table-valued Functions",
    es: "Funciones con Valor de Tabla",
  },
  "funcoes com valor escalar": {
    "pt-BR": "Funções com Valor Escalar",
    en: "Scalar-valued Functions",
    es: "Funciones con Valor Escalar",
  },

  // --- SQL Server Agent (requisito 36) --------------------------------------
  // "SQL Server Agent" não entra no catálogo: é nome de produto, e o próprio
  // SSMS o mantém em inglês nas três línguas.
  "agente ativo": {
    "pt-BR": "SQL Server Agent em execução",
    en: "SQL Server Agent is running",
    es: "SQL Server Agent en ejecución",
  },
  "agente parado": {
    "pt-BR": "SQL Server Agent parado: os trabalhos agendados não vão rodar.",
    en: "SQL Server Agent is stopped: scheduled jobs will not run.",
    es: "SQL Server Agent detenido: los trabajos programados no se ejecutarán.",
  },
  "agente desconhecido": {
    "pt-BR": "Não foi possível verificar o serviço do SQL Server Agent — falta permissão para consultá-lo.",
    en: "Could not check the SQL Server Agent service — no permission to query it.",
    es: "No se pudo verificar el servicio de SQL Server Agent — falta permiso para consultarlo.",
  },
  "nenhum trabalho": {
    "pt-BR": "Nenhum trabalho",
    en: "No jobs",
    es: "Ningún trabajo",
  },
  "trabalho desabilitado": {
    "pt-BR": "desabilitado",
    en: "disabled",
    es: "deshabilitado",
  },
  "nunca executado": {
    "pt-BR": "nunca executado",
    en: "never run",
    es: "nunca ejecutado",
  },
  "parado": { "pt-BR": "parado", en: "stopped", es: "detenido" },
  "desconhecido": { "pt-BR": "desconhecido", en: "unknown", es: "desconocido" },
  "execucao com sucesso": { "pt-BR": "com êxito", en: "succeeded", es: "con éxito" },
  "execucao com falha": { "pt-BR": "com falha", en: "failed", es: "con error" },
  "execucao cancelada": { "pt-BR": "cancelada", en: "canceled", es: "cancelada" },
  "execucao em nova tentativa": {
    "pt-BR": "em nova tentativa",
    en: "retrying",
    es: "reintentando",
  },
  "trabalho criado": {
    "pt-BR": "Trabalho criado no SQL Server Agent.",
    en: "Job created on SQL Server Agent.",
    es: "Trabajo creado en SQL Server Agent.",
  },

  // Botão direito sobre um trabalho.
  "iniciar trabalho na etapa": {
    "pt-BR": "Iniciar Trabalho na Etapa…",
    en: "Start Job at Step…",
    es: "Iniciar trabajo en el paso…",
  },
  "parar trabalho": { "pt-BR": "Parar Trabalho", en: "Stop Job", es: "Detener trabajo" },
  "habilitar": { "pt-BR": "Habilitar", en: "Enable", es: "Habilitar" },
  "desabilitar": { "pt-BR": "Desabilitar", en: "Disable", es: "Deshabilitar" },
  "renomear": { "pt-BR": "Renomear", en: "Rename", es: "Cambiar nombre" },
  "propriedades": { "pt-BR": "Propriedades", en: "Properties", es: "Propiedades" },
  "propriedades do trabalho": {
    "pt-BR": "Propriedades do Trabalho",
    en: "Job Properties",
    es: "Propiedades del trabajo",
  },
  // O nome do trabalho no título: com vários abertos ao longo do dia, "Propriedades
  // do Trabalho" três vezes na barra de tarefas não diz qual é qual.
  "propriedades do job": {
    "pt-BR": 'Propriedades do Job - "{nome}"',
    en: 'Job Properties - "{nome}"',
    es: 'Propiedades del Job - "{nome}"',
  },
  "iniciar": { "pt-BR": "Iniciar", en: "Start", es: "Iniciar" },
  "escolha a etapa inicial": {
    "pt-BR": "Escolha a etapa em que “{trabalho}” deve começar.",
    en: "Choose the step where “{trabalho}” should start.",
    es: "Elija el paso en que “{trabalho}” debe empezar.",
  },
  "renomear trabalho": {
    "pt-BR": "Renomear trabalho",
    en: "Rename job",
    es: "Cambiar nombre del trabajo",
  },
  "excluir trabalho": { "pt-BR": "Excluir trabalho", en: "Delete job", es: "Eliminar trabajo" },
  "excluir trabalho mensagem": {
    "pt-BR":
      "Excluir “{trabalho}”? As etapas, o histórico e as agendas que só ele usa saem junto, e isso não se desfaz.",
    en: "Delete “{trabalho}”? Its steps, history and the schedules only it uses go with it, and this cannot be undone.",
    es: "¿Eliminar “{trabalho}”? Sus pasos, el historial y las programaciones que solo él usa se van con él, y esto no se deshace.",
  },
  "gravando o trabalho": {
    "pt-BR": "Gravando o trabalho…",
    en: "Saving the job…",
    es: "Guardando el trabajo…",
  },
  "trabalho gravado": {
    "pt-BR": "“{trabalho}” foi atualizado.",
    en: "“{trabalho}” was updated.",
    es: "“{trabalho}” fue actualizado.",
  },
  // Tela de acompanhamento da execução de um trabalho.
  "execucao do trabalho": {
    "pt-BR": "Execução do Trabalho",
    en: "Job Execution",
    es: "Ejecución del trabajo",
  },
  "execucao de": {
    "pt-BR": "Execução do Trabalho — “{trabalho}”",
    en: "Job Execution — “{trabalho}”",
    es: "Ejecución del trabajo — “{trabalho}”",
  },
  "iniciando o trabalho": {
    "pt-BR": "Iniciando…",
    en: "Starting…",
    es: "Iniciando…",
  },
  "a partir da etapa": {
    "pt-BR": "a partir da etapa “{etapa}”",
    en: "from step “{etapa}”",
    es: "a partir del paso “{etapa}”",
  },
  "aguardando o agent": {
    "pt-BR": "Aguardando o Agent pegar o trabalho…",
    en: "Waiting for the Agent to pick up the job…",
    es: "Esperando que el Agent tome el trabajo…",
  },
  "trabalho em execucao": {
    "pt-BR": "Em execução",
    en: "Running",
    es: "En ejecución",
  },
  "aguardando thread": {
    "pt-BR": "Aguardando uma thread livre do Agent",
    en: "Waiting for an Agent thread",
    es: "Esperando un hilo libre del Agent",
  },
  "entre tentativas": {
    "pt-BR": "Entre tentativas",
    en: "Between retries",
    es: "Entre intentos",
  },
  "execucao suspensa": {
    "pt-BR": "Execução suspensa",
    en: "Execution suspended",
    es: "Ejecución suspendida",
  },
  "concluindo execucao": {
    "pt-BR": "Concluindo",
    en: "Finishing up",
    es: "Finalizando",
  },
  "situacao desconhecida": {
    "pt-BR": "O Agent não informou a situação",
    en: "The Agent did not report the status",
    es: "El Agent no informó la situación",
  },
  "na etapa": {
    "pt-BR": "etapa {etapa}",
    en: "step {etapa}",
    es: "paso {etapa}",
  },
  "decorrido": {
    "pt-BR": "decorrido {tempo}",
    en: "elapsed {tempo}",
    es: "transcurrido {tempo}",
  },
  "em tempo": { "pt-BR": "em {tempo}", en: "in {tempo}", es: "en {tempo}" },
  "iniciada em": {
    "pt-BR": "iniciada em {momento}",
    en: "started at {momento}",
    es: "iniciada el {momento}",
  },
  "trabalho terminou ok": {
    "pt-BR": "Concluído com êxito",
    en: "Completed successfully",
    es: "Completado con éxito",
  },
  "trabalho terminou com falha": {
    "pt-BR": "O trabalho falhou",
    en: "The job failed",
    es: "El trabajo falló",
  },
  "trabalho foi cancelado": {
    "pt-BR": "A execução foi cancelada",
    en: "The run was canceled",
    es: "La ejecución fue cancelada",
  },
  "trabalho em nova tentativa": {
    "pt-BR": "A etapa vai ser repetida",
    en: "The step will be retried",
    es: "El paso será reintentado",
  },
  "trabalho terminou sem resultado": {
    "pt-BR": "Terminou, e o Agent não registrou o resultado",
    en: "It finished, and the Agent recorded no outcome",
    es: "Terminó, y el Agent no registró el resultado",
  },
  "trabalho nao iniciou": {
    "pt-BR": "O trabalho não foi iniciado",
    en: "The job was not started",
    es: "El trabajo no fue iniciado",
  },
  "trabalho ja estava rodando": {
    "pt-BR": "Este trabalho já estava em execução; o que a tela mostra é essa execução.",
    en: "This job was already running; what the screen shows is that run.",
    es: "Este trabajo ya estaba en ejecución; lo que la pantalla muestra es esa ejecución.",
  },
  "parada pedida": {
    "pt-BR": "Pedido de parada enviado ao Agent.",
    en: "Stop request sent to the Agent.",
    es: "Solicitud de detención enviada al Agent.",
  },
  "acompanhamento interrompido": {
    "pt-BR": "O acompanhamento parou; o trabalho segue no Agent",
    en: "Tracking stopped; the job carries on in the Agent",
    es: "El seguimiento se detuvo; el trabajo sigue en el Agent",
  },
  "parar": { "pt-BR": "Parar", en: "Stop", es: "Detener" },
  "trabalho continua rodando": {
    "pt-BR": "“{trabalho}” continua rodando no Agent.",
    en: "“{trabalho}” is still running in the Agent.",
    es: "“{trabalho}” sigue ejecutándose en el Agent.",
  },
  "trabalho concluido ok": {
    "pt-BR": "“{trabalho}” terminou com êxito em {tempo}.",
    en: "“{trabalho}” completed successfully in {tempo}.",
    es: "“{trabalho}” terminó con éxito en {tempo}.",
  },
  "trabalho concluido com falha": {
    "pt-BR": "“{trabalho}” falhou. {mensagem}",
    en: "“{trabalho}” failed. {mensagem}",
    es: "“{trabalho}” falló. {mensagem}",
  },
  "trabalho parado": {
    "pt-BR": "Pedido de parada enviado para “{trabalho}”.",
    en: "Stop request sent for “{trabalho}”.",
    es: "Solicitud de detención enviada para “{trabalho}”.",
  },
  "trabalho habilitado": {
    "pt-BR": "“{trabalho}” foi habilitado.",
    en: "“{trabalho}” was enabled.",
    es: "“{trabalho}” fue habilitado.",
  },
  "trabalho desabilitado aviso": {
    "pt-BR": "“{trabalho}” foi desabilitado e não vai mais rodar nas agendas dele.",
    en: "“{trabalho}” was disabled and will no longer run on its schedules.",
    es: "“{trabalho}” fue deshabilitado y ya no se ejecutará en sus programaciones.",
  },
  "trabalho renomeado": {
    "pt-BR": "“{de}” passou a se chamar “{para}”.",
    en: "“{de}” is now called “{para}”.",
    es: "“{de}” ahora se llama “{para}”.",
  },
  "trabalho excluido": {
    "pt-BR": "“{trabalho}” foi excluído.",
    en: "“{trabalho}” was deleted.",
    es: "“{trabalho}” fue eliminado.",
  },

  // Tela de Novo Trabalho.
  "novo trabalho": { "pt-BR": "Novo Trabalho", en: "New Job", es: "Nuevo trabajo" },
  "selecione uma pagina": {
    "pt-BR": "Selecione uma página",
    en: "Select a page",
    es: "Seleccione una página",
  },
  "geral": { "pt-BR": "Geral", en: "General", es: "General" },
  "etapas": { "pt-BR": "Etapas", en: "Steps", es: "Pasos" },
  "agendas": { "pt-BR": "Agendas", en: "Schedules", es: "Programaciones" },
  "proprietario": { "pt-BR": "Proprietário", en: "Owner", es: "Propietario" },
  "categoria": { "pt-BR": "Categoria", en: "Category", es: "Categoría" },
  "descricao": { "pt-BR": "Descrição", en: "Description", es: "Descripción" },
  "habilitado": { "pt-BR": "Habilitado", en: "Enabled", es: "Habilitado" },
  "habilitada": { "pt-BR": "Habilitada", en: "Enabled", es: "Habilitada" },
  "origem": { "pt-BR": "Origem", en: "Source", es: "Origen" },
  "criado em": { "pt-BR": "Criado em", en: "Created", es: "Creado" },
  "ultima modificacao": {
    "pt-BR": "Última modificação",
    en: "Last modified",
    es: "Última modificación",
  },
  "ultima execucao": { "pt-BR": "Última execução", en: "Last executed", es: "Última ejecución" },
  "preenchido pelo servidor": {
    "pt-BR": "Estes quatro campos são preenchidos pelo servidor depois que o trabalho é criado.",
    en: "These four fields are filled in by the server once the job is created.",
    es: "El servidor completa estos cuatro campos después de crear el trabajo.",
  },
  "nova": { "pt-BR": "Nova", en: "New", es: "Nueva" },
  "editar": { "pt-BR": "Editar", en: "Edit", es: "Editar" },
  "mover para cima": { "pt-BR": "Mover para cima", en: "Move up", es: "Mover arriba" },
  "mover para baixo": { "pt-BR": "Mover para baixo", en: "Move down", es: "Mover abajo" },
  "nenhuma etapa": { "pt-BR": "Nenhuma etapa", en: "No steps", es: "Ningún paso" },
  "nenhuma agenda": {
    "pt-BR": "Nenhuma agenda",
    en: "No schedules",
    es: "Ninguna programación",
  },
  "sim": { "pt-BR": "Sim", en: "Yes", es: "Sí" },
  "nao": { "pt-BR": "Não", en: "No", es: "No" },
  "informe o nome do trabalho": {
    "pt-BR": "Informe o nome do trabalho.",
    en: "Enter the job name.",
    es: "Indique el nombre del trabajo.",
  },
  "informe ao menos uma etapa": {
    "pt-BR": "Um trabalho sem etapa nunca faz nada: acrescente ao menos uma.",
    en: "A job with no steps never does anything: add at least one.",
    es: "Un trabajo sin pasos nunca hace nada: agregue al menos uno.",
  },
  "criando o trabalho": {
    "pt-BR": "Criando o trabalho…",
    en: "Creating the job…",
    es: "Creando el trabajo…",
  },

  // Tela de etapa.
  "etapa": { "pt-BR": "Etapa do Trabalho", en: "Job Step", es: "Paso del trabajo" },
  "nome da etapa": { "pt-BR": "Nome da etapa", en: "Step name", es: "Nombre del paso" },
  "tipo": { "pt-BR": "Tipo", en: "Type", es: "Tipo" },
  "banco de dados singular": {
    "pt-BR": "Banco de dados",
    en: "Database",
    es: "Base de datos",
  },
  "comando": { "pt-BR": "Comando", en: "Command", es: "Comando" },
  // Só o rótulo do CmdExec tem palavra traduzível; T-SQL e PowerShell são nomes.
  "etapa sistema operacional": {
    "pt-BR": "Sistema operacional (CmdExec)",
    en: "Operating system (CmdExec)",
    es: "Sistema operativo (CmdExec)",
  },
  "ao ter exito": { "pt-BR": "Ao ter êxito", en: "On success", es: "Al tener éxito" },
  "ao falhar": { "pt-BR": "Ao falhar", en: "On failure", es: "Al fallar" },
  "sair informando exito": {
    "pt-BR": "Sair informando êxito",
    en: "Quit reporting success",
    es: "Salir informando éxito",
  },
  "sair informando falha": {
    "pt-BR": "Sair informando falha",
    en: "Quit reporting failure",
    es: "Salir informando error",
  },
  "ir para a proxima etapa": {
    "pt-BR": "Ir para a próxima etapa",
    en: "Go to the next step",
    es: "Ir al paso siguiente",
  },
  "informe o nome da etapa": {
    "pt-BR": "Informe o nome da etapa.",
    en: "Enter the step name.",
    es: "Indique el nombre del paso.",
  },
  "informe o comando da etapa": {
    "pt-BR": "Informe o comando que a etapa executa.",
    en: "Enter the command the step runs.",
    es: "Indique el comando que ejecuta el paso.",
  },

  // Tela de agenda.
  "agenda": { "pt-BR": "Agenda do Trabalho", en: "Job Schedule", es: "Programación" },
  "tipo de agenda": {
    "pt-BR": "Tipo de agenda",
    en: "Schedule type",
    es: "Tipo de programación",
  },
  "recorrente": { "pt-BR": "Recorrente", en: "Recurring", es: "Periódica" },
  "uma vez": { "pt-BR": "Uma vez", en: "One time", es: "Una vez" },
  "ao iniciar o agente": {
    "pt-BR": "Iniciar automaticamente com o SQL Server Agent",
    en: "Start automatically when SQL Server Agent starts",
    es: "Iniciar automáticamente con SQL Server Agent",
  },
  "quando a cpu ficar ociosa": {
    "pt-BR": "Iniciar quando as CPUs ficarem ociosas",
    en: "Start whenever the CPUs become idle",
    es: "Iniciar cuando las CPU estén inactivas",
  },
  "data": { "pt-BR": "Data", en: "Date", es: "Fecha" },
  "hora": { "pt-BR": "Hora", en: "Time", es: "Hora" },
  "frequencia": { "pt-BR": "Frequência", en: "Frequency", es: "Frecuencia" },
  "ocorre": { "pt-BR": "Ocorre", en: "Occurs", es: "Se produce" },
  "diariamente": { "pt-BR": "Diariamente", en: "Daily", es: "Diariamente" },
  "semanalmente": { "pt-BR": "Semanalmente", en: "Weekly", es: "Semanalmente" },
  "mensalmente": { "pt-BR": "Mensalmente", en: "Monthly", es: "Mensualmente" },
  "recorre a cada": { "pt-BR": "Recorre a cada", en: "Recurs every", es: "Se repite cada" },
  "dias minusculo": { "pt-BR": "dia(s)", en: "day(s)", es: "día(s)" },
  "semanas minusculo": { "pt-BR": "semana(s) em", en: "week(s) on", es: "semana(s) en" },
  "meses minusculo": { "pt-BR": "mês(es)", en: "month(s)", es: "mes(es)" },
  "horas minusculo": { "pt-BR": "hora(s)", en: "hour(s)", es: "hora(s)" },
  "minutos minusculo": { "pt-BR": "minuto(s)", en: "minute(s)", es: "minuto(s)" },
  "dias da semana": { "pt-BR": "Dias da semana", en: "Days of the week", es: "Días de la semana" },
  "de cada": { "pt-BR": "de cada", en: "of every", es: "de cada" },
  "artigo do mes relativo": { "pt-BR": "O", en: "The", es: "El" },
  "frequencia diaria": {
    "pt-BR": "Frequência diária",
    en: "Daily frequency",
    es: "Frecuencia diaria",
  },
  "ocorre uma vez as": {
    "pt-BR": "Ocorre uma vez às",
    en: "Occurs once at",
    es: "Se produce una vez a las",
  },
  "ocorre a cada": { "pt-BR": "Ocorre a cada", en: "Occurs every", es: "Se produce cada" },
  "comecando as": { "pt-BR": "começando às", en: "starting at", es: "empezando a las" },
  "terminando as": { "pt-BR": "terminando às", en: "ending at", es: "terminando a las" },
  "duracao": { "pt-BR": "Duração", en: "Duration", es: "Duración" },
  "data de inicio": { "pt-BR": "Data de início", en: "Start date", es: "Fecha de inicio" },
  "data de termino": { "pt-BR": "Data de término", en: "End date", es: "Fecha de término" },
  "sem data de termino": {
    "pt-BR": "Sem data de término",
    en: "No end date",
    es: "Sin fecha de término",
  },
  "resumo": { "pt-BR": "Resumo", en: "Summary", es: "Resumen" },
  "informe o nome da agenda": {
    "pt-BR": "Informe o nome da agenda.",
    en: "Enter the schedule name.",
    es: "Indique el nombre de la programación.",
  },
  "marque ao menos um dia": {
    "pt-BR": "Marque ao menos um dia da semana; sem nenhum, o trabalho nunca roda.",
    en: "Check at least one weekday; with none, the job never runs.",
    es: "Marque al menos un día de la semana; sin ninguno, el trabajo nunca se ejecuta.",
  },
  "termino antes do inicio": {
    "pt-BR": "A data de término é anterior à de início.",
    en: "The end date comes before the start date.",
    es: "La fecha de término es anterior a la de inicio.",
  },
  "hora final antes da inicial": {
    "pt-BR": "A hora final precisa ser posterior à inicial.",
    en: "The ending time must come after the starting time.",
    es: "La hora final debe ser posterior a la inicial.",
  },

  // Ordens e unidades da frequência mensal relativa.
  "primeiro": { "pt-BR": "primeiro", en: "first", es: "primero" },
  "segundo": { "pt-BR": "segundo", en: "second", es: "segundo" },
  "terceiro": { "pt-BR": "terceiro", en: "third", es: "tercero" },
  "quarto": { "pt-BR": "quarto", en: "fourth", es: "cuarto" },
  "ultimo": { "pt-BR": "último", en: "last", es: "último" },
  "dia": { "pt-BR": "dia", en: "day", es: "día" },
  "dia util": { "pt-BR": "dia útil", en: "weekday", es: "día hábil" },
  "dia de fim de semana": {
    "pt-BR": "dia de fim de semana",
    en: "weekend day",
    es: "día de fin de semana",
  },
  "domingo": { "pt-BR": "domingo", en: "Sunday", es: "domingo" },
  "segunda-feira": { "pt-BR": "segunda-feira", en: "Monday", es: "lunes" },
  "terca-feira": { "pt-BR": "terça-feira", en: "Tuesday", es: "martes" },
  "quarta-feira": { "pt-BR": "quarta-feira", en: "Wednesday", es: "miércoles" },
  "quinta-feira": { "pt-BR": "quinta-feira", en: "Thursday", es: "jueves" },
  "sexta-feira": { "pt-BR": "sexta-feira", en: "Friday", es: "viernes" },
  "sabado": { "pt-BR": "sábado", en: "Saturday", es: "sábado" },

  // Resumo da agenda por extenso — a frase que o SSMS mostra em "Resumo".
  "agenda ao iniciar agente": {
    "pt-BR": "Inicia junto com o SQL Server Agent.",
    en: "Starts when SQL Server Agent starts.",
    es: "Se inicia junto con SQL Server Agent.",
  },
  "agenda cpu ociosa": {
    "pt-BR": "Inicia quando as CPUs ficarem ociosas.",
    en: "Starts whenever the CPUs become idle.",
    es: "Se inicia cuando las CPU estén inactivas.",
  },
  "agenda uma vez": {
    "pt-BR": "Ocorre uma vez em {data}, às {hora}.",
    en: "Occurs once on {data} at {hora}.",
    es: "Se produce una vez el {data} a las {hora}.",
  },
  "agenda todo dia": { "pt-BR": "Ocorre todos os dias", en: "Occurs every day", es: "Se produce todos los días" },
  "agenda a cada dias": {
    "pt-BR": "Ocorre a cada {n} dias",
    en: "Occurs every {n} days",
    es: "Se produce cada {n} días",
  },
  "agenda toda semana": {
    "pt-BR": "Ocorre toda semana em {dias}",
    en: "Occurs every week on {dias}",
    es: "Se produce cada semana en {dias}",
  },
  "agenda a cada semanas": {
    "pt-BR": "Ocorre a cada {n} semanas em {dias}",
    en: "Occurs every {n} weeks on {dias}",
    es: "Se produce cada {n} semanas en {dias}",
  },
  "nenhum dia": { "pt-BR": "nenhum dia", en: "no day", es: "ningún día" },
  "agenda todo mes no dia": {
    "pt-BR": "Ocorre no dia {dia} de todo mês",
    en: "Occurs on day {dia} of every month",
    es: "Se produce el día {dia} de cada mes",
  },
  "agenda a cada meses no dia": {
    "pt-BR": "Ocorre no dia {dia} a cada {n} meses",
    en: "Occurs on day {dia} every {n} months",
    es: "Se produce el día {dia} cada {n} meses",
  },
  "agenda todo mes relativa": {
    "pt-BR": "Ocorre no {ordem} {unidade} de todo mês",
    en: "Occurs on the {ordem} {unidade} of every month",
    es: "Se produce el {ordem} {unidade} de cada mes",
  },
  "agenda a cada meses relativa": {
    "pt-BR": "Ocorre no {ordem} {unidade} a cada {n} meses",
    en: "Occurs on the {ordem} {unidade} every {n} months",
    es: "Se produce el {ordem} {unidade} cada {n} meses",
  },
  "agenda as horas": { "pt-BR": ", às {hora}.", en: " at {hora}.", es: ", a las {hora}." },
  "agenda a cada horas": {
    "pt-BR": ", a cada {n} hora(s) entre {inicio} e {fim}.",
    en: ", every {n} hour(s) between {inicio} and {fim}.",
    es: ", cada {n} hora(s) entre {inicio} y {fim}.",
  },
  "agenda a cada minutos": {
    "pt-BR": ", a cada {n} minuto(s) entre {inicio} e {fim}.",
    en: ", every {n} minute(s) between {inicio} and {fim}.",
    es: ", cada {n} minuto(s) entre {inicio} y {fim}.",
  },
  "agenda vigencia": {
    "pt-BR": "A agenda vale a partir de {inicio}.",
    en: "The schedule is used starting {inicio}.",
    es: "La programación vale a partir de {inicio}.",
  },
  "agenda vigencia ate": {
    "pt-BR": "A agenda vale de {inicio} até {fim}.",
    en: "The schedule is used from {inicio} to {fim}.",
    es: "La programación vale de {inicio} a {fim}.",
  },

  // --- Tela de atualização (requisito 30) -----------------------------------
  "atualizacao disponivel": {
    "pt-BR": "Atualização disponível",
    en: "Update available",
    es: "Actualización disponible",
  },
  "atualizacao": { "pt-BR": "Atualização", en: "Update", es: "Actualización" },
  "baixar e instalar": {
    "pt-BR": "Baixar e instalar",
    en: "Download and install",
    es: "Descargar e instalar",
  },
  "agora nao": { "pt-BR": "Agora não", en: "Not now", es: "Ahora no" },
  "instalador de": {
    "pt-BR": "Instalador de {tamanho}.",
    en: "{tamanho} installer.",
    es: "Instalador de {tamanho}.",
  },
  "baixando": { "pt-BR": "Baixando…", en: "Downloading…", es: "Descargando…" },
  "baixando porcento": {
    "pt-BR": "Baixando… {porcento}% de {total}",
    en: "Downloading… {porcento}% of {total}",
    es: "Descargando… {porcento}% de {total}",
  },
  "baixando quanto": {
    "pt-BR": "Baixando… {baixados}",
    en: "Downloading… {baixados}",
    es: "Descargando… {baixados}",
  },
  "iniciando o instalador": {
    "pt-BR": "Iniciando o instalador…",
    en: "Starting the installer…",
    es: "Iniciando el instalador…",
  },
  "vai fechar para instalar": {
    "pt-BR": "O Cururu vai fechar para a instalação continuar.",
    en: "Cururu will close so the installation can continue.",
    es: "Cururu se cerrará para que la instalación continúe.",
  },
  "tentar de novo": {
    "pt-BR": "Tentar de novo",
    en: "Try again",
    es: "Intentar de nuevo",
  },
  "abrir a pagina do release": {
    "pt-BR": "Abrir a página do release",
    en: "Open the release page",
    es: "Abrir la página del release",
  },
  "em execucao versao": {
    "pt-BR": "Em execução: {versao}",
    en: "Running: {versao}",
    es: "En ejecución: {versao}",
  },
  "o que mudou": { "pt-BR": "O que mudou", en: "What changed", es: "Qué cambió" },
  "cururu esta atualizado": {
    "pt-BR": "O Cururu está atualizado.",
    en: "Cururu is up to date.",
    es: "Cururu está actualizado.",
  },
  "versao mais recente": {
    "pt-BR": "A versão {versao} é a mais recente publicada.",
    en: "Version {versao} is the latest published.",
    es: "La versión {versao} es la más reciente publicada.",
  },
  "esta e a mais recente": {
    "pt-BR": "Esta é a versão mais recente publicada.",
    en: "This is the latest published version.",
    es: "Esta es la versión más reciente publicada.",
  },
  "release sem descricao": {
    "pt-BR": "A versão publicada não trouxe descrição.",
    en: "The published version came with no description.",
    es: "La versión publicada no trajo descripción.",
  },

  // --- Tela de filtro (requisito 4) -----------------------------------------
  "filtrar titulo": { "pt-BR": "Filtrar {alvo}", en: "Filter {alvo}", es: "Filtrar {alvo}" },
  "incluir": { "pt-BR": "Incluir", en: "Include", es: "Incluir" },
  "excluir grupo": { "pt-BR": "Excluir", en: "Exclude", es: "Excluir" },
  "criterio": { "pt-BR": "Critério", en: "Criterion", es: "Criterio" },
  "exclusao": { "pt-BR": "Exclusão", en: "Exclusion", es: "Exclusión" },
  "filtro ajuda inclusao": {
    "pt-BR": "{alvo} entra se corresponder a pelo menos um critério.",
    en: "{alvo} is listed if it matches at least one criterion.",
    es: "{alvo} entra si corresponde a por lo menos un criterio.",
  },
  "filtro ajuda exclusao": {
    "pt-BR": "{alvo} não aparece se corresponder a qualquer critério.",
    en: "{alvo} is hidden if it matches any criterion.",
    es: "{alvo} no aparece si corresponde a cualquier criterio.",
  },
  "alvo": { "pt-BR": "Alvo", en: "Target", es: "Objetivo" },
  "operador comeca com": {
    "pt-BR": "começa com",
    en: "starts with",
    es: "empieza con",
  },
  "operador termina com": {
    "pt-BR": "termina com",
    en: "ends with",
    es: "termina con",
  },
  "operador": { "pt-BR": "Operador", en: "Operator", es: "Operador" },
  "valor": { "pt-BR": "Valor", en: "Value", es: "Valor" },
  "logica resultante": {
    "pt-BR": "Lógica resultante",
    en: "Resulting logic",
    es: "Lógica resultante",
  },
  "nenhum criterio": { "pt-BR": "Nenhum critério", en: "No criteria", es: "Ningún criterio" },
  "aplicar filtro botao": {
    "pt-BR": "Aplicar filtro",
    en: "Apply filter",
    es: "Aplicar filtro",
  },
  "alvo fixo": {
    "pt-BR": "Alvo fixo: {alvo}",
    en: "Fixed target: {alvo}",
    es: "Objetivo fijo: {alvo}",
  },
  "use porcento como curinga": {
    "pt-BR": "use % como curinga",
    en: "use % as wildcard",
    es: "use % como comodín",
  },
  "remover criterio": {
    "pt-BR": "Remover critério",
    en: "Remove criterion",
    es: "Quitar criterio",
  },
  "operador de": {
    "pt-BR": "Operador de {grupo}",
    en: "{grupo} operator",
    es: "Operador de {grupo}",
  },
  "valor do criterio de": {
    "pt-BR": "Valor do critério de {grupo}",
    en: "{grupo} criterion value",
    es: "Valor del criterio de {grupo}",
  },
  "excluir objetos gerados": {
    "pt-BR": "Excluir objetos gerados pelo banco",
    en: "Exclude database-generated objects",
    es: "Excluir objetos generados por la base",
  },

  // --- Tela de recuperação (requisito 12) -----------------------------------
  "recuperar consultas titulo": {
    "pt-BR": "Recuperar Consultas",
    en: "Recover Queries",
    es: "Recuperar consultas",
  },
  "abrir numa aba": {
    "pt-BR": "Abrir numa aba",
    en: "Open in a tab",
    es: "Abrir en una pestaña",
  },
  "descartar": { "pt-BR": "Descartar", en: "Discard", es: "Descartar" },
  "nenhuma consulta guardada": {
    "pt-BR": "Nenhuma consulta guardada.",
    en: "No saved queries.",
    es: "Ninguna consulta guardada.",
  },
  "vazia": { "pt-BR": "(vazia)", en: "(empty)", es: "(vacía)" },

  // --- Telas Versões e Sobre ------------------------------------------------
  "nenhuma versao registrada": {
    "pt-BR": "Nenhuma versão registrada.",
    en: "No versions recorded.",
    es: "Ninguna versión registrada.",
  },
  "em execucao marca": { "pt-BR": "em execução", en: "running", es: "en ejecución" },
  "sobre o cururu": {
    "pt-BR": "Sobre o Cururu",
    en: "About Cururu",
    es: "Acerca de Cururu",
  },
  "cliente sql": { "pt-BR": "Cliente SQL", en: "SQL client", es: "Cliente SQL" },
  "versao rotulo": {
    "pt-BR": "Versão {versao}",
    en: "Version {versao}",
    es: "Versión {versao}",
  },
  "desenvolvido por": {
    "pt-BR": "Desenvolvido por ",
    en: "Developed by ",
    es: "Desarrollado por ",
  },

  // --- Importação de conexões (requisito 25) --------------------------------
  "importar conexao": {
    "pt-BR": "Importar conexão",
    en: "Import connection",
    es: "Importar conexión",
  },
  "conexoes encontradas": {
    "pt-BR": "{n} conexão(ões) encontrada(s)",
    en: "{n} connection(s) found",
    es: "{n} conexión(es) encontrada(s)",
  },
  "endereco": { "pt-BR": "Endereço", en: "Address", es: "Dirección" },
  "senha nao e importada": {
    "pt-BR":
      "A senha não é importada — ela fica cifrada com chave própria na ferramenta de origem. " +
      "Informe a senha depois de importar.",
    en:
      "The password is not imported — it is encrypted with the source tool's own key. " +
      "Enter the password after importing.",
    es:
      "La contraseña no se importa — queda cifrada con clave propia en la herramienta de origen. " +
      "Indique la contraseña después de importar.",
  },
  // --- Mensagens de rodapé e de status --------------------------------------
  "consultas guardadas": {
    "pt-BR": "{n} consulta(s) guardada(s); as cópias duram no máximo 7 dias.",
    en: "{n} saved query(ies); copies last at most 7 days.",
    es: "{n} consulta(s) guardada(s); las copias duran como máximo 7 días.",
  },
  "sem conexao": { "pt-BR": "sem conexão", en: "no connection", es: "sin conexión" },
  "descartar copia": {
    "pt-BR": "Descartar a cópia de “{titulo}”?",
    en: "Discard the copy of “{titulo}”?",
    es: "¿Descartar la copia de “{titulo}”?",
  },
  "informe um label": {
    "pt-BR": "Informe um label para o botão.",
    en: "Enter a label for the button.",
    es: "Informe una etiqueta para el botón.",
  },
  "label no maximo oito": {
    "pt-BR": "O label pode ter no máximo 8 caracteres.",
    en: "The label can have at most 8 characters.",
    es: "La etiqueta puede tener como máximo 8 caracteres.",
  },
  "atalho ja pertence": {
    "pt-BR": "Esse atalho já pertence ao botão {n}.",
    en: "That shortcut already belongs to button {n}.",
    es: "Ese atajo ya pertenece al botón {n}.",
  },
  "posicao entre": {
    "pt-BR": "A posição precisa estar entre 1 e {limite}.",
    en: "The position must be between 1 and {limite}.",
    es: "La posición debe estar entre 1 y {limite}.",
  },
  "copias duram sete dias": {
    "pt-BR": "As cópias duram no máximo 7 dias.",
    en: "Copies last at most 7 days.",
    es: "Las copias duran como máximo 7 días.",
  },
  "erros de compilacao": {
    "pt-BR": "{n} erro(s) de compilação.",
    en: "{n} compilation error(s).",
    es: "{n} error(es) de compilación.",
  },
  "exportado para": {
    "pt-BR": "Exportado: {caminho}",
    en: "Exported: {caminho}",
    es: "Exportado: {caminho}",
  },
  "ddl copiado": {
    "pt-BR": "DDL de {objeto} copiado.",
    en: "DDL for {objeto} copied.",
    es: "DDL de {objeto} copiado.",
  },
  "falha ao ler conexoes": {
    "pt-BR": "Não foi possível ler as conexões salvas: {erro}",
    en: "Could not read the saved connections: {erro}",
    es: "No fue posible leer las conexiones guardadas: {erro}",
  },
  "conexao excluida": {
    "pt-BR": "Conexão “{conexao}” excluída.",
    en: "Connection “{conexao}” deleted.",
    es: "Conexión “{conexao}” eliminada.",
  },
  "conexao cancelada": {
    "pt-BR": "conexão cancelada",
    en: "connection canceled",
    es: "conexión cancelada",
  },
  "falha interna do aplicativo": {
    "pt-BR": "Falha interna do aplicativo: {detalhe}",
    en: "Internal application failure: {detalhe}",
    es: "Falla interna de la aplicación: {detalhe}",
  },
  "backup sem lista de botoes": {
    "pt-BR": "O backup não contém uma lista válida de botões.",
    en: "The backup does not contain a valid list of buttons.",
    es: "La copia de seguridad no contiene una lista válida de botones.",
  },
  "atalhos invalidos": {
    "pt-BR": "A configuração de atalhos é inválida.",
    en: "The shortcut configuration is invalid.",
    es: "La configuración de atajos no es válida.",
  },
  "carregar backup substitui": {
    "pt-BR":
      "Carregar este backup substituirá as conexões salvas e todos os botões personalizados. Continuar?",
    en: "Loading this backup will replace the saved connections and every custom button. Continue?",
    es: "Cargar esta copia sustituirá las conexiones guardadas y todos los botones personalizados. ¿Continuar?",
  },

  // --- Diálogos de arquivo do Windows ---------------------------------------
  "arquivo script sql": { "pt-BR": "Script SQL", en: "SQL script", es: "Script SQL" },
  "arquivo backup do cururu": {
    "pt-BR": "Backup do Cururu",
    en: "Cururu backup",
    es: "Copia de seguridad Cururu",
  },
  "arquivo todos": {
    "pt-BR": "Todos os arquivos",
    en: "All files",
    es: "Todos los archivos",
  },
  "arquivo csv": { "pt-BR": "Arquivo CSV", en: "CSV file", es: "Archivo CSV" },
  "arquivo planilha do excel": {
    "pt-BR": "Planilha do Excel",
    en: "Excel spreadsheet",
    es: "Hoja de cálculo de Excel",
  },

  // --- Bind variables (requisito 15) ----------------------------------------
  // Sem o atalho atrás: aqui o botão confirma os valores, e o `t("executar")`
  // da barra de ferramentas anuncia o Ctrl+Enter, que não vale nesta tela.
  "executar binds": { "pt-BR": "Executar", en: "Run", es: "Ejecutar" },
  "informe o valor": {
    "pt-BR": "Informe o valor",
    en: "Enter the value",
    es: "Indique el valor",
  },
  "informe os valores": {
    "pt-BR": "Informe os valores",
    en: "Enter the values",
    es: "Indique los valores",
  },
  "substituicoes no script": {
    "pt-BR": "Valores de &variável substituem o texto do script, inclusive dentro de aspas. Valores de :variável são enviados separadamente ao banco.",
    en: "Values for &variable replace script text, including inside quotes. Values for :variable are sent separately to the database.",
    es: "Los valores de &variable sustituyen el texto del script, incluso entre comillas. Los valores de :variable se envían por separado a la base.",
  },
  "binds vao como texto": {
    "pt-BR":
      "Os valores são enviados como texto; o banco converte para número ou data conforme a coluna.",
    en: "Values are sent as text; the database converts them to number or date according to the column.",
    es: "Los valores se envían como texto; la base los convierte a número o fecha según la columna.",
  },

  // --- Grid, abas e divisor -------------------------------------------------
  "ajuda linhas por vez": {
    "pt-BR": "Quantidade de linhas trazidas em cada fetch.",
    en: "How many rows each fetch brings.",
    es: "Cantidad de filas traídas en cada fetch.",
  },
  "arraste para mudar a largura": {
    "pt-BR": "Arraste para mudar a largura (duplo clique ajusta ao conteúdo)",
    en: "Drag to resize (double-click fits the content)",
    es: "Arrastre para cambiar el ancho (doble clic ajusta al contenido)",
  },
  "arraste para mudar a altura": {
    "pt-BR": "Arraste para mudar a altura do conjunto (duplo clique abre e volta ao padrão)",
    en: "Drag to change this result set's height (double-click expands it and back)",
    es: "Arrastre para cambiar la altura del conjunto (doble clic lo expande y vuelve)",
  },
  "arraste para redimensionar": {
    "pt-BR": "Arraste para redimensionar (duplo clique volta ao padrão)",
    en: "Drag to resize (double-click restores the default)",
    es: "Arrastre para redimensionar (doble clic vuelve al valor estándar)",
  },
  "fechar aba nomeada": { "pt-BR": "Fechar {aba}", en: "Close {aba}", es: "Cerrar {aba}" },
  "caracteres": {
    "pt-BR": "{n} caractere(s)",
    en: "{n} character(s)",
    es: "{n} carácter(es)",
  },
  "linha coluna erro": {
    "pt-BR": "Linha {linha}, coluna {coluna}: {texto}",
    en: "Line {linha}, column {coluna}: {texto}",
    es: "Línea {linha}, columna {coluna}: {texto}",
  },

  // --- Espécie do objeto na lista do auto-complete --------------------------
  // `view`, `procedure` e `package body` são termo técnico: iguais nas três.
  "especie tabela": { "pt-BR": "tabela", en: "table", es: "tabla" },
  "especie tabela temporaria": {
    "pt-BR": "tabela temporária",
    en: "temporary table",
    es: "tabla temporal",
  },
  "especie view": { "pt-BR": "view", en: "view", es: "view" },
  "especie view materializada": {
    "pt-BR": "view materializada",
    en: "materialized view",
    es: "view materializada",
  },
  "especie sinonimo": { "pt-BR": "sinônimo", en: "synonym", es: "sinónimo" },
  "especie pacote": { "pt-BR": "pacote", en: "package", es: "paquete" },
  "especie procedure": { "pt-BR": "procedure", en: "procedure", es: "procedure" },
  "especie funcao": { "pt-BR": "função", en: "function", es: "función" },
  "especie sequencia": { "pt-BR": "sequência", en: "sequence", es: "secuencia" },
  "especie tipo": { "pt-BR": "tipo", en: "type", es: "tipo" },

  // --- Nome das teclas na notação dos atalhos --------------------------------
  // A notação **guardada** é sempre a portuguesa: é ela que vai para o
  // `localStorage`, para o backup e para o keymap do CodeMirror. Estas chaves
  // traduzem só o que aparece na tela, para que trocar de idioma não invalide
  // atalho nenhum já gravado.
  "tecla espaco": { "pt-BR": "Espaço", en: "Space", es: "Espacio" },
  "tecla seta acima": { "pt-BR": "Seta acima", en: "Up arrow", es: "Flecha arriba" },
  "tecla seta abaixo": { "pt-BR": "Seta abaixo", en: "Down arrow", es: "Flecha abajo" },
  "tecla seta esquerda": {
    "pt-BR": "Seta esquerda",
    en: "Left arrow",
    es: "Flecha izquierda",
  },
  "tecla seta direita": {
    "pt-BR": "Seta direita",
    en: "Right arrow",
    es: "Flecha derecha",
  },

  // --- Botões rápidos (requisitos 33 e 38) ----------------------------------
  "dica do botao rapido": {
    "pt-BR": "{situacao}{atalho}. Clique direito para configurar; arraste para mover.",
    en: "{situacao}{atalho}. Right-click to configure; drag to move.",
    es: "{situacao}{atalho}. Clic derecho para configurar; arrastre para mover.",
  },
  "novo botao": { "pt-BR": "Novo botão", en: "New button", es: "Nuevo botón" },
  "configurar botao": {
    "pt-BR": "Configurar botão {n}",
    en: "Configure button {n}",
    es: "Configurar botón {n}",
  },
  "label do botao": { "pt-BR": "Label", en: "Label", es: "Label" },
  "tecla de atalho do botao": {
    "pt-BR": "Tecla de atalho",
    en: "Keyboard shortcut",
    es: "Tecla de acceso rápido",
  },
  "ajuda atalho do botao": {
    "pt-BR": "Selecione o campo e pressione a combinação. O atalho é opcional.",
    en: "Select the field and press the combination. The shortcut is optional.",
    es: "Seleccione el campo y presione la combinación. El atajo es opcional.",
  },
  "posicao": { "pt-BR": "Posição", en: "Position", es: "Posición" },
  "conteudo": { "pt-BR": "Conteúdo", en: "Content", es: "Contenido" },
  "use modificador no atalho": {
    "pt-BR": "Use Ctrl, Alt ou Shift junto com a tecla (ou uma tecla F1–F24).",
    en: "Use Ctrl, Alt or Shift together with the key (or an F1–F24 key).",
    es: "Use Ctrl, Alt o Shift junto con la tecla (o una tecla F1–F24).",
  },
} satisfies Record<string, Record<Idioma, string>>;
