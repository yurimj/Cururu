# Requisitos do Cururu

O que o produto faz, requisito a requisito.

**A numeração é referência de código**: há 106 comentários em 38 arquivos
apontando para "requisito N". Por isso um número nunca é reaproveitado nem
renumerado — requisito que sai vira requisito vazio, não some. (O 9 já não
existe, e é assim que fica.)

O *porquê* das escolhas está em [premissas.md](premissas.md); como trabalhar no
projeto, em [AGENTS.md](AGENTS.md).

### 1. Tela de conexões

Diálogo de nova/seleção de conexão espelhando o do Oracle SQL Developer:

- Lista lateral de conexões salvas com a coluna única *Nome da Conexão* (os detalhes ficam na dica do mouse); a primeira conexão já abre marcada e, logo abaixo da última, há o atalho **+ Nova Conexão**
- Campo **Name** e cor obrigatória de identificação da conexão
- A tela oferece, nesta ordem, uma paleta categórica de cores bem distintas: **azul, laranja, verde, roxo, turquesa, rosa, mostarda, vermelho coral, índigo, marrom, verde oliva e azul claro**; preto, branco, cinza e tons muito claros ou escuros não são aceitos
- Uma conexão nova recebe automaticamente a primeira cor da paleta ainda não utilizada; nomes que indiquem ambiente reservam informalmente **vermelho coral para Produção/PRD**, **laranja para Homologação/HML** e **verde para Desenvolvimento/DEV**
- A cor forte aparece somente em amostras, ícones, bordas ou faixas; o nome da conexão permanece sempre escrito. Se a cor for usada como fundo de texto, deve ser misturada com aproximadamente **82% de branco**, mantendo a fonte em **`#1C1C1C`**
- **Tipo de Banco de Dados**: **Oracle** ou **SQL Server**. O campo comanda o resto do formulário — os campos do banco escolhido aparecem e os do outro somem. Nome, cor, barra de status e os botões do rodapé são comuns aos dois; trocar o tipo no meio do cadastro **não** apaga o que já foi digitado
- **O tipo de banco só se escolhe ao cadastrar.** Numa conexão já salva o campo fica desabilitado, com a explicação na dica do mouse: trocá-lo não seria editar um campo, seria trocar a conexão por outra — outro servidor, outros dados, outro driver. Quem precisa do outro banco cadastra uma conexão nova
- Usuário e senha são **um dado só do perfil**, mostrado nos dois formulários: quem troca de banco não redigita a credencial, e o segredo não é gravado duas vezes

Com **Oracle** selecionado:

- Abas **Informações do Usuário** / **Usuário Proxy**
- **Tipo de Autenticação**, **Nome do Usuário**, **Senha**, opção **Salvar Senha**, **Atribuição** (padrão / SYSDBA / SYSOPER)
- **Tipo de Conexão** (Básico / TNS / Avançado) com abas **Detalhes** / **Avançado**
- **Nome do Host**, **Porta**, e seleção entre **SID** e **Nome do Serviço**

Com **SQL Server** selecionado, os campos são os do SSMS:

- **Nome do Servidor**, num campo só — aceita `servidor`, `servidor\instância` e `servidor,porta`
- **Autenticação**: *Autenticação do SQL Server* ou *Autenticação do Windows*
- **Logon** e **Senha** na autenticação do SQL Server; na do Windows os mesmos campos se chamam **Nome de usuário** e **Senha**, e deixá-los **em branco** vale a identidade de quem está logado no Windows
- Opção **Lembrar senha**
- **Auto-commit**

- Barra de **Status** e botões **Importar, Salvar, Excluir, Testar, Conectar e Cancelar**
- **Excluir** fica habilitado somente para uma conexão salva selecionada, pede confirmação e remove também os filtros associados à conexão
- O botão **Conectar** executa, nesta ordem: conecta ao banco, salva/atualiza o perfil e fecha o diálogo somente após as duas operações concluírem

**Versões atendidas:** Oracle Database 12c (12.1) em diante e **SQL Server 2014 em diante**.

**Estado da conexão com SQL Server:** **Testar** e **Conectar** funcionam. A árvore abre com o nó de bancos e o do SQL Server Agent (requisitos 2 e 36) e a aba de consulta nasce junto (requisito 3). O que ainda não existe é **executar SQL**: grid, cursores, transação, autocomplete e leitura de fonte continuam só do Oracle, e nessas operações uma conexão de SQL Server responde dizendo que a operação ainda não existe para ela.

O Testar aceita o "Nome do Servidor" nas formas do SSMS — `SRV`, `SRV\INSTANCIA`, `SRV,PORTA`, `SRV\INSTANCIA,PORTA`, com `tcp:` opcional na frente, e `.` ou `(local)` para a máquina local. Sem porta explícita, uma instância nomeada é resolvida pelo **SQL Server Browser** (UDP 1434). A espera tem teto de **5 segundos**: o servidor que responde responde em menos de um segundo, e um teto mais alto só faria esperar por uma resposta que não vem.

### 2. Árvore do banco conectado

Somente **6 nós**, sem o restante da árvore do SQL Developer:

- Tabelas
- Views
- Pacotes
- Procedures
- Funções
- Programador

Comportamento igual ao do SQL Developer: expandir o nó lista os objetos; expandir um objeto mostra seu conteúdo; é possível abrir o fonte. No caso de pacote, **Spec** e **Body** separados.

Ao abrir um fonte, arquivo ou outra tela, se a espera passar de um segundo e ainda não houver um aviso no próprio painel, aparece **Carregando…** com o nome do que está sendo aberto. O aviso não bloqueia a janela e some quando a abertura termina ou falha.

O botão direito de uma tabela oferece, nesta ordem, **Exibir Dados**, **Exibir DDL** e **Exibir Estrutura**. As três ações abrem o mesmo visualizador da tabela, com abas internas para alternar sem criar documentos duplicados:

- **Dados** executa a leitura da tabela no grid e usa o fetch parcial já existente.
- **DDL** mostra, somente para leitura e com destaque de sintaxe, o DDL obtido do banco.
- **Estrutura** mostra em grid o nome, o tipo e a nulabilidade de cada coluna.

**Escopo dos objetos:** a árvore mostra **apenas os objetos do schema do usuário conectado**. Não há nó de outros usuários nem listagem de objetos de outros owners.

**A lista vem em páginas.** Cada categoria traz **200 objetos por vez** e termina num **Listar mais…** que traz os seguintes: há schemas com dezenas de milhares de tabelas, e listar tudo de uma vez é lento no banco e impraticável na tela. O número é a opção **Objetos listados por vez** da seção Geral das Configurações (requisito 41), de 10 a 5000, e vale para todas as categorias. Trocá-la vale a partir da próxima página listada, sem reconectar; o **Listar mais…** que já está na tela traz o que o rótulo dele promete.

**No SQL Server a árvore é outra, e tem dois nós:** **Banco de Dados** e **SQL Server Agent** (requisito 36), na mesma ordem em que o SSMS os mostra.

**Banco de Dados** lista os bancos do servidor a que o login tem acesso. Ficam de fora os que estão offline ou em restauração, os que a conta não pode abrir e os **bancos de sistema** (`master`, `tempdb`, `model` e `msdb`) — a árvore mostra onde se trabalha. **Conectar já abre o nó Banco de Dados**, com a lista à vista: quem abre uma conexão de SQL Server vai olhar os bancos, e exigir um segundo clique para isso seria só atrito. Não há paginação nem filtro nesse nível: um servidor tem dezenas de bancos, não as dezenas de milhares de objetos que um schema Oracle pode ter.

**Cada banco expande em quatro pastas**, na ordem e com os nomes do SSMS:

- **Tabelas**
- **Views**
- **Stored Procedures**
- **Functions**, que não lista nada: dentro dela ficam **Funções com Valor de Tabela** e **Funções com Valor Escalar**

A separação das funções é a do SSMS, e não é enfeite: uma entra no `FROM` e a outra não. *Stored Procedures* e *Functions* ficam em inglês nas três línguas porque é assim que o SSMS as escreve, inclusive na versão em português — e a árvore do Cururu é lida ao lado da dele.

**O escopo aqui é o banco, e não o schema.** No Oracle a árvore mostra o schema do usuário conectado e nada mais; um banco do SQL Server tem vários esquemas, e por isso o objeto é identificado por `esquema.nome` — que é o nome do SSMS e, não por acaso, o que se escreve numa consulta. Na tela, o esquema **`dbo` não aparece** (requisito 44). Objetos do próprio servidor (`is_ms_shipped`) ficam de fora. A listagem é paginada como a do Oracle, com o mesmo **Listar mais…**, e o **filtro** do requisito 4 vale aqui como lá.

**As ações são as mesmas do Oracle.** Tabela e view expandem em colunas e oferecem, no botão direito, **Exibir Dados**, **Exibir DDL** e **Exibir Estrutura**, no mesmo visualizador de abas internas. O duplo clique abre o fonte: o DDL, na tabela e na view; o `CREATE` guardado, na procedure e na função — que sai de `sys.sql_modules` exatamente como foi executado, e diz quando o módulo está criptografado em vez de aparecer vazio. Aberto o fonte numa aba, editá-lo e executá-lo é o caminho normal de alterar o objeto; *Compilar* continua sendo do Oracle.

**O DDL de tabela é montado pelo Cururu.** O Oracle tem `DBMS_METADATA.GET_DDL`; o SQL Server não tem equivalente em T-SQL — o *Script Table as* do SSMS é código do próprio SSMS, não do servidor. O script sai na ordem dele: colunas com tipo, `IDENTITY`, coluna calculada e nulidade, chave primária e restrições de unicidade dentro do `CREATE TABLE`; depois, cada um no seu comando, os `DEFAULT`, os `CHECK`, as chaves estrangeiras e os índices — com as colunas incluídas, quando há. Índice que não se escreve com essa sintaxe (columnstore, XML, espacial) fica de fora: um script errado é pior do que um índice de menos.

**O grid de Dados abre sessão própria no SQL Server**, como uma aba de consulta (requisito 7). Ali a conexão atende um pedido de cada vez, e um grid deixado pela metade prenderia a sessão da árvore — expandir um nó ficaria esperando alguém rolar o grid até o fim. DDL e Estrutura continuam na sessão da conexão: são leituras curtas de catálogo.

### 3. Nova Consulta SQL

Botão direito sobre o nó do banco abre uma **Nova Consulta SQL**.

**Conectar já abre uma consulta.** Abrir a conexão na árvore abre junto uma aba de Nova Consulta SQL, com o cursor dentro dela: conectar é o gesto de quem vai consultar, e a aba vazia é o destino em todos os casos menos um. Reexpandir o nó ou usar **Atualizar** não abre outra — só a conexão de verdade abre, e por isso clicar não empilha abas vazias. Havendo arquivo mandado pelo Windows à espera de uma conexão (requisito 12), é ele que abre no lugar da aba vazia.

Vale para os dois bancos. **No SQL Server a aba nasce apontada para o primeiro banco da lista** — o mesmo primeiro nome que aparece sob **Banco de Dados**.

**Uma consulta pode nascer num banco escolhido.** Sobre um banco da árvore, três gestos abrem a aba **naquele banco** em vez de no banco da conexão: o **botão direito → Nova Consulta SQL**, o **atalho de nova consulta** (`Ctrl+T` de fábrica, configurável pelo requisito 32) com o ponteiro do mouse em cima dele, e o botão da barra com o banco selecionado. O ponteiro tem precedência sobre a seleção — apontar para um banco e pedir a consulta é o gesto direto, e não deve exigir um clique antes.

Na barra da aba, no lugar em que o Oracle mostra a Saída DBMS (requisito 8), o SQL Server traz uma **lista de bancos**: escolher outro passa a aba a executar nele. **Aqui os bancos de sistema aparecem**, ao contrário da árvore — consultar o catálogo do `master` é trabalho legítimo de quem administra, e escondê-lo impediria justamente isso. A troca vale **só para aquela aba**, como o `USE` do SSMS vale só para a janela em que foi dado, porque cada aba tem a sua sessão (requisito 7). Recusada a troca — sem permissão no banco escolhido, por exemplo —, a lista volta a mostrar o banco em que a aba realmente está. Durante uma execução a lista fica travada.

**Com transação aberta a troca é recusada**, dizendo o que falta: o SQL Server não aceita `USE` dentro de transação, e confirmar ou desfazer por conta própria para poder trocar seria decidir pelo usuário o destino do trabalho dele. É por isso que uma conexão nova de SQL Server nasce com **auto-commit ligado** (requisito 7).

**Um `use` digitado no editor também troca o banco da aba** — é o mesmo comando que o SSMS manda —, e a lista da barra acompanha.

O editor tem **autocomplete**: ao digitar `TBL_`, sugere todos os objetos que começam com esse prefixo, de forma **insensível a maiúsculas/minúsculas** (`TBL_` e `tbl_` produzem o mesmo resultado). **Vale nos dois bancos.**

No SQL Server o alcance é o **banco em que a aba está** — trocar o banco na lista da barra troca o que é sugerido. Vêm tabelas, views, sinônimos, procedures e funções do catálogo daquele banco, com a mesma regra de lugar do Oracle e o mesmo teto de 300 nomes a partir de dois caracteres digitados. O **esquema** aparece ao lado do tipo quando não é `dbo`, no lugar em que o Oracle mostra o dono, e os objetos de `dbo` vêm primeiro, por serem os que se alcançam escrevendo só o nome da tabela.

Quem responde é a sessão **da conexão**, e não a da aba: no SQL Server uma conexão atende um pedido de cada vez, e perguntar à sessão da aba faria a tecla digitada esperar a consulta que estivesse correndo ali. Por isso a consulta ao catálogo leva o banco no nome (`[Vendas].sys.objects`).

