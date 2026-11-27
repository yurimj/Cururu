# Premissas do Projeto — Cururu

## Identificação

- **Nome do aplicativo:** Cururu
- **Plataforma:** **Windows apenas.** Não há suporte previsto a Linux ou macOS.
- **Stack:** Rust + Tauri
- **Banco alvo:** Oracle e SQL Server

## Regra de ouro

**Em caso de dúvida sobre qualquer coisa não coberta por este documento, seguir o mesmo padrão do Oracle SQL Developer.**

Isso vale para comportamento de tela, atalhos de teclado, nomenclatura, ordem e nome de menus, formato de saída, mensagens, ícones, posicionamento de painéis e qualquer outro detalhe não especificado aqui. O SQL Developer é a referência de comportamento do projeto. Só se afasta dele quando este documento disser explicitamente o contrário.

**No que for específico do SQL Server, a referência é o SSMS** — pela mesma razão: é a ferramenta que quem usa já tem aberta ao lado. Onde os dois discordam sobre algo que não é específico de banco nenhum, vale o SQL Developer, que é o formato da tela.

## Idioma

**O projeto é escrito em português**: documentação, nomes, comentários de código e o texto-fonte de tudo. Essa regra não mudou e vale para quem trabalha no Cururu.

**A interface é traduzível** (requisito 31). O texto que o usuário lê sai de um catálogo em português, inglês e espanhol; o resto do projeto continua em português e não se traduz — código, comentário e documentação.

Exceção nos três idiomas: termos técnicos de uso comum, que permanecem no original — por exemplo `view`, `commit`, `rollback`, `select`, `grid`, `fetch`, `script`, `log`. O que já estava em inglês por ser termo técnico continua em inglês em português e em espanhol.

A outra exceção é **`Language`**, a opção de menu que troca o idioma: ela aparece em inglês nos três. Não é termo técnico, é a saída de emergência — quem abriu o aplicativo num idioma que não lê precisa reconhecer a palavra para chegar até ela, e traduzi-la esconderia a porta atrás da parede. É a única linha da interface a que a regra do catálogo não se aplica.

## Foco da ferramenta

1. **Leveza = pouco consumo de memória RAM.** Este é o significado de "leve" neste projeto. **Não** se refere a espaço em disco — o tamanho do instalador e do diretório de instalação não são critério e podem crescer quando isso beneficiar o consumo de memória, a rapidez ou a facilidade de uso. Em qualquer decisão de arquitetura, trocar disco por RAM é um bom negócio; o contrário não é.
2. **Rapidez** — resposta imediata na interface; nenhuma operação de banco pode congelar a tela.
3. **Relatórios simplificados (futuro)** — gerar relatórios a partir dos resultados do grid produzido pelas consultas SQL. As decisões de arquitetura de hoje devem manter esse caminho aberto (o resultado da consulta precisa ficar acessível como dado estruturado, não apenas como pintura de tela).


## Onde está cada coisa

Este arquivo guarda **o que não muda**: a regra de ouro, o foco, o idioma e o
*porquê* das decisões técnicas — o motivo pelo qual uma alternativa foi
descartada, que é justamente o que o código nunca conta.

- **O que o produto faz**, requisito a requisito → [requisitos.md](requisitos.md)
- **Como trabalhar no projeto** — verificar, versionar, compilar, publicar → [AGENTS.md](AGENTS.md)
- **O que mudou em cada versão** → [versoes.md](versoes.md)

A separação é por ritmo de mudança. O que está aqui quase nunca precisa ser
tocado; o que sai daqui muda a cada entrega, e misturar os dois era o que fazia
este arquivo parecer sempre desatualizado.

## Decisões técnicas tomadas

### Driver Oracle e Instant Client

Não existe driver Oracle puro (*thin*) maduro em Rust. As opções reais (`oracle`/ODPI-C, `sibyl`) chamam a biblioteca OCI da Oracle, que vem no **Oracle Instant Client**. Portanto o Instant Client é obrigatório na máquina que roda o app.

**Decisão:** o **Instant Client será embutido e distribuído junto com o aplicativo**, na versão **Basic** (completa) — não a Basic Lite.

Consequências:

- O app carrega as bibliotecas **da própria pasta do aplicativo**, não do sistema. Não há dependência de `ORACLE_HOME`, `PATH` ou instalação prévia, e não há conflito com outros clientes Oracle já presentes na máquina.
- A pasta do aplicativo fica maior (na casa de 100 MB por causa do Instant Client Basic). **Isso é aceitável e não conflita com o foco em leveza**, que trata de memória RAM, não de disco.
- A versão Basic garante mensagens de erro em todos os idiomas e o conjunto completo de charsets, eliminando riscos de acentuação e de bases com charset menos comum.
- A redistribuição das bibliotecas é **permitida**, inclusive fora da empresa. A licença que acompanha o Instant Client Basic 19.31 é a **Oracle Free Use Terms and Conditions (FUTC)** — o arquivo `BASIC_LICENSE` dentro do pacote —, e ela autoriza redistribuir os Programas sem modificação, desde que não se cobre do usuário final uma taxa pelo uso do Instant Client. As condições que recaem sobre este projeto são três: não alterar as bibliotecas (o `sqlnet.ora` próprio é configuração ao lado delas, não modificação delas); **levar o `BASIC_LICENSE` junto** de toda distribuição; e não remover os avisos de propriedade da Oracle. O build já cumpre as três — o `BASIC_LICENSE` acompanha as `.dll` tanto na pasta portátil (`target/release/instantclient/`) quanto no instalador, onde o `resources` de `tauri.instalador.conf.json` copia a pasta inteira do Instant Client. Não é parecer jurídico, mas é o que diz o texto da própria licença embutida.

