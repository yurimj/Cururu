// Valida os fontes antes do build e usa versoes.md como origem das notas.
import { readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const ler = (caminho) => readFileSync(caminho, 'utf8');
const json = (caminho) => JSON.parse(ler(caminho));
const etiqueta = process.env.ETIQUETA;
if (!/^v\d+\.\d+\.\d+$/.test(etiqueta ?? '')) throw new Error('Etiqueta inválida.');
const versao = etiqueta.slice(1);
const pacote = json('package.json');
const lock = json('package-lock.json');
const cargo = ler('src-tauri/Cargo.toml').match(/\[package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/)[1];
const cargoLock = ler('src-tauri/Cargo.lock').match(/\[\[package\]\]\s*name = "cururu"\s*version = "([^"]+)"/)[1];
for (const atual of [pacote.version, lock.version, lock.packages[''].version, cargo, cargoLock, json('src-tauri/tauri.conf.json').version]) {
  if (atual !== versao) throw new Error(`Versão ${atual} diverge da etiqueta ${etiqueta}.`);
}

const repositorio = process.env.REPOSITORIO_RELEASES;
if (repositorio !== 'yurimj/Cururu') throw new Error('Destino de publicação inesperado.');
const resposta = await fetch(`https://api.github.com/repos/${repositorio}/releases?per_page=100`, {
  headers: { Authorization: `Bearer ${process.env.GH_TOKEN}`, Accept: 'application/vnd.github+json' },
});
if (!resposta.ok) throw new Error(`Falha ao consultar releases: HTTP ${resposta.status}`);
const releases = await resposta.json();
if (releases.some((release) => release.tag_name === etiqueta)) {
  throw new Error('Esta etiqueta já possui release (inclusive rascunho). Não substituir arquivos publicados.');
}
const numeros = (valor) => valor.replace(/^v/, '').split('.').map(Number);
const comparar = (a, b) => {
  const esquerda = numeros(a), direita = numeros(b);
  for (let i = 0; i < 3; i++) if (esquerda[i] !== direita[i]) return esquerda[i] - direita[i];
  return 0;
};
const anteriores = releases.filter((r) => !r.draft && !r.prerelease && /^v\d+\.\d+\.\d+$/.test(r.tag_name));
anteriores.sort((a, b) => comparar(b.tag_name, a.tag_name));
const anterior = anteriores[0]?.tag_name;
if (anterior && comparar(etiqueta, anterior) <= 0) throw new Error('A versão deve superar a última publicada.');
const secoes = [...ler('versoes.md').matchAll(/^## (\d+\.\d+\.\d+) — [^\n]+\n([\s\S]*?)(?=^## |$(?![\s\S]))/gm)];
if (!secoes.some((s) => s[1] === versao)) throw new Error('Versão ausente de versoes.md.');
if (anterior && !secoes.some((s) => s[1] === anterior.slice(1))) throw new Error('Versão anterior ausente de versoes.md.');
const notas = secoes.filter((s) => comparar(s[1], versao) <= 0 && (!anterior || comparar(s[1], anterior) > 0)).map((s) => s[0].trim()).join('\n\n');
const commit = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
const origem = `https://github.com/yurimj/Cururu/tree/${commit}`;
const execucao = `https://github.com/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`;
writeFileSync('notas-release.md', `${notas}\n\n---\nFontes: ${origem}\n\nBuild e atestação: ${execucao}\n\nSHA-256: consulte SHA256SUMS.txt. Instruções de verificação: https://github.com/yurimj/Cururu/blob/${commit}/PUBLICACAO.md\n`);