##### O nome de três partes alcança os outros bancos

Uma consulta de SQL Server não está presa ao banco da aba: `BANCO_EXEMPLO..tbl_nota` é nome legítimo, e o autocomplete acompanha o nome enquanto ele é escrito.

- **O banco aparece na lista** onde cabe uma tabela. O que se digita depois de `FROM` tanto pode ser o começo do nome de uma tabela quanto o de um banco, e sem o banco ali não haveria como chegar ao nome de três partes. Ele vem depois dos objetos, rotulado como `Banco de dados`. Depois de **`use`** a lista é só de bancos: é o comando que troca o banco da aba, e ali não cabe objeto nenhum.
- **Escrito o banco, as sugestões passam a ser dele.** Em `BANCO_EXEMPLO..tbl` vêm as tabelas daquele banco que começam com `tbl` — e só tabelas, porque o lugar continua sendo lugar de tabela. Vale igual com o esquema escrito (`BANCO_EXEMPLO.dbo.tbl`) e com o esquema sozinho, no banco da aba (`dbo.tbl`); nos dois casos o esquema corta a lista, para dela não sair o que aquele nome não alcança.
- **As colunas seguem a tabela.** O apelido de `FROM BANCO_EXEMPLO..tbl_nota n` resolve `n.` para as colunas daquele banco, e não para as de uma tabela de mesmo nome no banco da aba (requisito 23).

A primeira parte só é tratada como banco quando **é** um banco do servidor — a lista já está em memória, lida uma vez por aba. Sem essa conferência, cada `x.y` digitado viraria uma consulta a `[x].sys.objects`, que não existe e responde com erro em vez de lista vazia.

**O alcance do autocomplete é maior que o da árvore, de propósito.** A árvore mostra o schema do usuário conectado (requisito 2), mas uma consulta pode ler qualquer objeto a que o usuário tenha acesso — inclusive de outro dono, alcançado por sinônimo público. "Tudo que comece com" quer dizer tudo que dá para consultar, e não só o que é do próprio usuário.

Como isso é feito sem ficar lento:

- Os objetos do **próprio usuário** são lidos uma vez por sessão e ficam em memória — filtrar a cada tecla não vai ao banco.
- Os de **outros donos** são procurados no banco por prefixo, com predicado direto na coluna (para o índice funcionar), teto de 300 linhas e só a partir de **dois caracteres** digitados.
- Na lista, os objetos do próprio usuário vêm primeiro, depois os sinônimos públicos, depois o resto. Os de outro dono mostram o dono ao lado do tipo.
- O sublinhado é literal também aqui: `EXEMPLO_` procura o caractere, não um curinga.

#### Onde cada sugestão aparece

Sugerir em todo lugar atrapalha mais do que ajuda: quem escreve `SELECT nome` não quer a lista de tabelas do banco por cima do texto. A sugestão é oferecida onde a consulta pede aquele tipo de nome:

- **Tabela, view e sinônimo** apenas depois de **`FROM`**, **`JOIN`**, **`UPDATE`** e **`DELETE`** — e na continuação da lista do `FROM`, depois da vírgula, que é o mesmo lugar (`FROM cliente c, ped…`).
- **Coluna** depois de **`WHERE`**, **`AND`**, **`OR`** e de **`(`**, que são os lugares onde se escreve condição. As colunas oferecidas são as das tabelas citadas no comando — todas as do `FROM`, as de cada `JOIN` e a do `UPDATE`. Com mais de uma tabela, cada coluna mostra de qual delas veio, porque duas tabelas costumam ter colunas de mesmo nome.
- **Pacote, procedure e função** continuam sendo sugeridos ao digitar em qualquer lugar: é o que se chama no meio de um bloco PL/SQL (requisito 13), e não há palavra-chave que anuncie um.
- **Banco** depois de **`use`**, e junto das tabelas nos lugares em que cabe uma (SQL Server): ali as duas coisas podem estar começando a ser escritas.
- Depois do nome de uma package e do ponto — por exemplo, **`DEM.`** — o autocomplete oferece as procedures e funções públicas declaradas nela. Vale em qualquer lugar em que o ponto apareça, `EXECUTE DEM_INT.` incluído. O mesmo ponto depois de um alias ou nome presente no `FROM` continua oferecendo as colunas da tabela.
- A leitura das rotinas é **sempre pelo `object_id`** da package, tirado antes de `USER_OBJECTS` com `LIKE` e `FETCH FIRST` — a forma da consulta pesa mais que o filtro, e neste dicionário `object_name = :nome` custa quinze segundos enquanto o `LIKE` com prefixo na mesma visão custa um — que é leitura de índice por nome. Não é detalhe de implementação, é o que faz a lista abrir: as visões `USER_PROCEDURES` e `USER_ARGUMENTS` juntam `obj$`, `procedureinfo$` e `argument$` com lógica de edição no meio, e um filtro por nome não chega até o índice. Medido em DEM_INT, com 17 subprogramas: por nome, `USER_ARGUMENTS` levava 20,8 s e `USER_PROCEDURES` 15,6 s; pelo `object_id`, 28 ms. O ganho maior nem é no pacote — é em cada nome digitado que **não** é pacote, porque o editor manda conferir todo nome com ponto atrás.
- Se a leitura das espécies falhar, a lista **aparece com tudo como procedure**, em vez de não aparecer: o que se perde é a etiqueta, e a falta fica no log. O log também diz qual das duas consultas demorou, quando alguma passa de meio segundo.
- O resultado fica guardado **por sessão**, e não por aba: pedir `PKG.` numa aba nova, ou abrir o pacote na árvore, aproveita a leitura já feita. Recompilar invalida, como já invalidava as colunas.

**Dentro de aspas simples não há sugestão nenhuma.** Ali se escreve o valor de um dado, não o nome de um objeto. A mesma regra vale para comentário — e é obrigatório que valha: uma aspa solta dentro de um comentário faria todo o resto do texto parecer literal, e o autocomplete emudeceria sem motivo.

O escopo é o **comando**, não a aba: numa aba com vários comandos separados por `;`, as tabelas de um `SELECT` não são oferecidas no `UPDATE` de baixo.

A forma alternativa de literal do Oracle, `q'[…]'`, não é reconhecida. O pior que acontece é a sugestão aparecer dentro dela.

### 4. Filtro de objetos

**Todas as categorias podem ser filtradas, nos dois bancos** — as seis do Oracle e as cinco do SQL Server, não apenas Tabelas e Views —, e também os **trabalhos do SQL Server Agent** (requisito 36), que num servidor cheio deles são o mesmo problema de achar uma tabela. O nó exibe a indicação de filtro ativo (ex.: `Tabelas (Filtrado)`, `SQL Server Agent (Filtrado)`).

O filtro dos trabalhos é guardado na mesma chave do **Programador**, que é a categoria de job do Oracle: uma conexão é de um banco só, então as duas nunca disputam o lugar, e a tela já chama o alvo de `Job` nas duas.

O diálogo separa os critérios em **Incluir** e **Excluir**. As inclusões são combinadas por **OU**; as exclusões são reunidas como `AND NOT`, evitando que dois `NOT LIKE` façam quase todos os objetos passarem. O alvo é fixo e acompanha a categoria selecionada (`Tabela`, `View`, `Pacote`, `Procedure`, `Função` ou `Job`). Cada linha possui operador, valor e lixeira; novas linhas são acrescentadas dentro do grupo correspondente.

Opções extras, exibidas apenas onde fazem sentido:

- **Excluir objetos gerados pelo banco** — usa a coluna `GENERATED` de `USER_OBJECTS`; não vale para o Programador, e não existe no SQL Server, onde `is_ms_shipped = 0` já vale sempre.
- **Incluir sinônimos** — só para Tabelas e Views.

Fica de fora o "Sobrescrever Filtro de Esquema" do SQL Developer: ele existe lá porque há filtro no nível de esquema, e o Cururu trabalha apenas com o schema conectado.

O filtro é acessível de duas formas: **botão direito** no nó da categoria, ou o **botão de filtro** da barra do painel de conexões, que age sobre a categoria selecionada.

**O filtro é guardado em disco**, por conexão e por categoria, em `%APPDATA%\Cururu\filtros.json`. Sobrevive a fechar o aplicativo, porque é configuração da conexão — como a cor. Excluir a conexão apaga junto os filtros dela.

**Por conexão, e não por banco.** No SQL Server a mesma categoria aparece embaixo de cada banco do servidor, e todas usam o critério guardado para aquela conexão. Um nó de outro banco que já esteja aberto passa a mostrar o rótulo de filtrado depois de recarregado.

Isso não conflita com o requisito 12: o que **não** se guarda ali é o conteúdo das abas de consulta SQL.

#### No SQL Server o critério é o mesmo, o SQL é outro

O filtro age sobre o **nome sem o esquema** — o mesmo que se digita e o mesmo que a árvore mostra. Três diferenças em relação ao Oracle, e nenhuma delas é de dialeto:

- **O `LIKE` do T-SQL tem um curinga a mais:** o colchete abre classe de caracteres. `TB[01]` é escapado, para valer como o nome literal em vez de casar com `TB0` e `TB1`.
- **A caixa depende do collation do banco**, e não do valor: `Cliente` e `cliente` são dois nomes possíveis. Sem distinção de caixa, os dois lados sobem para maiúsculas; com ela marcada, a comparação sai em collation binário — sem isso, um banco *case-insensitive* ignoraria o pedido em silêncio.
- **A situação não entra:** no SQL Server não existe objeto inválido esperando recompilação. Um critério sobre situação é descartado em vez de virar SQL sobre uma coluna que não existe.

#### Divergência proposital do SQL Developer: o sublinhado no LIKE

No `LIKE` do Oracle, `_` é curinga de um caractere. **No Cururu, `_` é literal**; só `%` continua curinga.

Motivo: filtrar por `%_REL_%` deve trazer nomes com sublinhado antes e depois de `REL` — `S_REL_020` —, e não `SREL020`. Internamente o valor é escapado e a consulta usa `ESCAPE`.

Nos operadores **começa com** e **termina com**, o texto digitado é inteiramente literal, inclusive `%`.

#### Limitação conhecida: nomes criados entre aspas

Sem marcar "Distinção entre Maiúsculas e Minúsculas", a comparação sobe **apenas o valor procurado** para maiúsculas, deixando a coluna intocada.

Motivo: `UPPER(coluna)` impede o Oracle de usar o índice do dicionário — numa base grande isso levou uma filtragem de 2 s para 25 s. Como nomes de objeto no Oracle são maiúsculos salvo quando criados entre aspas, a troca vale a pena.

Consequência: um objeto criado como `"minha_tabela"` não é encontrado por busca sem distinção de caixa. Para achá-lo, marque a distinção e digite o nome como ele é.

### 5. Execução

- `CTRL+Enter` ou botão **▶ (Play)**: executa **apenas o trecho selecionado**; sem seleção, executa tudo que está na tela.
- `F5`: mesma regra de seleção, porém em **modo script**.
- Os atalhos de cliente **`EXEC`** e **`EXECUTE`** são aceitos como no SQL Developer. Antes do envio ao Oracle, `EXECUTE procedimento` é convertido internamente em `BEGIN procedimento; END;`.
- O **`SHOW ERRORS`** do SQL*Plus também é aceito no Oracle, como no SQL Developer — mandado ao banco, ele volta `ORA-00900`. Sozinho, mostra os erros do último objeto que um `CREATE` da aba compilou; com tipo e nome (`SHOW ERRORS PROCEDURE DONO.NOME`), os daquele objeto. A lista sai no formato do SQL*Plus, `LINE/COL` e `ERROR`, com a linha contada no objeto. Vale a abreviação (`SHO ERR`), e o comando termina na quebra de linha, com ou sem `;`.
- Em modo script, **`SET SERVEROUTPUT ON|OFF`** liga ou desliga a Saída DBMS da aba, e **`WHENEVER SQLERROR EXIT ... ROLLBACK`** interrompe no primeiro erro e desfaz a transação pendente, como no SQL Developer. As linhas das diretivas são preservadas na contagem dos erros. Os outros comandos do SQL*Plus (`SET TIMING`, `PROMPT`, `SPOOL`…) continuam indo ao banco como estão.
- **Um `CREATE` que compila com erro não sai como executado.** O Oracle aceita o comando, grava o objeto **inválido** e só avisa (`ORA-24344`) — o driver entrega isso como aviso, e não como erro. A mensagem diz que o objeto ficou inválido, em vermelho, e cada erro entra no painel de mensagens como os do botão Compilar (requisito 13). O script continua, como no SQL Developer.
- **Um script que para no meio mostra o que já rodou**, e o cursor vai para o comando que falhou (requisito 18). Um `CREATE` que valeu antes da falha já está no banco, e um DML que rodou está pendente: a tela não pode mostrar só o erro do comando seguinte. O log registra em que comando o script parou.
- O **`*` solto numa lista de seleção que tem outros itens** é qualificado antes do envio: `SELECT id_registro, * FROM financeiro` sai como `SELECT id_registro, financeiro.* FROM financeiro`. MySQL, PostgreSQL e SQL Server aceitam essa forma; o Oracle não, e responde `ORA-00923`. Com mais de uma tabela no `FROM`, o asterisco vira o de cada uma, na ordem em que aparecem.

  **A troca só alcança texto que o Oracle já recusa**, e é isso que a torna segura: um `*` colado a `SELECT` ou a uma vírgula de um lado, e a uma vírgula ou a `FROM` do outro, não tem operando de nenhum lado e portanto não pode ser multiplicação. `preco * 2` e `COUNT(*)` ficam de fora por construção, e `SELECT * FROM tabela` — que é válido — segue intocado. Nenhuma consulta que funciona muda de comportamento.

  Na dúvida, não se mexe: subconsulta, view em linha ou qualquer coisa que a análise não entenda por inteiro fazem o texto seguir como está, e o erro do Oracle aparece como apareceria. **Toda vez que o texto enviado difere do digitado, isso fica no log** — uma consulta que roda diferente do que está na tela não pode depender de adivinhação.