### WebView2 na distribuição portátil

A pasta portátil completa leva o **WebView2 Fixed Version x64** em `webview2/`,
ao lado de `cururu.exe`. Isso permite copiar e executar sem instalação prévia do
runtime. O Cururu prepara as permissões dos contêineres de renderização no destino,
inclusive no Windows 10, e preserva a compatibilidade com outro Cururu já aberto.
Sem a pasta local, continua usando o runtime instalado. Uma escolha explícita por
`WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` tem prioridade.

O runtime fixo não se atualiza sozinho: novas distribuições devem atualizar sua
versão periodicamente. Os arquivos e avisos da Microsoft acompanham o pacote sem
modificações. A execução precisa ser em disco local, pois o runtime fixo não suporta
compartilhamentos de rede. O espaço adicional em disco é aceitável pelo foco em RAM.

Para gerar o executável sem instalador e o ZIP limpo de distribuição:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ferramentas/preparar-portatil.ps1
```

O script baixa a versão oficial indicada nele, valida a assinatura Microsoft,
compila com `--no-bundle` e separa os arquivos de entrega em
`src-tauri/target/portatil/`. Nunca distribua a pasta de build inteira, que contém
artefatos de desenvolvimento. Conexões, senhas e logs ficam fora dessa entrega.

### SQL Server: versões e forma da tela

**Requisito: SQL Server 2014 em diante.**

A tela de conexões trata o tipo de banco como o campo que comanda os outros: escolhido o banco, aparecem os campos dele e somem os do outro. Três decisões dessa tela merecem registro, porque são justamente onde a alternativa parecia óbvia e era pior.

**"Nome do Servidor" é um campo só, e não host + porta.** É assim que o SSMS pede, e é a única forma em que uma instância nomeada cabe: `SRV\INSTANCIA` não se parte em dois campos sem perder a instância. O mesmo campo aceita `SRV`, `SRV\INSTANCIA` e `SRV,1433`. Quem interpreta a cadeia é o backend, na hora de conectar — a tela não adivinha.

**A credencial é a mesma dos dois lados.** "Logon" no SSMS e "Nome do Usuário" no SQL Developer são o mesmo dado, então o perfil guarda **um** par usuário/senha e só o rótulo da tela muda. Guardar um segundo par gravaria o mesmo segredo duas vezes no DPAPI, e faria quem trocasse o tipo de banco redigitar a senha sem motivo. O que se duplica é só o controle na tela, porque cada banco tem o seu layout.

**Na autenticação do Windows o usuário continua editável, e vazio quer dizer alguma coisa.** Em branco vale a identidade de quem está logado — o caminho normal do SSMS. Preenchido, vale outra credencial de domínio (`DOMINIO\usuario`). Desabilitar os campos, como o SSMS faz, fecharia o segundo caso sem ganhar nada; a dica dentro do campo diz o que o branco significa.

**Os dois formulários ficam montados o tempo todo**, e trocar de banco só alterna `hidden`. Não se recria elemento nenhum: o que está oculto não custa layout nem pintura, e a troca não perde o que já foi digitado no outro lado.

**O tipo de banco é escolha da criação, não campo editável.** Numa conexão salva ele fica desabilitado. Trocá-lo depois não seria editar um campo: seria trocar a conexão por outra, deixando para trás os dados do banco anterior — e o perfil ficaria com metade de cada um.

### Driver de SQL Server: `tiberius`

**Decisão:** o cliente TDS puro em Rust, e não o ODBC.

Motivo, na ordem em que pesou:

- **Nenhuma biblioteca nativa entra no processo.** O ODBC exigiria o `msodbcsql18` mapeado ao lado do Instant Client, e mais um instalador a distribuir. O TDS puro é código Rust no binário — cresce o disco, que não é critério, e não a memória, que é.
- **O buffer do ODBC é pré-alocado por lote.** O modelo dele é colunar: reserva `tamanho do lote × maior largura declarada da coluna` antes de a primeira linha chegar. Trinta colunas `NVARCHAR(4000)` num lote de 500 linhas dão 120 MB num buffer só. É o caminho mais curto para estourar a memória, que é exatamente o que este projeto recusa.
- **A runtime assíncrona já estava no binário.** O `tiberius` é assíncrono e exige tokio; o Tauri já traz o tokio. Não foi dependência nova, foi dependência já paga.

**A ponte entre o síncrono e o assíncrono é uma thread própria, e precisa ser.** Todo comando marcado `#[tauri::command(async)]` sobre função síncrona roda **dentro** de um worker do tokio do Tauri, e abrir uma runtime dentro de outra é pânico imediato — "Cannot start a runtime from within a runtime". Então o teste de conexão abre uma thread com uma runtime `current_thread` só sua, que nasce e morre com o clique: nenhum pool de workers fica de pé entre um teste e outro, e o custo em memória volta a zero. Há teste fixando isso, porque a falha só apareceria no clique de quem usa.

**Cifra: o login vai cifrado, o resto não, e o certificado não é validado.** É o comportamento do SSMS com "Encrypt connection" desmarcado — a senha nunca atravessa a rede em claro, mas exigir certificado válido pediria uma autoridade certificadora que instalação interna de SQL Server quase nunca tem, e o Testar falharia em toda base da casa. Se um dia a tela ganhar as opções de cifra do SSMS, é aqui que o padrão muda.

### A sessão de SQL Server mora numa thread

O driver do Oracle e o do SQL Server não têm forma comum, e tentar dar uma a eles seria pior que aceitar a diferença.

