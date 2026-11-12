# Cururu

Cliente de bancos de dados para Windows, desenvolvido com Tauri, TypeScript e
Rust. Oferece editor SQL, árvore de objetos, grid de resultados e conexões com
Oracle, SQL Server e MongoDB.

Criado por **[@yurijiacomo](https://www.instagram.com/yurijiacomo)**.

## Código-fonte e downloads

- [Código-fonte público](https://github.com/yurimj/Cururu)
- [Downloads e releases](https://github.com/yurimj/Cururu-releases/releases)

Este repositório contém os fontes com exemplos fictícios e histórico iniciado
para a publicação. Dados locais e bibliotecas proprietárias não fazem parte dele.

## Como usar

Baixe o instalador na página de [downloads e releases](https://github.com/yurimj/Cururu-releases/releases),
instale e abra o Cururu pelo atalho criado. Se recebeu a pasta portátil, mantenha
os arquivos juntos e execute `cururu.exe`.

A interface faz parte do aplicativo. Para usar o Cururu, não é necessário abrir
um endereço no navegador nem instalar Node.js ou Rust.

## Desenvolvimento

As instruções abaixo são para quem vai alterar o código-fonte.

Instale Node.js, Rust e os pré-requisitos do Tauri para Windows. Depois:

```powershell
npm ci
npm run dev
```

Esse comando inicia apenas o servidor de desenvolvimento da interface, disponível
em `http://localhost:1420` para verificação no navegador. Esse endereço é usado
somente durante o desenvolvimento. No navegador, o aviso de ausência da ponte
`invoke` do Tauri é esperado; as operações de banco exigem o aplicativo nativo.
Para desenvolver o aplicativo nativo:

```powershell
npm run tauri dev
```

Conexões Oracle exigem o Instant Client Basic, obtido separadamente da Oracle.
Consulte [premissas.md](premissas.md) para localização, configuração e condições
de redistribuição. [AGENTS.md](AGENTS.md) descreve as verificações e os builds;
[requisitos.md](requisitos.md) documenta o comportamento do produto.

## Licença

GPLv3 com preservação do crédito de autoria na interface e permissão adicional
para ligação com o Oracle Instant Client. Consulte [LICENCA.md](LICENCA.md),
[LICENSE](LICENSE) e [TERMOS-ADICIONAIS.txt](TERMOS-ADICIONAIS.txt).
