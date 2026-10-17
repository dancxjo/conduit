import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { completePublicationHistory, publicationHistorySnapshot } from '../../tools/ci/pipeline/complete-publication-history.mjs';
import { requireExactAncestor } from '../../tools/ci/pipeline/exact-ancestor.mjs';

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-publication-history-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (cwd, args, options = {}) => execFileSync('git', args, { cwd, encoding: 'utf8', ...options }).trim();
  git(root, ['init', '-q']);
  const tree = git(root, ['mktree'], { input: '' });
  const env = { ...process.env, GIT_AUTHOR_NAME: 'Proof', GIT_AUTHOR_EMAIL: 'proof@example.invalid',
    GIT_COMMITTER_NAME: 'Proof', GIT_COMMITTER_EMAIL: 'proof@example.invalid' };
  const ancestor = git(root, ['commit-tree', tree], { input: 'root\n', env });
  const head = git(root, ['commit-tree', tree, '-p', ancestor], { input: 'head\n', env });
  git(root, ['update-ref', 'refs/heads/main', head]);git(root, ['symbolic-ref', 'HEAD', 'refs/heads/main']);
  const clone = path.join(root, 'shallow');
  git(root, ['clone', '-q', '--depth=1', `file://${root}`, clone]);
  return { root, clone, ancestor, head, git };
}

test('genuine shallow file-origin restoration admits exact ancestor and retains HEAD', t => {
  const f = fixture(t);
  const before = publicationHistorySnapshot('fixture-before', f.clone);
  assert.equal(before.shallow, 'true');assert.equal(before.boundary.count, 1);
  assert.deepEqual(before.boundary.sha, [f.head]);assert.ok(before.boundary.inode > 0);
  assert.throws(() => requireExactAncestor(f.head, f.head, { cwd: f.clone }), /non-shallow/);
  completePublicationHistory(f.head, { cwd: f.clone });
  assert.equal(f.git(f.clone, ['rev-parse', 'HEAD']), f.head);
  assert.equal(f.git(f.clone, ['rev-parse', '--is-shallow-repository']), 'false');
  assert.doesNotThrow(() => requireExactAncestor(f.ancestor, f.head, { cwd: f.clone }));
});

for (const missing of [true, false]) {
  test(`shallow ${missing ? 'missing' : 'invalid'} origin refuses without changing HEAD`, t => {
    const f = fixture(t);
    if (missing) f.git(f.clone, ['remote', 'remove', 'origin']);
    else f.git(f.clone, ['remote', 'set-url', 'origin', path.join(f.root, 'absent-origin')]);
    assert.throws(() => completePublicationHistory(f.head, { cwd: f.clone }), /fetch failed/);
    assert.equal(f.git(f.clone, ['rev-parse', 'HEAD']), f.head);
    assert.equal(f.git(f.clone, ['rev-parse', '--is-shallow-repository']), 'true');
  });
}

test('complete repository with invalid origin performs no fetch', t => {
  const f = fixture(t);
  f.git(f.root, ['remote', 'add', 'origin', path.join(f.root, 'absent-origin')]);
  assert.doesNotThrow(() => completePublicationHistory(f.head, { cwd: f.root }));
  assert.equal(f.git(f.root, ['rev-parse', 'HEAD']), f.head);
});

test('HEAD mismatch and noncanonical identity refuse before any fetch', t => {
  const f = fixture(t);f.git(f.clone, ['remote', 'remove', 'origin']);
  assert.throws(() => completePublicationHistory(f.ancestor, { cwd: f.clone }), /HEAD mismatch before/);
  assert.throws(() => completePublicationHistory('HEAD', { cwd: f.clone }), /exact full/);
  assert.equal(f.git(f.clone, ['rev-parse', '--is-shallow-repository']), 'true');
});
