import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { command } from '../../tools/ci/pipeline/targets/common.mjs';

test('pipeline identifies the command that makes a full checkout shallow', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-history-'));
  const origin = path.join(root, 'origin');
  const checkout = path.join(root, 'checkout');
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  try {
    git('init', origin);
    git('-C', origin, 'config', 'user.name', 'History test');
    git('-C', origin, 'config', 'user.email', 'history@example.invalid');
    git('-C', origin, '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'first');
    git('-C', origin, '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'second');
    const head = git('-C', origin, 'rev-parse', 'HEAD');
    git('clone', '--no-local', origin, checkout);
    assert.equal(git('-C', checkout, 'rev-parse', '--is-shallow-repository'), 'false');
    command('git', ['rev-parse', 'HEAD'], { cwd: checkout, stdio: 'pipe' });
    assert.throws(() => command('git', ['fetch', '--depth', '1', 'origin', head], {
      cwd: checkout, stdio: 'pipe',
    }), /made the full source checkout shallow: git fetch --depth 1/);
    assert.equal(git('-C', checkout, 'rev-parse', '--is-shallow-repository'), 'true');
  } finally { rmSync(root, { recursive: true, force: true }); }
});
