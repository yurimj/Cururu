# Versões do Cururu

Registro do que mudou em cada versão. Este arquivo **é** o conteúdo da tela
**Versões**, aberta pelo menu do painel de conexões: o aplicativo lê o texto
daqui, então não existe uma segunda lista para manter em dia.

Cada versão nova — a regra de quando incrementar está em [AGENTS.md](AGENTS.md)
— acrescenta a sua seção aqui, no topo, com o que mudou.

Formato: `## versão — data`, seguido dos itens em lista. É o que a tela sabe
ler; parágrafos como este ficam de fora dela.

As versões anteriores à 0.1.4 foram reconstruídas a partir do histórico do
repositório, porque a tela de Versões só passou a existir na 0.1.4.

## 0.1.105 — 13/11/2026

- **Instalador compilado pelo GitHub Actions.** A publicação inclui checksum SHA-256 e atestação de origem vinculada aos fontes e à execução do build.
- **Atualizações no repositório público do Cururu.** O aplicativo passa a procurar novas versões nas Releases do mesmo repositório que contém o código-fonte.

## 0.1.104 — 12/11/2026

- **Entrega consolidada das melhorias recentes.** Inclui conexões MongoDB, indicador de abertura, acesso à senha salva e alertas de consulta com envio ao Teams, execução imediata e logs por regra.

## 0.1.103 — 11/11/2026

- **Execute um alerta imediatamente.** O menu do botão direito passa a oferecer **Executar**, **Logs** e **Propriedades**, em ordem alfabética. A execução usa o fluxo de avisos e envio ao Teams existente; alertas pausados mantêm a opção desabilitada.

## 0.1.102 — 10/11/2026

- **Logs de execução por alerta.** Clique com o botão direito numa regra e abra **Logs** para conferir as últimas verificações, falhas e tentativas de envio ao Teams.

## 0.1.101 — 10/11/2026

- **As propriedades do alerta fecham ao salvar.** A árvore mostra a regra atualizada após a gravação.

## 0.1.100 — 10/11/2026

- **O resultado da consulta vira a mensagem do alerta.** A janelinha e o Teams exibem o texto da primeira linha. Use **Repetir após** para voltar a avisar mesmo quando o texto continuar igual.
- **Novo alerta fecha ao salvar.** A regra aparece na árvore assim que a gravação é concluída.

## 0.1.99 — 10/11/2026

- **Regras agrupadas na árvore de Alertas.** Expanda Alertas para ver as regras da conexão e dê duplo clique numa delas para editar. A URL salva do Teams aparece no cadastro para ser copiada e reutilizada.

## 0.1.98 — 10/11/2026

- **Alertas de consulta no Oracle e no SQL Server.** Cadastre um SELECT e o intervalo no nó Alertas da conexão. Se o resultado passar de 1, o Cururu mostra um aviso e pode enviar ao chat pessoal do Teams por um fluxo do Power Automate.
- **Cole a URI completa do MongoDB Compass.** O Cururu preenche usuário e senha automaticamente e mantém o endereço, o banco e as opções no campo da URI.

## 0.1.97 — 10/11/2026

- **Aviso de carregamento nas aberturas demoradas.** Ao abrir uma procedure, package, arquivo SQL ou outra tela que leve mais de um segundo para abrir, o Cururu mostra o nome do item em carregamento. O aviso some quando a abertura termina ou falha.

## 0.1.96 — 09/11/2026

- **Conexões MongoDB para consultas de leitura.** Cadastre uma URI, navegue pelos bancos e coleções e filtre os documentos com Extended JSON. A aba mostra documentos expansíveis e sugere campos encontrados em uma amostra carregada em segundo plano.

## 0.1.95 — 09/11/2026

- **Mostre ou copie a senha de uma conexão Oracle ou SQL Server.** O editor da conexão pode revelar a senha já salva no Windows, copiar o valor mesmo com o campo mascarado e receber uma nova senha colada. O armazenamento continua protegido pelo DPAPI.

## 0.1.94 — 09/11/2026

- **O autocomplete do SQL Server usa uma cópia local do catálogo do banco da aba.** Nomes de tabelas e colunas podem aparecer enquanto o servidor ainda está respondendo. O Cururu atualiza a cópia em uma conexão à parte, ao conectar ou trocar de banco.
- **A janela de acompanhamento do SQL Server Agent respeita o tema do Cururu.**

## 0.1.93 — 06/11/2026

- **A execução de um trabalho do SQL Server Agent abre em uma janela própria.** Arraste pela barra de título para acompanhar o status em outro monitor enquanto usa a janela principal. Fechar só encerra o acompanhamento; Parar solicita a interrupção do trabalho.

## 0.1.92 — 07/10/2026

- **A exportação para Excel passa a gerar um `.xlsx` de verdade.** O que saía era o XML do Excel de 2003 (SpreadsheetML) gravado como `consulta.xls`, e o Excel avisava a cada abertura que *o formato de arquivo e a extensão de 'consulta.xls' não correspondem*, com o alerta de arquivo possivelmente corrompido e a pergunta se era para abrir mesmo assim. A conferência é dele desde 2007: `.xls` promete o formato binário antigo, que aquele XML não era. O arquivo abria depois do aviso; agora abre direto, com o nome `consulta.xlsx` e o ícone de planilha.
- **Código com zero à esquerda deixa de virar número na planilha.** Um CEP `01310` chegava à célula como 1310, e um número com mais de quinze dígitos — um `NUMBER` grande do Oracle — chegava arredondado, porque é até aí que o Excel guarda um número sem mexer nele. Os dois passam a ir como texto, igual ao que o grid mostra. O que é número de verdade continua número, para somar e ordenar na planilha, e o cabeçalho continua em negrito.

## 0.1.91 — 05/10/2026

- **Abrir Recente no menu principal.** Lista os últimos 15 arquivos abertos com sucesso, do mais recente para o mais antigo, incluindo os recebidos pelo Windows. O histórico permanece ao fechar o Cururu, não repete caminhos e move um arquivo reaberto para o topo. Sem histórico, a opção fica desabilitada; sem conexão aberta, é possível consultar a lista, mas abrir um arquivo exige uma conexão para receber a aba.
- **O autocomplete do Oracle parou de repetir o nome da tabela.** Uma tabela que existisse no esquema do usuário e também em outro aparecia quatro vezes na lista, em vez de duas: o índice local e o dicionário respondem ao mesmo prefixo de propósito — um abre a lista em milissegundos, o outro confirma meio segundo depois —, e as duas listas chegavam inteiras à tela. Os homônimos de donos diferentes continuam listados separadamente, que é o que o dono ao lado do nome existe para distinguir.

## 0.1.90 — 05/10/2026

- **Scripts Oracle com `SET DEFINE ON/OFF` passam a executar.** A diretiva é tratada pelo Cururu, em vez de chegar ao Oracle e interromper o script com `ORA-00922`. Com a substituição ligada, o diálogo pede os valores de `&nome` e `&&nome` antes da execução, inclusive quando aparecem dentro de literais; com `OFF`, o texto é preservado. `DEFINE nome = valor` e `UNDEFINE nome` também são reconhecidos, com definições válidas para a execução atual.
- **O diálogo distingue substituições textuais de bind variables.** Variáveis de substituição aparecem com `&`, binds continuam com `:`, e a explicação informa como cada valor será usado. Um script fictício integra os testes: seus seis blocos PL/SQL são preservados, incluindo a procedure local e os pseudorregistros do trigger, junto da variável de substituição e da diretiva de rollback.

## 0.1.89 — 01/10/2026

- **Iniciar, Parar, Renomear, Excluir, Habilitar e Desabilitar um trabalho do Agent passam a funcionar.** Nenhum deles chegava ao servidor: o comando era montado como `exec msdb.dbo.sp_start_job @job_id = cast(@P1 as uniqueidentifier)`, e numa chamada de procedure o T-SQL aceita constante ou variável como valor de parâmetro, nunca uma expressão — o servidor recusava o lote inteiro no parse, com `Incorrect syntax near '@P1'`. Clicar em **Iniciar Trabalho na Etapa…** não disparava nada; o log registrou a recusa em 25/09, em 30/09 e de novo em 01/10. As telas de Novo Trabalho e Propriedades nunca sofreram disso, porque já declaravam a variável antes de chamar a procedure — é o que todas fazem agora.
- **Iniciar um trabalho do SQL Server Agent passa a mostrar o que está acontecendo.** O `sp_start_job` agenda a execução e volta na hora, e era só isso que o Cururu tinha a dizer: nenhum andamento, nenhum fim — ao contrário do SSMS, que abre uma tela de progresso. Agora abre a tela **Execução do Trabalho**, que pergunta ao Agent de segundo em segundo e mostra a situação, a etapa em curso — `2 (Carregar dados do dia)`, como o Agent a numera —, uma barra de andamento e o tempo decorrido. No fim, o veredito: *Concluído com êxito* ou *O trabalho falhou*, com a duração medida pelo servidor e a mensagem que o Agent gravou no histórico, que é a mesma do SSMS e a única que diz **por que** um trabalho falhou. Fechar a tela não para o trabalho — ele é do Agent e segue rodando —, e o botão **Parar** continua à mão para quem quiser interromper.
- **Disparar um trabalho que já estava rodando deixa de responder com a recusa seca do servidor.** Em vez de *refused because the job is already running*, a tela acompanha a execução que está em curso e avisa que é dela que se trata.
- **A barra de status e a árvore passam a saber o fim.** Terminada a execução, a barra repete o veredito e a lista de trabalhos é relida, porque a última execução que a dica do mouse mostra acabou de mudar.
- **Duração acima de uma hora deixa de ser contada em minutos.** `187 min` virou `3 h 7 min`, no tempo de um trabalho e no rodapé da consulta, que usam a mesma formatação.

## 0.1.88 — 30/09/2026

- **A lista de bancos deixa de esperar a consulta ao catálogo.** Ela já está na memória da aba e responde na hora, mas saía no mesmo `Promise.all` da busca de objetos e esperava por ela. Em 30/09 uma consulta que costuma levar 15 ms levou 4 s — a primeira depois de horas com a conexão parada — e escondeu por quatro segundos justamente os bancos que estavam sendo digitados, em `BANCO_EXEMPLO..`. Agora é fonte própria: a consulta lenta não apaga mais a lista que a aba tem pronta.
- **A lista de objetos que veio inteira responde aos prefixos maiores sem sair da aba.** O cache do lado Rust já sabia derivar `tbl_812` de uma busca de `tbl` que tenha vindo completa, mas para chegar até ele cada tecla passava pela fila de uma consulta por vez — e a tecla digitada enquanto uma busca lenta está no ar era descartada. Na mesma consulta de 30/09, os 82 nomes que voltaram depois de 3,3 s já continham a resposta de todas as teclas do meio. A aba passa a fazer essa conta antes da fila. Lista que bateu no teto de 300 continua sem servir de base, pelo mesmo motivo que tirou o `validFor` daqui: ela veio cortada.

## 0.1.87 — 29/09/2026

- **Tabelas temporárias do SQL Server passam a sugerir coluna.** Num script com `select * into #t from tbl_501_nota_stat` em cima e `join #t t` embaixo, o `t.` não sugeria nada: o nome com `#` na frente não era reconhecido, e a referência inteira — tabela e apelido — era descartada. Agora `#t` e `##g` são lidos como tabela, e as colunas delas saem da definição escrita no próprio script: `create table #t (...)` traz nome e tipo, e `select ... into #t from origem` traz os nomes da lista e os tipos da origem. A procura é no script inteiro, porque a definição quase sempre está num comando acima.
- **`FROM #` e `JOIN #` oferecem as temporárias do script**, sem consultar o banco. Tabela temporária mora em `tempdb`, com escopo da sessão da aba, e o catálogo que responde ao autocomplete é o da conexão: perguntar por um nome com `#` era uma ida vazia ao banco a cada tecla. Antes, o editor ainda lia o apelido perdido como nome de esquema e disparava uma segunda consulta inútil.
- **`SELECT ... INTO destino FROM origem` deixa de apelidar o destino de `from`.** A lista de tabelas de uma cláusula não terminava no `FROM` seguinte, e a palavra virava o apelido da tabela de destino.

## 0.1.86 — 29/09/2026

- **O ponto depois do apelido de um `UPDATE` volta a sugerir as colunas.** No `UPDATE` do T-SQL o apelido é escrito onde uma tabela estaria — `update t501 ... from tbl_501_nota_stat t501` —, e o editor o lia como nome de tabela. O `t501.` ia então pedir as colunas de uma tabela `t501`, que não existe, e não sugeria nada. Agora o apelido é procurado antes do nome de tabela, e `UPDATE tabela SET` continua resolvendo pelo nome.

## 0.1.85 — 23/09/2026

- **Autocomplete do SQL Server respeita nomes independentemente de maiúsculas e minúsculas.** A busca no catálogo usa comparação insensível a caixa mesmo quando o banco usa outra regra; um resultado vazio para `TBL` não pode ocultar `tbl_503_nota_serv` nas próximas teclas.

## 0.1.84 — 22/09/2026

- **Triggers Oracle com `:NEW.campo` e `:OLD.campo` executam sem pedir valores.** Esses pseudorregistros pertencem ao trigger e são preenchidos pelo Oracle quando ele dispara; o Cururu deixa de tratá-los como bind variables do script.

## 0.1.83 — 22/09/2026

- **Cada conjunto de resultados passa a ter altura regulável.** Com vários conjuntos empilhados — o caso de uma procedure de depuração —, a faixa embaixo de cada um arrasta a altura dele para cima ou para baixo; o duplo clique abre o conjunto e devolve ao tamanho padrão, e as setas do teclado andam de linha em linha. Nada é tirado do conjunto vizinho: a pilha cresce e rola.
- **Conjuntos largos deixam de aparecer com a última linha cortada.** A altura de cada conjunto passa a reservar o espaço da barra de rolagem horizontal, que antes comia os últimos pixels — num conjunto de uma linha só e muitas colunas, era a única linha que ficava pela metade.

