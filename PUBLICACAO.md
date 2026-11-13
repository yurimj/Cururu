# Publicação pelo GitHub Actions

O workflow [Publicar Cururu](https://github.com/yurimj/Cururu/actions/workflows/publicar.yml)
compila os fontes de uma etiqueta em um runner Windows do GitHub usando
`tauri-apps/tauri-action`. Não utiliza o executável da máquina do desenvolvedor.
O instalador fica em `yurimj/Cururu`, junto dos fontes, acompanhado por
`SHA256SUMS.txt` e `proveniencia.jsonl`. Existe apenas um anexo `.exe`, para
preservar a consulta de atualização do aplicativo.

## Autenticação

O workflow usa o `GITHUB_TOKEN` temporário fornecido pelo GitHub, com
`contents: write` para publicar no próprio repositório e permissões de OIDC e
atestação. Não é necessário criar token pessoal nem secret de publicação.
Esse token existe apenas durante a execução e não entra no aplicativo.

Para passar a receber atualizações deste repositório, versões anteriores à
migração precisam da instalação manual da primeira versão com o novo endereço.

## Publicar uma versão

1. Atualize a versão nos cinco arquivos descritos em `AGENTS.md` e registre as
   mudanças em `versoes.md`. Envie os fontes revisados ao repositório.
2. Crie e envie a etiqueta correspondente ao commit, por exemplo:
   `git tag v0.1.105` e `git push origin v0.1.105`.
3. Abra o workflow, escolha **Run workflow** e selecione a própria etiqueta no
   campo **Use workflow from**. Informe a mesma etiqueta no formulário e marque
   **Gerar e publicar o instalador desta versão**. Também é possível usar:
   `gh workflow run publicar.yml --ref v0.1.105 -f etiqueta=v0.1.105 -f gerar_instalador=true`.
4. Confira a execução e o release. O workflow publica somente depois de
   compilar, calcular o checksum e gerar a atestação. Se o upload falhar,
   permanece um rascunho para inspeção; a repetição recusa etiquetas que já
   possuam release. Remova apenas o rascunho incompleto antes de tentar novamente.

O acionamento é manual: um push comum não gera instalador. A etiqueta precisa
conter também os arquivos deste workflow e de `ferramentas/publicacao/`.
A execução recusa branches ou etiquetas divergentes, e o checkout usa o SHA
do evento para a atestação identificar exatamente o commit compilado.
As notas incluem todas as seções desde a última versão estável publicada,
além de links para o commit e a execução. Versões antigas não são sobrescritas.
Nenhuma versão é incrementada automaticamente pelo workflow.

O Oracle Instant Client 19.31 é baixado diretamente da Oracle, com SHA-256
fixado no workflow e preservação de `BASIC_LICENSE`. A atualização dessa
dependência exige revisar URL, checksum e configuração de recursos juntos.
As Actions também estão fixadas por commit para permitir revisão das mudanças.

## Conferir um download

Baixe o instalador, `SHA256SUMS.txt` e `proveniencia.jsonl` do mesmo release.
No PowerShell, compare o resultado com a linha de `SHA256SUMS.txt`:

```powershell
Get-FileHash .\Cururu_0.1.105_x64-setup.exe -Algorithm SHA256
```

Para verificar a origem usando o GitHub CLI:

```powershell
gh attestation verify .\Cururu_0.1.105_x64-setup.exe --repo yurimj/Cururu --bundle .\proveniencia.jsonl --signer-workflow yurimj/Cururu/.github/workflows/publicar.yml
```

Confira também o commit dos fontes no release e a etiqueta na execução.
O checksum detecta alteração do arquivo; a atestação vincula esse arquivo ao
build do GitHub e aos fontes utilizados. Isso não promete builds idênticos bit
a bit nem substitui uma assinatura Authenticode do Windows.
