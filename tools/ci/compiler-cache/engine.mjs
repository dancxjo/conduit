import { createHash } from 'node:crypto';
import { promises as fs } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { CacheRefusal } from './refusal.mjs';

// Engine contracts: https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md
// https://github.com/mozilla/sccache/blob/v0.19.0/docs/Local.md
// Foreground custody uses SCCACHE_START_SERVER + SCCACHE_NO_DAEMON from
// pinned src/cmdline.rs and src/util.rs; this is deliberately version-specific.
export const SCCACHE_ENGINE = Object.freeze({
  version: '0.19.0',
  url: 'https://github.com/mozilla/sccache/releases/download/v0.19.0/sccache-v0.19.0-x86_64-unknown-linux-musl.tar.gz',
  archiveSha256: 'aa73c7270c997894038c0d8f0d297865f57569e1687883322bc431713d4bb9dd',
  binarySha256: '0b44834be857e21b026f7db49a7762023fedb5eb5ed60725a66f698d8d79a34b',
  member: 'sccache-v0.19.0-x86_64-unknown-linux-musl/sccache',
});
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const refuse = code => { throw new CacheRefusal(code); };

// Explicit task-local acquisition. No global PATH/config installation and no
// unauthenticated "latest" resolution: both archive and executable are pinned.
export async function acquirePinnedEngine(destination) {
  if (process.platform !== 'linux' || process.arch !== 'x64' || !path.isAbsolute(destination)) refuse('unsupported-engine-platform');
  await fs.mkdir(destination, { mode: 0o700 });
  const response = await fetch(SCCACHE_ENGINE.url, { signal: AbortSignal.timeout(60000) });
  if (!response.ok) refuse('engine-download-failed');
  let size = 0; const chunks = [];
  for await (const chunk of response.body) {
    size += chunk.length; if (size > 16 * 1024 * 1024) refuse('engine-download-bound'); chunks.push(chunk);
  }
  const archive = Buffer.concat(chunks);
  if (digest(archive) !== SCCACHE_ENGINE.archiveSha256) refuse('engine-archive-mismatch');
  const archivePath = path.join(destination, 'sccache.tar.gz');
  await fs.writeFile(archivePath, archive, { flag: 'wx', mode: 0o600 });
  // tar streams only the named member; no archive paths are extracted.
  const binary = path.join(destination, 'sccache');
  await new Promise((resolve, reject) => {
    const child = spawn('/usr/bin/tar', ['-xOzf', archivePath, SCCACHE_ENGINE.member], { stdio: ['ignore', 'pipe', 'pipe'] });
    const parts = []; let bytes = 0; let failure;
    child.stdout.on('data', chunk => { bytes += chunk.length; if (bytes > 32 * 1024 * 1024) { failure = new CacheRefusal('engine-extraction-bound'); child.kill(); } else parts.push(chunk); });
    child.stderr.resume(); child.on('error', reject);
    child.on('close', async code => {
      try {
        if (failure) throw failure;
        if (code !== 0) refuse('engine-extraction-failed');
        const executable = Buffer.concat(parts);
        if (digest(executable) !== SCCACHE_ENGINE.binarySha256) refuse('engine-binary-mismatch');
        await fs.writeFile(binary, executable, { flag: 'wx', mode: 0o700 }); resolve();
      } catch (error) { reject(error); }
    });
  });
  await fs.writeFile(path.join(destination, 'acquisition.json'), JSON.stringify(SCCACHE_ENGINE, null, 2), { flag: 'wx', mode: 0o600 });
  return binary;
}
