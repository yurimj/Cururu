# Preparação dos fontes para publicação

O histórico do repositório de trabalho é privado. A publicação dos fontes deve
partir de uma cópia saneada com histórico novo, sem alterar a visibilidade do
repositório de trabalho e sem enviar suas branches ou etiquetas.

Na raiz do projeto, depois de revisar as alterações e executar as verificações:

```powershell
python ferramentas/preparar-publicacao.py --destino ../Cururu-publico
```

O destino precisa ser uma pasta nova, fora do repositório de trabalho. O script:

- copia o conteúdo atual dos arquivos rastreados que ainda existem, incluindo
  alterações ainda não commitadas;
- inclui explicitamente este documento, o próprio script, o README e os arquivos
  de licença, mesmo antes do primeiro commit deles; outros arquivos novos
  precisam ser adicionados ao Git;
- recusa links simbólicos, arquivos locais de credenciais e alguns padrões
  reconhecíveis de segredos e endereços privados;
- cria um repositório independente, com um único commit e endereço `noreply`;
- não configura remoto, não faz push e não publica nada na internet.

O `.git` original, seus metadados e os arquivos não rastreados ou ignorados não
são copiados. Isso exclui dependências instaladas, builds, logs, dados locais e
o Instant Client. As bibliotecas da Oracle continuam sob sua licença própria.

As verificações automáticas são uma barreira adicional: revisar os exemplos,
documentos e imagens continua necessário. Nomes de tabelas e regras de negócio
reais não devem entrar em testes; use exemplos fictícios. O teste manual de
relações Oracle recebe `CURURU_PERFIL_TESTE` e `CURURU_OBJETO_TESTE` pelo ambiente,
sem gravar a identidade do ambiente no código.

Para repetir a preparação, escolha outro destino novo. Não copie a pasta de
trabalho inteira e não envie o histórico privado ao repositório público.