#### No SQL Server o lote é o `GO`, não o `;`

As duas regras acima — `EXEC` e o asterisco solto — são do Oracle, e o texto de uma aba de SQL Server segue intocado para o servidor.

O que muda é a **separação**: no T-SQL o `;` separa instruções *dentro* do lote, e quem encerra o lote é a linha com **`GO`**, que é marca do cliente e não chega ao servidor. Cortar no `;` partiria ao meio todo `IF … BEGIN … END`, todo `WHILE` e toda variável declarada e usada logo abaixo. É a mesma regra do SSMS, então um script colado de lá roda como está. O `GO` é reconhecido sozinho na linha, com contagem (`GO 5`, cuja repetição não é feita) ou com comentário atrás, e não dentro de literal nem de comentário.

Um lote pode devolver **vários conjuntos de resultados** (`select a; select b;`), e a tela mostra todos, empilhados (requisito 6). O fetch parcial (requisito 10) fica no **primeiro**, que é o único que dá para deixar aberto: alcançar o segundo obriga a ler o primeiro até o fim. Os conjuntos seguintes vêm até o tamanho do bloco, e quando o servidor mandou mais do que isso a mensagem diz quantas linhas o grid está mostrando — o número de linhas obtidas não pode prometer o que o grid não tem.

**O erro no fim do lote não apaga o que já veio.** O servidor manda os conjuntos de resultados e só então a falha: uma procedure de depuração mostra dez grids e estoura no último comando, e um lote que rode com auto-commit desligado pode terminar em `Uncommittable transaction is detected at the end of the batch` depois de já ter mandado tudo. Descartar essa saída perderia informação que não volta — ela sobe junto com o erro, e a aba mostra os resultados **e** a falha, que continua sendo falha: mensagem em vermelho, código do servidor, salto até a linha (requisito 18) e registro no log. Num script de vários lotes vale o mesmo: os lotes que rodaram aparecem, e a execução para no que falhou.

### 6. Grid de resultado

Execuções via Play / `CTRL+Enter` exibem o resultado em **grid**, com opção de **copiar incluindo o cabeçalho** das colunas.

**Cada conjunto de resultados ganha o seu grid, empilhados na ordem em que o banco os devolveu** — a mesma pilha do SSMS. Um lote de T-SQL devolve vários (`exec` de uma procedure de depuração devolve dezenas) e uma aba do Oracle com várias consultas soltas devolve um por comando; mostrar só o último escondia dados que já tinham vindo do banco e custado a viagem. Com um conjunto só, o grid ocupa a área inteira, como sempre ocupou; com mais de um, cada grid fica na sua altura — até dez linhas à mostra, rolando por dentro se tiver mais — e a pilha rola por fora.

Copiar, exportar e o menu de contexto agem sobre o grid em que se mexeu por último. O fetch parcial (requisito 10) continua sendo de cada grid: no Oracle todos podem pedir mais linhas, cada um com o seu cursor; no SQL Server só o primeiro do lote.

Em uma consulta nova, a área de resultado e seu divisor ficam ocultos para o editor ocupar todo o espaço disponível. Eles só aparecem quando houver um grid de resultado ou uma saída de script para exibir.

**Uma execução que falha também os oculta.** O grid que ficaria na tela é o da execução anterior: já não responde pelo que está no editor, e disputava o olho com a mensagem de erro, espremida no painel de baixo. O editor toma o espaço, o erro fica sozinho no painel de mensagens (requisito 18), e o cursor daquele grid é fechado, como seria pelo próximo resultado. O resultado seguinte traz a área de volta, na divisão em que o divisor a deixou. A exceção é o lote que devolveu grids **antes** de falhar — uma procedure que mostra o que carregou e estoura no fim: esses grids são desta execução, e continuam à mostra, com o erro por último.

- Clicar em uma **célula** seleciona somente aquela célula; `Shift` e `Ctrl` permitem selecionar outras linhas da mesma coluna.
- Clicar no **cabeçalho de uma coluna** seleciona todos os valores carregados daquela coluna e mantém a ordenação pelo cabeçalho.
- `Ctrl+C`, **Copiar**, **Copiar com cabeçalho** e a cópia para Teams respeitam as colunas selecionadas: uma única coluna selecionada nunca copia a linha inteira.
- A duração mostrada ao usuário usa **segundos** e, a partir de 60 segundos, **minutos e segundos**; milissegundos permanecem apenas nos logs técnicos.

### 7. Transação

A ferramenta respeita o modo auto-commit. Com auto-commit desligado, **não** há commit implícito após um update. Devem existir botões de **commit** e **rollback** próximos ao Play.

Cada **aba de consulta tem a sua própria sessão** no banco, aberta ao abrir a aba. A transação é, portanto, **da aba**: commit, rollback e o interruptor de auto-commit valem só para o que aquela aba executou, e um DML sem commit numa aba não é visto pelas outras. Fechar a aba encerra a sessão dela — o que estiver sem commit é desfeito, já que fechar não pergunta nada (requisito 12). O mesmo vale para uma sessão que precisou ser reaberta (requisito 35): a sessão nova é outra sessão do banco, e a transação que estava aberta na anterior ficou para trás.

**O que está pendente fica visível.** Com o auto-commit desligado, a mensagem de cada comando termina em *"— transação aberta, falta commit"*, e o botão **✔** fica aceso na cor de aviso enquanto houver o que confirmar naquela aba. Antes só existia o aviso do caso oposto — o "(commit automático)" —, e a pendência era dita por omissão dele, que é a pior forma de dizer alguma coisa: fechar a aba desfaz tudo sem perguntar, e no SQL Server isso alcança também o DDL. Um `alter procedure` executado com sucesso e nunca confirmado desaparece inteiro, e quem vem do Oracle — onde o DDL confirma sozinho — não tem por que desconfiar disso. O ✔ acende só quando há o que perder: comando acende, consulta não; ligar o auto-commit apaga, porque a troca confirma o pendente.

**No Oracle, DDL não pede commit.** `CREATE`, `ALTER`, `DROP`, `TRUNCATE`, `RENAME`, `COMMENT`, `GRANT`, `REVOKE` e os demais DDL são confirmados pelo próprio Oracle, antes e depois de rodar — e o commit de antes leva junto o que estava pendente na aba. A mensagem diz *"DDL: o Oracle confirma sozinho"*, e o ✔ apaga em vez de acender. `ALTER SESSION` e `ALTER SYSTEM` começam como DDL e não são: não mexem na transação. `COMMIT` e `ROLLBACK` digitados no editor valem o mesmo que os botões; `ROLLBACK TO SAVEPOINT` desfaz só até o ponto marcado, e a transação continua aberta. Bloco PL/SQL acende o ✔ como o DML: o Cururu não tem como saber o que ele fez lá dentro, e dizer que sobrou o que não sobrou custa um clique, enquanto o contrário custa o trabalho.

> Observação técnica: no Oracle o auto-commit é um comportamento do cliente, não uma configuração do servidor. É uma opção **por aba de consulta**, **desligada por padrão** (mesmo padrão do SQL Developer). Cada aba tem sessão própria para que uma consulta pesada numa aba não bloqueie as outras nem o autocomplete — o Oracle serializa as chamadas de uma mesma sessão, uma de cada vez.

#### No SQL Server o auto-commit é do servidor

Ali ele é o `SET IMPLICIT_TRANSACTIONS`, e não uma decisão do cliente. Com ele desligado (transações implícitas ligadas), **todo** comando abre transação — `select` inclusive. Daí três diferenças:

- **Uma conexão nova de SQL Server nasce com auto-commit ligado**, ao contrário da de Oracle: cada banco nasce com o padrão da ferramenta de referência dele, e o do SSMS é esse. Sem isso, trocar de banco na barra da aba seria recusado depois de qualquer comando (requisito 3).
- **Ligar o auto-commit confirma a transação aberta**, como fazem os drivers padrão. Deixá-la aberta faria o interruptor mentir: a tela diria "auto-commit" e nada seria confirmado, porque a transação aberta engole os comandos seguintes.
- **Quem diz se sobrou transação é o servidor.** Depois de cada comando o Cururu lê o `@@TRANCOUNT`, na mesma ida em que lê o `@@ROWCOUNT`. Assim o DDL — que ali é transacional, e um `alter procedure` sem commit se perde ao fechar a aba — acende o ✔ como o DML; um `commit` digitado apaga; e um `begin tran` acende mesmo com o auto-commit ligado, onde antes a mensagem dizia *"(commit automático)"* de uma transação que continuava aberta.
- **Commit e rollback sem nada pendente não são erro**, como no Oracle. Ali um `COMMIT` sem transação aberta é erro do servidor; o botão é o mesmo nos dois bancos, e apertá-lo não pode devolver erro num e silêncio no outro.

A **sessão da conexão** — a que atende árvore e autocomplete — abre sempre com auto-commit ligado, qualquer que seja o do perfil: ela nunca executa SQL do usuário, e uma transação aberta ali seguraria lock e log sem que ninguém tivesse pedido nada.

### 8. Saída DBMS

Janela de **Saída DBMS** (`DBMS_OUTPUT`), **minimizada por padrão**, aparecendo somente quando ativada — igual ao SQL Developer. Inclui controle de **tamanho do buffer**.

**É só do Oracle.** O SQL Server não tem equivalente, e numa aba dele o lugar da marcação da Saída DBMS é ocupado pela **lista de bancos** (requisito 3).

### 10. Fetch parcial

Consultas sem `where` são permitidas: a ferramenta traz apenas um bloco de registros por vez (fetch), como o SQL Developer.

O **tamanho do fetch fica visível no rodapé** e pode ser alterado facilmente para trazer mais registros quando desejado.

**Vale nos dois bancos**, com uma diferença que aparece no SQL Server: largar um grid pela metade — executando outra coisa, trocando de banco, fechando o resultado — obriga a **ler o que sobrou** antes do próximo comando, porque no TDS a conexão atende um resultado de cada vez e não existe o gesto de fechar um cursor pela metade. A leitura tem prazo de três segundos; passado ele, a conexão é reaberta (custa um login) e a tela avisa que a sessão é outra, como no requisito 35.

### 11. Abas

Várias consultas SQL abertas **simultaneamente**, em abas.

- **`Ctrl+T`** abre uma nova consulta na conexão que estiver conectada e selecionada na árvore. Sem uma conexão aberta selecionada, não faz nada e não tenta conectar sozinho.
- **`Ctrl+W`** fecha a aba visível, seguindo o comportamento usual de editores e navegadores.
- **Arrastar uma aba muda o lugar dela na barra**, como nos navegadores: com cinco abas abertas, a quarta vai para a primeira posição arrastada até lá. A aba segue o ponteiro e troca de lugar com cada vizinha que ultrapassa, qualquer que seja a largura das duas. Com mais abas do que cabem na barra, a aba que encosta na borda faz a barra rolar sozinha até as que estavam escondidas.
- A ordem da tela é a ordem das abas: **`Ctrl+PageUp`** e **`Ctrl+PageDown`** andam por ela, e fechar a aba visível mostra a primeira da barra.
- O arrasto só começa depois de o ponteiro andar alguns pixels, então o clique continua só selecionando a aba. Apertar o **✕** e escorregar não arrasta nada.

### 12. Salvar e fechar

**Abrir Recente**, no menu principal após **Abrir Consulta SQL**, lista até 15 arquivos abertos com sucesso, do mais recente para o mais antigo. O histórico persiste ao fechar o aplicativo e não repete caminhos, mesmo com diferenças de maiúsculas ou separadores. Abrir pelo Windows também alimenta a lista; reabrir move o arquivo para o topo. Sem histórico, a opção fica desabilitada. Sem conexão, a lista pode ser consultada, mas seus arquivos ficam desabilitados até haver uma conexão para receber a aba.

É possível **salvar a consulta em arquivo `.sql`** e **abrir um `.sql` do disco** numa aba nova. O menu principal traz, nesta ordem, **Salvar Consulta SQL (Ctrl+S)**, **Salvar Consulta SQL Como…** e **Abrir Consulta SQL (Ctrl+O)**. Salvar age sobre a aba visível; abrir amarra a aba nova à conexão selecionada na árvore ou, na falta dela, à da aba visível (requisito 26). Abrir de novo um arquivo já aberto volta para a aba dele, em vez de criar uma segunda cópia.

**A aba fica amarrada ao arquivo.** O caminho só é perguntado quando a aba ainda não tem arquivo — porque nunca foi salva nem veio de um. Dali em diante, Salvar grava direto por cima, como em qualquer editor. Quem quer escolher outro lugar usa **Salvar Como**, que pergunta sempre e passa a valer como o novo arquivo da aba. Salvar em outro caminho também muda a origem mostrada na tela de recuperação.

Os dois atalhos valem na **janela inteira**, e não só com o editor em foco: **`Ctrl+S`** salva a aba visível e **`Ctrl+O`** abre um arquivo numa aba nova.

O arquivo é gravado em UTF-8 sem BOM. Na leitura, a codificação é descoberta pelo conteúdo — UTF-8 com ou sem BOM, UTF-16 do Bloco de Notas, ou o ANSI/Windows-1252 que o SQL Developer gravava —, porque ler tudo como UTF-8 estragaria os acentos dos comentários.