## 0.1.82 — 22/09/2026

- **Tabelas, views e sinônimos do Oracle passam a aparecer pelo índice local no autocomplete.** Os nomes guardados são oferecidos sem esperar consultas em andamento; o Oracle complementa o mesmo menu em segundo plano. O índice considera os filtros de tabelas, distingue os schemas e é atualizado ao conectar. A primeira utilização prepara o acervo; as seguintes aproveitam o JSON já salvo, separado do índice de pacotes.
- **Nomes qualificados por schema também completam depois de FROM e JOIN**, como `EXEMPLO.DEM_CAD_`, sem confundir o ponto com uma chamada de package.
- **Tabelas, Views e Procedures filtradas abrem pelo índice local**, com a mesma resposta imediata de Pacotes. Os filtros, a situação válida ou inválida, a exclusão de objetos gerados, os sinônimos, a paginação e os comentários das views são aplicados sem consultar o dicionário a cada expansão. **Atualizar** no menu da categoria força uma nova leitura do Oracle.
- **Scripts do SQL Developer com `SET SERVEROUTPUT ON|OFF` e `WHENEVER SQLERROR EXIT ... ROLLBACK` passam a rodar diretamente.** A Saída DBMS é ligada ou desligada na própria sessão; no primeiro erro, o script para e desfaz a transação quando pediu `ROLLBACK`. As diretivas deixam de ser enviadas ao Oracle como SQL inválido, sem deslocar a linha indicada por erros posteriores.

## 0.1.81 — 21/09/2026

- **Pacotes do Oracle abrem pelo índice local**, sem consultar o dicionário a cada listagem. Os nomes já guardados nas versões anteriores também são aproveitados; a atualização em segundo plano acrescenta a situação dos pacotes. Filtros e paginação são aplicados ao índice, e **Atualizar** no menu da categoria relê os pacotes do banco.

## 0.1.80 — 17/09/2026

- **O Ctrl+clique e o Ir para definição passam a valer no SQL Server.** Sobre uma tabela ou view, abre a **Estrutura**; sobre procedure ou função, o fonte. Quem resolve o nome é o próprio servidor, do jeito que a consulta o escreve — `cliente`, `dbo.cliente` ou `Vendas.dbo.cliente` —, seguindo sinônimo; um nome de três partes abre o objeto no banco dele. O sublinhado com o Ctrl apertado veio junto, com as palavras reservadas do T-SQL.
- **Os trabalhos do SQL Server Agent podem ser filtrados**, como as categorias da árvore: pelo botão direito no nó, ou pelo botão de filtro da barra com ele selecionado. Com filtro ativo o nó vira **SQL Server Agent (Filtrado)**, e a dica do mouse diz o critério ao lado do estado do serviço. O filtro fica guardado com a conexão, como os outros.

## 0.1.79 — 17/09/2026

- **O Ctrl+clique e o Ir para definição abrem na hora.** Achar o destino chegava a levar dezenas de segundos — um clique numa tabela levou 21 s no PRD —, e agora quem o acha é o próprio Oracle, em milissegundos, com o mesmo resultado de antes.
- **Enquanto o destino é procurado, o editor mostra que está trabalhando**, e clicar de novo nesse meio-tempo não faz nada. Antes, sem sinal nenhum na tela, cada clique repetido virava mais uma procura na fila, e a estrutura da tabela esperava atrás de todas.
- **Com o Ctrl apertado, o editor mostra que está buscando os nomes.** Enquanto a resposta do banco não chega, uma linha fina corre no topo do editor, na cor de destaque, e o cursor vira o de "trabalhando em segundo plano" sobre o texto. Os nomes já sublinhados continuam com a mãozinha. Resposta rápida não acende indicador nenhum.
- **O primeiro sublinhado chega mais cedo.** A conexão que responde a ele passa a abrir junto com a aba de consulta, já pronta, e a pergunta sai no instante em que o Ctrl desce, sem esperar o instante que o sublinhado espera para aparecer. No PRD, o primeiro Ctrl levava um segundo inteiro só com o login e a preparação.

## 0.1.78 — 17/09/2026

- **O sublinhado do Ctrl aparece na hora.** A pergunta ao banco sobre quais nomes o Ctrl+clique abre levava de 8 a 42 segundos — tempo em que o Ctrl já tinha sido solto e o sublinhado não chegava a aparecer. Agora quem resolve os nomes é o próprio Oracle, e não as visões do dicionário: 53 nomes levaram 0,1 s no HML, contra 23 s antes, com a mesma resposta nome a nome.
- Nenhum link de banco é aberto para isso: o nome que leva a outro banco continua sem sublinhado, como no Ctrl+clique. Se o banco recusar a resolução nova, o Cururu volta ao caminho antigo sozinho e registra o motivo no log.

## 0.1.77 — 16/09/2026

- **Script com `CREATE OR REPLACE EDITIONABLE PROCEDURE` roda inteiro, como no SQL Developer.** Com o `EDITIONABLE` — que o SQL Developer escreve em todo fonte que exporta — o Cururu não reconhecia o bloco e cortava o corpo no primeiro `;`: mandava o cabeçalho sozinho ao banco, que o gravava **inválido por cima da procedure boa**, e parava no pedaço seguinte com `ORA-00900: invalid SQL statement`. Valia também para `NONEDITIONABLE` e para o `CREATE AND COMPILE JAVA`. Agora o comando só termina na barra.
- **`SHOW ERRORS` é aceito.** Sozinho, mostra os erros do último objeto compilado na aba; com tipo e nome (`SHOW ERRORS PROCEDURE DONO.NOME`), os daquele objeto, no formato do SQL*Plus. Antes ele ia ao Oracle e voltava `ORA-00900`. Escrito sem `;`, termina na quebra de linha e não leva junto o comando de baixo.
- **`CREATE` que compila com erro não sai mais como executado.** O Oracle aceita o comando e só avisa, e o Cururu dizia *"Comando executado"*. Agora a mensagem diz que o objeto ficou inválido, em vermelho, e cada erro vai para o painel de mensagens com a linha do editor — clicável, como no botão Compilar.
- **Script que para no meio mostra o que já rodou.** Antes aparecia só o erro, sem sinal de que os comandos anteriores tinham valido — nem o ✔ de um `UPDATE` que ficou pendente. O cursor vai para o comando que falhou, e o log diz em qual deles o script parou.
- **O salto até a linha do erro conta a partir de onde a seleção começa**, e não do topo do editor. Vale para o erro de execução, nos dois bancos, e para os erros de compilação, que também passaram a descontar o comentário de cabeçalho antes do `CREATE`.
- O log de cada execução diz em que conexão ela rodou.

## 0.1.76 — 16/09/2026

- **Com o Ctrl apertado, os nomes que o Ctrl+clique abre ficam sublinhados**, na cor de link, e o ponteiro vira a mãozinha sobre eles. Só esses: palavra reservada, coluna, apelido, texto entre aspas, comentário e o que o banco não reconhece ficam como estão. Vale para tabela, view, procedure e função do schema, sinônimo que aponte para um deles, `DONO.OBJETO` e membro de package — as mesmas regras do Ctrl+clique e do **Ir para definição**.
- O sublinhado aparece com o Ctrl apertado sozinho por um instante, ou já no primeiro movimento do mouse. `Ctrl+C`, `Ctrl+V` e `Ctrl+Enter` não piscam a tela. Soltar o Ctrl apaga.
- A primeira vez de cada nome passa pelo banco, numa conexão à parte: o autocomplete e o Ctrl+Enter não esperam por ela. A resposta fica guardada na aba, e o Ctrl seguinte é imediato. Só no Oracle, como o próprio Ctrl+clique.

## 0.1.75 — 16/09/2026

- **As abas mudam de lugar arrastando.** Com cinco abas abertas, a quarta vai para a primeira posição arrastada até lá: ela segue o ponteiro e troca de lugar com cada vizinha que ultrapassa. Com mais abas do que cabem na barra, a aba que encosta na borda faz a barra rolar sozinha até as que estavam escondidas.
- **Ctrl+PageUp e Ctrl+PageDown seguem a ordem nova**, e fechar a aba visível mostra a primeira da barra como ela está na tela.
- O clique na aba continua só selecionando: o arrasto começa depois de o ponteiro andar alguns pixels. Apertar o **✕** e escorregar não arrasta a aba.

## 0.1.74 — 15/09/2026

- **Um segundo Cururu voltou a abrir depois de o Windows atualizar o WebView2.** Com um Cururu aberto desde antes da atualização, o novo ficava vivo sem nunca mostrar a janela — e cada clique no atalho deixava mais um processo invisível para trás, sem erro e sem nada no log. As duas janelas dividem a pasta de dados do WebView2, que só uma versão usa por vez; a nova agora entra na versão da que já está aberta. O log passa a dizer qual WebView2 cada janela usa e em quanto tempo ela ficou pronta.
- **Uma execução que dá erro tira da tela o grid da execução anterior.** O editor toma o espaço e o erro fica sozinho no painel de mensagens, em vez de disputar o olho com um resultado que já não correspondia ao que está no editor. O próximo resultado traz o grid de volta, na divisão em que ele estava. Lote que devolveu grids antes de falhar continua mostrando esses grids, que são daquela execução.
- **O segundo erro seguido não fecha mais o painel de mensagens.** O painel alternava a cada erro: o primeiro o abria, o seguinte o fechava — e a mensagem sumia justamente quando havia o que ler. Erro, compilação, aviso de reconexão e a Saída DBMS ligada agora só abrem o painel; fechar é com o clique na aba.
- **No Oracle, DDL não pede mais commit.** `ALTER TABLE`, `CREATE`, `DROP`, `TRUNCATE`, `GRANT` e os demais DDL saíam com *"transação aberta, falta commit"* e acendiam o ✔ — mas o Oracle confirma DDL sozinho, antes e depois de rodar, e leva junto o que estava pendente. A mensagem agora diz *"DDL: o Oracle confirma sozinho"*, e o ✔ apaga. `ALTER SESSION` não acende mais o ✔, e `COMMIT` e `ROLLBACK` digitados no editor valem o mesmo que os botões.
- **No SQL Server, quem diz se sobrou transação é o servidor.** O DDL continua pedindo commit, porque lá ele é transacional — um `alter procedure` sem commit se perde ao fechar a aba. Mas o Cururu passou a perguntar o `@@TRANCOUNT` depois de cada comando: um `commit` digitado apaga o ✔, e um `begin tran` o acende mesmo com o auto-commit ligado.
- **Quantos objetos a árvore lista por vez virou opção:** **Objetos listados por vez**, na seção Geral das Configurações, de 10 a 5000. O padrão passou de 100 para **200**. Vale para todas as categorias, no Oracle e no SQL Server, a partir da próxima página listada.

## 0.1.73 — 14/09/2026

- **As views voltaram ao autocomplete do `FROM`** com a opção Tabelas em **Apenas as do filtro da conexão**. `SELECT * FROM VW_REL_` não sugeria nada: nessa opção quem respondia era o índice local, que guardava só rotinas e as tabelas do filtro — view, sinônimo e objeto de outro dono ficavam de fora, sem aviso. Depois de `FROM`, `JOIN`, `UPDATE` e `DELETE` a resposta volta a vir da consulta ao banco, com o filtro aplicado dentro do SQL.
- **Uma palavra digitada fora do `FROM` não esvazia mais a lista dele.** A lista de rotinas que o índice devolve nos outros lugares ficava guardada como se fosse a resposta inteira daquele prefixo, e depois de um `SELECT V` o `FROM V…` saía dela sem tabela nem view. Valia nas duas opções de Tabelas.
- **O índice local passou a guardar só o que se chama** — pacotes, procedures, funções e os subprogramas dos pacotes —, que é o que continua abrindo na hora em `EXECUTE ` e em `PACOTE.`. No `FROM`, o primeiro prefixo volta a esperar a consulta ao banco, perto de um segundo pelo log; os seguintes saem do cache.

## 0.1.72 — 11/09/2026

- **Um `.sql` aberto pelo Windows abre no Cururu**: duplo clique no Explorer, **Abrir com** ou o arquivo arrastado até o `cururu.exe`. Antes o caminho chegava e era ignorado — o Cururu abria vazio.
- **Com o Cururu já aberto, o arquivo vai para uma aba da janela que está na tela**, e ela vem para a frente, em vez de abrir um segundo Cururu. Vários arquivos selecionados e abertos juntos caem todos na mesma janela. Abrir o Cururu sem arquivo continua abrindo outra janela, como sempre.
- **Sem conexão aberta, o arquivo espera**: a barra de status pede uma conexão, e ele abre na primeira que for selecionada — ao conectar, no lugar da aba vazia.
- **O instalador passa a pôr o Cururu no "Abrir com" do `.sql`**, sem se fazer programa padrão: torná-lo o padrão é escolha de quem usa, em **Abrir com → Escolher outro aplicativo → Cururu → Sempre**. Na pasta portátil o caminho é o mesmo, apontando o `cururu.exe` em **Procurar mais aplicativos neste PC**.

## 0.1.71 — 09/09/2026

- **O texto que tinha escapado da tradução foi para o catálogo.** Em inglês ou espanhol, a árvore ainda dizia *carregando…* ao abrir um nó e *(vazio)* quando não havia nada dentro; a dica do nó filtrado saía inteira em português — *"Filtro ativo: (NOME like X) E NÃO (…)"* —, e o visualizador de tabela era português do começo ao fim: as abas **Dados** e **Estrutura**, os botões, *Carregando dados…*, o rodapé com "linha(s)" e "coluna(s)", e as colunas **Nome da coluna**, **Tipo** e **Nulo?** com **Sim** e **Não**.
- **O nome da tecla nos atalhos passou a acompanhar o idioma:** `Ctrl+Shift+Seta acima` aparece como `Ctrl+Shift+Up arrow` em inglês e `Ctrl+Shift+Flecha arriba` em espanhol. O que se **guarda** continua em português, de propósito: assim o atalho já gravado, e o backup feito num idioma, continuam valendo nos outros dois.
- **Também foram traduzidos:** a espécie do objeto na lista do auto-complete (*tabela*, *sinônimo*, *sequência*…), o alvo na tela de filtro (**Package**, **Table**, **Job**), a pergunta dos bind variables, os cabeçalhos e o aviso de Importar conexão, o tipo de etapa **Sistema operacional (CmdExec)** do SQL Server Agent, o release sem descrição na tela de atualização, o nome dos formatos no diálogo de arquivo do Windows (**Todos os arquivos**, **Arquivo CSV**, **Planilha do Excel**), a confirmação de carregar backup e a moldura *"Falha interna do aplicativo: …"* que embrulha erro inesperado.
- **O botão Excluir do editor de botão rápido voltou a ter nome.** Ele mostrava, literalmente, `${t("excluir")}`: o trecho estava dentro de aspas simples, e o texto não chegava a ser substituído.

