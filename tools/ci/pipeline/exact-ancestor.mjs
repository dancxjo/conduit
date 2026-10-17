import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';

const COMMIT = /^[a-f0-9]{40}$/;
const MAX_WALK_BYTES = 64 * 1024 * 1024;

/** Admit ancestry only after a complete, unpruned walk of actual commit objects. */
function checkExactAncestor(source, publication, cwd) {
  if (!COMMIT.test(source ?? '') || !COMMIT.test(publication ?? '')) {
    throw new Error('Exact ancestry requires full lowercase 40-character commit SHAs');
  }
  if (process.env.GIT_GRAFT_FILE || process.env.GIT_SHALLOW_FILE) {
    throw new Error('Exact ancestry refuses external graft or shallow files');
  }
  const git = args => {
    const result = spawnSync('git', ['--no-replace-objects', '-c', 'core.commitGraph=false', ...args], {
      cwd, encoding: 'utf8', maxBuffer: MAX_WALK_BYTES, timeout: 120_000,
      env: { ...process.env, GIT_NO_LAZY_FETCH: '1' },
    });
    if (result.error || result.signal || result.status !== 0) {
      throw new Error(`Exact ancestry Git failed (${result.status ?? result.signal ?? 'spawn'}): ${result.error?.message ?? result.stderr?.trim() ?? ''}`);
    }
    return result.stdout;
  };
  if (git(['rev-parse', '--is-shallow-repository']).trim() !== 'false') {
    throw new Error('Exact ancestry requires a non-shallow repository');
  }
  const graft = git(['rev-parse', '--git-path', 'info/grafts']).trim();
  if (existsSync(path.resolve(cwd, graft))) throw new Error('Exact ancestry refuses graft files');
  for (const commit of [source, publication]) {
    if (git(['cat-file', '-t', commit]).trim() !== 'commit') {
      throw new Error('Exact ancestry requires commit objects');
    }
  }
  // No range/exclusion/date pruning. Do not accept an early match: a later
  // missing parent, truncated output or failed process must still refuse.
  const walk = git(['rev-list', '--parents', '--full-history', publication]);
  if (Buffer.byteLength(walk, 'utf8') > MAX_WALK_BYTES) throw new Error('Exact ancestry walk exceeds bound');
  let found = false;
  for (const line of walk.trimEnd().split('\n')) {
    const tokens = line.split(' ');
    if (!tokens.length || tokens.some(token => !COMMIT.test(token))) {
      throw new Error('Exact ancestry received malformed commit walk');
    }
    if (tokens[0] === source) found = true;
  }
  if (!found) throw new Error(`Commit ${source} is not in publication ${publication} ancestry`);
}

/** Safe checkout diagnostics accompany refusal; no environment or remote URLs. */
export function requireExactAncestor(source, publication, { cwd = process.cwd() } = {}) {
  try { checkExactAncestor(source, publication, cwd); }
  catch (cause) {
    const diagnostic = args => {
      const result = spawnSync('git', ['--no-replace-objects', '-c', 'core.commitGraph=false', ...args], {
        cwd, encoding: 'utf8', maxBuffer: 4096, timeout: 5000,
        env: { ...process.env, GIT_NO_LAZY_FETCH: '1' },
      });
      return result.status === 0 ? result.stdout.trim() : 'unavailable';
    };
    throw new Error(`${cause.message}; git=${diagnostic(['--version'])}; shallow=${diagnostic(['rev-parse', '--is-shallow-repository'])}; topdir=${diagnostic(['rev-parse', '--show-toplevel'])}`, { cause });
  }
}