A `Connection` do OCI é `Sync`: fica atrás de um `Arc`, é chamada de qualquer thread, e é isso que permite o botão Parar interromper de fora uma consulta em andamento. O `Client` do `tiberius` é o oposto — assíncrono, exige `&mut` em toda operação e precisa de uma runtime viva por perto.

**Decisão:** cada sessão de SQL Server é uma **thread própria**, dona do cliente e de uma runtime `current_thread`, com quem se conversa por canal. Quem precisa do banco manda um pedido e espera a resposta.

Não é rodeio para contornar o driver: é a forma de que a execução de consulta precisa. O resultado do `tiberius` empresta o cliente e não sobrevive sozinho num mapa, como o `ResultSet` do Oracle sobrevive. Com o cliente preso a uma thread, o cursor vira um laço dentro dela — a aba pede mais linhas pelo mesmo canal por onde pediu as primeiras —, e o fetch parcial (requisito 10) fica de pé sem materializar o resultado inteiro na memória, que é a condição que este documento fixava para a abstração entre os dois bancos.

Custo: uma thread do sistema por sessão, e há **uma sessão por aba de consulta** (requisito 7). A pilha é *reservada*, não residente — só o que ela toca é comprometido —, e a runtime `current_thread` não tem pool de workers para manter. Entre um pedido e outro a thread fica parada no `recv`, sem runtime rodando: o TDS é pergunta e resposta, e não há nada a escutar no intervalo.

**A conexão entre os dois bancos é um `enum`, não um objeto de trait.** É o que faz o compilador cobrar de cada caminho a declaração de que banco ele atende. Todo o código de árvore, execução e metadados que existe hoje é do Oracle e passa por um acessador que só devolve conexão Oracle; com uma sessão de SQL Server ele para com a mensagem certa, em vez de fazer coisa errada em silêncio. Um trait com implementação vazia do outro lado daria o mesmo resultado em tempo de execução, mas sem ninguém avisando quem escreve o código.

### Executar SQL no SQL Server: o lote é o `GO`, não o `;`

No Oracle o `;` encerra o comando, e é assim que o modo script separa um do outro. **No T-SQL o `;` separa instruções dentro do lote**, e quem encerra o lote é a linha com `GO` — marca do cliente, que nem chega ao servidor.

**Decisão:** para SQL Server, o texto é separado só pelo `GO`. Cortar no `;` partiria ao meio todo `IF … BEGIN … END`, todo `WHILE` e toda variável declarada e usada logo abaixo — que é como o T-SQL é escrito. É também o que o SSMS faz, então um script colado de lá roda como está.

Consequência: um lote pode devolver **vários conjuntos de resultados**. O cursor fica no primeiro deles, e só nele: alcançar o segundo obriga a ler o primeiro até o fim, e aí não haveria fetch parcial nenhum.

### O bind continua sendo `:nome`, e não `@nome`

O marcador natural do T-SQL seria `@nome`. Ele **já significa outra coisa**: é a variável local declarada com `DECLARE`. Tratá-lo como bind faria todo script com `DECLARE @x int` parar para perguntar um valor que o próprio script já dá.

**Decisão:** o marcador é o mesmo dos dois lados, `:nome` (requisito 15), traduzido para `@P1`, `@P2`… antes do envio. `:nome` não é sintaxe válida em lugar nenhum do T-SQL, então não há o que confundir — e quem escreve nos dois bancos escreve do mesmo jeito.

### Largar um grid pela metade custa, e o custo tem prazo

No Oracle, abandonar um cursor é fechá-lo: o servidor recolhe o que sobrou e a conexão fica livre. **No TDS não existe esse gesto.** A conexão atende um resultado de cada vez, e o pendente só sai da frente sendo lido até o fim — o `tiberius` faz isso sozinho, em silêncio, antes do comando seguinte. Num `select` sobre tabela grande isso é a rede inteira, e a tela pareceria travada sem que o log dissesse por quê.

**Decisão:** o resto do resultado abandonado é lido **por nós**, com prazo de três segundos. Passado o prazo, a conexão é reaberta — custa um login — e o próximo resultado chega marcado como reconectado, que é o mesmo aviso do requisito 35: a sessão nova é outra sessão do banco, e o que estava sem commit ficou para trás.

O mesmo mecanismo atende o botão **Parar** (requisito 14). O `tiberius` não expõe o sinal de atenção do TDS, que é o que o SSMS manda; largar o socket faz o servidor abortar a consulta do mesmo jeito — é o que um `KILL` faria —, e a conexão volta em seguida com o banco e o auto-commit no lugar. O `KILL` de fora foi descartado por exigir `ALTER ANY CONNECTION`, permissão que a conta de quem consulta quase nunca tem.

### O auto-commit do SQL Server é estado do servidor

No Oracle o auto-commit é decisão do cliente: o driver não commita nada sozinho, e a camada de execução dá o `commit` depois de um DML quando o interruptor está ligado. **No SQL Server ele é `SET IMPLICIT_TRANSACTIONS`**, que vale na sessão do servidor.

Isso traz uma diferença que não dá para esconder: com transações implícitas ligadas, **todo** comando abre transação — `select` inclusive. Duas consequências foram decididas assim:

- **A sessão da conexão abre sempre com auto-commit ligado**, qualquer que seja o do perfil. É ela que atende a árvore e o autocomplete; com o interruptor desligado ela ficaria com uma transação aberta a vida inteira, segurando lock e log por consulta nenhuma do usuário. Quem respeita o interruptor é a sessão da aba (requisito 7).
- **Uma conexão nova de SQL Server nasce com auto-commit ligado**, ao contrário da de Oracle. Cada banco nasce com o padrão da ferramenta de referência dele — o SQL Developer abre com ele desligado, o SSMS com ele ligado. Não é gosto: com ele desligado, o `USE` da lista de bancos da barra passa a ser recusado depois de qualquer comando, porque o SQL Server não aceita `USE` dentro de transação.