## 0.1.70 — 09/09/2026

- **O que foi executado sem commit passou a se anunciar.** Com o auto-commit desligado, a mensagem de um comando termina agora em *"— transação aberta, falta commit"*, e o botão **✔** da barra fica aceso na cor de aviso enquanto houver o que confirmar. Antes, a única pista era a **ausência** do "(commit automático)" — dizer o contrário por omissão.
- **É o SQL Server que cobrava isso.** Lá o DDL também é transacional: um `alter procedure` executado com sucesso, sem commit, desaparece inteiro quando a aba fecha — e fechar não pergunta nada (requisito 12). Quem vem do Oracle, onde o DDL confirma sozinho, não tem por que desconfiar. O aviso vale nos dois bancos, porque um `update` sem commit se perde igual.
- O ✔ só acende quando há o que perder: consulta não acende, lote que só trouxe grid não acende, e ligar o auto-commit apaga — é o que o servidor faz, confirmando o pendente na troca.

## 0.1.69 — 05/09/2026

- **A interface passou a estar realmente traduzida.** Telas inteiras ainda saíam em português com o idioma em inglês ou espanhol: Recuperar Consultas, o filtro de objetos, a tela de atualização, Versões, Sobre, Importar conexão, bind variables, o editor de botão rápido e as dicas do grid, das abas e do divisor. Todo esse texto foi para o catálogo, com as três traduções.
- **Language voltou ao menu principal**, sem sair das Configurações. É a única escolha em dois lugares, e de propósito: quem abriu o Cururu num idioma que não lê não sabe o que quer dizer "Configurações", "Settings" nem "Configuración" — a porta de saída não pode estar atrás de uma palavra que é preciso entender para achar.
- **O instalador pergunta o idioma**, com a lista já pré-selecionada no idioma da máquina, e o Cururu abre no que foi escolhido ali. Português e espanhol viram português e espanhol; qualquer outro idioma vira inglês.
- **Sem instalador, o idioma da primeira abertura passou a cair em inglês** quando a máquina não está em português nem em espanhol. Antes caía em português, o que entregava a um alemão ou a um francês uma tela que ele não lê.
- **A opção Tabelas do Auto-Complete passou a valer no SQL Server.** Com "apenas as do filtro da conexão", o autocomplete de tabelas obedece ao filtro da categoria Tabelas lá também; view, procedure, função e sinônimo continuam inteiros. O texto da tela, que dizia que a opção não tinha efeito no SQL Server, foi corrigido.

## 0.1.68 — 05/09/2026

- **O `dbo.` sumiu da tela.** Tabelas, views, procedures e funções do SQL Server aparecem sem o esquema padrão na árvore, no título da aba e nas mensagens do rodapé. Esquema diferente de `dbo` continua aparecendo — é ele que distingue `vendas.pedido` de `compras.pedido`. O que vai ao servidor continua qualificado, para um login com esquema padrão diferente não abrir uma tabela e ver outra.
- **O filtro de objetos passou a valer no SQL Server.** As cinco categorias — Tabelas, Views, Stored Procedures e as duas famílias de Functions — aceitam os mesmos critérios de inclusão e exclusão do Oracle, pelo botão direito no nó ou pelo botão de filtro da barra. O filtro é da conexão, e vale para a mesma categoria em todos os bancos daquele servidor.
- O critério é o mesmo; o SQL é outro. No T-SQL o colchete é curinga e passa a ser escapado, a comparação sem distinção de caixa sobe os dois lados para maiúsculas (lá o nome guardado não é maiúsculo como no Oracle) e a distinção de caixa, quando marcada, sai em collation binário — sem isso um banco *case-insensitive* ignoraria o pedido.
- A opção **Excluir objetos gerados pelo banco** não aparece no filtro do SQL Server: `is_ms_shipped = 0` já vale sempre na listagem, e uma caixa que não muda nada é pior do que caixa nenhuma.
- **A espera por uma conexão de SQL Server caiu de 15 para 5 segundos.** O servidor que responde, responde em menos de um segundo; os outros catorze só faziam esperar por uma resposta que não vinha.

## 0.1.67 — 05/09/2026

- **A seção Auto-Complete das Configurações ganhou a opção Tabelas**, com duas escolhas: **Todas** (o padrão) e **Apenas as do filtro da conexão** — o mesmo filtro da categoria Tabelas que recorta a árvore de objetos. Cada uma traz a explicação do que faz ao lado, porque o que separa as duas não é o rótulo, é a consequência.
- **O padrão passou a ser Todas, e isso conserta um efeito colateral do índice local.** Desde a 0.1.64, uma conexão com filtro de Tabelas configurado tinha o autocomplete silenciosamente recortado por ele: o índice só guarda as tabelas do filtro, e era ele quem respondia. Agora o recorte é uma escolha, e não uma consequência de outra configuração.
- **O recorte é aplicado no SQL**, e não sobre a lista já devolvida — filtrar depois filtraria o que sobrou do teto de 300 nomes, e a tabela procurada poderia ter ficado fora do corte por causa de outras que a opção descartaria.
- **Trocar o filtro de Tabelas descarta na hora as tabelas guardadas no índice**, mantendo rotinas e subprogramas. Antes, o autocomplete continuava respondendo pelo filtro anterior até a próxima conexão.
- A opção vale só para tabelas: pacotes, procedures, funções, views e sequências continuam sendo sugeridos por inteiro. Sem filtro configurado as duas escolhas dão no mesmo, e no SQL Server — onde a árvore ainda não filtra — a opção não tem efeito.

## 0.1.66 — 05/09/2026

- **Tela de Configurações**, no menu principal, com as seções **Geral** (Language e Tema), **Teclas de Atalho** e **Auto-Complete** — esta última ainda vazia. As três entradas que ocupavam o menu solto — Teclas de Atalho, Language e Tema — saíram de lá: agora existe uma porta só para cada preferência.
- **O que se mexe nas Configurações é rascunho até o Salvar**, inclusive idioma e tema, que antes valiam no clique do submenu. Cancelar, ✕ e Esc saem sem gravar nada.
- **Esc fecha qualquer janela que se abra por cima do Cururu**, e fecha sem salvar e sem perguntar. Antes valia só em algumas telas, e mesmo nelas só enquanto o foco estivesse dentro da janela: bastava clicar numa linha da grade para o Esc parar de responder. A escuta passou para a janela inteira, e as dezesseis telas do aplicativo usam o mesmo caminho.
- Com telas empilhadas, cada Esc fecha uma — a de cima. As únicas esperas são a operação em andamento que não pode ser abandonada no meio (o download da atualização, a gravação de um trabalho do Agent) e o Esc que o editor SQL já usou para fechar o autocomplete ou o Localizar.
- **Esc deixou de ser uma combinação gravável** nos campos de captura de tecla, na tela de Configurações e no editor de botões rápidos: ali ele fecha a janela, como em todo o resto do aplicativo.

## 0.1.65 — 04/09/2026

- **O índice do autocomplete ficou restrito ao que precisa ser imediato:** packages, procedures, functions e os subprogramas das packages. Types, views, sequências e sinônimos continuam fora dele.
- **Tabelas só são indexadas quando existe filtro de Tabelas**, e somente as que passam por esse filtro. Sem filtro, o autocomplete de tabelas conserva a consulta por prefixo que já existia.
- **O JSON anterior responde imediatamente enquanto uma conexão separada confere novidades em background.** Ao terminar, ela substitui o retrato e invalida o cache antigo, permitindo descobrir packages, procedures, functions e membros novos sem colocar a espera na digitação.
- Sugestões atendidas pelo índice não esperam mais a consulta de objetos de outros schemas. O formato do arquivo passou para a versão 3 e o índice amplo das versões anteriores é descartado automaticamente.

## 0.1.64 — 04/09/2026

- **`EXECUTE ` abre a lista de pacotes sem nada digitado.** As rotinas — pacotes, procedures, funções e tipos — passam a ser indexadas **inteiras, sem filtro**: são poucas, e são o que se chama. A lista vem do disco, então não custa consulta nenhuma e aparece mesmo com prefixo vazio. Vale para `EXEC` e `CALL` também.
- **O filtro de Tabelas da árvore manda no que é indexado.** Num ERP as tabelas são a esmagadora maioria dos nomes — 19.698 dos 20.000 primeiros no schema medido —, e é o filtro que o usuário já configurou que diz quais interessam. Sem filtro, entram todas as que couberem no teto.
- **Cada grupo é inteiro ou não existe.** Antes, o índice inteiro caía junto quando os dados estouravam o teto; agora as rotinas continuam valendo mesmo num schema em que as tabelas não cabem — que é exatamente o caso do EXEMPLO PRD.
- Fora de lugar de tabela o autocomplete passa a pedir **só rotinas**, que é o que cabe ali: é essa distinção que deixa o índice responder por um grupo sem depender do outro.
- O formato do arquivo de índice mudou, então o gravado pelas versões anteriores é descartado sozinho — inclusive o 1,5 MB de nomes cortados que a 0.1.61 deixou. Na primeira abertura o acervo é lido de novo, e o `DEM_INT` volta a ser guardado no primeiro `DEM_INT.` que você digitar.


## 0.1.63 — 04/09/2026

- **`execute dem` volta a sugerir `DEM_INT` — e nunca foi maiúscula contra minúscula.** A lista de objetos vem com teto de 300 nomes, e o resultado era marcado como filtrável na tela: o editor parava de consultar e ia peneirando a lista cortada a cada tecla nova. Digitar o `p` depois de `sa` nunca chegava a perguntar por `dem`.
- **Por que some justamente o `DEM_INT`:** `SA` tem mais de 300 objetos nesse schema, o corte é alfabético, e `_` ordena **depois** das letras — o pacote fica atrás de todo `DEMZ…`, fora dos 300 primeiros. Já `DEM` traz 173 nomes, lista inteira, com ele dentro. O que decidia era o acaso da digitação: quando a rajada de teclas fazia a consulta cair em `dem`, aparecia; quando caía em `sa`, sumia — e as duas tentativas do relato caíram uma de cada lado.
- Medido com o teto simulado: com a marca, só `sa` era consultado e a lista saía sem o pacote; sem a marca, `sa` e depois `dem`, com o pacote. As listas de coluna, subprograma e banco continuam marcadas — essas vêm completas, e filtrá-las na tela é sempre certo.


## 0.1.62 — 04/09/2026

- **Correção do índice da 0.1.61: `execute dem` parou de sugerir `DEM_INT`.** O schema tem mais de vinte mil objetos, a leitura do índice trouxe os primeiros vinte mil em ordem alfabética, e o autocomplete passou a servir essa lista cortada como se fosse o schema inteiro — mudo de `SA` em diante. Agora **o índice de objetos é inteiro ou não existe**: passar do teto descarta o índice, e a conexão volta a consultar por prefixo como sempre fez. Arquivo gravado pela 0.1.61 é descartado sozinho, sem precisar apagar nada.
- **A leitura do índice saiu da conexão que a tela usa.** Ela levou 29 s naquele schema, e a conexão de metadados atende um pedido por vez: no log da 0.1.61 dá para ver uma sugestão esperando 25 s atrás dela. Agora ela abre uma conexão só para isso e a fecha ao terminar.
- O índice de **subprogramas** não muda: é ele que faz `execute dem_int.` abrir na hora, e continua valendo entre aberturas do aplicativo.


## 0.1.61 — 04/09/2026

- **Índice local dos metadados (requisito 40): a primeira leitura do dia deixou de ser cara.** O que o autocomplete lê do dicionário passa a ficar guardado em `%APPDATA%/Cururu/indice/`, um arquivo por conexão. O cache em memória já resolvia a segunda chamada de `EXECUTE DEM_INT.`; o índice resolve a primeira, porque a resposta ficou gravada da vez passada — inclusive depois de fechar e abrir o aplicativo.
- **Os objetos do usuário são lidos de uma vez, logo depois de conectar, sem ninguém esperando.** Com o índice montado, o prefixo digitado é filtrado em memória e só a busca em outros donos vai ao banco. É a mesma leitura que a versão antiga fazia na primeira tecla e que travava o autocomplete em schema grande: o que mudou é o momento, e o teto de 20.000 objetos.
- **A lista vazia também fica guardada**, e é ela que mais rende: o editor manda conferir se é pacote todo nome com ponto atrás, e cada um desses custava uma leitura do dicionário.
- O índice é **sugestão, e não verdade**: alimenta só o autocomplete, e executar, abrir fonte e o grid continuam perguntando ao banco. Sai pelo mesmo caminho do cache em memória.
- **Terceira tentativa na consulta do id do pacote.** O `LIKE` sem curinga da 0.1.60 não mudou nada — 14,4 s contra 14,8 s. A única forma que este dicionário responde rápido, medida no log, é a do autocomplete: prefixo seguido de `%`, que faz varredura de faixa pelo índice de nome. A consulta passou a pedir o prefixo e escolher o nome exato em memória, sobre um punhado de linhas. Com o índice local, essa leitura acontece **uma vez por instalação**, e não a cada abertura.


## 0.1.60 — 04/09/2026

