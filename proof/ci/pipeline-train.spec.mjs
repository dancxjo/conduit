import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { captureTrain, verifyTrainRun, verifyTrainProducts } from '../../tools/ci/pipeline/train.mjs';
import { sealTarget } from '../../tools/ci/pipeline/receipts.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';

const repository = 'example/conduit';
const repo = { full_name: repository };

async function fixture(t) {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'conduit-train-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const git = (...args) => execFileSync('git', ['-C', directory, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  git('init', '-b', 'base');
  git('config', 'user.email', 'fixture@example.invalid');
  git('config', 'user.name', 'Train fixture');
  const commit = async name => {
    await writeFile(path.join(directory, name), name);
    git('add', name); git('commit', '-m', name);
    return git('rev-parse', 'HEAD');
  };
  const base = await commit('base');
  git('checkout', '-b', 'first'); const first = await commit('first');
  git('checkout', '-b', 'second', base); const second = await commit('second');
  git('checkout', '-b', 'assembled', first); git('merge', '--no-ff', second, '-m', 'assembled');
  const source = git('rev-parse', 'HEAD');
  const selections = [first, second].map((head, i) => ({
    pr: { number: i + 1, state: 'open', draft: false, mergeable: true,
      head: { sha: head, repo }, base: { ref: 'dev', sha: base, repo } },
    candidateRun: { repository: repo, head_repository: repo, event: 'pull_request',
      path: '.github/workflows/candidate.yml', head_sha: head, status: 'completed',
      conclusion: 'success', pull_requests: [{ number: i + 1 }] },
  }));
  const capture = overrides => captureTrain({ repository, base, selections, source, directory, ...overrides });
  const manifest = capture();
  const observation = { development: base, selections, run: { id: 123, repository: repo,
    head_repository: repo, path: '.github/workflows/integration.yml', event: 'workflow_dispatch',
    head_branch: manifest.branch, head_sha: source, status: 'completed', conclusion: 'success' } };
  return { directory, git, base, source, selections, capture, manifest, observation };
}

test('train binds actual combined commit/tree and canonical input order', async t => {
  const f = await fixture(t);
  assert.equal(f.manifest.tree, f.git('rev-parse', `${f.source}^{tree}`));
  assert.deepEqual(f.capture({ selections: [...f.selections].reverse() }), f.manifest);
  assert.equal(verifyTrainRun(f.manifest, f.observation).integrationRun, 123);
});

test('a claimed source that omits a selected head is refused by actual Git ancestry', async t => {
  const f = await fixture(t);
  assert.throws(() => f.capture({ source: f.selections[0].pr.head.sha }));
  assert.throws(() => f.capture({ source: 'f'.repeat(40) }));
});

test('draft, failed, forked, stale-base and duplicate PRs cannot enter a train', async t => {
  const f = await fixture(t);
  for (const mutate of [
    s => { s[0].pr.draft = true; },
    s => { s[0].pr.mergeable = null; },
    s => { s[0].candidateRun.conclusion = 'failure'; },
    s => { s[0].candidateRun.status = 'in_progress'; },
    s => { s[0].candidateRun.head_sha = f.base; },
    s => { s[0].candidateRun.pull_requests = []; },
    s => { s[0].pr.head.repo = { full_name: 'fork/conduit' }; },
    s => { s[0].pr.base.sha = f.source; },
    s => { s[1] = s[0]; },
  ]) {
    const selections = structuredClone(f.selections); mutate(selections);
    assert.throws(() => f.capture({ selections }));
  }
});

test('changed development or selected PR invalidates landing evidence', async t => {
  const f = await fixture(t);
  for (const mutate of [
    o => { o.development = f.source; },
    o => { o.selections[0].pr.head.sha = f.base; },
    o => { o.selections[0].pr.draft = true; },
    o => { o.selections.pop(); },
  ]) {
    const observation = structuredClone(f.observation); mutate(observation);
    assert.throws(() => verifyTrainRun(f.manifest, observation));
  }
});

test('wrong source, workflow, repository, branch, event and unfinished run refuse', async t => {
  const f = await fixture(t);
  for (const changes of [
    { head_sha: f.base }, { path: '.github/workflows/candidate.yml' },
    { repository: { full_name: 'fork/conduit' } }, { head_branch: 'dev' },
    { event: 'pull_request' }, { conclusion: 'failure' }, { status: 'in_progress' }, { id: 0 },
  ]) {
    const observation = structuredClone(f.observation); Object.assign(observation.run, changes);
    assert.throws(() => verifyTrainRun(f.manifest, observation));
  }
  for (const changes of [{ source: f.base }, { tree: f.base }, { branch: 'train/other' },
    { digest: '0'.repeat(64) }, { authorize: true }]) {
    assert.throws(() => verifyTrainRun({ ...f.manifest, ...changes }, f.observation));
  }
});

test('successful run metadata is insufficient without exact exhaustive product custody', async t => {
  const f = await fixture(t);
  const directory = path.join(f.directory, 'bundle'); await mkdir(directory);
  await assert.rejects(verifyTrainProducts(f.manifest, f.observation, directory));
  for (const target of TARGETS) {
    const product = path.join(directory, target.id); await mkdir(product);
    await writeFile(path.join(product, 'fixture'), 'inert fixture, not target execution');
    await sealTarget({ directory: product, target: target.id, sha: f.source, proofClass: target.proofClass });
  }
  const verified = await verifyTrainProducts(f.manifest, f.observation, directory);
  assert.equal(verified.receipts.length, TARGETS.length);
  await writeFile(path.join(directory, TARGETS[0].id, 'fixture'), 'changed bytes');
  await assert.rejects(verifyTrainProducts(f.manifest, f.observation, directory));
});
