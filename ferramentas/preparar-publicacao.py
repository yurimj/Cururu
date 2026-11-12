"""Prepara uma cópia dos fontes com histórico novo, sem publicar na internet."""

import argparse
import os
from pathlib import Path
import re
import subprocess


RAIZ = Path(__file__).resolve().parent.parent
NOVOS = (
    "PUBLICACAO.md", "ferramentas/preparar-publicacao.py", "README.md",
    "LICENSE", "LICENCA.md", "TERMOS-ADICIONAIS.txt",
)
EXTENSOES_PRIVADAS = {".pem", ".key", ".pfx", ".p12", ".log"}
NOMES_PRIVADOS = {"conexoes.json", "perfis.json", "alertas.json"}
PADROES = {
    "chave privada": re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
    "token reconhecível": re.compile(
        rb"\b(?:gh[pousr]_[A-Za-z0-9_]{20,}|github_pat_[A-Za-z0-9_]{20,}"
        rb"|AKIA[A-Z0-9]{16}|xox[baprs]-[A-Za-z0-9-]{16,})"
    ),
    "endereço de rede privada": re.compile(
        rb"(?<![\d.])(?:10\.(?:\d{1,3}\.){2}\d{1,3}"
        rb"|192\.168\.\d{1,3}\.\d{1,3}"
        rb"|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3})(?![\d.])"
    ),
}


def git(*argumentos, pasta=RAIZ):
    return subprocess.check_output(["git", *argumentos], cwd=pasta)


def preparar(destino):
    destino = destino.resolve()
    if destino == RAIZ or destino in RAIZ.parents or RAIZ in destino.parents:
        raise ValueError("o destino deve ficar fora do repositório de trabalho")
    if destino.exists():
        raise ValueError("o destino já existe; escolha uma pasta nova")

    nomes = set(os.fsdecode(git("ls-files", "-z")).rstrip("\0").split("\0"))
    nomes.update(NOVOS)
    fontes = []
    problemas = []
    for nome in sorted(nomes):
        if not nome:
            continue
        relativo = Path(nome)
        origem = RAIZ / relativo
        if relativo.is_absolute() or ".." in relativo.parts or ".git" in relativo.parts:
            raise ValueError("caminho inválido no conjunto de fontes")
        if origem.is_symlink() or any(p.is_symlink() for p in origem.parents if p != RAIZ):
            raise ValueError(f"link simbólico não permitido: {nome}")
        if not origem.exists():
            continue  # Um arquivo removido da árvore de trabalho não é exportado.
        if not origem.is_file() or not origem.resolve().is_relative_to(RAIZ):
            raise ValueError(f"arquivo não permitido: {nome}")
        if (origem.suffix.lower() in EXTENSOES_PRIVADAS
                or origem.name.lower() in NOMES_PRIVADOS
                or (origem.name.startswith(".env") and origem.name != ".env.example")):
            problemas.append(f"{nome}: tipo de arquivo privado")
        conteudo = origem.read_bytes()
        if b"\0" not in conteudo[:8000]:
            for descricao, padrao in PADROES.items():
                if padrao.search(conteudo):
                    problemas.append(f"{nome}: {descricao} (valor omitido)")
        fontes.append((relativo, conteudo))
    if problemas:
        raise ValueError("revisar antes da exportação:\n" + "\n".join(problemas))

    destino.mkdir(parents=True)
    for relativo, conteudo in fontes:
        alvo = destino / relativo
        alvo.parent.mkdir(parents=True, exist_ok=True)
        alvo.write_bytes(conteudo)
    git("init", "--initial-branch=main", pasta=destino)
    git("config", "user.name", "yurimj", pasta=destino)
    git("config", "user.email", "yurimj@users.noreply.github.com", pasta=destino)
    git("config", "core.autocrlf", "false", pasta=destino)
    git("add", "--all", pasta=destino)
    git("-c", "commit.gpgsign=false", "commit", "-m",
        "Publica a base inicial dos fontes com exemplos fictícios", pasta=destino)
    assert git("rev-list", "--count", "--all", pasta=destino).strip() == b"1"
    assert not git("remote", pasta=destino).strip()
    assert not git("status", "--porcelain", pasta=destino).strip()
    print(f"Cópia preparada em {destino}: {len(fontes)} arquivos, um commit, sem remoto.")


if __name__ == "__main__":
    argumentos = argparse.ArgumentParser(description=__doc__)
    argumentos.add_argument("--destino", type=Path, required=True)
    try:
        preparar(argumentos.parse_args().destino)
    except (ValueError, subprocess.CalledProcessError) as erro:
        raise SystemExit(str(erro))