- **A leitura do id do pacote deixou de custar quinze segundos.** O log da 0.1.59 pôs os dois lados na mesma visão do dicionário: `object_name = :nome` em `user_objects` levou 14,8 s, e `object_name LIKE 'DEM%'`, que é o formato do autocomplete, levou 1,2 s trazendo 173 nomes. Não é o filtro, é a forma: com `LIKE` e `FETCH FIRST` o otimizador escolhe o plano de primeiras linhas, que usa o índice de nome. A consulta do id passou a ser escrita assim — com o nome inteiro, sem curinga, e o sublinhado escapado, que é igualdade escrita de outro jeito.
- Não é o maiúsculo/minúsculo. `EXECUTE DEM_INT.` e `execute dem_int.` seguem o mesmo caminho, e o log mostra a leitura acontecendo com o nome como foi digitado: quem responde instantâneo é a **segunda** chamada em diante, guardada por sessão desde a 0.1.56. A primeira do dia é que pagava a conta.


## 0.1.59 — 03/09/2026

- **Correção do freio novo da 0.1.58: nome nenhum era sugerido depois da primeira letra digitada.** Escrever `select` fazia o `s` ser consultado, e o dicionário devolve vazio sem consultar nada para prefixo de uma letra só (é regra dele, para o prefixo curto não casar com meio schema). O freio tomava aquele vazio por "não existe" e riscava a letra: dali em diante, na mesma aba, nenhum nome começado com S aparecia — nem `SALDOS`, nem `SAIDAS`. Agora só vazio de prefixo que o banco realmente olhou conta, e prefixo de uma letra nem sai da aba.


## 0.1.58 — 03/09/2026

- **O autocomplete parou de disparar uma consulta por tecla, e é isso que fazia a lista demorar.** A conexão de metadados atende um pedido por vez; digitar `EXECUTE DEM_INT.` enfileirava catorze consultas — `E`, `EX`, `EXE`, `EXEC`… —, cada uma de um a três segundos, e o pedido do ponto era o último da fila. O log mostrava o sintoma sem disfarce: quatro consultas terminando no mesmo milissegundo, todas com 13 a 15 segundos de duração. Nenhuma era lenta; todas esperavam.
- **São três freios**: prefixo repetido reaproveita a busca que já está no ar; prefixo que voltou vazio dispensa os maiores — se `EXE` não achou nada, `EXEC` também não acha, e isso corta toda palavra reservada digitada; e passa a haver **uma consulta de objeto por vez** na aba, com a que ficou velha sendo abandonada sem ir ao banco. Medido com as catorze teclas de `EXECUTE DEM_INT`: catorze idas ao banco antes, duas depois.
- Trocar o banco na barra (SQL Server) passa a esquecer também os objetos e os subprogramas lidos, e não só as colunas: eram do banco anterior.


## 0.1.57 — 03/09/2026

- **`EXECUTE DEM_INT.` abre a lista na hora.** A 0.1.56 tinha acelerado a metade errada: o log dela, que passou a medir cada etapa em separado, mostrou `USER_ARGUMENTS` em 28 ms e `USER_PROCEDURES` em 15,6 s. As duas visões custam caro filtradas por **nome** e são baratas filtradas por **`object_id`** — elas juntam `obj$`, `procedureinfo$` e `argument$` com lógica de edição no meio, e o nome não chega até o índice. Agora o `object_id` sai antes de tudo de `USER_OBJECTS`, que é leitura de índice, e as duas consultas passam a usá-lo.
- **Quem ganha mais é o nome que não é pacote.** O editor manda conferir todo nome com ponto atrás, e antes cada `STATUS.` custava uma varredura de `USER_PROCEDURES` — 1,5 s medidos. Agora para na primeira leitura, que é de índice.
- Se nada vier pelo `object_id`, a leitura por nome é refeita como rede de segurança: lenta, mas a lista aparece, e o log diz que foi por ali.


## 0.1.56 — 03/09/2026

- **Depois de um operador o autocomplete passa a oferecer as colunas do comando, e não só as rotinas.** `UPDATE DEM_CLIENTES SET ID_REGISTRO = '2' || IDR` não sugeria `ID_REGISTRO`: à direita de `=`, `>` ou `||` a lista era só de função e package. Agora traz as duas coisas, com as colunas na frente no desempate — são as do comando que está na tela, e as rotinas são o dicionário inteiro. **Vale para o `+` do SQL Server**, que ali faz o papel do `||`. `WHERE dt_inc > SYSDATE - 2` continua achando a função, que era o motivo de a lista ser só de rotina.
- **Correção: no tema escuro, metade das linhas não parecia escolhida.** Marcar a linha inteira pintava umas e não outras — as de fundo mais claro ficavam iguais —, e a marca só aparecia enquanto o mouse estava por cima. Era conta de CSS: a regra da linha par pesa mais que a da linha escolhida e a apagava; só o `:hover` da escolhida empatava com ela. Vale para os dois temas, mas no claro os dois fundos são tão próximos que passava despercebido.
- **As duas cores do grid no tema escuro ficaram um pouco mais distintas**: a linha par passou de `#292929` para `#2f2f30`, contra o fundo `#252526` — de quatro para dez pontos em cada canal, contraste de 1,05 para 1,15. O bastante para a vista seguir a linha até o fim da tela, e pouco para o painel continuar escuro.
- **`EXECUTE DEM_INT.` mostra os subprogramas do pacote sem a espera.** A lista já existia e funcionava; o que não dava era o tempo — a leitura levava 20,8 s naquele pacote, e a lista abria muito depois de quem digitou ter desistido. Dizer se a rotina é procedure ou função obriga a ler `USER_ARGUMENTS`, e o filtro era por `package_name`, que não é chave de nada; agora é por `object_id`, que é como o dicionário guarda os argumentos.
- **Os subprogramas de um pacote passam a ficar guardados por sessão**, e não por aba: pedir `PKG.` numa aba nova, ou abrir o pacote na árvore, aproveita a leitura já feita. Recompilar invalida, como já invalidava as colunas.
- Se a leitura das espécies falhar, **a lista aparece com tudo como procedure** em vez de não aparecer: o que se perde é a etiqueta ao lado do nome, e a falta vai para o log. O log também passou a dizer **qual das duas consultas** demorou — sem isso, a primeira tentativa de acelerar isto aqui mexeu na consulta errada.


## 0.1.55 — 03/09/2026

- **Gerar script de INSERT, no menu do botão direito do grid.** Põe no clipboard um `INSERT` por linha escolhida — ou por todas, quando não há linha marcada, como em copiar e exportar —, com todas as colunas do resultado. O item **só aparece quando o comando daquele grid lê uma tabela só**: com `JOIN`, com duas tabelas no `FROM` ou com subconsulta não existe resposta certa para "inserir onde", e o palpite escreveria na tabela errada. A tabela entra no script com o nome exatamente como o comando o escreveu, `Vendas..cliente` incluído.
- **O mesmo item está na aba de Dados do painel de objeto**, e ali é mais direto: a consulta daquele grid é `SELECT * FROM` a própria tabela, então não há o que analisar para saber onde inserir. Na aba de Estrutura não aparece — aquele grid mostra o dicionário, não a tabela. O menu daquele painel, que estava escrito só em português, passou a sair do catálogo de idiomas como o resto da interface (requisito 31).
- **O valor é desformatado antes de virar literal.** O grid mostra texto de tela (requisito 21): `1234,56` volta a `1234.56`, e `27/08/2026 15:11:05` vira `TO_DATE('27/08/2026 15:11:05','DD/MM/YYYY HH24:MI:SS')` no Oracle — `TO_TIMESTAMP` quando a coluna é `TIMESTAMP` — ou `CONVERT(datetime, '2026-08-27 15:11:05', 120)` no SQL Server. A conversão é explícita para o script não depender de `NLS_DATE_FORMAT` nem de `DATEFORMAT`: um script que só roda na sessão de quem o gerou não serve para nada. Apóstrofo dobra, `NULL` sai `NULL`, texto vazio sai `''` e coluna `bit` sai `1`/`0` sem aspas.
- **O que o grid não tem, o script não inventa.** Coluna binária sai `NULL`, porque a célula mostra `(binário, 320 bytes)` — a descrição, não o conteúdo —, e o mesmo vale para valor que o driver não converteu; o rodapé diz quais colunas saíram assim. Ele avisa também quando algum literal passa de 4000 caracteres, que é o limite que o Oracle aceita. Coluna que não é coluna — `COUNT(*)`, expressão, concatenação — recusa o script inteiro dizendo qual: o comando sairia inválido de qualquer jeito. O clipboard recebe só SQL, sem comentário de aviso no meio, e sem `COMMIT`: confirmar é decisão de quem roda.
- **Correção: o botão direito desfazia a escolha de várias linhas.** Marcar cinco linhas e clicar com o direito numa delas deixava só aquela marcada, e o menu que abria agia sobre uma linha em vez de cinco. Valia para o **Copiar** desde sempre. Agora o direito sobre linha já escolhida não mexe na escolha; sobre linha não escolhida, escolhe aquela, como é o costume.


## 0.1.54 — 03/09/2026

- **A coluna do número da linha fica parada na esquerda do grid.** Rolar para a direita levava o número junto com as colunas, e num resultado de dezenas de colunas não sobrava nenhuma referência de qual linha se estava lendo. Agora ela acompanha a rolagem horizontal como o cabeçalho acompanha a vertical: as colunas passam por baixo dela, o canto do cabeçalho fica parado do mesmo jeito, e o número continua à vista em qualquer posição da barra.
- **A linha escolhida continua marcada na coluna do número.** Clicar no número seleciona a linha inteira, como já selecionava; o que muda é que a marca agora aparece também na própria coluna do número — fundo destacado e um traço na borda esquerda —, que é o que sobra na tela depois de a rolagem levar as células escolhidas para fora. Vale também quando a escolha é de uma célula só: o número diz de que linha ela é.
- As setas do teclado passam a **parar a célula ao lado da coluna do número**, e não debaixo dela: caminhar para a esquerda até a primeira coluna escondia a célula escolhida atrás da coluna que ficou parada.

## 0.1.53 — 02/09/2026

- **Os conjuntos de resultados aparecem todos, um grid embaixo do outro** — a mesma pilha do SSMS. `exec ampl_001m3_0 @fl_debug = 1` devolve vinte conjuntos; a tela mostrava um, o último, e os outros dezenove sumiam depois de terem sido lidos do banco. Agora cada um tem o seu grid, na ordem em que o servidor os mandou.
- Com **um** conjunto só, nada muda: o grid ocupa a área inteira como sempre ocupou. A pilha só existe a partir do segundo, e aí cada grid fica com até dez linhas à mostra — rolando por dentro se tiver mais — enquanto a pilha rola por fora. O limite não é enfeite: o grid só desenha as linhas visíveis, e um grid alto o bastante para caber tudo desenharia tudo.
- **Copiar, exportar e o menu do botão direito agem sobre o grid em que se mexeu por último**, e não sobre o primeiro da pilha.
- Vale para os dois bancos. No Oracle, uma aba com várias consultas soltas passa a mostrar todas — e cada grid pede mais linhas pelo seu próprio cursor. No SQL Server só o primeiro conjunto do lote pode pedir mais, porque alcançar o segundo obriga a ler o primeiro até o fim; **quando um conjunto seguinte vem maior que o bloco, a mensagem passa a dizer quantas linhas o grid está mostrando**, em vez de anunciar uma contagem que ele não tem.

## 0.1.52 — 02/09/2026

- **Correção: a execução que falhava no fim do lote não mostrava nada, nem o que já tinha vindo.** `exec ampl_001m3_0 @fl_debug = 1` numa conexão de SQL Server devolvia vinte conjuntos de resultados e só então a falha do lote; a aba mostrava a mensagem de erro e grid nenhum. A procedure tinha rodado, os dados tinham chegado na máquina — e eram jogados fora na saída do laço de leitura, porque o erro subia sozinho e levava junto tudo o que estava lido.
- **Agora o que o servidor mandou antes da falha sobe com ela.** A aba exibe os resultados e, por cima, o erro — que continua sendo erro: mensagem em vermelho no painel Mensagens, código do servidor e salto até a linha que falhou. Num script de vários lotes vale o mesmo: os lotes que rodaram aparecem, e a execução para no que falhou.
- O caso que trouxe isto à tona é o do **auto-commit desligado**: com `SET IMPLICIT_TRANSACTIONS ON`, um erro de conversão dentro de `BEGIN TRY` deixa a transação impossível de confirmar, e o servidor derruba o lote inteiro no fim com `Uncommittable transaction is detected at the end of the batch` — depois de já ter mandado todos os grids. O `TRY … CATCH` da procedure não salva disso, e o mesmo lote em auto-commit passa sem reclamar; era exatamente por isso que o problema não aparecia no SSMS.
- Continua valendo a regra do requisito 6: **o grid é o do último conjunto de resultados do lote**. Para ver os vinte de uma procedure de depuração, `F5` (modo script) mostra todos em texto.

## 0.1.51 — 01/09/2026

- **Correção: o autocomplete não sugeria coluna no `SET` de um `UPDATE`.** `UPDATE financeiro SET status = 'PS', dt_` devolvia tipos do dicionário — `DT_ATTR_T293_T`, de `ORDSYS` — em vez de `DT_INC` e `DT_PROCESSAMENTO`. A tabela já era encontrada; o que faltava era o `SET` estar entre os lugares de coluna, tanto logo depois da palavra quanto a cada vírgula da lista. Vale também no `SET` do `MERGE`.
- **A lista de colunas do `INSERT` ganhou o mesmo conserto.** O parêntese de `INSERT INTO t (` já dava a primeira coluna, mas da vírgula em diante caía no mesmo "lugar de nada". O `VALUES (1, ` continua de fora de propósito: ali se escreve valor, não coluna.
- Depois de `=`, `>` e `||` a lista segue sendo a de funções e packages, e não a de colunas: à direita de um operador cabem as duas coisas, e `SYSDATE - 2` é tão comum quanto comparar duas colunas. À esquerda do `=` de um `SET` só cabe coluna, e é só esse lado que mudou.
- **O painel Mensagens - Log ganhou um botão Limpar**, no mesmo padrão da Saída DBMS. O painel acumula tudo o que a aba relatou desde que foi aberta, e depois de algumas tentativas seguidas o erro que interessa fica enterrado; limpar antes de executar de novo deixa na tela só o que aquela execução disse. É a tela apenas — o registro em disco não é tocado. A barra fica parada no topo enquanto a lista rola, para o botão não sumir justamente quando há mensagem demais para ler.