Porém, ao fechar a ferramenta, ela **fecha tudo direto, sem perguntar nada** sobre salvar.

**Nada de abas restauradas na abertura seguinte.** Abrir o Cururu não traz de volta o que estava aberto antes: quem fechou uma aba, fechou.

#### Abrir pelo Windows

Um `.sql` também abre **de fora do Cururu**: duplo clique no Explorer, **Abrir com**, ou o arquivo arrastado até o `cururu.exe`. O Windows entrega o caminho na linha de comando, e o arquivo abre numa aba nova pelas regras do **Abrir Consulta SQL** — inclusive a de voltar para a aba de um arquivo que já está aberto.

- **Com o Cururu já aberto, o arquivo vai para a janela que está na tela**, e ela vem para a frente. Cada duplo clique inicia um processo novo, e sem essa entrega cada arquivo abriria outro Cururu — outra árvore, outras conexões, a senha pedida de novo — para mostrar um texto que cabia numa aba. Vários arquivos selecionados e abertos juntos caem todos na mesma janela.
- **Abrir o Cururu sem arquivo continua abrindo outra janela.** A entrega é do arquivo, e não uma trava de instância única: quem trabalha com duas janelas lado a lado continua podendo. Vale também depois de o Windows atualizar o WebView2 com um Cururu aberto: as janelas dividem a mesma pasta de dados do WebView2, que só uma versão usa por vez, e a janela nova entra na versão da que já está aberta — em vez de ficar esperando para sempre, sem aparecer.
- **Sem conexão para amarrar a aba (requisito 26), o arquivo espera.** A barra de status pede uma conexão, e o arquivo abre na primeira que for selecionada. Ao conectar, ele abre **no lugar** da aba vazia do requisito 3: quem mandou abrir um arquivo quer ver o arquivo.

**O Cururu não se faz programa padrão sozinho.** O instalador o põe no **Abrir com** do `.sql`, e torná-lo o padrão é escolha do usuário — **Abrir com → Escolher outro aplicativo → Cururu**, com **Sempre**, ou pelas Configurações do Windows. O Windows reserva essa escolha a quem usa, e um instalador que a tomasse trocaria em silêncio o SSMS de quem abre `.sql` nele. Desinstalar tira o Cururu da lista.

Na **pasta portátil** não há instalador para registrar nada, e o executável não se registra sozinho (premissas.md explica por quê): o caminho é o mesmo **Abrir com**, apontando o `cururu.exe` em **Procurar mais aplicativos neste PC**.

#### Cópia de recuperação

O que existe — e é diferente de salvamento automático — é uma **cópia de recuperação**, guardada sozinha, para o conteúdo não se perder num travamento ou num fechamento sem querer.

Regras:

