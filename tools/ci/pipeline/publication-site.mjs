import { cp, mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

// The caller authenticates the complete bundle before composition. Never reuse
// an old output directory: stale pages must not satisfy this source's links.
export async function preparePublicationSite({ directory, source, command = spawnSync }) {
  if (!/^[a-f0-9]{40}$/.test(source ?? '')) throw new Error('Invalid Pages source identity');
  const site = path.join(await mkdtemp(path.join(os.tmpdir(), 'conduit-publication-site-')), 'site');
  await cp(path.join(directory, 'browser', 'site'), site, { recursive: true, errorOnExist: true, force: false });
  const scripts = [
    ['stage-conduitos-pages-evidence.mjs', path.join(directory, 'conduitos-x86_64', 'evidence', 'journey-frames'), site, source],
    ['verify-site-links.mjs', site],
  ];
  for (const [script, ...args] of scripts) {
    const result = command(process.execPath, [fileURLToPath(new URL(`../${script}`, import.meta.url)), ...args],
      { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
    if (result.error || result.status !== 0) {
      throw new Error(`Pages preparation ${script} failed: ${result.error?.message ?? result.stderr ?? result.status}; retained ${site}`);
    }
  }
  return site;
}