Ligar o auto-commit com transação aberta **confirma o que estava pendente**, como fazem os drivers padrão. Deixá-la aberta faria o interruptor mentir: a tela diria "auto-commit" e nada seria confirmado, nem o que veio antes nem o que viesse depois, porque a transação aberta engole os comandos seguintes.

### Quantas linhas o comando afetou vem numa segunda pergunta

O servidor manda a contagem de linhas na marca de fim de comando; o `tiberius` **descarta essas marcas** ao devolver o fluxo de linhas, e as expõe só na API que descarta as linhas. Não dá para ter as duas de uma vez.

**Decisão:** quando o lote não devolve conjunto de resultados nenhum — ou seja, é comando e não consulta —, a contagem vem de um `select @@rowcount` logo em seguida. O `@@ROWCOUNT` é da sessão, e no lote seguinte ainda é o da última instrução executada. Custa uma ida e volta, e só nos lotes que não trouxeram linhas.

### O autocomplete pergunta à conexão, com o banco no nome

No Oracle o autocomplete usa a sessão da conexão para poder compartilhar o cache entre as abas. No SQL Server isso deixa de ser economia e vira necessidade: a conexão da aba pode estar com uma consulta correndo ou com um grid pela metade, e no TDS ela atende um pedido de cada vez — a tecla digitada esperaria a consulta terminar, ou pior, derrubaria o cursor do grid.

Só que a aba pode estar em outro banco (requisito 3). **Decisão:** as consultas de catálogo levam o banco no nome — `[Vendas].sys.objects` —, e o cache é indexado por sessão **mais** banco. Um `use` na sessão da conexão resolveria a leitura e estragaria a árvore, que está justamente olhando o banco de onde ela veio.

O mesmo vale para o nome de três partes que o usuário escreve: `BANCO_EXEMPLO..tbl` sugere as tabelas **daquele** banco, e não as do banco da aba. **Decisão:** a primeira parte só é tratada como banco quando consta da lista de bancos do servidor, lida uma vez por aba e guardada em memória. A alternativa — mandar a primeira parte ao servidor e ver no que dá — transformaria cada `x.y` digitado numa consulta a `[x].sys.objects`, que não existe e responde com **erro**, não com lista vazia. O esquema, esse, vai como parâmetro e entra na chave do cache: a lista de `dbo.` não é a lista de `rh.`, e servir uma no lugar da outra é a forma mais rápida de o autocomplete deixar de ser confiável.

### SQL Server Agent: as procedures, e não as tabelas do `msdb`

O Agent guarda tudo em tabelas do `msdb` — `sysjobs`, `sysjobsteps`, `sysjobschedules`. Ler e escrever nelas direto é tentador e funciona hoje.

**Decisão:** ler pelas *views* que o próprio Agent oferece e escrever **só** pelas procedures dele (`sp_add_job`, `sp_add_jobstep`, `sp_add_jobschedule`, `sp_add_jobserver`), que é o caminho que o SSMS usa.

Duas razões. A primeira é permissão: `msdb.dbo.sysjobs_view` aplica a regra de visibilidade do Agent — quem é `sysadmin` vê todos os trabalhos, os demais veem os seus. Ler a tabela mostraria a lista inteira a quem não deveria vê-la, ou seria recusada. A segunda é que as tabelas do `msdb` não são contrato: as procedures são a interface suportada, e um `insert` direto é o que quebra na próxima versão do servidor — sem falar nos identificadores e nas linhas de apoio que a procedure cria e que ninguém lembra de replicar.

**A tela de agenda fala a língua do `sp_add_jobschedule`.** "Toda segunda às 3h" chega ao backend como `freq_type = 8, freq_interval = 2, freq_subday_type = 1`, e não como um vocabulário próprio traduzido depois. É onde o SSMS também faz a tradução, e inventar um formato intermediário só acrescentaria mais uma conversão para errar. O preço é que a tela precisa conhecer os números — e é por isso que ela mostra o **Resumo** por extenso: `32, 16, 9` é "no último dia útil do mês", e ninguém confere isso olhando.

**A criação é um lote só, e o `set xact_abort on` sozinho não basta.** O trabalho, cada etapa, cada agenda e o `sp_add_jobserver` valem juntos ou não valem: um trabalho que ficou sem etapa, ou que não chegou a ser atribuído ao Agent local, aparece na lista e nunca roda. Uma chamada por procedure deixaria esse meio-termo possível.

A armadilha está em achar que a transação mais o `xact_abort` resolvem. **Não resolvem**: as procedures do `msdb` recusam entrada com `raiserror` seguido de `return 1`, e erro levantado por `raiserror` não interrompe o lote nem derruba a transação — o `xact_abort` age sobre erro de *execução*. Foi assim que uma etapa com `@on_success_action` inválido chegou a produzir, num teste contra um servidor de verdade, um trabalho gravado **com zero etapas**: a procedure reclamou, o lote seguiu para o `sp_add_jobserver` e o `commit` aconteceu. Por isso cada procedure do lote é seguida de `if (@@error <> 0 or @retorno <> 0) goto desfazer` — que é exatamente o que o script gerado pelo SSMS faz, e agora se sabe por quê.