- A cópia **nunca toca no arquivo do usuário**. Uma consulta aberta de `relatorio.sql`, ou já salva nele, continua com o arquivo intocado no disco: a cópia vai para `%APPDATA%\Cururu\recuperacao\`. Quem não salvou continua sem ter salvo.
- A gravação acontece **dois segundos depois da última tecla**, e só quando o texto mudou desde a anterior. Textos acima de **5 MB** não são copiados — reescrever o documento inteiro a cada pausa seria o único caso em que isto custaria algo.
- A cópia **sobrevive ao fechamento da aba**, e não só ao do aplicativo: fechar aba sem querer é o acidente mais comum.
- As cópias duram **no máximo 7 dias**, contados da última gravação, e no máximo **200 entradas**. A limpeza é feita **na subida do aplicativo**, não por temporizador.
- Nada disso aparece na tela enquanto se trabalha: não há aviso, ícone ou indicação de "salvo". Silencioso é silencioso.
- A recuperação é **manual e sob demanda**, pela opção **Recuperar Consultas** do menu principal, que lista o que existe — data, aba, conexão, arquivo de origem e as primeiras linhas do texto — e permite abrir numa aba nova ou descartar.
- O texto é guardado **em claro**, e não cifrado pelo DPAPI. É decisão consciente: recuperação que só o próprio aplicativo consegue ler é pior recuperação, porque o dia em que ela importa é justamente o dia em que o Cururu pode não abrir. Consequência aceita: um script que contenha credencial escrita no meio dele fica legível na pasta do perfil do usuário por até 7 dias.

### 13. Edição e compilação de PL/SQL

Pacotes, procedures e funções **não são somente leitura**. O comportamento é o mesmo do SQL Developer: abrir o fonte, **editar e compilar**, com **painel de erros de compilação** apontando linha e mensagem.

A linha de cada erro é a **do editor**. O Oracle conta a partir da linha do tipo (`PROCEDURE …`); o Cururu soma o que vem antes dela — comentário de cabeçalho, `CREATE OR REPLACE` sozinho na linha — e a linha em que a seleção começa, e o clique na mensagem leva ao erro. Os erros são lidos de `ALL_ERRORS`, com o dono escrito no `CREATE`, para valer também em `SCHEMA.OBJETO`.

**É só do Oracle**, por ser PL/SQL. Numa aba de SQL Server o botão **Compilar** não aparece na barra, e o atalho (Ctrl+Shift+C) responde dizendo por que não faz nada — um botão que só sabe dar erro não deve ocupar a barra, mas uma tecla que emudece sem explicação é pior ainda.

### 14. Cancelar consulta em andamento

Deve existir botão **Parar** ao lado do Play para cancelar uma execução em andamento. **Vale nos dois bancos.**

Toda execução de banco roda **fora da thread de interface** — a tela nunca congela enquanto uma consulta corre.

No Oracle a interrupção é o `break_execution` do driver, e a sessão continua a mesma. **No SQL Server ela custa a conexão**: o driver não expõe o sinal de atenção do TDS, que é o que o SSMS manda, e o que resta é largar o socket — o servidor aborta a consulta do mesmo jeito, como faria um `KILL`. A conexão volta em seguida, com o banco e o auto-commit no lugar, e a tela avisa que a sessão é outra, pelo mesmo caminho do requisito 35: o que estava sem commit ficou para trás.

### 15. Blocos PL/SQL e bind variables

- Suporte a **blocos PL/SQL anônimos** (`DECLARE` / `BEGIN` / `END;`), incluindo o uso da barra (`/`) como terminador de comando no modo script.
- O **`CREATE` de procedure, function, package, trigger, type, library e Java** também só termina na barra: o `;` dentro do corpo não encerra o comando. Vale com ou sem `OR REPLACE` e com os modificadores que o Oracle aceita antes do tipo — `EDITIONABLE`, que o SQL Developer escreve em todo fonte que exporta, `NONEDITIONABLE` e o `AND COMPILE` do Java. Quem decide é a palavra do tipo, onde quer que ela caia: cortar o corpo no primeiro `;` mandaria o cabeçalho sozinho ao banco, que o grava inválido por cima do objeto bom.
- Suporte a **bind variables** (`:parametro`), com janela solicitando os valores antes da execução, como no SQL Developer.

**No SQL Server o marcador é o mesmo, `:parametro`** — e não o `@parametro` do T-SQL, que já significa outra coisa ali: é a variável local declarada com `DECLARE`. Tratá-lo como bind faria todo script com `DECLARE @x int` parar para perguntar um valor que o próprio script já dá. O `:nome` não é sintaxe válida em lugar nenhum do T-SQL, então não há o que confundir, e quem escreve nos dois bancos escreve do mesmo jeito. A tradução para os parâmetros do servidor acontece antes do envio.

Blocos PL/SQL e a barra (`/`) são do Oracle; no SQL Server o equivalente é o lote separado por `GO` (requisito 5).

### 16. Exportar o grid

Exportação do resultado do grid para **CSV e Excel**. É a base do objetivo futuro de relatórios simplificados.

O Excel recebe um **`.xlsx`**, com o cabeçalho em negrito na primeira linha. O que o grid mostra como número no padrão brasileiro vira número na planilha, para o Excel poder somar; o resto vai como texto — inclusive o que só *parece* número: código com zero à esquerda, que perderia o zero, e número com mais dígitos do que o Excel guarda sem arredondar.

### 17. Rodapé informativo

O rodapé exibe **tempo de execução** e **número de linhas** retornadas ou afetadas, além do tamanho do fetch (item 10).

Durante a execução ou compilação, essas informações dão lugar a uma **barra de progresso indeterminada** acompanhada do texto traduzido “Executando”; ao terminar, o rodapé volta às informações normais.

### 18. Painel de mensagens e erros

Painel exibindo o erro `ORA-` completo, com **salto para a linha** do comando que falhou.

Quando a execução falha sem ter devolvido nada, o grid da execução anterior sai de vista (requisito 6): na tela ficam o editor e o erro. O painel **abre** com o erro e continua aberto no erro seguinte — um erro nunca fecha o painel que o anterior abriu. Fechar é com o clique na aba dele.

No SQL Server o erro traz o número do servidor no mesmo lugar (`MSSQL-102`) e a **linha**, que o servidor conta dentro do lote e o Cururu converte para a linha do editor antes do salto.

No Oracle, o erro diz a posição **dentro do comando**, e num script de vários comandos isso não basta: o Cururu a converte na linha do comando que falhou. Nos dois bancos o salto conta a partir de onde o texto executado começa, e não do topo do editor — executar uma seleção no meio do arquivo leva à linha certa.

Um botão **Limpar** na barra do painel esvazia a lista, como o da **Saída DBMS**. O painel acumula tudo o que a aba relatou desde que foi aberta, e depois de algumas tentativas seguidas o erro que interessa fica enterrado; limpar antes de executar de novo deixa na tela só o que aquela execução disse. É a tela apenas — o registro em disco (requisito 28) não é tocado, e é dele que sai o diagnóstico de um defeito.

A barra fica parada no topo enquanto a lista rola, para o botão não sumir de vista justamente quando há mensagem demais para ler.

### 19. Histórico SQL

O histórico continua sendo registrado durante a sessão, mas seu botão está **temporariamente oculto**. Na faixa inferior aparecem apenas Mensagens/Log e Saída DBMS.

### 20. Recursos do grid

- **Ordenar por coluna**
- Distinguir visualmente **`NULL` de vazio**
- **Visualizador** para CLOB, BLOB e campos grandes
- **Largura de coluna ajustável**: arrastar a divisa no canto direito do cabeçalho muda a largura da coluna, e o duplo clique sobre ela ajusta a coluna ao maior valor já carregado. A escolha vale enquanto aquele resultado estiver na tela; uma execução nova recomeça com a largura padrão.

### 21. Formatação pt-BR

Datas e números exibidos no grid seguem o padrão brasileiro (`NLS_DATE_FORMAT` e separador decimal com vírgula). **Nos dois bancos**: no SQL Server valem os mesmos `dd/mm/aaaa`, a hora só quando existe, e a vírgula decimal, para `date`, `time`, `datetime`, `datetime2`, `smalldatetime` e `datetimeoffset`. Coluna `bit` sai como `1`/`0`, que é o que se escreve de volta num `where`, e conteúdo binário vira a descrição do tamanho, como no Oracle.

### 22. Recursos do editor

- Destaque de sintaxe
- Numeração de linha
- Localizar e substituir
- Comentar e descomentar bloco
- **Enter mantém a indentação da linha atual.** Quem alinhou a segunda coluna de
  um `SELECT` à mão continua alinhado ao escrever a terceira, em vez de voltar
  para a coluna 1. Não é a indentação automática do SQL: o editor repete o
  espaço que já está lá e não reposiciona nada por conta própria.
- **`Ctrl` + roda do mouse amplia e reduz o editor**, de 60% a 300%, em passos de
  10%. A escolha fica guardada na máquina e vale para todos os editores — janela
  de consulta, fonte de objeto, comando de etapa. A lista do autocomplete
  acompanha o tamanho. Não há tecla para isso: `Ctrl+-` já é o **Comentar
  linhas**, e o Cururu não deixa dois comandos dividirem a mesma tecla
  (requisito 32).
- **`Ctrl+clique`** sobre Tabela ou View abre a aba **Estrutura**; sobre Procedure
  ou Função abre o fonte. Quando a rotina pertence a um package, abre o **Body**
  se ele existir e, na falta dele, a Spec.
- **Vale nos dois bancos.** No SQL Server quem resolve o nome é o `OBJECT_ID` do
  próprio servidor: ele entende o nome como a consulta o entende — `cliente`,
  `dbo.cliente`, `Vendas.dbo.cliente` —, com o esquema padrão do login e no
  banco em que a aba está. Sinônimo é seguido uma vez, e um nome de três partes
  abre o objeto **no banco dele**, e não no da aba. Ficam de fora o que a árvore
  também não abre — gatilho, restrição, tipo — e o servidor ligado, de quatro
  partes.
- O botão direito sobre esses mesmos identificadores oferece **Ir para definição**
  dentro da própria janela de consulta e usa o mesmo destino do `Ctrl+clique`.
- **O destino sai em milissegundos**, resolvido pelo próprio Oracle, e não pelas
  visões do dicionário. Enquanto ele é procurado, o editor mostra o mesmo
  indicador de carregamento do sublinhado (abaixo), e um segundo clique nesse
  meio-tempo não faz nada — nem enfileira outra procura.
- **Com o `Ctrl` apertado, os nomes que o `Ctrl+clique` abre ficam sublinhados**,
  na cor de link, e o ponteiro vira a mãozinha sobre eles. Só esses: palavra
  reservada, coluna, apelido, texto entre aspas, comentário e nome que o banco
  não reconhece ficam como estão, com o cursor de texto. Quem decide é o banco,
  com as mesmas regras do clique — tabela, view, procedure e função do schema,
  sinônimo que aponte para um deles, `DONO.OBJETO` e membro de package.
  - O sublinhado vale para o editor que está sob o ponteiro, e só para o trecho
    visível; rolar com o `Ctrl` apertado sublinha o que aparece.
  - Aparece depois de o `Ctrl` ficar apertado **sozinho por 0,2 s**, ou logo no
    primeiro movimento do mouse com ele apertado. A espera é o que impede
    `Ctrl+C`, `Ctrl+V` e `Ctrl+Enter` de piscar a tela: outra tecla junto com
    o `Ctrl` apaga o sublinhado até ele ser solto. Soltar o `Ctrl` ou sair do
    editor com o ponteiro também apaga.
  - A primeira vez de cada nome passa pelo banco, e o sublinhado chega quando a
    resposta chega — em décimos de segundo, porque quem resolve os nomes é o
    próprio Oracle, e não as visões do dicionário. A pergunta sai **no instante
    em que o `Ctrl` desce**, sem esperar os 0,2 s, que são só do desenho. A
    resposta fica guardada na aba — o `Ctrl` seguinte é imediato —, e é
    esquecida depois de um DDL, para a tabela recém-criada ganhar o sublinhado.
  - **Enquanto a resposta não chega, o editor mostra que está carregando**: uma
    linha fina que corre no topo do editor, na cor de destaque, e o cursor de
    "trabalhando em segundo plano" sobre o texto. O nome já sublinhado continua
    com a mãozinha. Resposta que chega em menos de 0,12 s não acende indicador
    nenhum, para ele não piscar.
  - A conexão que responde ao sublinhado **abre junto com a aba de consulta**, e
    não no primeiro `Ctrl`: o login e a primeira execução da resolução eram a
    maior parte da espera do primeiro sublinhado.
  - Vale nos dois bancos, como o `Ctrl+clique`, e cada um filtra as **suas**
    palavras reservadas antes de perguntar: `FILE`, `SESSION` e `NUMBER` não
    são nome de objeto no Oracle, mas são no SQL Server. O `Ctrl+clique` continua
    valendo sobre qualquer nome, sublinhado ou não: o sublinhado avisa, não
    decide.

### 23. Autocomplete de colunas

Além dos objetos (item 3), o autocomplete sugere **colunas após o alias da tabela** (`t.`) e nos lugares de condição descritos no item 3. **Vale nos dois bancos**: no SQL Server as colunas vêm do catálogo do banco em que a aba está, com o tipo escrito como o T-SQL o escreve (`varchar(30)`, `numeric(10,2)`, `nvarchar(max)`). Trocar o banco na barra esquece as colunas guardadas — as tabelas do banco novo são outras.

O alias é resolvido lendo a **lista inteira** de cada cláusula, e não apenas a primeira tabela: em `FROM cliente c, pedido p`, tanto `c.` quanto `p.` acham a sua. Vale para `AS`, para `JOIN` e para `INSERT INTO`.

**Toda lista de colunas é lugar de coluna**, e não só o `WHERE` e o `ORDER BY`:

- A do **`SELECT`**, logo depois da palavra — com ou sem `DISTINCT` — e a cada vírgula dela: `SELECT status,` e `SELECT *,` pedem a coluna seguinte da mesma tabela.
- A do **`UPDATE ... SET`**, onde à esquerda de cada `=` só cabe coluna da tabela que está sendo atualizada. Vale também no `SET` do `MERGE`.
- A do **`INSERT INTO t (a, b)`**, do parêntese em diante.

O **`VALUES (1, `** fica de fora de propósito: ali se escreve valor, não coluna.

Sem nenhuma tabela escrita ainda no comando não há coluna a oferecer, e é o caso normal de quem começa pelo `SELECT` e só depois escreve o `FROM`; ali a lista traz os objetos que não são tabela, que é onde moram as funções.

**Depois de `=`, `>`, `||`, `+` e afins a lista traz as duas coisas**: primeiro as colunas das tabelas do comando, depois as funções e packages. À direita de um operador cabem as duas, e oferecer só uma seria escolher pelo usuário — `UPDATE DEM_CLIENTES SET ID_REGISTRO = '2' || IDR` pede a coluna `ID_REGISTRO`, e `WHERE dt_inc > SYSDATE - 2` pede a função. **Vale para o `+` do SQL Server**, que ali faz o papel do `||`.

As colunas vêm na frente no desempate: são as do comando que está na tela, enquanto as rotinas são o dicionário inteiro. À esquerda do `=` de um `SET` só cabe coluna, e é por isso que aquele lado tem regra própria.

#### Onde um comando termina

As tabelas que dão as colunas são as **do comando em que o cursor está**, nunca as da consulta de baixo. O corte é pelo `;` — e também pela **linha em branco seguida de palavra que abre comando** (`SELECT`, `INSERT`, `UPDATE`, `CREATE`, `EXECUTE`…), porque na prática ninguém põe `;` numa aba de rascunho.

Sem essa segunda regra, uma aba com dez consultas soltas era lida como um comando só: as colunas oferecidas vinham das tabelas de todas elas, misturadas, e a lista ficava inútil exatamente quando a tela tinha mais informação. Exigir a palavra de abertura é o que preserva o comando escrito com linha em branco no meio — `SELECT a, b` em cima, `FROM t` embaixo: `FROM`, `WHERE`, `SET` e `VALUES` continuam um comando começado acima, e por isso não cortam.

**O banco vai junto com o nome.** Em `FROM BANCO_EXEMPLO..tbl_nota n`, o `n.` traz as colunas de `BANCO_EXEMPLO` — o nome de três partes do SQL Server aponta para fora do banco da aba (requisito 3), e o esquema, quando escrito, vai junto: `Vendas.rh.func` procura `func` em `rh`. Por isso o banco entra também na memória das colunas: `Vendas..cliente` e `cliente` são duas tabelas, e podem ter colunas diferentes.

As colunas de uma tabela são lidas **uma vez por aba** e ficam em memória. Sem isso, sugerir colunas depois do `WHERE` numa consulta com três tabelas seriam três idas ao banco cada vez que a lista abre, porque `listar_colunas` não guarda nada. A aba está amarrada a uma conexão (requisito 26), então a memória tem o escopo certo — é o mesmo caminho já adotado para os objetos do usuário. O resultado vazio também é guardado: um alias que não é tabela nenhuma seria, de outro modo, uma consulta perdida ao banco a cada tecla.

O que fica guardado é a **busca**, e não o resultado dela. A diferença importa: assim, quem pedir as mesmas colunas enquanto a primeira consulta ainda está no ar recebe aquela mesma consulta, em vez de abrir outra. Sem isso, digitar `WHERE STATUS` dispara uma ida ao banco por tecla, porque nenhuma delas termina a tempo de a seguinte encontrar algo guardado.

#### Tabelas temporárias do SQL Server

`#t` e `##g` também sugerem coluna. Quem diz quais elas são é o **próprio script**, e não o banco.

É exceção necessária, não preferência. A tabela temporária mora em `tempdb` com escopo da sessão que a criou, e quem responde ao autocomplete é a sessão da **conexão**, não a da aba (requisito 3) — aquela não enxerga o `#t` desta. Perguntar ao banco também chegaria tarde: o `#t` só passa a existir depois de o `SELECT INTO` rodar, e a sugestão é pedida enquanto o script está sendo escrito. No momento em que a ajuda serve, o texto é a única fonte que sabe a resposta.

Duas formas definem a tabela, e são as duas que se escrevem:

- **`CREATE TABLE #t (...)`**, que traz nome e tipo de cada coluna.
- **`SELECT <lista> INTO #t FROM <origem>`**, em que a lista traz os nomes e a origem traz os tipos: `*` são todas as colunas da origem, `t.*` são as de uma delas, `id_nota` é a coluna copiada com o tipo que ela tem lá, e `count(*) AS qtd` é o apelido, sem tipo, porque o texto não o diz.

A procura é no **documento inteiro**, e não no comando: `#t` tem escopo de sessão, e a definição dela costuma estar num comando acima — a linha em branco antes do `UPDATE` é justamente onde o comando é cortado. Quando o script define a mesma temporária duas vezes, vale a última: ele se lê de cima para baixo.

`INSERT INTO #t` não define nada — preenche. Um `#t` que o script apenas cita veio de outro lugar, de uma procedure ou de um script rodado antes, e dele não se sabe coluna nenhuma.

**Nome de temporária não vai ao catálogo.** Depois de `FROM #` ou `JOIN #` a lista traz as temporárias que o script define, e nenhuma consulta é disparada: `#t` não está em `sys.objects` do banco da aba, e perguntar ali seria uma ida garantidamente vazia por tecla — a mesma economia da regra do prefixo de uma letra.

#### A lista de objetos não é filtrada na tela

A lista vem do banco **com teto de 300 nomes**, e quem responde avisa quando cortou. Por isso o resultado de objetos **não** é marcado como filtrável localmente: se fosse, o editor pararia de consultar e iria peneirando a lista cortada a cada tecla nova.

Foi assim que `execute dem` deixou de sugerir `DEM_INT` num schema onde `DE` tem mais de 300 objetos. O corte é por ordem alfabética, e **`_` ordena depois das letras**: `DEM_INT` fica atrás de todo `DEMZ…`, fora dos 300 primeiros. Digitar o `m` nunca chegava a perguntar por `DEM` — que traz 173 nomes, lista inteira, com o pacote dentro. Parecia defeito de maiúsculas porque o que decidia era o acaso da digitação: quando a rajada de teclas fazia a consulta cair em `dem`, aparecia; quando caía em `de`, sumia.

Sem a marca, cada tecla volta a pedir a lista do prefixo atual — o que **não** é uma consulta por tecla: quem responde é o cache da aba, ou o cache por sessão do outro lado, que sabe derivar de um prefixo menor **só quando ele veio inteiro**. É essa conta que a marca atropelava.

As outras listas mantêm a marca: coluna, subprograma e banco vêm completos, e filtrá-los na tela é sempre certo.

#### Uma consulta de objeto por vez, e a velha é abandonada

A conexão de metadados atende **um pedido por vez**, e o autocomplete disparava uma consulta por tecla. Digitar `EXECUTE DEM_INT.` enfileirava catorze — `E`, `EX`, `EXE`, `EXEC`… —, cada uma de um a três segundos, e o pedido do ponto era o último da fila. No log dá para ver o sintoma: quatro consultas terminando no mesmo milissegundo, todas com 13 a 15 segundos de duração. Nenhuma delas era lenta; todas estavam esperando.

O cache por sessão do lado do Rust não resolve esse caso: ele só ajuda depois de a primeira consulta **voltar**, e ali nenhuma tinha voltado ainda.

São três freios, na aba:

- **Prefixo repetido reaproveita a busca** — a promessa, e não o resultado, como nas colunas.
- **Prefixo que voltou vazio dispensa os maiores**: se `EXE` não achou nada, `EXEC` também não vai achar. É o que acontece com toda palavra reservada que se digita, e sozinho corta metade das consultas. Vale só para prefixo de **duas letras ou mais** — abaixo disso o dicionário não é consultado, e tomar aquele vazio por resposta apagaria da aba todo nome começado por aquela letra.
- **Prefixo de uma letra não vai ao banco**, do mesmo modo que já não ia do outro lado: era uma ida garantidamente vazia por tecla.
- **Uma consulta de objeto por vez**, e a que ficou velha é abandonada sem ir ao banco: enquanto uma está no ar, as teclas seguintes não abrem consulta nenhuma, e quando ela volta só o prefixo atual é consultado.

Medido com as catorze teclas de `EXECUTE DEM_INT`, numa conexão que atende um pedido por vez: **catorze idas ao banco antes, duas depois**.

#### As colunas são lidas antes de serem pedidas

Guardar não basta: na **primeira** vez a memória está vazia, e a consulta ao banco só começaria quando a sugestão fosse pedida. A lista apareceria um segundo depois — tarde demais para servir, quando quem digitou já desistiu e apagou.

Por isso as colunas das tabelas do comando são buscadas **assim que a tabela é escrita**, sem esperar que alguém peça uma sugestão. O `FROM cliente` é escrito antes do `WHERE`, e esse intervalo é o tempo que o banco tem para responder. São dois gatilhos, porque as pessoas digitam de dois jeitos:

- **A digitação parar** por um quarto de segundo — cobre quem escreve o `FROM` e para para pensar no resto, que é quando há mais tempo a ganhar.
- **Qualquer tentativa de sugestão** — cobre quem escreve sem parar. Ao digitar o `W` de `WHERE` a busca já sai, embora ali ainda não se sugira coluna nenhuma. Não custa nada: a análise do texto já foi feita para decidir o lugar.

A palavra colada no cursor fica de fora dos dois: ela ainda está sendo escrita, e pedir as colunas dela mandaria ao banco cada pedaço do nome da tabela, um por tecla.

### 24. Senhas salvas

A senha marcada como **Salvar Senha** é protegida pelo **DPAPI do Windows**. **Nunca** é gravada em texto puro.

No editor da conexão, **Mostrar senha** revela a senha digitada ou salva. **Copiar senha** envia o valor para a área de transferência, mesmo com o campo mascarado. Ao abrir ou trocar de perfil, o campo começa mascarado.

As conexões são gravadas em arquivo local no perfil do usuário.

### 25. Importar conexões existentes

Importação das conexões já configuradas no **Oracle SQL Developer** e/ou do **`tnsnames.ora`**, evitando redigitação.

### 26. Vínculo entre aba e conexão

Cada aba de consulta SQL fica **amarrada a uma conexão**, identificada visualmente pela **cor** definida na tela de conexão (item 1).

### 27. Ícone do aplicativo

O aplicativo terá um **ícone de sapo-cururu**, em coerência com o nome do produto.

- Aplicado ao **executável**, à **janela** e à **barra de tarefas** do Windows.
- Gerado como **`.ico` multirresolução** (16, 24, 32, 48, 64, 128 e 256 px), conforme o Tauri espera no Windows.
- O desenho precisa continuar **legível a 16 px** — silhueta simples, poucos detalhes e bom contraste, sem depender de traços finos que somem em tamanho pequeno.

### 28. Registro de erros (logs)

**Tudo é registrado em log.** Não só o que quebra: toda operação que fala com o banco registra início, fim, duração e resultado. O objetivo é que qualquer defeito relatado possa ser diagnosticado pelo log, sem depender de reproduzir o problema.

O que precisa ir para o log:

- Toda chamada de comando do backend: qual comando, com que parâmetros relevantes (**nunca senhas**), quanto tempo levou e se deu certo.
- Em toda execução de SQL, **em que conexão** ela rodou e, quando um script para no meio, **em que comando e linha** — é o que separa, pelo log, um erro em produção de um em homologação.
- Todo erro, com o `ORA-` completo quando vier do banco.
- Todo *panic* do Rust, com a mensagem e o local — é o que explica o aplicativo fechar sozinho.
- Subida e encerramento do aplicativo, e a situação do Instant Client.

Formato: **texto puro, uma ocorrência por linha**, com data e hora local, nível (`INFO`, `AVISO`, `ERRO`) e origem. Precisa ser legível direto, sem ferramenta para abrir.

Local: `%APPDATA%\Cururu\logs\`.

### 29. Menu e botão de conexão

- A tela **Sobre o Cururu** é aberta pela opção **Sobre** do menu principal; não existe botão separado `[?]` no título do painel de conexões.
- A tela **Versões** é aberta pela opção **Versões** do mesmo menu e lista, da mais nova para a mais antiga, o que mudou em cada versão do aplicativo, marcando a que está em execução. O conteúdo vem de **`versoes.md`**, na raiz do projeto, embutido no pacote durante o build: cada versão nova acrescenta a sua seção nesse arquivo, e a tela apenas o lê — não existe uma segunda lista dentro do código para manter em dia.
- A opção **Configurações** abre a tela do requisito 41. Ela tomou o lugar das três entradas que existiam soltas no menu — **Teclas de Atalho**, **Language** e **Tema** —, que agora são seções de lá.
- O botão **Nova conexão** usa o símbolo de um **plugue macho** e segue as mesmas cores neutras e os mesmos estados visuais dos demais botões da barra de ferramentas.

### 30. Aviso de versão nova

O Cururu consulta o repositório público de releases e avisa quando existe uma versão mais nova que a em execução.

**A decisão é sempre do usuário.** Nada é baixado nem instalado sozinho: a tela mostra o número da versão, a data e **o que mudou** — a lista do release —, e só então oferece **Baixar e instalar** ou **Agora não**. Aceito o download, a barra mostra o andamento, o instalador é executado e o Cururu se fecha, porque o instalador precisa substituir o `cururu.exe` que está em uso.

Quando ocorre a consulta:

- **Na subida do aplicativo**, em silêncio e sem esperar: a janela já está utilizável antes de a resposta chegar. Só aparece tela se houver versão nova de verdade. Sem internet, ou com o proxy barrando, **nada aparece** — abrir o Cururu fora da rede da empresa não pode virar um aviso de erro na cara de quem só quer trabalhar. A falha fica no log, que é onde ela é procurada.
- **Sob demanda**, pela opção **Procurar Atualizações…** do menu principal. Aí quem perguntou merece resposta: se já estiver na versão mais recente, a tela diz isso; se falhar, o erro aparece na barra de status.

Decisões de implementação que valem registro:

- O acesso à rede usa o **WinHTTP do próprio Windows**, e não uma biblioteca HTTP em Rust. O Cururu roda em máquina corporativa: o WinHTTP respeita a configuração de proxy do Windows, inclusive PAC e detecção automática, e valida o certificado pela loja de certificados do sistema — uma raiz interna instalada pela empresa é aceita sem configuração nenhuma. Nenhuma dependência nova entrou no projeto por causa disso.
- A comparação de versões é feita **número a número**, e não como texto: na ordem alfabética `0.1.9` é maior que `0.1.10`, e a atualização nunca seria oferecida.
- O download só aceita `https` e **só servidores do GitHub**. Dali sai um executável que vai rodar na máquina do usuário; uma resposta adulterada não pode apontar o download para outro servidor.
- O comando que **executa** o instalador não recebe caminho da interface. Quem baixou foi o backend, e é ele quem sabe onde o arquivo está.

### 31. Idioma da interface

O título do painel de conexões **não tem texto**: no lugar da palavra "Conexões" fica o botão de **Menu**. O de minimizar continua encostado na direita.

A escolha do idioma aparece em **dois lugares, de propósito**: no menu principal, como **Language** com submenu lateral, e como primeiro campo da seção **Geral** da tela de Configurações (requisito 41).

Duas portas para a mesma escolha normalmente é defeito — e aqui é a exceção que se justifica. **Language** é a única linha da interface que **não se traduz**: ela aparece assim nos três idiomas, porque quem abriu o aplicativo num idioma que não lê precisa de uma palavra que reconheça para achar a saída. Esconder essa porta atrás de "Configurações" — ou de "Settings", ou de "Configuración" — exigiria entender a palavra que está justamente no idioma que a pessoa não lê. No menu a troca vale no clique; nas Configurações ela é rascunho até o Salvar, como o resto daquela tela.

Os idiomas oferecidos são **English**, **Español** e **PT-BR**, cada um escrito no próprio idioma, com o que está em uso marcado no menu e selecionado na lista.

**Quem decide o idioma da primeira abertura**, em ordem:

1. A escolha guardada na máquina, se já houver uma.
2. O idioma escolhido no **instalador**, gravado por ele em `HKCU\Software\Cururu\Idioma`. O instalador oferece a lista de idiomas já pré-selecionada com o da máquina; escolher inglês ali abre o Cururu em inglês.
3. O idioma da máquina, lido pela própria janela: **português** vira português, **espanhol** vira espanhol, e **qualquer outro vira inglês**. Cair em português por padrão entregava a um alemão ou a um francês uma tela que ele não lê — e com o nome do idioma escrito nela também em português.

**Todo texto que o usuário lê sai do catálogo**, sem exceção: título de tela, rótulo de botão, dica de ferramenta, mensagem de rodapé e texto de erro de formulário. O catálogo exige as três traduções de cada chave, e a falta de uma **impede a compilação** — sem isso, a ausência só apareceria na tela de quem usa o idioma esquecido.

**A troca vale na hora, sem reabrir o aplicativo.** Quase toda a tela é montada no instante em que aparece — menu, diálogo, dica de botão —, então já nasce no idioma novo. O que fica montado desde a abertura da janela se reescreve por assinatura, no mesmo lugar em que o rótulo é criado, para não existirem dois lugares dizendo o mesmo texto.

### 32. Dois atalhos por função

Cada função listada na tela **Teclas de Atalho** aceita até duas combinações de teclas, não só uma. A primeira é obrigatória, como antes; a segunda é opcional e pode ficar em branco. As duas colunas concorrem pela mesma restrição de unicidade: nenhuma tecla pode servir a duas funções, seja como atalho principal ou alternativo de cada uma.

### 33. Execução silenciosa dos botões rápidos

Além do **Executar automaticamente** (requisito já existente dos botões rápidos), cada botão aceita a opção **Não inserir SELECT na janela de consulta**. Marcada, o conteúdo do botão é enviado direto para execução, sem aparecer no editor — nem inserido, nem selecionado. Tem prioridade sobre o Executar automaticamente, que depende do texto estar na tela para poder selecioná-lo.

### 34. Tema

O campo **Tema**, logo abaixo de **Language** na seção Geral das Configurações (requisito 41), alterna entre **Claro** e **Escuro**. A troca vale assim que a tela é salva, sem reabrir o aplicativo. A escolha fica guardada na máquina; sem escolha guardada, vale o tema claro.

### 35. Reconexão depois de queda

Perder a rede — uma VPN que cai, o servidor que derruba a sessão — **não invalida a aba**. Antes, a conexão morria e todo comando dali em diante falhava com `ORA-03113`; a única saída era fechar a aba e abrir outra.

Agora a própria tentativa seguinte reabre a sessão, com as credenciais já resolvidas da conexão e sem repedir a senha:

- **Consulta** (`select`, `with`): reconecta e **executa**. O usuário aperta Play uma vez só.
- **Comando que altera dados** (DML, DDL, bloco PL/SQL, commit, rollback): reconecta, mas **não repete sozinho**. Não há como saber se o comando chegou a valer no banco antes de a conexão cair, e repetir um `insert` por conta própria poderia gravar duas vezes. A aba volta utilizável e a mensagem diz que basta executar de novo.
- **Árvore de objetos, colunas e autocomplete**: reconectam e repetem, sem aviso — são só leitura, e não há o que o usuário decidir.

Em todo caso em que a sessão é reaberta, o painel de mensagens e a barra de status avisam, porque a sessão nova é **outra** sessão do banco: o que estava sem commit se perdeu junto com a conexão anterior, e o `DBMS_OUTPUT` é religado do jeito que estava (o auto-commit, por ser do Cururu e não do banco, continua valendo sozinho).

### 36. SQL Server Agent

Abaixo de **Banco de Dados** (requisito 2), a árvore de uma conexão de SQL Server tem o nó **SQL Server Agent**, com os **trabalhos** pendurados direto nele. Não há a pasta *Trabalhos* que o SSMS interpõe: lá dentro do Agent há também alertas, operadores e proxies, e a pasta existe para separá-los; aqui só há trabalho, e a pasta seria um clique a mais para chegar ao único conteúdo.

**O nó mostra quando o serviço está parado**, com o símbolo trocado e a palavra *parado* ao lado, em vermelho. Não é enfeite: um trabalho agendado num Agent fora do ar não roda, e ver a lista cheia sem essa informação é a maneira mais fácil de perder uma manhã. Há um terceiro estado, **desconhecido**, para quando a conta não tem permissão de consultar o serviço — dizer "parado" a quem apenas não pode olhar mandaria o usuário procurar defeito num serviço que está funcionando.

Cada trabalho traz na dica do mouse o que a linha não tem espaço para mostrar: descrição, categoria, proprietário, data de criação e a última execução com o resultado dela. **Trabalho desabilitado aparece em cinza**, com a palavra *desabilitado* ao lado. Os trabalhos não expandem: o que há dentro de um trabalho — etapas e agendas — a tela de propriedades mostra melhor do que uma árvore mostraria.

O botão direito sobre **SQL Server Agent** oferece **Novo Trabalho**, **Aplicar Filtro…**, **Limpar Filtro** e **Atualizar**. O filtro é o das categorias (requisito 4), sobre o nome do trabalho, e o botão de filtro da barra também age sobre o nó quando ele está selecionado. Com filtro ativo o nó vira `SQL Server Agent (Filtrado)`, e a dica do mouse diz o critério ao lado do estado do serviço.

#### Botão direito sobre um trabalho

Na ordem do SSMS, com os separadores onde ele os põe:

- **Novo Trabalho** — o mesmo do nó do Agent, à mão de onde se está.
- **Iniciar Trabalho na Etapa…** — abre a grade de etapas para escolher por onde começar; um trabalho de cinco etapas quase nunca precisa rodar inteiro quando se está consertando a terceira. O duplo clique numa etapa escolhe e inicia. Escolhida a etapa, abre a **tela de acompanhamento**, descrita abaixo.
- **Parar Trabalho** — envia o pedido de parada.
- **Exibir DDL** — abre, numa aba do editor, o script que recria o trabalho: o mesmo destino do fonte de uma procedure do Oracle, com destaque de sintaxe, e uma aba que se pode salvar. O formato é o do *Script Job as CREATE* do SSMS — `USE [msdb]`, transação, conferência de retorno depois de cada procedure e `GO` —, para rodar como está em outro servidor. A categoria vai junto, com o `if not exists` que o SSMS também gera: um trabalho levado para outro servidor costuma chegar antes da categoria dele.
- **Habilitar** / **Desabilitar** — a opção que não faria nada aparece desabilitada, em vez de aparecer e não surtir efeito.
- **Renomear** — pede o nome novo numa caixa do próprio aplicativo, e não na do Windows, que ignoraria o tema.
- **Excluir** — pede confirmação dizendo o que sai junto: etapas, histórico e as agendas que só aquele trabalho usa.
- **Atualizar** — relista os trabalhos e reconfere o serviço.
- **Propriedades** — abre a mesma tela do cadastro, preenchida. O duplo clique sobre o trabalho leva ao mesmo lugar.

#### Tela de acompanhamento da execução

Mandar rodar e não ver nada acontecer é o mesmo que não ter mandado. O `sp_start_job` **agenda** a execução e volta na hora — quem executa é o serviço, e ele não avisa quando termina —, e durante um tempo a resposta do Cururu foi dizer isso na barra de status. Não basta: o SSMS mostra uma tela de progresso, e quem vem dele sente falta dela.

Ao iniciar o trabalho, abre-se a janela **Execução do Trabalho**, com o nome do trabalho no título. A janela pode ser arrastada pela barra de título para outro monitor, e a janela principal continua disponível. O acompanhamento pergunta ao Agent de segundo em segundo e mostra:

- **a situação**, com as palavras do próprio Agent: *Aguardando o Agent pegar o trabalho*, *Em execução*, *Aguardando uma thread livre*, *Entre tentativas*, *Execução suspensa* ou *Concluindo*;
- **a etapa em curso**, como o Agent a numera — `2 (Carregar dados do dia)`. Vem do `sp_help_job`, que pergunta ao processo do serviço; a tabela do `msdb` só sabe a última etapa **concluída**, e durante a etapa 2 ainda diria 1;
- **uma barra indeterminada** e o tempo decorrido. A barra não tem porcentagem porque o Agent não informa progresso, e uma barra que avançasse sozinha inventaria um número que ninguém mediu;
- **o fim**: *Concluído com êxito* em verde, *O trabalho falhou* em vermelho, ou o que mais o histórico disser — com a duração medida pelo servidor, a hora de início e **a mensagem que o Agent gravou**, que é a mesma que o SSMS mostra e, num trabalho que falhou, o único lugar que diz por quê.

O botão **Parar** faz o mesmo que o item de menu. **Fechar não para o trabalho** — ele é do Agent e segue rodando sem ninguém olhando; a barra de status diz isso ao fechar a tela no meio. Quando a execução termina, a barra de status repete o veredito e a árvore relista, porque a última execução que ela mostra na dica do mouse acabou de mudar.

A tela também cobre o caso de **o trabalho já estar rodando**: em vez da recusa seca do servidor — *refused because the job is already running* —, ela acompanha a execução que está em curso e diz que é dela que se trata.

Conhecer o fim exige distinguir esta execução da anterior, e é para isso que serve a leitura feita **antes** de disparar: ela guarda a instância do histórico que já existia. Sem esse número, o primeiro "não está executando" — que acontece sempre, porque o Agent leva alguns instantes para pegar o trabalho — seria lido como fim, e a tela anunciaria o resultado da véspera como se fosse o de agora.

#### Tela de Novo Trabalho e de Propriedades

**É a mesma tela para os dois.** São os mesmos campos, e um segundo formulário só para editar seria a mesma coisa escrita duas vezes, com duas chances de divergir. Sem trabalho indicado ela se chama **Novo Trabalho** e cadastra; com um trabalho, **Propriedades do Job — "nome do trabalho"** — nasce preenchida e grava por cima.

Espelha a do SSMS, bem mais curta: das seis páginas dela ficam **três**. Alertas e notificações dependem de operador cadastrado e de Database Mail; destinos só existe em administração multisservidor, que o Cururu não faz.

A ordem das páginas é **Etapas, Agendas e Geral** — ao contrário do SSMS, que começa pelo Geral. Quem abre um trabalho que já existe vai ver o que ele faz; nome, dono e categoria raramente mudam, e não precisam ocupar a primeira página. Por isso as propriedades **abrem nas Etapas**, e o cadastro de um trabalho novo abre no **Geral**: sem nome não há o que gravar.

**O título traz o nome do trabalho** — `Propriedades do Job - "Carga noturna"`. Com vários abertos ao longo do dia, três janelas escritas *Propriedades do Trabalho* não dizem qual é qual.

**As telas do Agent são janelas.** A de **Propriedades do Job** se move pela barra de título; a de **etapa** se move, muda de tamanho pelas bordas e pelos cantos, e **maximiza** — pelo botão ao lado do de fechar ou pelo duplo clique na barra —, com o comando crescendo junto. Um comando de cem linhas não se lê por uma fresta. A janela não se perde: por mais que se arraste, a barra de título continua na tela, e um Cururu que encolha traz a janela de volta para dentro. O **Esc** fecha a tela de cima, e só ela — com a etapa aberta por cima do trabalho, o primeiro Esc é da etapa —, e fecha com o foco onde estiver, mesmo depois de um clique numa linha da grade. Dentro do editor ele é do editor: primeiro fecha o autocomplete ou o localizar, e só depois a tela. Nas quatro telas, arrastar o mouse sobre rótulo, cabeçalho de grade ou aviso não seleciona nada — isto é um aplicativo do Windows; só o que se edita fica selecionável.

**Etapas** — grade com ordem, nome, tipo e o que fazer ao ter êxito e ao falhar, com **Nova**, **Editar**, **Excluir** e as setas de mover. A tela de etapa tem nome, tipo (**Transact-SQL**, **Sistema operacional (CmdExec)** ou **PowerShell**), banco de dados — só no T-SQL —, comando e as duas ações. A etapa nova nasce saindo com êxito, e a que era a última passa a seguir para a seguinte: quem sai informando êxito no meio do trabalho interrompe tudo o que vem depois sem erro nenhum registrado.

**O comando é o editor de consultas**, e não uma caixa de texto: mesmo destaque de sintaxe — comentário em verde, palavra reservada realçada —, Tab e Shift+Tab indentando o bloco, localizar/substituir, comentar linha e o autocomplete do banco escolhido na própria tela. Quem escreve o T-SQL de uma etapa escreve SQL, e não havia por que ser pior do que a janela de consulta.

**Agendas** — grade com nome, se está habilitada e a agenda escrita por extenso, com **Nova**, **Editar** e **Excluir**. A tela de agenda é a do SSMS: tipo (**Recorrente**, **Uma vez**, **ao iniciar o SQL Server Agent** ou **quando as CPUs ficarem ociosas**), frequência diária/semanal/mensal — com dia do mês ou o *primeiro/segundo/terceiro/quarto/último* dia, dia útil, dia de fim de semana ou dia da semana —, frequência dentro do dia (uma vez às, ou a cada N minutos/horas entre duas horas) e duração. O **Resumo** mostra a agenda por extenso enquanto ela é montada, porque os números que o SQL Server guarda não se leem: "no último dia útil do mês" não se confere olhando `32, 16, 9`.

**Geral** — nome, proprietário, categoria, descrição e **Habilitado**. Abaixo, sem edição, os quatro campos que o SSMS também mostra ali: **Origem**, **Criado em**, **Última modificação** e **Última execução**. Quem os preenche é o servidor, e não a tela; num cadastro eles saem vazios, como no SSMS, e nas propriedades é onde finalmente têm o que mostrar. Um proprietário que não esteja na lista de logins — porque a conta atual não o enxerga — é acrescentado a ela e mantido, para a gravação não trocar o dono do trabalho sem ninguém perceber.

**Criar e gravar são tudo ou nada.** O trabalho, as etapas e as agendas entram numa transação só: um trabalho que ficou sem etapa, ou que não chegou a ser atribuído ao Agent local, aparece na lista e nunca roda. Um trabalho **sem etapa nenhuma** é recusado — o servidor o aceitaria, e ele não faria nada.

**Ao gravar, etapas e agendas são trocadas por inteiro**, não conciliadas uma a uma. É a diferença mais visível para o SSMS, e é deliberada: casar o que está na tela com o que está no servidor exigiria identificar cada etapa entre as duas listas, e a tela não tem esse identificador — ela deixa reordenar, renomear e excluir livremente. A consequência real é uma só: a agenda perde o identificador dela no `msdb`. Uma agenda **compartilhada** com outro trabalho continua de pé para ele, porque só se apaga a que ninguém mais usa; o que se perde é o compartilhamento com este trabalho, que a tela também não sabe montar.

**Permissões.** Listar trabalhos exige acesso ao `msdb` (`SQLAgentUserRole` ou acima); quem não é `sysadmin` vê apenas os trabalhos de que é dono, que é a mesma regra do SSMS. Escolher outro proprietário exige `sysadmin`. Iniciar e parar exigem o serviço no ar — com ele parado, o servidor recusa com a mensagem certa, e é ela que aparece na barra de status. Faltando permissão, a mensagem do servidor aparece no lugar da lista.

### 37. Comentário da view

Uma view chamada `VW_FAT_CONS_01` não diz o que é. O nome sozinho raramente diz: quem escreveu sabia, quem abre a árvore seis meses depois não. Por isso a view — e só ela — carrega um **comentário**, mostrado na árvore e editável ali mesmo.

A tabela ficou de fora de propósito: uma tabela se explica pelas colunas, que a árvore já expande. Uma view esconde a consulta que a define, e é justamente isso que o comentário conta.

**Na árvore**, o comentário aparece entre parênteses ao lado do nome, em cinza:

```
◫ VW_FATURAMENTO (Faturamento por filial, mês fechado)
```

São os **primeiros 50 caracteres**, com reticências quando há mais. Quebra de linha vira espaço, porque a linha da árvore é uma só. Nada se perde: a **dica do mouse** traz o texto inteiro, do jeito que foi escrito, abaixo do nome e da situação.

Num painel estreito é o comentário que encolhe, nunca o nome: saber qual é a view importa mais do que ler a descrição inteira, que continua a um passar de mouse de distância.

**Para editar**, botão direito sobre a view → **Editar Comentário…**. A caixa abre com o texto atual, aceita várias linhas e confirma com Ctrl+Enter, como o editor SQL. Deixar a caixa **vazia apaga** o comentário — e é por isso que esvaziar e cancelar são coisas diferentes: só o Cancelar (ou Esc) não faz nada.

O limite é de **3750 caracteres**, o menor dos dois bancos, para o mesmo texto caber onde quer que ele seja gravado.

#### O mecanismo, que é diferente nos dois bancos

No **Oracle** é o `COMMENT ON TABLE` de sempre — que apesar do nome vale para view —, lido de `USER_TAB_COMMENTS`. Vale registrar o efeito colateral: `COMMENT ON` é DDL, e DDL no Oracle **confirma a transação aberta**. É o mesmo que aconteceria digitando o comando na janela de consulta, mas convém saber que o item de menu também o faz.

No **SQL Server** não existe `COMMENT ON`. O equivalente é a *extended property* chamada **`MS_Description`** — o mesmo nome que o SSMS lê e escreve na caixa *Description*, e é essa a razão de ser ele: comentário posto pelo Cururu aparece no SSMS, e o posto pelo SSMS aparece no Cururu. Gravar exige escolher entre três procedures (`sp_add`, `sp_update`, `sp_drop`), porque a primeira recusa a propriedade que já existe; a escolha é feita no mesmo lote da gravação.

Uma diferença que o usuário sente: no SQL Server a propriedade **não sobrevive** a um `DROP`/`CREATE` da view. Onde a view é recriada por script, o comentário precisa ir no script.

### 38. Botão rápido por conexão

Cada botão de texto rápido diz **em quais conexões aparece**. O campo **Conexão** fica na janela de configuração do botão, logo antes de **Posição**, e é uma lista de marcação: **Todas** no topo e uma linha por conexão salva, com a bolinha de cor de cada uma.

O padrão é **Todas**, e é o que todo botão criado antes desta versão continua sendo — a lista vazia quer dizer "toda conexão", de modo que nada do que já existia mudou de lugar.

Marcando `EXEMPLO PRD` e `EXEMPLO HML`, o botão aparece só nas abas dessas duas; numa aba de `Sistema Exemplo PRD` ele não aparece. **O atalho acompanha**: a tecla de um botão escondido não faz nada naquela aba, porque a tecla não pode fazer o que o botão que ela representa não faria.

**Todas** e as conexões específicas se excluem na tela: marcar **Todas** desmarca as outras, marcar uma específica desmarca **Todas**, e desmarcar a última específica devolve o **Todas**. Assim não existe estado que o Salvar tenha de recusar.

O filtro guarda o **identificador** da conexão, não o nome: renomear a conexão não solta o botão dela. O identificador de uma conexão excluída simplesmente deixa de casar com alguma coisa, e não é apagado da lista — a conexão pode voltar por importação (requisito 25).

### 39. Gerar script de INSERT

No menu do botão direito do grid, **Gerar script de INSERT** põe no clipboard um `INSERT` por linha escolhida — ou por todas, quando não há linha marcada, como em copiar e exportar. São sempre **todas as colunas** do resultado, mesmo quando a escolha é de uma célula só: quem clicou numa célula quer o registro dela, e um INSERT de uma coluna não serve para nada.

O mesmo item aparece na **aba de Dados do painel de objeto**, e ali sem análise de texto nenhuma: a consulta daquele grid é `SELECT * FROM` a própria tabela aberta, com o nome já citado como o banco exige. Na aba de **Estrutura** ele não aparece — aquele grid mostra o dicionário, não a tabela.

No grid da consulta o item **só aparece quando o comando daquele grid lê uma tabela só**. Com duas no `FROM`, com `JOIN` ou com subconsulta não existe resposta certa para "inserir onde", e um palpite escreveria na tabela errada. A tabela entra no script com o nome **exatamente como o comando o escreveu**, `Vendas..cliente` incluído: é esse nome que resolve na sessão de quem vai rodar o script.

Coluna que não é coluna — `COUNT(*)`, uma expressão, uma concatenação — recusa o script inteiro, com a mensagem dizendo qual: o comando sairia inválido de qualquer forma. É o que acontece com `SELECT COUNT(*) FROM financeiro`, que lê uma tabela só e ainda assim não tem INSERT possível.

O valor vira literal a partir do que o grid mostra, e o grid mostra texto formatado (requisito 21). Então: número volta de `1234,56` para `1234.56`; data volta de `27/08/2026 15:11:05` para `TO_DATE('27/08/2026 15:11:05','DD/MM/YYYY HH24:MI:SS')` no Oracle — `TO_TIMESTAMP` quando a coluna é `TIMESTAMP` — e para `CONVERT(datetime, '2026-08-27 15:11:05', 120)` no SQL Server. A conversão é explícita nos dois bancos porque literal solto depende de `NLS_DATE_FORMAT` e de `DATEFORMAT`, e um script que só roda na sessão de quem o gerou não serve para nada. `NULL` sai `NULL`, texto vazio sai `''`, apóstrofo dobra, e coluna `bit` do SQL Server sai `1`/`0` sem aspas.

**O que o grid não tem, o script não inventa.** Coluna binária sai `NULL`, porque a célula mostra `(binário, 320 bytes)` — a descrição do conteúdo, não o conteúdo —, e o mesmo vale para valor que o driver não converteu. O rodapé diz quais colunas saíram assim, e também avisa quando algum literal passa de 4000 caracteres, que é o limite que o Oracle aceita. Pela mesma razão há duas perdas conhecidas: fração de segundo de `TIMESTAMP` não chega até o grid, e `datetimeoffset` chega sem o deslocamento.

O clipboard recebe **só SQL** — nenhum comentário de aviso no meio do script, para o texto poder ser colado e executado como está. Nenhum `COMMIT` é acrescentado: confirmar é decisão de quem roda.

O **botão direito sobre uma linha já escolhida não desfaz a escolha**, e é o que permite gerar o script de várias linhas de uma vez. Sobre linha não escolhida ele escolhe aquela, como é o costume. Vale para todo o menu do grid: copiar várias linhas com o botão direito também dependia disso.

### 40. Índice local dos metadados

As categorias **Tabelas, Views, Pacotes e Procedures do Oracle** usam o índice, com filtros e paginação locais. O arquivo guarda situação e indicação de objeto gerado; para views guarda também o comentário exibido pela árvore. Índices antigos fornecem os nomes de pacotes imediatamente, com situação desconhecida até a atualização. **Atualizar** no menu de uma categoria indexada força uma nova leitura e preserva o restante do índice.

O autocomplete guarda em disco o que leu do dicionário, em `%APPDATA%/Cururu/indice/<conexão>.json`, um arquivo por conexão. **Neste banco toda primeira leitura do dia é cara**: a lista de subprogramas de um pacote custou de 14 a 23 segundos, medida no log. O cache em memória já resolvia a segunda chamada; o índice resolve a primeira, porque a resposta ficou gravada da vez passada.

O acervo é lido de uma vez logo depois de conectar, **numa conexão aberta só para isso**: a conexão de metadados atende um pedido por vez, e essa leitura levou 29 s num schema grande — feita ali, seguraria o autocomplete e a árvore por todo esse tempo.

O arquivo de rotinas guarda o que se **chama**:

- **Packages, procedures e functions do usuário**, inteiras e sem filtro. É por isso que `EXECUTE DEM...` pode sugerir `DEM_INT` sem consultar o banco.
- **Todos os subprogramas das packages**, lidos em lote; assim `DEM_INT.` também responde do JSON na primeira tentativa.

Ao conectar, o JSON existente fica disponível imediatamente e uma conexão separada confere o dicionário em background. O novo retrato substitui o anterior quando termina; assim procedures, functions, packages e membros novos passam a fazer parte do índice sem colocar a espera na digitação do usuário.

**Depois de FROM, JOIN, UPDATE e DELETE, o autocomplete Oracle tem duas fontes independentes.** A primeira oferece tabelas, views, materialized views e sinônimos do índice local, sem esperar a fila de consultas. A segunda consulta o Oracle por prefixo e complementa o mesmo menu. O resultado local nunca é marcado como resposta completa do banco; um vazio local também não impede a procura remota. O dono distingue nomes iguais e permite buscar com `SCHEMA.PREFIXO`.

Os nomes consultáveis ficam em `%APPDATA%/Cururu/indice/<conexão>.relacoes.json`, separado do arquivo de rotinas para preservar a abertura rápida dos pacotes. São guardados sem o filtro da interface, com dono, situação e indicação de objeto gerado. A opção **Tabelas: apenas as filtradas** é aplicada ao consultar o índice, com as mesmas regras da consulta ao banco, sem excluir views nem sinônimos. O retrato do próprio usuário é publicado primeiro, e o dos outros donos acessíveis depois, antes da leitura dos subprogramas. Falhas preservam o retrato anterior e são registradas. Na primeira utilização, sem esse arquivo, o resultado vem do banco enquanto a carga inicial acontece; nas seguintes o JSON é carregado ao conectar. Os campos antigos `tabelas` e `tabelas_indexadas` continuam ignorados: eram um recorte incompleto.

A lista de rotinas que o índice devolve fora do lugar de tabela fica guardada **em chave própria** no cache de sugestões. Ela é só um pedaço do que o `FROM` precisa e vem marcada como completa; guardada junto das outras respostas, um `SELECT V` bastava para o `FROM V…` seguinte sair dela, sem tabela nem view.

**O índice é sugestão, e não verdade.** Ele responde o que existia da última vez, e alimenta as sugestões locais do autocomplete e a lista de Pacotes: executar, abrir fonte e o grid continuam perguntando ao banco. Um pacote recompilado no meio do dia não é problema — o que ele oferece a mais some assim que o índice é descartado.

O índice sai pelo mesmo caminho do cache em memória (`atualizar_metadados`), e um arquivo de versão diferente da esperada é ignorado inteiro em vez de ser lido pela metade. O nome do arquivo é higienizado: o nome do banco vem do usuário, e sem isso um `..` no meio sairia da pasta do índice.

Por ora vale só para o **Oracle**, que é onde a leitura do dicionário custa caro; a chave já prevê o banco atrás do perfil, que é o que o SQL Server vai precisar.

### 41. Tela de Configurações

Uma tela só para as preferências do aplicativo, aberta pela opção **Configurações** do menu principal. À esquerda, a lista de seções; à direita, o painel da seção escolhida:

- **Geral** — **Language** (requisito 31), **Tema** (requisito 34) e **Objetos listados por vez** (requisito 2).
- **Teclas de Atalho** — a mesma tabela de antes: duas combinações por função (requisito 32), a combinação já usada por outra ação troca de dono, e **Restaurar padrão** repõe as de fábrica.
- **Auto-Complete** — o que o editor oferece enquanto se digita (requisito 43).

**Language também fica no menu**, fora daqui: é a única exceção à porta única, e o requisito 31 explica por quê.

**O que se mexe é rascunho até o Salvar.** Vale para as três seções, inclusive idioma e tema, que antes valiam no clique do submenu. Sair pelo **Cancelar**, pelo **✕** ou pelo **Esc** (requisito 42) não grava nada — nem a metade que já tinha sido mexida.

**O número de objetos é conferido no Salvar.** Fora de 10 a 5000, ou com casas decimais, a tela diz o intervalo no rodapé, devolve o foco ao campo e não grava nada — nem o que foi mexido nas outras seções.

A tela move pela barra de título e muda de tamanho pelas bordas, como as do SQL Server Agent: a tabela de atalhos é longa, e quem confere a lista inteira ganha em esticá-la.

**Esc não é combinação gravável.** No campo que captura teclas, Esc fecha a tela em vez de virar o atalho da linha — a tecla está reservada pelo requisito 42, e um atalho que ninguém conseguiria usar sem fechar a janela não serviria para nada. O mesmo vale para o campo de atalho do editor de botões rápidos.

### 42. Esc fecha qualquer janela extra

**Toda janela que se abre por cima do Cururu fecha com Esc** — diálogo, formulário, aviso, editor, visor de valor, confirmação. Sem exceção e sem exigir que o foco esteja em lugar nenhum específico: basta a janela estar aberta.

**Esc descarta.** Fecha sem gravar e sem perguntar, exatamente como o Cancelar da mesma tela. Não existe "tem certeza?" no caminho do Esc; quem quer guardar clica em Salvar.

Com janelas empilhadas, cada Esc fecha **uma** — a de cima —, e a de baixo continua onde estava.

As duas únicas esperas são estas, e as duas são temporárias, não decisões do usuário:

- Uma operação em andamento que a tela não pode abandonar no meio — o download da atualização, a gravação de um trabalho do Agent. Enquanto ela corre, o Esc não responde, do mesmo jeito que o botão Fechar fica desabilitado.
- Um Esc que já teve dono dentro da própria tela — o autocomplete aberto no editor SQL, o painel de Localizar. Esse Esc fecha o que está por cima; o seguinte fecha a janela.

### 43. Escopo de tabelas do autocomplete

A seção **Auto-Complete** da tela de Configurações (requisito 41) tem a opção **Tabelas**, que decide quais tabelas o editor oferece enquanto se digita:

- **Todas** — o padrão. Qualquer tabela que a conexão enxergue pode ser sugerida, inclusive as de outros donos alcançadas por sinônimo. É o comportamento que o Cururu sempre teve.
- **Apenas as do filtro da conexão** — só as tabelas que passam pelo filtro da categoria **Tabelas** (requisito 4), o mesmo que recorta a árvore de objetos. Num schema com dezenas de milhares de tabelas é o que faz a lista caber no que a pessoa realmente usa.

**Vale só para tabelas.** Pacote, procedure, função, view, sequência, tipo e sinônimo continuam sendo sugeridos por inteiro: o filtro é o da categoria Tabelas, e usá-lo para recortar o que ele não descreve seria inventar um significado que ele não tem.

**Sem filtro configurado, as duas opções dão no mesmo** — não há o que recortar. Vale nos dois bancos: cada conexão aplica o filtro da própria categoria Tabelas, em SQL do Oracle ou em T-SQL.

O recorte é aplicado **no SQL**, e não sobre a lista já devolvida. Filtrar depois seria filtrar o que sobrou do teto de 300 nomes: a tabela procurada poderia ter ficado de fora do corte por causa de outras que a opção justamente descartaria.

A escolha é do aplicativo, não da conexão: ela vale para todas as conexões, e cada uma aplica o próprio filtro.

### 44. O `dbo.` não aparece na tela

No SQL Server o objeto é identificado por `esquema.nome`, porque um banco tem vários esquemas. Mas **`dbo` é o esquema padrão**: ninguém o escreve numa consulta, e repeti-lo em cada linha da árvore gasta a largura do painel dizendo o que já se sabe.

Então o `dbo.` sai do que se lê — árvore, título da aba, mensagens do rodapé —, e **esquema diferente continua aparecendo**, porque ali a informação é necessária: é ela que distingue `vendas.pedido` de `compras.pedido`.

**O que vai ao servidor continua qualificado.** Um login cujo esquema padrão não seja `dbo` resolveria `cliente` para outro objeto, e a árvore passaria a mostrar uma tabela e abrir outra. A regra é de exibição, e só.

No Oracle nada muda: lá o nome nunca vem com esquema (requisito 2). O autocomplete já seguia esta regra desde que passou a sugerir nomes de SQL Server — o esquema só acompanha a sugestão quando não é `dbo`.

### 45. Consultas ao MongoDB

O perfil MongoDB aceita URI `mongodb://` ou `mongodb+srv://`, inclusive com banco padrão e opções como `authSource`. Ao colar a URI completa do Compass, o Cururu separa usuário e senha nos campos próprios e mantém o endereço, o banco e as opções na URI. A senha salva recebe a mesma proteção do requisito 24.

A árvore abre os bancos acessíveis e, em cada banco, as coleções. Duplo clique numa coleção abre os documentos em uma aba. Também é possível abrir uma aba da conexão e escolher banco e coleção nela.

A consulta é somente leitura. O editor recebe um objeto Extended JSON, com `{}` para listar documentos. Os resultados mantêm os tipos BSON e permitem expandir objetos e arrays. A aba limita a quantidade retornada por consulta. O autocomplete sugere nomes observados em uma amostra pequena de documentos, carregada em segundo plano e guardada por coleção durante a conexão.

### 46. Alertas de consulta

Cada conexão Oracle ou SQL Server tem o nó expansível **Alertas** logo acima de **Programador** ou **SQL Server Agent**, respectivamente. O botão direito no nó oferece **Novo alerta**. As regras ficam aninhadas e o duplo clique abre as propriedades da regra. Cada regra tem nome, intervalo de consulta em minutos, intervalo opcional de repetição, estado ativo ou pausado e um único `SELECT` de uma coluna. No SQL Server a regra aponta para um banco escolhido na lista.

O monitor começa quando a conexão é aberta na árvore e para ao desconectar. A leitura usa uma sessão separada e tenta reabrir se a conexão falhar. Se o banco continuar inacessível, o Cururu registra no log e espera o próximo intervalo. O texto da primeira linha aparece no canto inferior direito, sem tomar o foco. Sem linha ou com texto em branco, o ciclo termina sem aviso. Para escolher a linha, use `ORDER BY` no Oracle ou `TOP (1)` com `ORDER BY` no SQL Server.

O aviso sai quando o texto muda ou volta a aparecer depois de um ciclo sem mensagem. **Repetir após** permite avisar novamente com o mesmo texto na primeira verificação após o prazo. Regras de contagem gravadas em versões anteriores conservam a condição maior que 1 até que se altere a consulta.

O botão direito sobre uma regra oferece **Executar**, inclusive quando estiver pausada. A execução manual mantém a regra pausada, sem retomar o agendamento. Também oferece **Logs**. A janela **Logs de Execução** mostra os 200 registros mais recentes daquela regra, incluindo verificações, falhas de consulta e tentativas de envio ao Teams. **Atualizar** relê o histórico local.

Cada regra pode enviar ao Teams o texto retornado, por um webhook do Power Automate. Para o chat pessoal, o fluxo usa **Postar uma mensagem para mim mesmo** na conta que receberá o aviso. A URL é protegida pelo DPAPI em disco e aparece nas propriedades para ser copiada e reutilizada. A tela avisa para não compartilhar o endereço.

No Power Automate, use o gatilho **Quando uma solicitação de webhook do Teams for recebida**, com a opção **Qualquer pessoa** para gerar a URL assinada. Na ação **Postar uma mensagem para mim mesmo**, selecione conteúdo de texto e use a expressão `triggerBody()?['attachments']?[0]?['content']?['body']?[0]?['text']`. Depois de salvar o fluxo, cole a URL em **Também enviar ao Teams** na regra.