## 0.1.50 — 01/09/2026

- **Enter mantém a indentação da linha.** Quem alinhou a segunda coluna de um `SELECT` à mão voltava para a coluna 1 ao pedir a terceira, e tinha de reindentar na unha. Agora a linha nova nasce com o mesmo espaço da linha de cima. Não é a indentação automática do SQL: o editor repete o que já está lá e não reposiciona nada por conta própria — quem escreveu `FROM` sem recuo continua sem recuo.
- **Correção: o autocomplete não sugeria coluna na lista do `SELECT`.** `SELECT *, dt_` devolvia tipos e procedures do dicionário — `DT_ATTR_T293_T`, de `ORDSYS` — em vez das colunas da tabela do `FROM`. A lista do `SELECT` não estava entre os lugares de coluna, nem logo depois da palavra nem a cada vírgula dela; só `WHERE`, `ORDER BY` e afins estavam. Agora `SELECT status,`, `SELECT *,` e `SELECT dt_inc,*` pedem a coluna seguinte da mesma tabela, com ou sem `DISTINCT`.
- **Correção: numa aba com várias consultas soltas, as colunas vinham todas misturadas.** O comando terminava só no `;`, e ninguém põe `;` numa aba de rascunho: dez consultas seguidas eram lidas como um comando só, e a lista trazia as colunas das tabelas de todas elas. Era exatamente na tela mais cheia que o autocomplete ficava pior. Agora a **linha em branco seguida de palavra que abre comando** também separa. `FROM`, `WHERE`, `SET` e `VALUES` não separam — eles continuam um comando começado acima —, então o `SELECT a, b` em cima e o `FROM t` embaixo seguem sendo uma consulta só.
- **`Ctrl` + roda do mouse amplia e reduz o editor**, de 60% a 300%, em passos de 10%. A escolha fica guardada na máquina e vale para todos os editores: janela de consulta, fonte de objeto, comando de etapa. A lista do autocomplete acompanha o tamanho. Sem tecla de atalho, de propósito: `Ctrl+-` já é o **Comentar linhas**, e tirá-lo de lá custaria mais do que ganharia.
- **Cada botão rápido agora escolhe em quais conexões aparece.** Um campo **Conexão** na configuração do botão, logo antes de **Posição**, com **Todas** no topo e uma linha por conexão salva, cada uma com a sua bolinha de cor. Marcando `EXEMPLO PRD` e `EXEMPLO HML`, o botão some das abas de `Sistema Exemplo PRD`. O atalho acompanha: a tecla de um botão escondido não faz nada naquela aba. O padrão continua **Todas**, e todo botão que já existia segue aparecendo em toda parte.
- O filtro guarda o identificador da conexão, e não o nome: renomear a conexão não solta o botão dela.

## 0.1.49 — 31/08/2026

- **A view agora tem comentário, e ele aparece na árvore.** Uma view chamada `VW_FAT_CONS_01` não diz o que é, e o nome sozinho raramente diz. O comentário sai entre parênteses ao lado do nome — `VW_FATURAMENTO (Faturamento por filial, mês fechado)` —, nos primeiros 50 caracteres, com reticências quando há mais. A **dica do mouse** traz o texto inteiro. Só as views o têm: uma tabela se explica pelas colunas, que a árvore já expande; uma view esconde a consulta que a define.
- **Botão direito sobre a view → Editar Comentário…**. A caixa abre com o texto atual, aceita várias linhas e confirma com Ctrl+Enter. Esvaziar a caixa **apaga** o comentário — por isso esvaziar e cancelar são coisas diferentes: só Cancelar e Esc não fazem nada. Gravado, a linha da árvore se atualiza no lugar, sem relistar a categoria.
- **Funciona nos dois bancos, por caminhos diferentes.** No Oracle é o `COMMENT ON TABLE`, lido de `USER_TAB_COMMENTS`; convém saber que ele é DDL, e que DDL no Oracle confirma a transação aberta. No SQL Server é a *extended property* `MS_Description` — o mesmo nome que o SSMS lê e escreve na caixa *Description*, de modo que os dois programas enxergam o comentário um do outro.
- **Num painel estreito é o comentário que encolhe, nunca o nome da view.**

## 0.1.48 — 29/08/2026

- **Correção: a data de compilação aparecia um dia à frente à noite.** Depois das 21h, a tela **Versões** anunciava um executável compilado no dia seguinte, e as datas das cópias da tela de recuperação iam junto. O fuso era descoberto comparando as *horas do dia* do relógio local e do UTC, e das 21h à meia-noite em Brasília o UTC já virou o dia: `21:12 − 00:12` dá +21 h onde o fuso é −3 h, uma diferença de exatamente um dia. Agora a conta é entre instantes inteiros, que atravessam a virada sem perder o dia no caminho.
- **Correção: uma falha ao ler a lista de bancos calava o nome de três partes pelo resto da aba** (SQL Server). A lista é lida uma vez por aba, e a leitura que não desse certo — a sessão da conexão ocupada, a rede caindo no meio — ficava guardada como se fosse resposta: `BANCO_EXEMPLO..tbl` parava de sugerir até a aba ser fechada, em silêncio e sem nada na tela que explicasse. A falha não é mais guardada, e a tecla seguinte tenta de novo.

## 0.1.47 — 29/08/2026

- **O autocomplete passou a sugerir bancos** onde cabe uma tabela (SQL Server). O que se digita depois de `FROM` tanto pode ser o começo do nome de uma tabela quanto o de um banco, e sem o banco na lista não havia como chegar ao nome de três partes. Ele aparece rotulado como *Banco de dados*, depois dos objetos. Depois de `use` a lista é só de bancos, que é a única coisa que o comando aceita.
- **Correção: `BANCO_EXEMPLO..tbl` não sugeria nada.** Escrito o banco, o autocomplete não reconhecia mais o nome — o `..` do SQL Server deixa o esquema vazio, e a leitura anterior exigia um nome antes do último ponto. Agora a lista traz as tabelas **daquele banco** que começam pelo prefixo, e só tabelas: o lugar continua sendo lugar de tabela, então procedure e função ficam de fora.
- **O esquema escrito também corta a lista**: `dbo.tbl` traz o que está em `dbo`, e `BANCO_EXEMPLO.dbo.tbl`, o que está em `dbo` daquele banco. Antes, um nome com esquema não sugeria nada.
- **As colunas seguem a tabela até o outro banco.** O apelido de `FROM BANCO_EXEMPLO..tbl_nota n` resolve `n.` para as colunas de `BANCO_EXEMPLO`, e não para as de uma tabela de mesmo nome no banco da aba.

## 0.1.46 — 28/08/2026

- **Correção: instância nomeada nunca conectava.** `SERVIDOR\INSTANCIA`, `192.0.2.10\INSTANCIA_EXEMPLO`, `.\SQLEXPRESS` — todas falhavam por tempo esgotado, com cara de firewall. Sem porta escrita, quem diz onde a instância atende é o **SQL Server Browser**, perguntado em **UDP 1434**; o Cururu fixava a porta em 1433 antes de perguntar, e a pergunta ia para a porta errada. Agora a porta só é informada quando o usuário a escreve. Com porta escrita (`servidor\instância,49812`) nada muda: não se pergunta a ninguém.
- **Correção: o exemplo do campo Nome do Servidor escondia a barra invertida** — mostrava *servidorinstância*, engolida pelo literal do TypeScript, justamente o separador que o exemplo existe para ensinar.
- **O banco do SQL Server passou a expandir**, com as quatro pastas do SSMS: **Tabelas**, **Views**, **Stored Procedures** e **Functions** — esta última sem lista própria, separando **Funções com Valor de Tabela** de **Funções com Valor Escalar**, como o SSMS separa. Uma entra no `FROM` e a outra não; misturá-las esconderia justamente a diferença que importa na hora de escrever a consulta.
- **O escopo é o banco, e não o schema.** Um banco tem vários esquemas, e por isso cada objeto aparece como `esquema.nome` — o nome do SSMS e, não por acaso, o que se escreve numa consulta. A listagem é paginada como a do Oracle, com o mesmo **Listar mais…**.
- **As ações de tabela e view são as mesmas do Oracle**: colunas ao expandir e, no botão direito, **Exibir Dados**, **Exibir DDL** e **Exibir Estrutura**, no mesmo visualizador de abas internas.
- **O duplo clique abre o fonte**: o `CREATE` guardado, na procedure e na função — que vem de `sys.sql_modules` exatamente como foi executado —, e o DDL montado, na tabela e na view. Módulo criptografado diz que está criptografado, em vez de abrir vazio. Editar o fonte na aba e executá-lo é o caminho de alterar o objeto.
- **O DDL de tabela é montado pelo Cururu**, porque o SQL Server não tem `GET_DDL`: o *Script Table as* do SSMS é código do SSMS, não do servidor. Sai na ordem dele — colunas com tipo, `IDENTITY`, coluna calculada e nulidade, chave primária e unicidade dentro do `CREATE TABLE`; depois `DEFAULT`, `CHECK`, chaves estrangeiras e índices, com as colunas incluídas quando há.
- **O grid de Dados abre sessão própria no SQL Server**, como uma aba de consulta. Ali a conexão atende um pedido de cada vez, e um grid deixado pela metade prenderia a sessão da árvore: expandir um nó ficaria esperando alguém rolar o grid até o fim.
- **O filtro de categoria continua sendo do Oracle.** Os critérios do requisito 4 são montados em SQL de lá, com bind nomeado; no SQL Server o menu da categoria oferece só *Atualizar*.

## 0.1.45 — 28/08/2026

- **O comando de uma etapa de trabalho passou a ser o editor de consultas**, e não uma caixa de texto: comentário em verde, palavra reservada realçada, **Tab** e **Shift+Tab** indentando o bloco, localizar/substituir e comentar linha, tudo pelos mesmos atalhos configuráveis do resto do Cururu. Quem escreve o T-SQL de uma etapa escreve SQL, e não havia por que ser pior do que a janela de consulta.
- **O autocomplete veio junto**, apontado para o banco escolhido na própria tela da etapa. Só no T-SQL: num CmdExec ou num PowerShell não há objeto de banco nenhum a sugerir.
- **A tela de etapa virou uma janela de verdade:** move pela barra de título, muda de tamanho pelas bordas e pelos cantos e **maximiza** — pelo botão ao lado do de fechar ou pelo duplo clique na barra —, com o comando crescendo junto. Um comando de cem linhas não se lê por uma fresta.
- **A tela de Propriedades do Trabalho também se move.** Ela é modal, e antes ficava presa no meio da janela tapando justamente a árvore que se quer conferir enquanto se preenche o formulário.
- **A janela não se perde:** por mais que se arraste, a barra de título continua alcançável na tela, e um Cururu que encolha traz a janela de volta para dentro.
- **Arrastar o mouse sobre rótulo, título, cabeçalho de grade ou aviso não seleciona mais nada** nas telas do SQL Server Agent — isto é um aplicativo do Windows, e ali o arrasto não pinta texto de azul. Só o que se edita continua selecionável. A barra de título de qualquer diálogo também deixou de selecionar, já que agora ela é a alça que move a janela.

## 0.1.44 — 28/08/2026

- **A janela de consulta passa a executar SQL no SQL Server.** Play, script, grid, fetch parcial, commit, rollback, auto-commit, Parar, bind variables, histórico, painel de mensagens e rodapé funcionam ali como já funcionavam no Oracle.
- **O lote é o `GO`, não o `;`.** No T-SQL o ponto e vírgula separa instruções *dentro* do lote, e quem encerra o lote é a linha com `GO` — que é marca do cliente e nem chega ao servidor. Cortar no `;` partiria ao meio todo `IF … BEGIN … END`, todo `WHILE` e toda variável declarada e usada logo abaixo. É a regra do SSMS, então um script colado de lá roda como está; o `GO` vale sozinho na linha, com contagem (`GO 5`) ou com comentário atrás, e não dentro de literal nem de comentário — que aqui já entende colchete e comentário de bloco aninhado.
- **Autocomplete de tabela, view, sinônimo, procedure e função**, lido do catálogo do **banco em que a aba está**: trocar o banco na lista da barra troca o que é sugerido, e esquece as colunas guardadas do banco anterior. O esquema aparece ao lado do tipo quando não é `dbo`, no lugar em que o Oracle mostra o dono, e o que é de `dbo` vem primeiro. **Colunas** também: depois do `WHERE`, depois do alias (`c.`) e nos demais lugares do requisito 3, com o tipo escrito como o T-SQL o escreve — `varchar(30)`, `numeric(10,2)`, `nvarchar(max)`.
- **Fetch parcial**: `select * from` sem `where` traz um bloco por vez, como no Oracle. Largar um grid pela metade custa ler o que sobrou — no SQL Server a conexão atende um resultado de cada vez e não existe fechar cursor pela metade —, e essa leitura tem prazo de três segundos: passado ele, a conexão é reaberta e a tela avisa que a sessão é outra.
- **Botão Parar.** O driver não expõe o sinal de atenção do TDS, que é o que o SSMS manda; o que resta é largar o socket, e o servidor aborta a consulta do mesmo jeito, como faria um `KILL`. A conexão volta em seguida com o banco e o auto-commit no lugar, e a tela avisa — a sessão nova é outra sessão do banco.
- **Bind variables continuam com `:nome`**, e não `@nome`: no T-SQL o `@` já é a variável local do `DECLARE`, e tratá-lo como bind faria todo script com `DECLARE @x int` parar para perguntar um valor que ele já tem.
- **Erro do servidor com número e salto para a linha**: `MSSQL-102` aparece como o `ORA-` aparece, e o painel de mensagens leva o cursor até a linha do editor — o servidor conta a linha dentro do lote, e a conta é ajustada antes do salto.
- **Datas e números no padrão brasileiro**, como no Oracle: `dd/mm/aaaa`, hora só quando existe, vírgula decimal. Vale para `date`, `time`, `datetime`, `datetime2`, `smalldatetime` e `datetimeoffset`. Coluna `bit` sai como `1`/`0`, que é o que se escreve de volta num `where`.
- **Conexão nova de SQL Server nasce com auto-commit ligado**, ao contrário da de Oracle: cada banco nasce com o padrão da ferramenta de referência dele, e o do SSMS é esse. Ali o auto-commit é estado do servidor (`SET IMPLICIT_TRANSACTIONS`), e desligado ele abre transação em **todo** comando, `select` inclusive — o que faria a lista de bancos da barra recusar a troca depois de qualquer consulta, já que o SQL Server não aceita `USE` dentro de transação. Com transação aberta a troca continua sendo recusada, dizendo o que falta.
- **Ligar o auto-commit confirma a transação pendente**, como fazem os drivers padrão; deixá-la aberta faria o interruptor mentir. **Commit e rollback sem nada pendente não são erro**, como no Oracle.
- **Um `use` digitado no editor troca o banco da aba de verdade**, e a lista da barra acompanha.
- **A conexão de SQL Server se reabre sozinha quando cai** (requisito 35), como a do Oracle: a leitura que falhou é repetida na conexão nova; o comando que escreve não é repetido, e a tela avisa que basta executar de novo.
- **Correção:** a sessão da conexão — a que atende árvore e autocomplete — abria com o auto-commit do perfil. Com ele desligado, ela ficava com uma transação implícita aberta desde a primeira leitura do catálogo, segurando lock e log sem que ninguém tivesse executado nada. Agora ela abre sempre com auto-commit ligado; quem respeita o interruptor é a sessão da aba.