**Na edição, etapas e agendas são trocadas por inteiro.** O SSMS concilia: descobre o que mudou em cada etapa e manda só isso. Aqui elas são apagadas (`sp_delete_jobstep` com `@step_id = 0`, que apaga todas de uma vez) e reinseridas na ordem da tela; as agendas saem por `sp_detach_schedule` num laço, com `@delete_unused_schedule = 1`.

Conciliar exigiria identificar cada etapa entre a lista da tela e a do servidor, e a tela **não tem esse identificador** — ela deixa reordenar, renomear e excluir livremente, e depois disso não há como saber se "Expurgar" na posição 2 é a mesma etapa que estava na posição 1 com outro nome. Trocar tudo dá o mesmo resultado observável com uma fração do código que pode errar.

O que se perde é o `schedule_id` da agenda. Como o `sp_detach_schedule` só apaga a agenda que **nenhum outro trabalho** usa, uma agenda compartilhada continua de pé para os outros; o que some é o compartilhamento com este trabalho — que a tela também não sabe montar, então não havia o que preservar.

**Parâmetro de procedure é constante ou variável — nunca expressão.** O trabalho é identificado por um `uniqueidentifier`, o driver manda o parâmetro como texto, e converter na própria chamada — `exec msdb.dbo.sp_start_job @job_id = cast(@P1 as uniqueidentifier)` — é recusado pelo servidor no parse, com `Incorrect syntax near '@P1'`. O lote inteiro morre ali. Em `select` o mesmo `cast` é legítimo, porque é expressão de `where`: foi por isso que a leitura do Agent sempre funcionou enquanto Iniciar, Parar, Renomear, Excluir e Habilitar não faziam nada havia meses, cada tentativa registrada no log e nenhuma visível na tela. Toda chamada de procedure do Agent declara a variável antes (`declare @trabalho uniqueidentifier = cast(@P1 as uniqueidentifier)`), que é o que o lote de criação já fazia desde o início — e por isso ele nunca sofreu do mesmo mal.

**Acompanhar a execução pede as duas fontes.** O que o trabalho está fazendo agora só o processo do Agent sabe, e `sp_help_job` é quem pergunta a ele: a tabela `sysjobactivity` só registra a última etapa **concluída**, que durante a etapa 2 ainda diz 1. Como terminou, ao contrário, só o histórico guarda — `sysjobhistory` com `step_id = 0`, a linha de resumo, que traz o resultado e a mensagem que o SSMS mostra. A tela de acompanhamento (requisito 36) lê as duas a cada segundo; sem permissão para a primeira, cai no `msdb` e perde só o nome da etapa em curso.

**A situação do serviço tem três estados, não dois.** `sys.dm_server_services` responde direto, mas exige `VIEW SERVER STATE` e não existe antes do SQL Server 2008 R2 SP1; a sessão `SQLAgent - Generic Refresher` em `sysprocesses` é o sinal indireto que o SSMS usa há décadas, e **prova** que o serviço está no ar — mas a ausência dela não prova o contrário, porque sem `VIEW SERVER STATE` a conta só enxerga as próprias sessões. Daí o terceiro estado, *desconhecido*. Dizer "parado" a quem apenas não tem permissão de olhar mandaria o usuário procurar defeito num serviço que está funcionando.

### Versões do Oracle suportadas

**Requisito: Oracle Database 12c (12.1) em diante.**

**Decisão:** usar o **Instant Client 19c**, não o 21c.

Motivo: a matriz de interoperabilidade da Oracle cobre, para cada versão de client, uma faixa de versões de servidor. O client 21c atende bases **12.1 e superiores** — atenderia o requisito, mas para na borda exata dele. O client **19c** atende **11.2 e superiores**, cobrindo o requisito com margem e sem nenhuma perda para bases 12c+. Como o 19c é a versão de suporte estendido da Oracle, é a escolha mais conservadora.

Consequência prática: se aparecer alguma base 11.2 (comum em instalações antigas de Oracle E-Business Suite), o aplicativo continua conectando. Com o client 21c, não conectaria.

**Cuidado associado:** bases que ainda guardam verificadores de senha antigos podem recusar clients novos com `ORA-28040 — Nenhum protocolo de autenticação correspondente`. Como o Instant Client vai embutido no aplicativo, o **`sqlnet.ora` também é nosso**: ele será distribuído junto, com `SQLNET.ALLOWED_LOGON_VERSION_CLIENT` ajustado para aceitar esses casos, sem depender de configuração na máquina do usuário.

## Notas de implementação

Decisões tomadas durante a construção que valem registro:

