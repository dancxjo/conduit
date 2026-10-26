import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { captureTrain, verifyTrainRun, verifyTrainProducts } from '../../tools/ci/pipeline/train.mjs';
import { sealTarget } from '../../tools/ci/pipeline/receipts.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';
import { fixture } from './helpers/train-fixture.mjs';

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

test('source ancestry cannot smuggle an extra change or omission into the selected tree', async t => {
  const f = await fixture(t);
  for (const mutate of [
    () => writeFile(path.join(f.directory, 'extra'), 'unselected change'),
    () => writeFile(path.join(f.directory, 'first'), 'silently altered selected change'),
  ]) {
    await mutate();
    f.git('add', '-A'); f.git('commit', '-m', 'unselected tree');
    const source = f.git('rev-parse', 'HEAD');
    for (const selected of f.selections) f.git('merge-base', '--is-ancestor', selected.pr.head.sha, source);
    assert.throws(() => f.capture({ source }), /differs from the selected PR merge tree/);
    f.git('reset', '--hard', f.source);
  }
});

test('draft, failed, forked, wrong-target and duplicate PRs cannot enter a train', async t => {
  const f = await fixture(t);
  for (const mutate of [
    s => { s[0].pr.draft = true; },
    s => { s[0].pr.mergeable = null; },
    s => { s[0].candidateRun.conclusion = 'failure'; },
    s => { s[0].candidateRun.status = 'in_progress'; },
    s => { s[0].candidateRun.head_sha = f.base; },
    s => { s[0].candidateRun.pull_requests = []; },
    s => { s[0].pr.head.repo = { full_name: 'fork/conduit' }; },
    s => { s[0].pr.base.ref = 'main'; },
    s => { s[1] = s[0]; },
  ]) {
    const selections = structuredClone(f.selections); mutate(selections);
    assert.throws(() => f.capture({ selections }));
  }
});

test('stale PR base metadata cannot override the captured live development ref', async t => {
  const f = await fixture(t);
  const selections = structuredClone(f.selections);
  selections[0].pr.base.sha = 'c'.repeat(40);
  assert.deepEqual(f.capture({ selections }), f.manifest);
  assert.equal(verifyTrainRun(f.manifest, { ...f.observation, selections }).base, f.base);
  assert.throws(() => verifyTrainRun(f.manifest, { ...f.observation, development: f.source }), /Development advanced/);
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
