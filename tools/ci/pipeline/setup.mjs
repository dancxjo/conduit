import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

function run(program, args) {
  const result = spawnSync(program, args, { stdio: 'inherit' });
  if (result.error || result.status !== 0) throw new Error(`setup ${program} failed: ${result.error ?? result.status}`);
}
export async function setupCi() {
  if (process.platform !== 'linux' || process.arch !== 'x64') throw new Error('CI validation tools require Linux x64');
  const directory = resolve('target/ci-tools');
  mkdirSync(directory, { recursive: true });
  const version = '1.7.7';
  const archive = `actionlint_${version}_linux_amd64.tar.gz`;
  const base = `https://github.com/rhysd/actionlint/releases/download/v${version}`;
  for (const name of [archive, `actionlint_${version}_checksums.txt`]) {
    const response = await fetch(`${base}/${name}`);
    if (!response.ok) throw new Error(`actionlint acquisition failed: HTTP ${response.status}`);
    writeFileSync(`${directory}/${name}`, Buffer.from(await response.arrayBuffer()));
  }
  const expected = readFileSync(`${directory}/actionlint_${version}_checksums.txt`, 'utf8').split('\n').find(line => line.endsWith(`  ${archive}`))?.split(' ')[0];
  const actual = createHash('sha256').update(readFileSync(`${directory}/${archive}`)).digest('hex');
  if (!expected || actual !== expected) throw new Error('actionlint checksum mismatch');
  run('tar', ['-xzf', `${directory}/${archive}`, '-C', directory, 'actionlint']);
}
export function setupUnit() {
  if (process.platform !== 'linux') throw new Error('unit runner requires Linux');
  run('sudo', ['apt-get', 'update', '-o', 'Acquire::Retries=1']);
  run('sudo', ['apt-get', 'install', '-y', '--no-install-recommends', 'build-essential', 'pkg-config', 'libasound2-dev', 'libudev-dev', 'libssl-dev', 'cmake', 'libclang-dev']);
}