- **Auto-commit é do Cururu, não do driver.** O driver só aceita trocar o auto-commit com acesso exclusivo à conexão, e a conexão precisa ficar compartilhada para o botão Parar poder interromper uma consulta em andamento. Então o auto-commit fica desligado no driver e a camada de execução dá `commit` explícito depois de um DML quando o interruptor da sessão está ligado. O efeito para quem usa é o mesmo.
- **Uma sessão por aba de consulta, não uma por conexão.** Cada aba abre a própria sessão no banco — reaproveitando as credenciais já resolvidas da conexão, sem repedir a senha —, porque o Oracle serializa as chamadas de uma mesma sessão: com sessão única, uma consulta pesada numa aba travava as outras abas e o autocomplete até terminar. O preço é que a transação passa a ser **da aba** (commit, rollback e auto-commit valem só para ela) e que a senha resolvida fica em memória enquanto a conexão está aberta, para abrir as sessões irmãs. Fechar a aba encerra a sessão dela, e sair do aplicativo encerra todas: o logoff limpo libera os locks na hora, em vez de deixá-los pendurados até o servidor perceber a conexão morta. Um fechamento **forçado** (crash, Gerenciador de Tarefas) não passa por esse encerramento — aí os locks de um DML sem commit ficam com o servidor até ele reciclar a sessão morta, o que pode depender do Dead Connection Detection do banco.
- **A sessão se reabre sozinha, mas só repete leitura.** Uma VPN que cai e volta deixa a conexão do driver morta, e o socket velho não avisa nada: o erro só aparece no comando seguinte. A sessão passou a guardar as credenciais já resolvidas e a trocar a própria conexão por uma nova quando reconhece um erro de queda (`ORA-03113`, `ORA-03114`, a família `ORA-125xx`, `DPI-1080` e companhia). O que **não** se faz é repetir sozinho um comando que altera dados: quando a conexão morre no meio, não dá para saber se o comando chegou a valer no banco antes de morrer, e um `insert` repetido por conta própria grava duas vezes. Então consulta e leitura de metadados reconectam **e repetem**; DML, DDL, commit e rollback reconectam e devolvem o erro marcado, para a tela dizer que basta executar de novo. Depois de uma reconexão que falha, há uma espera curta antes da tentativa seguinte — sem ela, o autocomplete, que pergunta a cada tecla, enfileiraria um login por tecla enquanto a rede está fora.
- **Exportação em dois formatos**: CSV com separador `;` e marca de ordem de bytes (é o que o Excel em português espera) e planilha `.xlsx`. A primeira escolha para o Excel foi o XML dele de 2003 (SpreadsheetML), que é arquivo de texto e por isso não pedia biblioteca nenhuma — mas ia com extensão `.xls`, e o Excel, que desde 2007 confere o conteúdo contra a extensão, avisava a cada abertura que o formato e a extensão não correspondiam. O `.xlsx` é um zip com XMLs dentro e é montado em [`src-tauri/src/xlsx.rs`](src-tauri/src/xlsx.rs): o zip são três cabeçalhos escritos à mão, e a compressão sai do `flate2`, que já vinha na árvore de dependências pelo `png` do Tauri. Continua valendo que nada novo entrou no projeto por causa da exportação.
- **A importação de conexões nunca traz a senha.** O SQL Developer guarda a dele cifrada com chave própria, e o `tnsnames.ora` não tem senha.
- **Grid virtualizado**: só as linhas visíveis existem no DOM. É o que mantém o consumo de memória baixo com resultados grandes.
- **Ordenação do grid vale sobre as linhas já trazidas**, não sobre a consulta inteira. Trazer mais linhas mantém a ordenação escolhida.
- **Uma porta só para cada preferência.** Teclas de Atalho, Language e Tema saíram do menu principal e viraram seções da tela de Configurações (requisito 41). Duas entradas para a mesma escolha são duas telas para manter em dia, e a marcação do "está valendo" tinha de ser reescrita em cada uma. O que se perdeu foi a troca em um clique; o que se ganhou foi a mesma regra de gravação para todas — antes, idioma e tema valiam no clique do submenu e os atalhos tinham botão de salvar, e a diferença não se explicava por nada além de como cada um foi construído.
- **Esc é tecla reservada, e não um atalho configurável.** Fecha a janela extra que estiver por cima, sempre, descartando o que estava sendo editado (requisito 42). Por isso os dois campos que capturam combinação de teclas — o da tela de Configurações e o do editor de botões rápidos — deixam o Esc passar em vez de gravá-lo: um atalho que só se aciona fechando a janela não serviria para nada.
- **O filtro tem duas escritas, e não uma parametrizada.** `condicao` sai em SQL do Oracle com bind nomeado; `condicao_tsql` sai em T-SQL com parâmetro posicional. Um método só, com um sinalizador de dialeto, esconderia que as diferenças não são de sintaxe: o colchete é curinga num e não no outro, a caixa depende do collation num e do valor no outro, e a situação existe num e não no outro. Cada uma dessas diferenças é uma decisão, e decisão dentro de um `if` de dialeto é decisão que ninguém revisa.
- **O `dbo.` é regra de exibição, e nunca de identidade.** O que a árvore mostra passa por `nomeVisivel`; o que vai ao servidor continua qualificado. Tirar o esquema do nome de verdade pareceria funcionar — `[Vendas]..cliente` resolve pelo esquema padrão do login, que quase sempre é `dbo` — e falharia exatamente para quem tem esquema padrão diferente: a árvore mostraria uma tabela e abriria outra.
- **Language está no menu e nas Configurações, e isso não é descuido.** A regra da porta única vale para tudo, menos para a porta de saída de quem não lê o idioma da tela: "Configurações" só se acha depois de entender a palavra, e ela está justamente no idioma que a pessoa não entende. Language é a única linha que não se traduz, e por isso é a única que pode ficar nos dois lugares.
- **O instalador conta ao Cururu o idioma escolhido, pelo registro.** O NSIS já pré-seleciona o idioma da máquina; o que faltava era o aplicativo saber o que a pessoa escolheu ali. O gancho `NSIS_HOOK_POSTINSTALL` grava em `HKCU\Software\Cururu\Idioma`, e a janela lê esse valor **antes de montar a tela** — depois seria tarde, e a barra de título piscaria em português antes de virar inglês. HKCU, e não HKLM, porque a instalação é por usuário e HKLM exigiria Administrador.
- **A opção de tabelas do autocomplete é lida na ponte, não em cada tela.** São três lugares pedindo sugestão — a aba de consulta, o DDL do painel de objeto e a etapa do Agent —, e passar a escolha como argumento em cada um deles seria criar três chances de um esquecer. `api.sugerirObjetos` lê a preferência e a envia junto; quem chama não precisa saber que ela existe.
- **A opção entra na chave do cache de sugestões.** O cache guarda o resultado de cada prefixo por sessão, e "todas" e "apenas as filtradas" respondem coisas diferentes para o mesmo prefixo. Sem separar as duas, trocar de opção seria atendido pelo resultado guardado da outra até a sessão morrer. O separador é o mesmo `|` que `invalidar` já usava para achar as chaves derivadas de uma sessão.
- **O recorte das tabelas vai no SQL, e não sobre a lista devolvida.** A consulta tem teto de 300 nomes: filtrar em memória filtraria o que sobrou do corte, e a tabela procurada poderia ter ficado de fora dele por causa de outras que a opção descartaria. O predicado é `AND (object_type <> 'TABLE' OR (…))` porque a mesma consulta traz oito tipos de objeto numa lista só — o filtro é o da categoria Tabelas, e um pacote não é uma tabela.
- **O índice local só responde pelo que se chama.** Até a 0.1.72 ele guardava também as tabelas do filtro e respondia sozinho no lugar de tabela, o que parecia a mesma coisa e não era: ali a lista precisa de view, de sinônimo e do que é de outro dono, e nada disso cabe no índice — os outros donos são o `ALL_OBJECTS` inteiro. A resposta saía na hora e pela metade, e uma lista pela metade não se distingue de "não existe". No lugar de tabela a consulta por prefixo custa perto de um segundo no primeiro prefixo; os seguintes saem do cache, derivados do menor que veio inteiro. Pelo mesmo motivo a lista de rotinas do índice tem **chave própria** no cache: guardada junto das outras, e marcada completa como vem, ela respondia pelo `FROM` do mesmo prefixo.
- **A escuta do Esc fica na janela, não no fundo do diálogo.** Cada tela costumava prender um `keydown` no próprio `.dialogo-fundo`, o que parece equivalente e não é: o `keydown` só sobe até ali enquanto o foco está dentro. Um clique numa linha da grade, que não é focável, devolve o foco ao `body` — e a partir daí o Esc não fechava mais nada. `fecharComEsc`, em `componentes/janela.ts`, escuta a janela inteira, confere se a tela é a última da pilha de `.dialogo-fundo` e devolve como desligar a escuta ao fechar. Hoje **toda** janela extra passa por ele; a versão presa ao fundo não existe mais em lugar nenhum.
- **O arquivo aberto pelo Windows vai para a janela que já está aberta, e isso não é trava de instância única.** O plugin oficial de instância única do Tauri resolveria a entrega, mas faz de **toda** segunda execução uma entrega: abrir o Cururu pelo atalho com ele já aberto passaria a só trazer a janela velha para a frente, e quem usa duas janelas lado a lado perderia isso sem ter pedido nada. A entrega é feita com o que o Windows oferece — um mutex nomeado que diz "há um Cururu vivo" e uma janela só de mensagens que recebe os caminhos por `WM_COPYDATA` — e só acontece quando há arquivo. O mutex está ali por um caso só: vários `.sql` selecionados e abertos com Enter iniciam vários processos no mesmo instante, e sem ele cada um abriria a própria janela antes de o primeiro ter onde receber.
- **A janela nova entra na versão do WebView2 da janela aberta.** Todo Cururu guarda a página na mesma pasta de dados do WebView2 — é ali que vivem as preferências, os atalhos e os botões rápidos —, e uma pasta dessas serve a um navegador por vez. Com o WebView2 atualizado pelo Windows e um Cururu aberto na versão velha, o novo carregava a nova e ficava esperando um navegador que nunca respondia: sem janela, sem erro, sem linha no log. Uma pasta de dados por janela resolveria a espera e partiria as preferências em duas. Então a janela nova procura o navegador que serve a pasta — pela janela de mensagens `Chrome_MessageWindow` que o próprio Chromium cria com a pasta por título, que é como ele mesmo descobre o dono — e, sendo de outra versão, usa a pasta daquela versão pela variável `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`, o jeito documentado de escolher o runtime. A variável sai do ambiente assim que a janela nasce: o instalador de uma atualização, aberto depois, reabriria o Cururu apontando para uma versão que o Windows apaga quando ninguém mais a usa.
- **O arrasto das abas usa eventos de ponteiro, e não o arrastar e soltar do HTML.** No Windows o Tauri tira o WebView2 do papel de destino de arrasto da janela e se registra no lugar dele, para receber os arquivos soltos ali — e o destino dele só reconhece arquivo. O arrasto de um elemento da própria página passa pelo mesmo mecanismo do sistema, e o `dragover` e o `drop` nunca chegam à página. A saída documentada, desligar o `dragDropEnabled`, devolveria ao WebView2 também o arquivo solto na janela, e um navegador abre no lugar da página o arquivo que ela não trata. O ponteiro com captura não passa pelo arrasto do sistema; é o mesmo mecanismo do divisor e das bordas das janelas. A captura fica na barra, e não na aba, porque a aba troca de lugar no meio do arrasto, e o elemento tirado do documento perde a captura.
- **O sublinhado do Ctrl pergunta ao próprio Oracle, e não ao dicionário.** A primeira versão (0.1.76) perguntava em lote às visões `ALL_SYNONYMS`, `ALL_OBJECTS` e `ALL_PROCEDURES` — seis consultas por rodada, em vez das três a cinco por nome do `resolver_referencia` —, e mesmo assim cada rodada levou de 8 a 42 s no log, com um nome só levando 25 s: neste banco o custo está nas visões, e não na quantidade de nomes. A 0.1.78 manda os nomes num bloco PL/SQL que os resolve com `DBMS_UTILITY.NAME_RESOLVE` — a resolução de nomes do próprio banco, com sinônimo privado e público — e confere o membro de pacote com `DBMS_DESCRIBE`, porque o `NAME_RESOLVE` devolve `PKG.NAO_EXISTE` como se existisse. Medido no HML com 53 nomes reais: 132 ms contra 23 s, com a mesma resposta nome a nome. Tabela e view saem do tipo que o contexto 0 devolve, sem `SELECT ... WHERE 1 = 0`: o parse daria a mesma resposta, mas abriria o link de um sinônimo remoto e recompilaria a view inválida só por olhá-la. As consultas ao dicionário continuam como reserva, para o banco que recusar o bloco. Na 0.1.79 o **Ctrl+clique** passou pelo mesmo caminho: pelo dicionário, um clique numa tabela levou 21 s no PRD, e três cliques seguidos, sem nada na tela, 21, 34 e 38 s; pelo Oracle, 28 nomes conferidos no HML deram o mesmo destino que o dicionário — categoria, nome e corpo ou especificação —, a maioria em 6 a 8 ms. Só o "o pacote tem corpo?" continua no dicionário, porque o `NAME_RESOLVE` não distingue, e só para membro de pacote. A pergunta roda numa conexão à parte porque a de metadados atende um pedido por vez, e a reserva seguraria o autocomplete pelos mesmos segundos; a da aba também não serve, porque é nela que o Ctrl+Enter executa. Ela é uma só por conexão, em vez de uma por pergunta, porque cada login custa perto de 350 ms; e nasce **com a primeira aba de consulta**, com o bloco já executado uma vez, e não no primeiro Ctrl — no PRD, o login e a primeira execução do bloco somaram um segundo inteiro, e eram quase toda a espera do primeiro sublinhado. O preço é uma sessão a mais por conexão Oracle, aberta mesmo para quem nunca segura o Ctrl.
- **No SQL Server, quem resolve o nome do Ctrl+clique é o `OBJECT_ID`.** Não há catálogo a vasculhar: a função do servidor aceita o nome como a consulta o escreve, aplica o esquema padrão do login e devolve o identificador do objeto, que o `sys.objects` daquele banco traduz em tipo e grafia. O sinônimo é seguido uma vez, pelo `base_object_name`. O sublinhado usa a mesma resolução, uma consulta por banco citado — e por banco porque o `object_id` só vale dentro do banco que o resolveu: cruzar o número de um com o catálogo de outro acharia o objeto errado.
- **O filtro dos trabalhos do Agent divide a chave com o Programador do Oracle.** São a mesma ideia — o job de cada banco —, a tela do filtro já chama os dois de `Job`, e uma conexão é de um banco só, então uma chave nova só serviria para duplicar o conceito no arquivo de filtros e em todo `match` de categoria.
- **O sublinhado espera o Ctrl ficar sozinho; a pergunta, não.** O Ctrl é a tecla mais apertada do editor, e quase nunca para navegar. Sem os 0,2 s de espera, todo `Ctrl+C` e todo `Ctrl+Enter` piscariam o sublinhado. Mexer o mouse com o Ctrl apertado dispensa a espera, porque ali a intenção é clara. A pergunta ao banco já sai quando o Ctrl desce: desde que o Oracle resolve os nomes ela custa décimos de segundo, só vai pelo que a aba ainda não sabe, e perguntar só depois da espera somava os 0,2 s a toda resposta. O indicador de carregamento tem a sua própria folga, de 0,12 s depois de o sublinhado ser pedido, para a resposta rápida não piscar nada; e é uma classe posta pelo `editorAttributes` do CodeMirror, e não direto no elemento, porque o CodeMirror reescreve o `class` do editor a cada troca de foco.
- **O executável não se registra no "Abrir com" sozinho; quem registra é o instalador.** Registrar a cada abertura parece mais cômodo para a pasta portátil, e é justamente ali que falha: numa máquina rodam cópias de vários lugares — a pasta de build, cópias de teste, versões antigas guardadas —, e a última a abrir tomaria a associação para si, às vezes apontando para uma cópia que some no dia seguinte. O instalador sabe onde o Cururu mora; a cópia solta, não.