## 0.1.43 — 27/08/2026

- **Botão direito sobre um trabalho do SQL Server Agent**, com o menu do SSMS: **Novo Trabalho**, **Iniciar Trabalho na Etapa…**, **Parar Trabalho**, **Exibir DDL**, **Habilitar**, **Desabilitar**, **Renomear**, **Excluir**, **Atualizar** e **Propriedades**.
- **Propriedades abre a mesma tela do cadastro, preenchida** — inclusive os quatro campos que só o servidor preenche: Origem, Criado em, Última modificação e Última execução, que num cadastro novo saem vazios e aqui finalmente têm o que mostrar. O duplo clique sobre o trabalho leva ao mesmo lugar.
- **Ao gravar, etapas e agendas são trocadas por inteiro**, e não conciliadas uma a uma: a tela deixa reordenar, renomear e excluir livremente, e depois disso não há como casar uma etapa da tela com a do servidor. A agenda perde o identificador dela no `msdb`; uma agenda compartilhada com outro trabalho continua de pé para ele, porque só se apaga a que ninguém mais usa.
- **Iniciar Trabalho na Etapa…** mostra a grade de etapas para escolher por onde começar — um trabalho de cinco etapas quase nunca precisa rodar inteiro quando se está consertando a terceira. O duplo clique numa etapa escolhe e inicia. O comando **agenda** a execução e volta na hora: quem executa é o serviço, e ele não avisa quando termina; a barra de status diz isso.
- **Exibir DDL** abre, numa aba do editor, o script que recria o trabalho — o mesmo destino do fonte de uma procedure do Oracle. O formato é o do *Script Job as CREATE* do SSMS, com `USE [msdb]`, transação, conferência de retorno depois de cada procedure e `GO`, para rodar como está em outro servidor. A categoria vai junto, com o `if not exists`.
- **Habilitar e Desabilitar**: a opção que não faria nada aparece desabilitada, em vez de aparecer e não surtir efeito.
- **Renomear e Excluir** perguntam numa caixa do próprio aplicativo, e não na do Windows — que apareceria clara por cima de uma tela escura. A confirmação de exclusão diz o que sai junto: etapas, histórico e as agendas que só aquele trabalho usa.

## 0.1.42 — 27/08/2026

- **A árvore do SQL Server ganhou o nó SQL Server Agent**, ao lado de Banco de Dados, com os **trabalhos automáticos pendurados direto nele**. Sem a pasta *Trabalhos* do SSMS: ela existe lá para separar os trabalhos dos alertas, operadores e proxies, e aqui só há trabalho.
- **O nó avisa quando o serviço está parado**, com o símbolo trocado e a palavra *parado* em vermelho ao lado. Um trabalho agendado num Agent fora do ar não roda, e ver a lista cheia sem essa informação é a maneira mais fácil de perder uma manhã. Quando a conta não tem permissão de consultar o serviço, o nó fica neutro e a dica do mouse explica por quê — dizer "parado" a quem apenas não pode olhar mandaria procurar defeito num serviço que está funcionando.
- **Cada trabalho traz na dica do mouse** a descrição, a categoria, o proprietário, a data de criação e a última execução com o resultado dela. **Trabalho desabilitado aparece em cinza**, com a palavra *desabilitado* ao lado.
- **Botão direito sobre SQL Server Agent oferece Novo Trabalho e Atualizar.**
- **Tela de Novo Trabalho**, espelhando a do SSMS em três páginas — das seis dele ficam **Geral**, **Etapas** e **Agendas**. Alertas e notificações dependem de operador cadastrado e de Database Mail; destinos só existe em administração multisservidor.
- **Geral** tem nome, proprietário, categoria, descrição e Habilitado, e abaixo, sem edição, os quatro campos que o SSMS também mostra ali: Origem, Criado em, Última modificação e Última execução — quem os preenche é o servidor.
- **Etapas** lista ordem, nome, tipo e o que fazer ao ter êxito e ao falhar, com Nova, Editar, Excluir e as setas de mover. A tela de etapa aceita **Transact-SQL**, **Sistema operacional (CmdExec)** e **PowerShell**, com o banco de dados aparecendo só no T-SQL. A etapa nova nasce saindo com êxito, e a que era a última passa a seguir para a seguinte: quem sai informando êxito no meio do trabalho interrompe tudo o que vem depois sem erro nenhum registrado.
- **Agendas** é a tela de agenda do SSMS: recorrente, uma vez, ao iniciar o Agent ou quando as CPUs ficarem ociosas; diária, semanal ou mensal — inclusive o *primeiro/segundo/terceiro/quarto/último* dia, dia útil, dia de fim de semana ou dia da semana; frequência dentro do dia e duração. O **Resumo** escreve a agenda por extenso enquanto ela é montada, porque os números que o SQL Server guarda não se leem.
- **Criar é tudo ou nada:** o trabalho, as etapas e as agendas entram numa transação só. Um trabalho que ficasse sem etapa, ou que não chegasse a ser atribuído ao Agent local, apareceria na lista e nunca rodaria. Um trabalho sem etapa nenhuma é recusado pela tela.

## 0.1.41 — 27/08/2026

- **Conectar no SQL Server já abre o nó Banco de Dados**, com a lista à vista. Quem abre a conexão vai olhar os bancos, e exigir um segundo clique para isso era só atrito.
- **O atalho de nova consulta (`Ctrl+T` de fábrica) com o ponteiro do mouse sobre um banco abre a consulta naquele banco.** O ponteiro tem precedência sobre a seleção: apontar e pedir a consulta é o gesto direto, e não deve exigir um clique antes. Sem ponteiro sobre banco nenhum, o atalho continua agindo sobre o que está selecionado na árvore, como antes.
- **Correção: os rótulos de "Nova Consulta SQL" anunciavam `Ctrl+T` mesmo depois de a tecla ter sido trocada.** O texto trazia a combinação de fábrica embutida; agora ele lê a tecla vigente, e some o parêntese quando a ação está sem atalho. Vale para o botão da barra, para a dica dele e para os menus de contexto da árvore.
- **Botão direito sobre um banco oferece Nova Consulta SQL**, também abrindo a aba naquele banco.
- Selecionar um banco e usar o botão **Nova Consulta SQL** da barra tem o mesmo efeito — os três caminhos levam ao mesmo lugar.

## 0.1.40 — 27/08/2026

- **A árvore do SQL Server passa a esconder os bancos de sistema** — `master`, `tempdb`, `model` e `msdb`. Sob **Banco de Dados** ficam só os bancos de trabalho, que é onde se trabalha. A conexão nova também nasce no primeiro banco de trabalho, e não mais no primeiro nome qualquer da lista.
- **O banco em uso na aba de consulta virou uma lista**, no lugar em que o Oracle mostra a Saída DBMS: dá para escolher outro banco e a aba passa a executar nele. **Aqui os bancos de sistema aparecem** — consultar o catálogo do `master` é trabalho legítimo de quem administra. A troca vale só para aquela aba, como o `USE` do SSMS vale só para a janela em que foi dado; recusada a troca, a lista volta a mostrar o banco em que a aba realmente está, e durante uma execução ela fica travada.
- **O botão Compilar não aparece mais em aba de SQL Server.** Compilar é PL/SQL e não tem equivalente lá; o atalho Ctrl+Shift+C continua respondendo, dizendo por que não faz nada.

## 0.1.39 — 27/08/2026

- **O botão Conectar passa a funcionar com SQL Server.** A conexão abre de verdade e fica de pé entre um comando e outro, como já acontecia no Oracle.
- **A árvore do SQL Server tem um nó: Banco de Dados**, com os bancos do servidor a que o login tem acesso. Ficam de fora os que estão offline ou em restauração e os que a conta não pode abrir — listar o que vai falhar ao ser clicado só atrapalha. Os bancos ainda não expandem.
- **Conectar já abre a aba de consulta também no SQL Server**, apontada para o **primeiro banco da lista** — o mesmo primeiro nome que aparece sob Banco de Dados. Abrir outra aba na mesma conexão mantém o banco, sem nova ida ao servidor para descobri-lo.
- **O banco em uso aparece na barra da aba**, no lugar em que o Oracle mostra a marcação de Saída DBMS. A Saída DBMS agora só aparece em conexões Oracle, que é o único banco que a tem.
- **Executar SQL ainda é só do Oracle.** Grid, cursores, transação, autocomplete e leitura de fonte respondem, numa conexão de SQL Server, que a operação ainda não existe para ela — em vez de tentar e falhar de um jeito confuso.

## 0.1.38 — 27/08/2026

- **O botão Testar passa a funcionar com SQL Server.** Ele abre a conexão de verdade, mostra o banner de versão do servidor e desliga — a mesma resposta que já dava no Oracle. O **Conectar** continua recusado: abrir sessão, árvore de objetos e execução de SQL ainda são só do Oracle.
- **O "Nome do Servidor" é aceito nas formas do SSMS:** `SRV`, `SRV\INSTANCIA`, `SRV,PORTA` e `SRV\INSTANCIA,PORTA`, com `tcp:` opcional na frente, e `.` ou `(local)` para a máquina local. Sem porta explícita, a instância nomeada é resolvida pelo **SQL Server Browser** em UDP 1434 — e, quando isso falha, a mensagem diz onde olhar em vez de devolver só o erro do socket.
- **As duas autenticações funcionam.** A do SQL Server usa logon e senha; a do Windows usa a identidade de quem está logado quando os campos ficam em branco, ou a credencial de domínio informada (`DOMINIO\usuario`).
- **A espera tem teto de 15 segundos**, como no SSMS: um servidor inalcançável não deixa mais o botão girando pelo tempo do TCP.
- **O tipo de banco agora só pode ser escolhido ao cadastrar a conexão.** Numa conexão já salva o campo fica desabilitado, com a explicação na dica do mouse — trocá-lo não seria editar um campo, seria trocar a conexão por outra.
- Erros do SQL Server chegam com o número do servidor (`MSSQL-18456` para login recusado, por exemplo), do mesmo jeito que os `ORA-` do Oracle, e as falhas de rede aparecem no idioma do Windows em vez do texto em inglês do driver.

## 0.1.37 — 27/08/2026

- **A tela de conexões passa a cadastrar conexões de SQL Server.** O campo **Tipo de Banco de Dados**, que só mostrava Oracle, agora oferece **Oracle** e **SQL Server** e comanda o resto do formulário: escolhido o banco, aparecem os campos dele e somem os do outro. Nome, cor, barra de status e os botões **Importar, Salvar, Excluir, Testar, Conectar e Cancelar** continuam iguais para os dois.
- **Com SQL Server, os campos são os do SSMS:** **Nome do Servidor** (num campo só, aceitando `servidor`, `servidor\instância` e `servidor,porta`), **Autenticação** (*do SQL Server* ou *do Windows*), **Logon** e **Senha** com a opção **Lembrar senha**, e **Auto-commit**. Na autenticação do Windows os dois campos passam a se chamar **Nome de usuário** e **Senha**, e deixá-los em branco vale a identidade de quem está logado no Windows.
- **Usuário e senha são um dado só do perfil**, mostrado nos dois formulários: trocar o tipo de banco no meio do cadastro não apaga o que já foi digitado nem obriga a redigitar a credencial, e o segredo não é gravado duas vezes.
- **Ainda não se conecta a SQL Server.** Salvar, listar, editar e excluir já funcionam; **Testar** e **Conectar** recusam um perfil de SQL Server dizendo que o driver ainda não existe, em vez de tentar abri-lo pelo cliente do Oracle e devolver um erro `ORA-` que não explica nada. Versões atendidas quando o driver entrar: **SQL Server 2014 em diante**.

## 0.1.36 — 26/08/2026

