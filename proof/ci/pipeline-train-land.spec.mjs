import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, readFile, lstat } from 'node:fs/promises';
import path from 'node:path';
import { fixture } from './helpers/train-fixture.mjs';
import { createTrainLander } from '../../tools/ci/pipeline/train-land.mjs';

async function setup(t) {
  const f = await fixture(t);
  const run = { ...f.observation.run, run_attempt: 1, check_suite_id: 777 };
  const protection = { required_status_checks: { contexts: ['candidate'], checks: [{ context: 'candidate', app_id: 15368 }], strict: false },
    required_pull_request_reviews: { required_approving_review_count: 0, require_code_owner_reviews: false,
      require_last_push_approval: false, dismiss_stale_reviews: false } };
  const check = { name: 'candidate', app: { id: 15368 }, head_sha: f.source,
    check_suite: { id: 777 }, status: 'completed', conclusion: 'success' };
  const inventory = { total_count: 1, workflow_runs: [run] };
  const state = { dev: f.base, rules: [], head: f.source, tree: f.manifest.tree, reject: false,
    ambiguous: false, postAdvance: false, prepares: 0, mutations: 0, restores: 0 };
  const evidence = path.join(f.directory, 'landing-evidence');
  const commands = {
    async prepareDispatch() { state.prepares++; return f.manifest; },
    async restore(_manifest, _run, _archives, destination) {
      state.restores++; await mkdir(destination);
      state.onRestore?.();
      return { source: f.source, tree: f.manifest.tree, manifestDigest: f.manifest.digest,
        runId: run.id, runAttempt: run.run_attempt, bundle: path.join(destination, 'bundle') };
    },
  };
  const command = (program, args, options) => {
    assert.equal(program, 'gh'); assert.equal(args[0], 'api'); const endpoint = args[1];
    let value;
    if (args.includes('PATCH')) {
      state.mutations++; assert.deepEqual(JSON.parse(options.input), { sha: f.source, force: false });
      if (state.reject) return { status: 1, stderr: 'Protected branch update refused' };
      state.dev = state.postAdvance ? 'a'.repeat(40) : f.source;
      if (state.ambiguous) return { status: 1, stderr: 'Network failed after update' };
      value = { ref: 'refs/heads/dev', object: { sha: f.source } };
    } else if (endpoint.endsWith('/branches/dev/protection')) value = protection;
    else if (endpoint.endsWith('/rules/branches/dev')) value = state.rules;
    else if (endpoint.includes('/workflows/integration.yml/runs?')) value = inventory;
    else if (endpoint.includes('/actions/runs/')) value = run;
    else if (endpoint.includes('/check-suites/')) value = { total_count: 1, check_runs: [check] };
    else if (endpoint.endsWith('/git/ref/heads/dev')) value = { object: { sha: state.dev } };
    else if (endpoint.includes('/git/ref/heads/train/')) value = { object: { sha: state.head } };
    else if (endpoint.includes('/git/commits/')) value = { tree: { sha: state.tree } };
    else throw new Error(`Unexpected endpoint ${endpoint}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  const land = createTrainLander({ command, commands, directory: f.directory });
  return { ...f, run, protection, check, inventory, state, evidence,
    land: () => land('manifest.json', run.id, 'archives', evidence) };
}

test('lands exactly authenticated source through one non-forced protected ref update', async t => {
  const f = await setup(t); const result = await f.land();
  assert.equal(result.landedDev, f.source); assert.equal(f.state.mutations, 1);
  assert.equal(f.state.restores, 1); assert.equal(f.state.prepares, 2);
  const intent = JSON.parse(await readFile(path.join(f.evidence, 'landing-intent.json'), 'utf8'));
  assert.equal(intent.force, false); assert.equal(intent.priorDev, f.base);
  await assert.rejects(f.land(), /previous intent/); assert.equal(f.state.mutations, 1);
});
for (const [name, mutate] of [
  ['newer failed Integration', f => { f.inventory.workflow_runs.push({ ...f.run, id: 999, conclusion: 'failure' }); f.inventory.total_count++; }],
  ['failed Candidate aggregate', f => { f.check.conclusion = 'failure'; }],
  ['skipped Candidate aggregate', f => { f.check.conclusion = 'skipped'; }],
  ['foreign checks app', f => { f.check.app.id = 42; }],
  ['foreign checks suite', f => { f.check.check_suite.id++; }],
  ['changed train ref', f => { f.state.head = 'a'.repeat(40); }],
  ['changed source tree', f => { f.state.tree = 'a'.repeat(40); }],
  ['advanced dev', f => { f.state.dev = 'a'.repeat(40); }],
  ['review requirement', f => { f.protection.required_pull_request_reviews.required_approving_review_count = 1; }],
  ['code-owner requirement', f => { f.protection.required_pull_request_reviews.require_code_owner_reviews = true; }],
  ['unknown required check', f => { f.protection.required_status_checks.contexts.push('security'); }],
  ['ruleset requirement', f => { f.state.rules.push({ type: 'pull_request' }); }],
  ['unknown branch requirement', f => { f.protection.unknown_gate = { enabled: true }; }],
]) test(`refuses ${name} without mutation`, async t => {
  const f = await setup(t); mutate(f); await assert.rejects(f.land()); assert.equal(f.state.mutations, 0);
});
for (const name of ['reject', 'ambiguous', 'postAdvance']) test(`retains intent on ${name} and never retries`, async t => {
  const f = await setup(t); f.state[name] = true;
  await assert.rejects(f.land()); assert.equal(f.state.mutations, 1);
  assert.equal((await lstat(path.join(f.evidence, 'landing-intent.json'))).isFile(), true);
  await assert.rejects(lstat(path.join(f.evidence, 'landing.json')), { code: 'ENOENT' });
  await assert.rejects(f.land(), /previous intent/); assert.equal(f.state.mutations, 1);
});

test('refuses a changed run attempt during authenticated restoration', async t => {
  const f = await setup(t); f.state.onRestore = () => { f.run.run_attempt++; };
  await assert.rejects(f.land(), /Restored evidence differs|attempt changed/);
  assert.equal(f.state.mutations, 0);
  await assert.rejects(lstat(path.join(f.evidence, 'landing-intent.json')), { code: 'ENOENT' });
});
