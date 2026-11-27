# Instruções para agentes — Cururu

Este arquivo trata de **como trabalhar** no Cururu: verificar, versionar, compilar e publicar.

- **O que o produto faz** → [requisitos.md](requisitos.md)
- **Por que as decisões foram tomadas assim** → [premissas.md](premissas.md)
- **O que mudou em cada versão** → [versoes.md](versoes.md)

## Repositório público e informações sensíveis

**Este repositório, [yurimj/Cururu](https://github.com/yurimj/Cururu), é público.** Tudo que for versionado, inclusive o histórico de commits, pode ser lido por qualquer pessoa.

- Nunca inclua senhas, tokens, chaves privadas, strings de conexão com credenciais ou dados reais de clientes em código, comentários, documentação, testes, exemplos, logs ou arquivos anexados a releases.
- Use dados fictícios em exemplos e testes. Antes de compartilhar logs ou capturas de tela, remova credenciais, dados pessoais e informações internas.
- Mantenha segredos fora do Git, em variáveis de ambiente ou arquivos locais ignorados. Não embuta segredos no executável distribuído. Adicionar um arquivo ao `.gitignore` não remove conteúdo já versionado.
- Antes de criar commits ou publicar arquivos, revise o conteúdo e o diff para evitar exposição de informações sensíveis.
- Se encontrar uma credencial exposta, avise o usuário sem reproduzir seu valor e providencie sua revogação ou troca pelo responsável. Apagar o arquivo ou criar um novo commit não elimina a exposição no histórico; qualquer limpeza do histórico deve ser combinada com o usuário.

## Como verificar uma alteração

O Cururu é um aplicativo Tauri: o frontend é web (`src/`) e o backend é Rust (`src-tauri/`). Isso dá dois ambientes de verificação, e escolher o errado custa caro — um `cargo build --release` leva cerca de quatro minutos, e pagá-lo para olhar um pixel é desperdício puro.

A ordem é esta.

### 1. Mudança de frontend — no navegador

Alterações em `src/` que sejam de layout, CSS, cor, texto, atalho de teclado, comportamento do CodeMirror ou estrutura de diálogo são verificadas no servidor de desenvolvimento:

```bash
npm run dev
```

Sobe em `http://localhost:1420`, já configurado em [`.claude/launch.json`](.claude/launch.json) sob o nome `vite`. No Claude Code, use o painel Browser (`preview_start` com `{"name": "vite"}`) e confira com `read_page`, `read_console_messages` e captura de tela.

O navegador também serve para **medir**, e não só para olhar: as funções puras do frontend podem ser importadas pelo console e chamadas com casos de teste, e `getBoundingClientRect` responde perguntas de alinhamento com número em vez de opinião.

**Nesta etapa não se gera executável nem se incrementa a versão.**

#### O aviso de `invoke` no navegador é esperado

Fora da webview do Tauri a ponte `invoke()` não existe, então a primeira chamada de [`src/api.ts`](src/api.ts) falha e o aplicativo mostra, no topo e na barra de status:

> Falha interna do aplicativo: Cannot read properties of undefined (reading 'invoke')

**Isso não é defeito e não deve ser investigado nem "corrigido".** É a assinatura normal do frontend rodando sem backend. O tratamento em [`src/main.ts`](src/main.ts) é deliberado — a falha de diagnóstico é capturada justamente para a janela não ficar em branco —, e o resto da interface monta normalmente: painel de conexões, barra de ferramentas, divisor, área de trabalho, barra de status e os diálogos.

### 2. Mudança de backend — no log, não no mouse

O navegador **não** serve para verificar nada que atravesse o Rust: árvore de objetos, execução de SQL, grid com dados reais, fetch parcial, commit/rollback, DPAPI, Instant Client, importação de conexões. Todas as chamadas de `src/api.ts` rejeitam ali.

Para essas, o executável é inevitável — mas a verificação primária é **ler o log**:

```
%APPDATA%\Cururu\logs\
```

### 3. Mouse na janela nativa — último recurso

Só quando:

- o usuário pedir explicitamente;
- a interação existir apenas na janela nativa — ícone da barra de tarefas, diálogo nativo de arquivo, foco, redimensionamento, comportamento de janela;
- o log e o navegador já tiverem se esgotado sem esclarecer o defeito.

Nunca use o `.exe` para conferir espaçamento, cor, rótulo, ordem de campos ou qualquer coisa que o navegador mostre igual.

### Resumo

| O que verificar | Onde |
|---|---|
| Layout, CSS, paleta de cores, tipografia | navegador (`npm run dev`) |
| Diálogos, abas, painéis, divisores | navegador |
| Editor SQL: destaque, atalhos, localizar/substituir | navegador |
| Árvore, execução, grid com dados, fetch | `.exe` + log |
| `.sql` aberto pelo Windows, entrega à janela aberta | `.exe` + log |
| Conexão, senha (DPAPI), Instant Client, importação | `.exe` + log |
| Consulta de atualização, download, instalador | `.exe` + log |
| Ícone, barra de tarefas, comportamento de janela | `.exe` |

O layout da tela de atualização é frontend e se confere no navegador como qualquer outro diálogo — basta importar `dialogo-atualizacao.ts` pelo console e chamá-lo com um objeto de exemplo, já que `verificarAtualizacao()` rejeita fora da webview. O que **não** se confere ali é a consulta em si, o download e a execução do instalador: tudo isso atravessa o Rust.

## Revisão dos logs

**Ao investigar qualquer defeito, o primeiro passo é ler o log** — antes de supor causa, antes de mexer no código.

Toda operação registra início, fim, duração e resultado exatamente para que o defeito seja diagnosticado pelo texto, sem precisar reproduzir o problema clicando na tela.

Também se revisa o log:

- Depois de qualquer alteração que envolva banco, para conferir que nada passou a falhar em silêncio.
- Quando o usuário relatar lentidão: a duração registrada em cada operação diz onde está o tempo.
- Antes de dar uma tarefa como concluída.

## Distribuição

### Duas formas de entrega

O Cururu é entregue de duas formas, que servem a momentos diferentes:

1. **Pasta portátil** — `cururu.exe` junto com as pastas `instantclient/` e `webview2/` (WebView2 Fixed Version). Copiar a pasta para um disco local e executar deve bastar, sem instalar WebView2. Para preparar a distribuição completa e o ZIP limpo, use `ferramentas/preparar-portatil.ps1`, que compila com `--no-bundle`. Nunca envie o diretório de build inteiro.
2. **Instalador** — `Cururu_<versão>_x64-setup.exe`, gerado **somente quando o usuário pedir** e publicado no repositório de releases. Instala na pasta do usuário, sem exigir Administrador, e leva o Instant Client junto.

**Proibição de gerar instalador sem pedido:** não deve ser criado, recompilado nem atualizado qualquer instalador (`setup.exe`, NSIS, MSI ou equivalente), a menos que o usuário peça **explicitamente** por um instalador. Um pedido para "gerar o executável", "gerar um novo `.exe`" ou concluir uma modificação autoriza somente o build portátil de **`src-tauri/target/release/cururu.exe`** e **não** autoriza empacotamento. O build rotineiro deve usar a opção **`--no-bundle`**:

```bash
npm run tauri build -- --no-bundle
```

É por isso que `bundle.active` continua **`false`** em `tauri.conf.json`: assim, mesmo um `tauri build` sem `--no-bundle`, por esquecimento, não produz instalador nenhum. O empacotamento exige a configuração separada **`src-tauri/tauri.instalador.conf.json`** — que é onde moram o alvo NSIS, o idioma do instalador e os arquivos do Instant Client —, passada explicitamente na linha de comando:

```bash
npm run tauri build -- --config src-tauri/tauri.instalador.conf.json
```

Esse mesmo build gera também o `src-tauri/target/release/cururu.exe` da regra abaixo; não são dois builds.

**Nome, local e substituição do executável:** todo build final deve gerar obrigatoriamente **`src-tauri/target/release/cururu.exe`**, substituindo o arquivo existente nesse caminho. Não devem ser usados sufixos como `-novo`, `-atualizado` ou números de versão no nome do binário, pois existe um atalho permanente apontando para esse caminho exato. A pasta `dist` não é o destino do executável. A pasta **`src-tauri/target/release/instantclient/`** deve permanecer presente e atualizada.

**Build com o Cururu aberto:** antes de compilar, tente excluir os backups antigos `cururu.exe.old` e `cururu.exe.*.old` de `src-tauri/target/release/`. Preserve os arquivos que o Windows ainda mantiver em uso.

Se o Cururu estiver rodando a partir dessa pasta, renomeie `cururu.exe` para `cururu.exe.old` sem fechar o aplicativo. Se o backup já existir, use um nome único, como `cururu.exe.20261106-163000.old`. Em seguida, compile para o caminho de sempre, `src-tauri/target/release/cururu.exe`. Se o build falhar e o caminho original estiver vazio, restaure o executável que acabou de ser renomeado. Só peça para fechar o aplicativo se o Windows impedir a renomeação e isso bloquear a entrega.

### Quando incrementar a versão

**A versão é incrementada quando sai um executável que alguém vai rodar** — não a cada pedido.

Isso significa: uma sessão de trabalho com cinco ajustes seguidos é **uma** versão, gerada quando o conjunto está pronto para ser usado. A regra anterior — uma versão por pedido — produzia versões que nunca eram abertas nem publicadas, e um build *release* de quatro minutos para mudanças de uma linha.

Ao incrementar, a mesma versão precisa constar em **cinco arquivos**: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` e `src-tauri/tauri.conf.json`. O binário contém essa versão nos metadados e a exibe na tela **Sobre**.

A versão nova também recebe a sua seção em **[versoes.md](versoes.md)**, que é o conteúdo da tela **Versões** (requisito 29). Uma entrega só está concluída depois que o build termina e esses artefatos são conferidos.

### Onde o instalador é publicado

O instalador e o código-fonte ficam no mesmo repositório público, **[github.com/yurimj/Cururu](https://github.com/yurimj/Cururu)**. A publicação pelo GitHub Actions, o checksum SHA-256 e a atestação de origem estão descritos em [PUBLICACAO.md](PUBLICACAO.md).

Consequência prática: a consulta por atualização é **anônima**, sem token, e não há segredo nenhum dentro do binário distribuído.

O corpo do release é a seção daquela versão em [versoes.md](versoes.md) — a mesma lista que a tela **Versões** mostra e que a tela de atualização exibe antes de baixar. Uma origem só. Quando o release anterior estiver várias versões atrás, o corpo leva todas as seções desde ele: quem atualiza recebe todas, e precisa ver o que está recebendo.

Convenção dos releases: título `Cururu 0.1.17`, etiqueta `v0.1.17`, um único anexo `.exe` — que é o que o aviso de atualização procura.

## Idioma

**O projeto é escrito em português** — código, comentários, documentação e nomes. Só termos técnicos de uso corrente ficam no original (`view`, `commit`, `select`, `grid`, `fetch`, `script`, `log`).

**A interface é traduzível** (requisito 31): o texto que o usuário lê sai do catálogo em [`src/idioma.ts`](src/idioma.ts), em português, inglês e espanhol. Texto novo de tela entra por lá, com as três traduções — o tipo do catálogo recusa a compilação se faltar alguma.
