import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const programa = new URL('./preparar.mjs', import.meta.url).href;
const diretorioGit = execFileSync('git', ['rev-parse', '--absolute-git-dir'], { encoding: 'utf8' }).trim();

function executar({ releases = [], etiqueta = 'v0.1.3', cargo = '0.1.3', notas, status = 200 } = {}) {
  const pasta = mkdtempSync(join(tmpdir(), 'cururu-publicacao-'));
  try {
    mkdirSync(join(pasta, 'src-tauri'));
    const gravar = (nome, conteudo) => writeFileSync(join(pasta, nome), conteudo);
    gravar('package.json', JSON.stringify({ version: '0.1.3' }));
    gravar('package-lock.json', JSON.stringify({ version: '0.1.3', packages: { '': { version: '0.1.3' } } }));
    gravar('src-tauri/tauri.conf.json', JSON.stringify({ version: '0.1.3' }));
    gravar('src-tauri/Cargo.toml', `[package]\nname = "cururu"\nversion = "${cargo}"\n`);
    gravar('src-tauri/Cargo.lock', '[[package]]\nname = "cururu"\nversion = "0.1.3"\n');
    gravar('versoes.md', notas ?? '# Versões\n\n## 0.1.3 — 13/11/2026\n\n- Três.\n\n## 0.1.2 — 12/11/2026\n\n- Dois.\n\n## 0.1.1 — 11/11/2026\n\n- Um.\n');
    const codigo = `globalThis.fetch = async () => ({ ok: ${status === 200}, status: ${status}, json: async () => ${JSON.stringify(releases)} }); await import(${JSON.stringify(programa)});`;
    const resultado = spawnSync(process.execPath, ['--input-type=module', '-e', codigo], {
      cwd: pasta, encoding: 'utf8',
      env: { ...process.env, GIT_DIR: diretorioGit, ETIQUETA: etiqueta, REPOSITORIO_RELEASES: 'yurimj/Cururu', GH_TOKEN: 'ficticio', GITHUB_REPOSITORY: 'yurimj/Cururu', GITHUB_RUN_ID: '123' },
    });
    return { ...resultado, notas: resultado.status === 0 ? readFileSync(join(pasta, 'notas-release.md'), 'utf8') : '' };
  } finally {
    rmSync(pasta, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

test('inclui todas as versões desde a anterior e links de origem', () => {
  const resultado = executar({ releases: [{ tag_name: 'v0.1.1' }] });
  assert.equal(resultado.status, 0, resultado.stderr);
  assert.match(resultado.notas, /- Três\./);
  assert.match(resultado.notas, /- Dois\./);
  assert.doesNotMatch(resultado.notas, /- Um\./);
  assert.match(resultado.notas, /Cururu\/tree\/[a-f0-9]{40}/);
  assert.match(resultado.notas, /actions\/runs\/123/);
});

test('primeiro release inclui o histórico disponível', () => {
  const resultado = executar();
  assert.equal(resultado.status, 0, resultado.stderr);
  assert.match(resultado.notas, /- Um\./);
});

for (const [nome, parametros, mensagem] of [
  ['versões divergentes', { cargo: '0.1.2' }, /diverge/],
  ['etiqueta inválida', { etiqueta: 'main' }, /Etiqueta inválida/],
  ['release já existente', { releases: [{ tag_name: 'v0.1.3', draft: true }] }, /já possui release/],
  ['versão regressiva', { releases: [{ tag_name: 'v0.1.4' }] }, /superar/],
  ['ausência de notas', { notas: '# Versões\n' }, /ausente de versoes/],
  ['falha na API', { status: 403 }, /HTTP 403/],
]) {
  test(`recusa ${nome}`, () => {
    const resultado = executar(parametros);
    assert.notEqual(resultado.status, 0);
    assert.match(resultado.stderr, mensagem);
  });
}
