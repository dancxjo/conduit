import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { apt, command } from './targets/common.mjs';
import { COMMON_APT, cacheRoot } from './acquisition/tools.mjs';
import { recordOperation, recordTool } from './acquisition/metrics.mjs';

export const ACTIONLINT = Object.freeze({
  version: '1.7.7',
  archive: 'actionlint_1.7.7_linux_amd64.tar.gz',
  sha256: '023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757',
});
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export async function setupCi() {
  const started = performance.now();
  if (process.platform !== 'linux' || process.arch !== 'x64') throw new Error('CI validation tools require Linux x64');
  const directory = join(cacheRoot(), 'actionlint', ACTIONLINT.version);
  const archive = join(directory, ACTIONLINT.archive);
  mkdirSync(directory, { recursive: true });
  const warm = existsSync(archive) && sha256(readFileSync(archive)) === ACTIONLINT.sha256;
  if (!warm) {
    const url = `https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT.version}/${ACTIONLINT.archive}`;
    for (let attempt = 1; attempt <= 2; attempt++) {
      const start = performance.now();
      try {
        const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
        if (!response.ok) throw new Error(`actionlint acquisition failed: HTTP ${response.status}`);
        const bytes = Buffer.from(await response.arrayBuffer());
        if (sha256(bytes) !== ACTIONLINT.sha256) throw new Error('actionlint checksum mismatch');
        writeFileSync(archive, bytes);
        recordOperation({ kind: 'download', program: 'fetch', args: [url], durationMs: performance.now() - start, outcome: 'success', attempt, downloadedBytes: bytes.length, cacheHit: false });
        break;
      } catch (error) {
        recordOperation({ kind: 'download', program: 'fetch', args: [url], durationMs: performance.now() - start, outcome: 'failure', attempt });
        if (attempt === 2) throw error;
      }
    }
  }
  // Verify cached executable against the pinned archive, not mutable metadata.
  const expected = command('tar', ['-xOf', archive, 'actionlint'], { stdio: ['ignore', 'pipe', 'inherit'], maxBuffer: 32 * 1024 * 1024 }).stdout;
  const binary = join(directory, 'actionlint');
  if (!existsSync(binary) || sha256(readFileSync(binary)) !== sha256(expected)) writeFileSync(binary, expected);
  chmodSync(binary, 0o755);
  const version = command(binary, ['-version'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).stdout.trim().split('\n')[0];
  if (version !== ACTIONLINT.version) throw new Error('actionlint executable version mismatch');
  const destination = resolve('target/ci-tools');
  mkdirSync(destination, { recursive: true });
  copyFileSync(binary, join(destination, 'actionlint'));
  chmodSync(join(destination, 'actionlint'), 0o755);
  recordOperation({ kind: 'cache', program: 'actionlint', args: [], durationMs: performance.now() - started, outcome: 'success', cacheHit: warm });
  recordTool('actionlint', { version, archiveSha256: ACTIONLINT.sha256, executableSha256: sha256(expected) });
}
export function setupUnit() {
  if (process.platform !== 'linux') throw new Error('unit runner requires Linux');
  apt(...COMMON_APT);
}