- **Correção: a aba de consulta parava de funcionar quando a conexão caía, e só voltava sendo fechada e aberta de novo.** Era o que acontecia com a VPN: ela caía, voltava, e a aba continuava respondendo `ORA-03113` a tudo, porque a conexão do driver tinha morrido junto e ninguém a substituía. Agora a própria tentativa seguinte reabre a sessão, com as credenciais já resolvidas da conexão e sem repedir a senha. Uma **consulta** reconecta e executa na hora — Play uma vez só. Um comando que **altera dados** (DML, DDL, bloco PL/SQL, commit, rollback) reconecta mas não é repetido sozinho: não há como saber se ele chegou a valer no banco antes de a conexão cair, e repetir um `insert` por conta própria gravaria duas vezes; a aba volta utilizável e a mensagem diz que basta executar de novo. Árvore de objetos, colunas e autocomplete reconectam e repetem em silêncio, por serem só leitura.
- **Toda reconexão é avisada no painel de mensagens e na barra de status.** A sessão reaberta é *outra* sessão do banco: o que estava sem commit se perdeu junto com a conexão anterior. A Saída DBMS é religada com o mesmo buffer, e o auto-commit, por ser do Cururu e não do banco, continua valendo sozinho.

## 0.1.35 — 18/08/2026

- **Tomar a tecla de outra ação agora sempre vale, inclusive quando ela é o atalho principal.** Na 0.1.34, gravar **F5** como atalho alternativo de um comando era recusado porque **F5** era o principal de *Executar script* — a ação antiga ficaria sem tecla. Agora a tecla muda de dono do mesmo jeito e a ação antiga fica sem atalho mesmo, o que é permitido: a barra de status avisa ("F5" foi removido de "Executar script") e o campo vazio fica à mostra na tabela, à espera de outra combinação. Uma ação sem tecla apenas deixa de responder; o resto continua funcionando.
- **Backspace ou Delete puro passa a limpar também o atalho principal**, não só o alternativo — uma ação pode ficar deliberadamente sem atalho, devolver um atalho roubado ou receber combinação nova num campo que ficou vazio.

## 0.1.34 — 18/08/2026

- **A tela Teclas de Atalho passa a trocar as teclas de dono quando a combinação já está em uso.** Antes, o campo aceitava a combinação e o conflito só aparecia ao clicar em **Salvar**, bloqueando tudo. Agora, quem tinha a tecla recebe na hora a combinação que estava no campo editado: gravar **Ctrl+D** em *Excluir a linha atual* deixa a exclusão com **Ctrl+D** e a duplicação com o **Ctrl+L** anterior — ninguém fica sem tecla, e a barra de status explica a troca. Quando o campo editado é um alternativo vazio, a ação anterior fica sem o alternativo, o que é permitido; já tomar o atalho **principal** de outra ação sem ter combinação para devolver é recusado com aviso, porque toda ação precisa do seu principal.
- **Correção: Backspace ou Delete com modificador gravavam nada e limpavam o campo alternativo.** Pressionar **Ctrl+Backspace** num atalho alternativo, por exemplo, apagava o valor em vez de gravar a combinação. Agora só Backspace/Delete puro limpa; com modificador, a combinação é gravada como qualquer outra.

## 0.1.33 — 18/08/2026

- **A tela Teclas de Atalho passa a aceitar duas combinações por comando.** Cada linha ganha uma segunda coluna, opcional, com o mesmo controle de duplicidade da primeira — nenhuma tecla pode responder por dois comandos, seja como atalho principal ou alternativo de qualquer um deles.
- **Os botões rápidos ganham a opção "Não inserir SELECT na janela de consulta".** Marcada, o botão executa o conteúdo direto, sem inserir nem selecionar nada no editor — o texto nunca aparece na tela. Tem prioridade sobre "Executar automaticamente", que depende do texto estar selecionado para funcionar.
- **Novo item Tema no menu, logo abaixo de Language, com Claro e Escuro.** A troca vale na hora, sem reabrir o aplicativo, e alcança toda a interface — inclusive o destaque de sintaxe do editor SQL, que ganhou uma paleta própria para o fundo escuro em vez de herdar as cores pensadas para o tema claro, e a barra de título da janela, redesenhada por inteiro (título e botões de minimizar, maximizar e fechar) para acompanhar o tema em vez de usar sempre a barra clara do Windows.
- **A janela de configuração do botão rápido separa o Excluir dos demais botões**, no canto esquerdo do rodapé, e passa a confirmar a exclusão numa caixa no estilo do aplicativo, e não mais no alerta nativo do sistema.
- **Correção: os submenus do menu principal (Language, Tema) não esperavam o mouse chegar até eles.** O caminho até um item do submenu é uma diagonal que passa por cima dos itens de baixo, e reagir a eles na hora fechava o submenu — ou o trocava pelo do vizinho — no meio do trajeto. Abrir e fechar por hover agora têm um pequeno atraso, cancelado assim que o mouse alcança o destino; clique e teclado continuam imediatos.

## 0.1.32 — 06/08/2026

- **O grid passa a ter navegação de teclado como no Excel.** As setas percorrem as células; Home e End vão à primeira e à última coluna da linha; Ctrl+Home e Ctrl+End alcançam a primeira e a última célula do resultado; Page Up e Page Down avançam uma página; Ctrl+Page Up e Ctrl+Page Down alternam entre as abas abertas. A rolagem acompanha a célula ativa mesmo nas linhas virtualizadas.
- **A tela Teclas de Atalho passa a permitir personalização.** Cada comando geral ou do editor SQL pode receber outra combinação, com validação de duplicidade, botão **Salvar** e botão **Restaurar padrão**. A mudança é aplicada também aos editores que já estão abertos.
- **Os atalhos personalizados passam a fazer parte do backup do Cururu.** Salvar backup grava as combinações junto das conexões e dos botões rápidos; carregar restaura o conjunto. Backups antigos, que ainda não possuem esse campo, continuam aceitos.
- **A tela de atalhos passa a acompanhar o idioma da interface.** Título, instruções, ações, validações e botões estão disponíveis em português, inglês e espanhol. A navegação interna do grid continua fora dessa lista e não pode ser capturada por um botão rápido configurado anteriormente.

## 0.1.31 — 06/08/2026

- **Commit e rollback passam a aparecer junto do resultado do comando.** Depois de um `UPDATE`, `INSERT` ou `DELETE`, a confirmação da transação agora fica visível na mesma saída que informa as linhas afetadas, além de continuar na barra de status e em **Mensagens - Log**.
- **A tecla Home no grid passa a funcionar como no Excel.** Com uma célula selecionada, ela leva a seleção para a primeira coluna da mesma linha e retorna a rolagem horizontal ao início.

## 0.1.30 — 05/08/2026

- **O botão direito em procedures, funções, jobs e packages passa a abrir opções do Cururu.** Antes, só Tabelas e Views tinham menu próprio; nesses outros objetos escapava o menu do WebView2 — "Voltar, Atualizar, Imprimir, Salvar como…", inútil aqui. Agora cada um oferece **Abrir**, que leva ao fonte (ou ao DDL, no caso do job), o mesmo destino do duplo clique. Nenhum nó da árvore cai mais no menu do navegador.
- **O menu de filtro da categoria passa a respeitar o idioma.** "Aplicar filtro…" e "Limpar filtro" estavam presos em português; agora saem do catálogo nas três línguas, como o resto da interface.
- **A barra de status deixa de guardar um erro `ORA-` antigo depois de uma consulta que deu certo.** O caminho de sucesso só atualiza o rodapé da aba, então o erro de uma falha anterior ficava na barra geral até a próxima falha; agora cada execução recomeça a barra.

## 0.1.29 — 05/08/2026

- **CTRL+A dentro do DDL passa a selecionar só o DDL.** No visualizador de DDL, que é somente-leitura, o CTRL+A escapava para o WebView2 e selecionava a janela inteira — árvore, barra de ferramentas, menus — em vez do texto. Agora o conteúdo do DDL recebe o foco de teclado, e o CTRL+A seleciona apenas o DDL, como em qualquer editor.

## 0.1.28 — 03/08/2026

- **Cada aba de consulta passa a ter a sua própria sessão no banco.** Uma consulta pesada numa aba não trava mais as outras abas nem o autocomplete; antes, todas dividiam uma sessão só e esperavam a vez, porque o Oracle atende uma chamada por vez em cada sessão.
- **A transação passa a ser de cada aba.** Commit, rollback e auto-commit valem só para o que aquela aba executou, e uma alteração sem commit numa aba não é vista pelas outras. Fechar a aba encerra a sessão dela e desfaz o que estava sem commit; sair do aplicativo encerra todas as sessões, liberando na hora os bloqueios pendentes.
- **A aba da consulta em execução passa a mostrar um símbolo girando.** Enquanto a consulta roda, um anel gira na própria aba, ao lado do título — e não só no rodapé —, então dá para ver qual aba está ocupada mesmo com outra aba na tela. O símbolo some assim que a execução termina.

## 0.1.27 — 03/08/2026

- **Enter no editor SQL passa a criar uma linha sem recuo automático.** A nova linha começa na coluna zero, sem os dois espaços que pareciam uma tabulação. Quando o autocomplete está aberto, Enter continua aceitando a sugestão normalmente.

## 0.1.26 — 03/08/2026

- **Os botões rápidos podem executar automaticamente o conteúdo inserido.** A nova opção vem desmarcada e, quando habilitada, cola a consulta, mantém somente esse trecho selecionado e o executa como uma consulta normal, exibindo o resultado no grid. Conteúdo vazio nunca executa por engano o restante do editor.

## 0.1.25 — 03/08/2026

- O botão **Histórico SQL** fica temporariamente oculto; o histórico continua sendo registrado durante a sessão.
- A área de resultado deixa de ocupar metade da consulta vazia e só aparece quando houver um grid ou uma saída de script para exibir.
- Durante uma execução ou compilação, o rodapé agora mostra uma barra de progresso indeterminada junto de **Executando**, nos três idiomas da interface.

## 0.1.24 — 03/08/2026

- **As sugestões de procedures e funções de packages deixam de consultar `USER_ARGUMENTS` uma vez para cada rotina.** O caso real de `DEM_INT.` levou 26,7 segundos; agora os nomes e as funções são lidos em duas consultas filtradas pela package e combinados em memória.

## 0.1.23 — 03/08/2026

- **O autocomplete passa a abrir as rotinas de uma package depois do ponto.** Ao digitar `DEM.`, a consulta oferece as procedures e funções públicas declaradas nela, identificando o tipo de cada uma. O ponto depois de alias ou tabela presente no comando continua oferecendo colunas.
- As rotinas da package ficam em memória na própria aba depois da primeira leitura, evitando nova consulta ao banco a cada tentativa de autocomplete.

## 0.1.22 — 03/08/2026

- **Language** abre um submenu lateral de verdade com **English**, **Español** e **PT-BR**, mantendo o menu principal visível.

## 0.1.21 — 03/08/2026

- **Views passam a ter as mesmas ações das Tabelas** no botão direito: Exibir Dados, Exibir DDL e Exibir Estrutura. O visualizador preserva o tipo do objeto, inclusive ao carregar o DDL.
- **A janela de consulta ganha navegação de objetos.** O botão direito sobre uma Tabela, View, Procedure, Função ou rotina de package oferece **Ir para definição**. Tabelas e Views abrem em Estrutura; código abre no fonte, com preferência pelo Body quando a rotina pertence a um package que o possui. `Ctrl+clique` continua disponível como atalho para a mesma ação.
- Comentários e literais não viram destinos por engano. A resolução também alcança sinônimos e nomes qualificados por schema, e usa o dicionário do próprio usuário no caminho comum para não pagar o custo de `ALL_OBJECTS`.
- A escolha de idioma passa para a opção **Language** do menu principal, reconhecível mesmo quando a interface foi aberta num idioma que não se lê.

## 0.1.20 — 01/08/2026

- **A aba de consulta passa a ser traduzida por inteiro**: a barra de execução, as abas inferiores — inclusive a **Mensagens - Log**, que tinha ficado para trás —, o **Linhas por vez**, o **Tamanho do Buffer**, os botões rápidos, o menu do grid com as exportações e as mensagens de rodapé.
- `Auto-commit`, `commit` e `rollback` seguem sem tradução nos três idiomas: são termos técnicos, e a regra de idioma os mantém no original.

## 0.1.19 — 01/08/2026

- **A tela de conexão inteira passa a ser traduzida** — título, lista, abas, todos os rótulos, os botões do rodapé e as mensagens de validação. O molde da tela virou função, e por isso ela já nasce no idioma corrente a cada abertura.
- Traduzidos também os pontos que faltavam na janela principal: o título das abas de consulta (`Consulta 1`), o `(Filtrado)` da árvore, o `Listar mais…`, e as abas inferiores **Mensagens**, **Histórico SQL** e **Saída DBMS**.
- **A documentação do projeto foi reorganizada por ritmo de mudança.** `premissas.md` caiu de 451 para 96 linhas e ficou só com o que não muda — regra de ouro, foco, idioma e o *porquê* das decisões técnicas. Os 30 requisitos saíram para **`requisitos.md`**, e as regras de build, versão e publicação foram para **`AGENTS.md`**, onde já morava o "como trabalhar".
- **A versão deixa de ser incrementada a cada pedido** e passa a ser incrementada quando sai um executável que alguém vai rodar. A regra anterior gerava versões que nunca eram abertas nem publicadas — e um build de quatro minutos para mudanças de uma linha.

## 0.1.18 — 01/08/2026

- **A interface passa a ter idioma.** São três: **English**, **Español** e **PT-BR**, escolhidos pelo botão `文A` no título do painel de conexões. A troca vale na hora, sem reabrir o aplicativo, e a escolha fica guardada para as próximas aberturas.
- **O título do painel deixou de ter a palavra "Conexões".** No lugar dela ficam o botão de Menu e, ao lado, o de Idioma; o de minimizar continua na direita.
- Sem escolha guardada, vale o idioma do sistema quando for um dos três — senão, português.
- Termo técnico não se traduz em idioma nenhum: `commit`, `select`, `grid`, `fetch`, `script`, `log`, `DDL` seguem iguais nos três, como já seguiam em português.
- **Esta versão traduz a janela principal**: o menu geral, a barra de ferramentas, o painel de conexões, a árvore de objetos e as mensagens da barra de status. Os diálogos — conexão, filtro, atalhos, sobre, versões, atualização, importação, recuperação, binds — e os textos do editor, do grid e da aba de consulta **ainda aparecem em português nos três idiomas**, e entram nas próximas versões.

