import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { requireExactAncestor } from '../../tools/ci/pipeline/exact-ancestor.mjs';

function fixture(t) {
  const cwd = mkdtempSync(path.join(tmpdir(), 'conduit-exact-ancestor-'));
  t.after(() => rmSync(cwd, { recursive: true, force: true }));
  const git = (args, options = {}) => execFileSync('git', args, { cwd, encoding: 'utf8', ...options }).trim();
  git(['init', '-q']);
  const tree = git(['mktree'], { input: '' });
  const commit = (parents = [], date = '2020-01-01T00:00:00Z') => git(
    ['commit-tree', tree, ...parents.flatMap(parent => ['-p', parent])], {
      input: `commit ${date}\n`, env: { ...process.env, GIT_AUTHOR_NAME: 'Proof',
        GIT_AUTHOR_EMAIL: 'proof@example.invalid', GIT_COMMITTER_NAME: 'Proof',
        GIT_COMMITTER_EMAIL: 'proof@example.invalid', GIT_AUTHOR_DATE: date, GIT_COMMITTER_DATE: date },
    });
  return { cwd, git, tree, commit, check: (source, publication) => requireExactAncestor(source, publication, { cwd }) };
}

test('complete actual parent walk admits self, ordinary, merge and reversed timestamps', t => {
  const f = fixture(t);
  const root = f.commit([], '2030-01-01T00:00:00Z');
  const left = f.commit([root], '2010-01-01T00:00:00Z');
  const right = f.commit([root], '2009-01-01T00:00:00Z');
  const merge = f.commit([left, right], '2000-01-01T00:00:00Z');
  for (const source of [root, left, right, merge]) assert.doesNotThrow(() => f.check(source, merge));
  assert.doesNotThrow(() => f.check(root, left));
  assert.throws(() => f.check(merge, root), /not in publication/);
});

test('unrelated, missing, noncommit and noncanonical identities refuse', t => {
  const f = fixture(t);const a = f.commit();const b = f.commit([], '2021-01-01T00:00:00Z');
  assert.throws(() => f.check(a, b), /not in publication/);
  assert.throws(() => f.check('f'.repeat(40), a), /Git failed/);
  assert.throws(() => f.check(a, 'f'.repeat(40)), /Git failed/);
  assert.throws(() => f.check(f.tree, a), /requires commit objects/);
  for (const invalid of [a.slice(0, 12), 'HEAD', a.toUpperCase(), `${a}\n`]) {
    assert.throws(() => f.check(invalid, a), /full lowercase/);
    assert.throws(() => f.check(a, invalid), /full lowercase/);
  }
});

test('missing reachable parent refuses even when source equals publication', t => {
  const f = fixture(t);const root = f.commit();const head = f.commit([root]);
  rmSync(path.join(f.cwd, '.git/objects', root.slice(0, 2), root.slice(2)));
  assert.throws(() => f.check(head, head), /Git failed/);
});

test('shallow repositories refuse', t => {
  const f = fixture(t);const root = f.commit();const head = f.commit([root]);
  f.git(['update-ref', 'refs/heads/main', head]);f.git(['symbolic-ref', 'HEAD', 'refs/heads/main']);
  const clone = path.join(f.cwd, 'shallow');
  f.git(['clone', '-q', '--depth=1', `file://${f.cwd}`, clone]);
  assert.throws(() => requireExactAncestor(head, head, { cwd: clone }), /non-shallow/);
});

test('replacement refs cannot invent ancestry or hide genuine ancestry', t => {
  const f = fixture(t);const root = f.commit();const head = f.commit([root]);
  const unrelated = f.commit([], '2022-01-01T00:00:00Z');
  const replacement = f.commit([unrelated], '2023-01-01T00:00:00Z');
  f.git(['replace', head, replacement]);
  assert.doesNotThrow(() => f.check(root, head));
  assert.throws(() => f.check(unrelated, head), /not in publication/);
});

test('graft files refuse even if empty', t => {
  const f = fixture(t);const root = f.commit();const head = f.commit([root]);
  mkdirSync(path.join(f.cwd, '.git/info'), { recursive: true });
  writeFileSync(path.join(f.cwd, '.git/info/grafts'), '');
  assert.throws(() => f.check(root, head), /graft files/);
});