## Fora do escopo do MVP

Itens deixados para depois, quando o projeto amadurecer:

- Explain Plan
- Edição de dados diretamente no grid
- Comparação de objetos
- **Atualização silenciosa**, que se instala sozinha sem passar pelo usuário. O aviso de versão nova existe (requisito 30), mas baixar e instalar continua sendo escolha de quem usa.

## Decisões técnicas em aberto

**Execução de SQL no SQL Server.** Conectar, árvore de bancos e a aba de consulta já existem; falta o que roda dentro dela — grid, cursores, transação, autocomplete.

O risco de memória **não está no driver**: está em materializar o resultado. O `ResultSet<'static, Row>` do Oracle é dono do próprio statement, e é por isso que ele pode ficar guardado num mapa entre um bloco de fetch e outro; o `QueryStream` do `tiberius` **empresta** o cliente e não é `'static`. O caminho de menor resistência, quando o borrow checker reclamar, é devolver `Vec<Linha>` — e aí um `select *` numa tabela grande sai de 500 linhas na memória para a tabela inteira.

A metade difícil já está resolvida: a sessão mora numa thread que possui o cliente, e o cursor tem onde viver. O que falta é fazer o bloco de linhas atravessar o canal em vez do resultado inteiro. **Manter o fetch parcial (requisito 10) é condição, não detalhe de implementação.**

Ponto a validar durante a implementação: o **charset das bases** de trabalho. O Instant Client Basic cobre todos os charsets, então não há risco de acentuação; resta apenas confirmar os valores de `NLS_LANG` adotados como padrão pelo aplicativo.
