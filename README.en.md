# Cururu

[Português](README.md) | [English](README.en.md)

A database client for Windows, built with Tauri, TypeScript, and Rust. It provides
a SQL editor, an object tree, a results grid, and connections to Oracle,
SQL Server, and MongoDB.

Created by **[@yurijiacomo](https://www.instagram.com/yurijiacomo)**.

## Source code and downloads

- [Public source code](https://github.com/yurimj/Cururu)
- [Downloads and releases](https://github.com/yurimj/Cururu/releases)

This repository contains the source code with fictional examples and a history
started for public distribution. Local data and proprietary libraries are not
included.

## Getting started

Download the installer from the [downloads and releases page](https://github.com/yurimj/Cururu/releases),
install it, and open Cururu using the shortcut created during installation.
If you received the portable folder, keep its files together and run `cururu.exe`.

The interface is included in the application. You do not need to open an address
in your browser or install Node.js or Rust to use Cururu.

## Development

The instructions below are for developers who want to modify the source code.

Install Node.js, Rust, and the Tauri prerequisites for Windows. Then run:

```powershell
npm ci
npm run dev
```

This command starts only the interface development server, available at
`http://localhost:1420` for browser checks. This address is used only during
development. In the browser, a warning about the missing Tauri `invoke` bridge
is expected; database operations require the native application.
To develop the native application, run:

```powershell
npm run tauri dev
```

Oracle connections require Instant Client Basic, obtained separately from Oracle.
See [premissas.md](premissas.md) for its location, configuration, and redistribution
conditions. [AGENTS.md](AGENTS.md) describes checks and builds;
[requisitos.md](requisitos.md) documents the product's behavior. These documents
are written in Portuguese.

Builds published through GitHub Actions include SHA-256 checksums and GitHub
build provenance attestations. See [publication and verification instructions](PUBLICACAO.md)
(in Portuguese).

## License

GPLv3, with preservation of the authorship credit in the interface and additional
permission to link with Oracle Instant Client. See [LICENCA.md](LICENCA.md),
[LICENSE](LICENSE), and [TERMOS-ADICIONAIS.txt](TERMOS-ADICIONAIS.txt).
