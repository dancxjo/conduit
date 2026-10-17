import { spawnSync } from 'node:child_process';
import { existsSync, openSync, readSync, closeSync, statSync } from 'node:fs';
import path from 'node:path';

const SHA = /^[a-f0-9]{40}$/;
function git(cwd, args, { acquisition = false } = {}) {
  const result = spawnSync('git', ['--no-replace-objects', '-c', 'core.commitGraph=false', ...args], {
    cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 120_000,
    env: { ...process.env, GIT_TERMINAL_PROMPT: '0', ...(!acquisition && { GIT_NO_LAZY_FETCH: '1' }) },
  });
  // Do not print fetch stderr: remote configuration may contain credentials.
  if (result.error || result.signal || result.status !== 0) {
    throw new Error(`Publication history Git ${args[0]} failed: status=${result.status ?? 'none'}, signal=${result.signal ?? 'none'}, code=${result.error?.code ?? 'none'}`);
  }
  return result.stdout.trim();
}

export function publicationHistorySnapshot(stage, cwd = process.cwd()) {
  const commonDir = path.resolve(cwd, git(cwd, ['rev-parse', '--git-common-dir']));
  const shallowPath = path.resolve(cwd, git(cwd, ['rev-parse', '--git-path', 'shallow']));
  let boundary = null;
  if (existsSync(shallowPath)) {
    const stat = statSync(shallowPath);
    const buffer = Buffer.alloc(64 * 1024);
    const fd = openSync(shallowPath, 'r');
    let length;
    try { length = readSync(fd, buffer, 0, buffer.length, 0); }
    finally { closeSync(fd); }
    const capped = stat.size > buffer.length;
    const lines = buffer.subarray(0, length).toString('utf8').trimEnd().split('\n').filter(Boolean);
    boundary = { bytes: stat.size, mtimeMs: stat.mtimeMs, inode: stat.ino, capped,
      count: capped ? null : lines.length, sha: lines.filter(value => SHA.test(value)).slice(0, 16) };
  }
  const snapshot = { stage, git: git(cwd, ['--version']), head: git(cwd, ['rev-parse', 'HEAD']),
    shallow: git(cwd, ['rev-parse', '--is-shallow-repository']), commonDir, boundary };
  console.log(`Publication history: ${JSON.stringify(snapshot)}`);
  return snapshot;
}

/** Acquire complete publication ancestry explicitly, never relax its verifier. */
export function completePublicationHistory(expectedHead, { cwd = process.cwd() } = {}) {
  if (typeof expectedHead !== 'string' || expectedHead.length !== 40 || !SHA.test(expectedHead)) {
    throw new Error('Publication history requires an exact full commit SHA');
  }
  const before = publicationHistorySnapshot('before-acquisition', cwd);
  if (before.head !== expectedHead) throw new Error('Publication history HEAD mismatch before acquisition');
  if (before.shallow === 'true') {
    git(cwd, ['fetch', '--unshallow', '--no-tags', 'origin'], { acquisition: true });
  } else if (before.shallow !== 'false') throw new Error('Publication history has unknown shallow state');
  const after = publicationHistorySnapshot('post-acquisition', cwd);
  if (after.head !== expectedHead) throw new Error('Publication history HEAD changed during acquisition');
  if (after.shallow !== 'false') throw new Error('Publication history remains shallow after acquisition');
}