## 0.1.17 — 01/08/2026

- **Conectar já abre uma consulta.** Abrir a conexão na árvore abre junto uma aba de Nova Consulta SQL, com o cursor dentro dela. Reexpandir o nó ou usar Atualizar não abre outra: só a conexão de verdade abre.
- **`SELECT id_registro, * FROM financeiro` passa a funcionar.** O asterisco solto no meio de uma lista de seleção é qualificado antes do envio — vira `financeiro.*` —, que é a forma que o Oracle aceita. MySQL, PostgreSQL e SQL Server aceitam a original; o Oracle responde `ORA-00923`. Com mais de uma tabela no `FROM`, o asterisco vira o de cada uma, na ordem.
- A troca só alcança texto que o Oracle já recusa hoje: um asterisco encostado em `SELECT` ou em vírgula de um lado e em vírgula ou `FROM` do outro não tem operando nenhum, e portanto não pode ser multiplicação. `preco * 2`, `COUNT(*)` e `SELECT * FROM tabela` seguem intocados — **nenhuma consulta que já funciona muda de comportamento**. Diante de subconsulta ou view em linha, o texto vai como está e o erro do Oracle aparece como antes.
- Toda vez que o texto enviado ao banco difere do digitado — aqui e no `EXEC`, que já virava bloco anônimo —, a linha enviada fica registrada no log.

## 0.1.16 — 01/08/2026

- **Correção: as colunas do grid saíam do lugar dos seus títulos no fim da rolagem horizontal.** O cabeçalho não gasta espaço com a barra de rolagem vertical, e o corpo gasta — então o cabeçalho podia rolar alguns pixels a menos que o corpo. Nos últimos pixels da rolagem o navegador travava o cabeçalho no limite dele enquanto o corpo continuava andando, e o resultado era o dado de uma coluna aparecendo sob o título da vizinha. Agora o cabeçalho reserva no fim exatamente o espaço da barra, medido em tempo de execução.
- O defeito precisava das duas coisas ao mesmo tempo — linhas bastantes para haver barra vertical e uma consulta larga o suficiente para chegar ao fim da rolagem horizontal —, e por isso aparecia só de vez em quando. Medido com 68 colunas: 15 px de deslocamento com 200 linhas, nenhum com 5.

## 0.1.15 — 01/08/2026

- **`ORDER BY` e `GROUP BY` passam a sugerir coluna**, das tabelas do `FROM` e de cada `JOIN`. A vírgula continua a lista: a segunda coluna do `ORDER BY`, na linha de baixo, é sugerida como a primeira. Com mais de uma tabela no comando, cada coluna mostra de qual veio.
- Pela mesma regra — lugar onde só cabe coluna —, passam a sugerir também `HAVING` e o `PARTITION BY` das funções analíticas.
- **Correção: o autocomplete parava de funcionar de vez depois de uma consulta lenta que não achava nada.** Digitar `WHERE` fazia o editor procurar no banco um objeto chamado `WHE`; quando essa procura demorava e voltava vazia, com o texto já mudado desde então, a fonte de sugestões ficava presa num estado pendente — sem nenhuma busca no ar e sem disparar a próxima. Dali em diante nenhuma tecla abria lista alguma, até que se digitasse mais um caractere. Agora a busca em andamento é descartada assim que o texto muda, e a seguinte começa limpa.

## 0.1.14 — 01/08/2026

- **As colunas de uma tabela passam a ser lidas em milissegundos.** A leitura vinha de `USER_TAB_COLUMNS`, uma visão do dicionário que junta meia dúzia de tabelas internas do banco: numa base de ERP ela levava de 1,7 a 4 segundos para uma tabela só, e a sugestão chegava depois de já se ter escrito a coluna à mão. Agora o nome e o tipo das colunas vêm do próprio cursor — abre-se um `SELECT *` que não devolve linha nenhuma e pergunta-se ao Oracle o que aquela consulta traria.
- **Coluna de sinônimo e de tabela de outro dono passa a ser sugerida.** O dicionário do usuário não enxerga nenhuma das duas, e o autocomplete de nomes sempre enxergou — quem escolhia da lista um nome alcançado por sinônimo não recebia coluna alguma depois. O describe enxerga exatamente o que a consulta enxergaria.
- **As colunas passam a ser guardadas por conexão, e não por aba.** Abrir a segunda aba na mesma conexão pagava a leitura de novo, e a árvore de objetos pagava a sua à parte, para as mesmas colunas.
- O log passa a registrar **quantas colunas voltaram e quanto custou**, uma linha por tabela e por sessão. Antes registrava só o tempo, e não dava para separar "demorou" de "não achou nada".
- Tipos que o dicionário deixava sem detalhe — `TIMESTAMP`, `INTERVAL`, `FLOAT` — passam a aparecer completos na lista de sugestões.

## 0.1.13 — 31/07/2026

- **Correção: a sugestão de coluna não aparecia na primeira vez.** Digitar `WHERE STATUS` não abria lista nenhuma; só depois de apagar e escrever de novo é que ela vinha. A leitura das colunas no banco só começava quando a sugestão era pedida, e a lista chegava um segundo depois — tarde demais, quando já se tinha desistido. Na segunda vez as colunas já estavam em memória, e por isso funcionava.
- As colunas passam a ser lidas **assim que a tabela é escrita**, sem esperar o pedido de sugestão: o `FROM cliente` vem antes do `WHERE`, e esse intervalo é o tempo que o banco tem para responder.
- Digitar `WHERE STATUS` deixa de disparar uma consulta ao banco por tecla. O que fica guardado agora é a busca, e não o resultado dela, então as teclas seguintes aproveitam a consulta que já está no ar em vez de abrir outra.
- Enquanto o nome da tabela está sendo escrito, nenhum pedaço dele vai ao banco.

## 0.1.12 — 31/07/2026

- **Tabela e view passam a ser sugeridas só onde a consulta pede uma**: depois de `FROM`, `JOIN`, `UPDATE` e `DELETE`, e depois da vírgula que continua a lista do `FROM`. Escrever `SELECT nome` não abre mais a lista de tabelas do banco por cima do texto.
- **Coluna passa a ser sugerida depois de `WHERE`, `AND`, `OR` e `(`**, sem precisar do `alias.` na frente. As oferecidas são as das tabelas do próprio comando; com mais de uma tabela, cada coluna mostra de qual veio.
- **Dentro de aspas simples não há mais sugestão.** A mesma regra vale para comentário.
- Correção: o alias de uma tabela depois da vírgula — o `p` de `FROM cliente c, pedido p` — não era reconhecido, e `p.` não sugeria coluna alguma. Agora a lista inteira do `FROM` é lida, e vale também para `AS`, `JOIN` e `INSERT INTO`.
- As colunas de cada tabela são lidas uma vez por aba e ficam em memória, em vez de ir ao banco a cada vez que a lista abre.
- Pacote, procedure e função continuam sendo sugeridos ao digitar em qualquer lugar — é o que se chama no meio de um bloco PL/SQL.

## 0.1.11 — 31/07/2026

- **O Cururu passa a ter instalador**: um `setup.exe` que instala na pasta do usuário, sem pedir Administrador, já com o Oracle Instant Client embutido.
- O instalador é publicado nas **[Releases do Cururu](https://github.com/yurimj/Cururu/releases)**, junto do código-fonte público.
- Nova opção **Procurar Atualizações…** no menu: consulta a versão publicada e, quando há uma mais nova, mostra **o que mudou** antes de qualquer download.
- Na subida, o Cururu confere se existe versão nova em silêncio. Sem internet, nada aparece na tela: a falha fica no log.
- Baixar e instalar é **decisão do usuário**, nunca automática. Escolhido "Baixar e instalar", o instalador desce com barra de progresso, é executado e o Cururu se fecha para ele poder substituir os arquivos em uso.
- O download usa o **WinHTTP do Windows**, então respeita o proxy da rede — inclusive PAC — e a loja de certificados da máquina.

## 0.1.10 — 31/07/2026

- **`Ctrl+T`** abre uma nova consulta na conexão que estiver conectada e selecionada na árvore.
- **`Ctrl+W`** fecha a aba visível.
- A tela **Teclas de Atalho** passa a listar as combinações em ordem, começando por `Ctrl+D`, e inclui os dois novos atalhos globais.

## 0.1.9 — 31/07/2026

- O botão direito de uma tabela passa a oferecer **Exibir Dados**, **Exibir DDL** e **Exibir Estrutura**.
- As três opções compartilham uma única aba de objeto, com navegação interna entre Dados, DDL e Estrutura.
- **Dados** usa o grid e o fetch parcial já existentes; **DDL** tem destaque de sintaxe e cópia; **Estrutura** lista nome, tipo e nulabilidade das colunas em grid.

## 0.1.8 — 31/07/2026

- **A aba passa a ficar amarrada ao seu arquivo**: `Ctrl+S` só pergunta onde salvar na primeira vez. Depois disso grava direto por cima, como em qualquer editor. Abrir um `.sql` já amarra a aba a ele.
- Nova opção **Salvar Consulta SQL Como…**, que pergunta sempre o caminho e passa a valer como o novo arquivo da aba.
- Salvar em outro caminho também atualiza a origem mostrada na tela de recuperação.
- Menu com os nomes revistos: **Salvar Consulta SQL (Ctrl+S)**, **Salvar Consulta SQL Como…**, **Abrir Consulta SQL (Ctrl+O)**, **Salvar backup Cururu** e **Carregar backup Cururu**.

## 0.1.7 — 31/07/2026

- **`Ctrl+S`** salva a consulta da aba visível e **`Ctrl+O`** abre um `.sql` numa aba nova. Valem na janela inteira, não só com o editor em foco.
- **Cópia de recuperação**: o texto das abas passa a ser guardado sozinho, dois segundos depois da última tecla, em `%APPDATA%\Cururu\recuperacao`. Ela **nunca escreve no arquivo do usuário** — quem não salvou continua sem ter salvo.
- Nova opção **Recuperar Consultas** no menu, listando o que foi guardado nos últimos 7 dias, com data, aba, conexão, arquivo de origem e as primeiras linhas do texto, para abrir numa aba ou descartar.
- A cópia sobrevive ao fechamento da aba, e não só ao do aplicativo: fechar aba sem querer é o acidente mais comum.
- Continua **não havendo restauração de sessão**: abrir o Cururu não traz de volta as abas da vez anterior. A recuperação é sempre a pedido.

## 0.1.6 — 31/07/2026

- Menu principal com **Salvar Consulta SQL** e **Carregar Consulta SQL**: a aba visível vai para um arquivo `.sql`, e um `.sql` do disco abre numa aba nova, já amarrada a uma conexão.
- Ao abrir, a codificação do arquivo é descoberta pelo conteúdo — UTF-8, UTF-16 do Bloco de Notas ou o ANSI do SQL Developer —, então acento em comentário não vira caractere estranho.
- Abrir de novo um arquivo já aberto volta para a aba dele, em vez de duplicar o texto.

## 0.1.5 — 31/07/2026

- Na tela **Sobre**, o nome do autor deu lugar ao perfil **@yurijiacomo**, que abre no navegador do sistema — e não dentro da janela do Cururu, que não teria como voltar.

## 0.1.4 — 31/07/2026

- Nova tela **Versões**, no menu do painel de conexões, com o que mudou em cada versão do aplicativo.
- Colunas do grid de resultado com largura ajustável: arraste a divisa no cabeçalho da coluna para alargar ou estreitar.
- Duplo clique na divisa ajusta a coluna à largura do maior valor já carregado.

## 0.1.3 — 31/07/2026

- Lista de conexões com a coluna única *Nome da Conexão*; os detalhes passaram para a dica do mouse.
- A primeira conexão salva já abre marcada, com o formulário preenchido e o cursor na senha.
- Atalho **+ Nova Conexão** logo abaixo da última conexão da lista, já com a primeira cor livre da paleta.
- `AGENTS.md`, registrando onde cada tipo de alteração deve ser verificada: navegador, log ou executável.

## 0.1.2 — 30/07/2026

- Backup das conexões e dos botões rápidos, com **Salvar backup** e **Carregar backup**.
- Menu no título do painel de conexões, reunindo backup, teclas de atalho e Sobre.
- Caixa **Teclas de Atalho**, listando os atalhos de edição do editor SQL.
- Botões rápidos personalizáveis na barra da consulta.
- A caixa **Sobre o Cururu** passa a mostrar a versão do executável em uso.
- As abas de consulta exibem o nome da conexão ao lado do título, sem abreviar: confundir produção com homologação custa caro.
- Cópia do grid em tabela pronta para colar no Microsoft Teams.
- Durações passam a aparecer em segundos, e em minutos a partir de 60 s; milissegundos ficam apenas no log.
- Correção no filtro de objetos: reabrir um filtro que já tem critérios deixa de acrescentar uma linha vazia no fim.

## 0.1.1 — 30/07/2026

- Autocomplete mais rápido: os objetos do próprio usuário são lidos uma vez por sessão e filtrados em memória, sem ir ao banco a cada tecla.
- Objetos de outros donos passam a ser procurados no banco por prefixo, a partir de dois caracteres digitados.
- Divisor arrastável entre o editor SQL e a área de resultado.
- `Tab` aceita a sugestão do autocomplete.
- Seleção retangular por colunas com `Alt+Shift` e as setas.

## 0.1.0 — 29/07/2026

- Primeira versão do Cururu: conexão com Oracle pelo Instant Client embutido, árvore com as seis categorias de objetos do schema conectado, editor SQL com destaque de sintaxe e autocomplete, execução com grid virtualizado, fetch parcial, exportação para CSV e Excel, filtro de objetos, importação de conexões do SQL Developer e do `tnsnames.ora`, senha protegida pelo DPAPI e registro de tudo em log.
