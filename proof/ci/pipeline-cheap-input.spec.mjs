import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { candidateAdmission, canonicalCandidateAdmission } from '../../tools/ci/pipeline/train-input-admission.mjs';
import { observeCheapTrainPolicy, requireCheapTrainPolicy } from '../../tools/ci/pipeline/train-cheap-policy.mjs';
import { selectTrainInputs, verifyTrainRun } from '../../tools/ci/pipeline/train.mjs';
import { fixture } from './helpers/train-fixture.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';

function cheap(selection) {
  const run = selection.candidateRun;
  const states = [['cheap-input/v1', 'success'], ['proof', 'skipped'], ['candidate', 'success']];
  selection.candidateJobs = { total_count: 3, jobs: states.map(([name, conclusion], i) => ({
    id: run.id * 100 + i, run_id: run.id, run_attempt: run.run_attempt, head_sha: run.head_sha,
    status: 'completed', name, conclusion,
  })) };
  return selection;
}
function policy(repository) {
  const f = machinePolicyFixture(repository);
  const protection = { required_status_checks: { contexts: ['train-proof'],
    checks: [{ context: 'train-proof', app_id: 15368 }], strict: false },
    required_pull_request_reviews: null, enforce_admins: { enabled: true },
    allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  const calls = [];
  const activation = { name: 'CONDUIT_PR_ADMISSION_TIER', value: 'cheap-input-v1' };
  const options = { config: f.config, repository, api: endpoint => {
    calls.push(['installation', endpoint]);
    if (endpoint === 'installation/repositories?per_page=100') return f.repositories;
    if (endpoint.endsWith(`/rulesets/${f.config.rulesetId}`)) return f.machineRuleset;
    throw new Error(`Unexpected installation ${endpoint}`);
  }, appApi: endpoint => endpoint === 'app' ? f.app : f.installation,
  policyApi: endpoint => {
    calls.push(['policy', endpoint]);
    if (endpoint === 'user') return f.observerUser;
    if (endpoint.endsWith('/actions/variables/CONDUIT_PR_ADMISSION_TIER')) return activation;
    if (endpoint.endsWith(`/rulesets/${f.config.rulesetId}`)) return f.ruleset;
    if (endpoint.endsWith('/rules/branches/dev?per_page=100')) return f.branchRules;
    if (endpoint.endsWith('/branches/dev/protection')) return protection;
    throw new Error(`Unexpected policy ${endpoint}`);
  } };
  return { f, protection, activation, calls, grant: () => observeCheapTrainPolicy(options) };
}

test('cheap records retain versioned exact run/attempt/head and every job', async t => {
  const f = await fixture(t); const selected = cheap(f.selections[0]);
  const a = candidateAdmission(selected.candidateRun, selected.candidateJobs);
  assert.equal(a.schema, 'conduit.candidate-cheap-input/v1');
  assert.equal(a.tier, 'cheap-input-v1'); assert.equal(a.runId, selected.candidateRun.id);
  assert.equal(a.runAttempt, 1); assert.equal(a.head, selected.pr.head.sha);
  assert.equal(a.jobs.length, 3); assert.deepEqual(a.deferredNativeProducts, []);
  assert.deepEqual(canonicalCandidateAdmission(a), a);
  assert.throws(() => f.capture(), /fresh external/);
  selected.cheapPolicy = {}; assert.throws(() => f.capture(), /fresh external/);
  const p = policy(f.manifest.repository); selected.cheapPolicy = p.grant();
  const manifest = f.capture(); assert.equal(manifest.inputs[0].admission.tier, 'cheap-input-v1');
  assert.equal(JSON.stringify(manifest).includes('cheapPolicy'), false);
  assert.throws(() => f.capture(), /fresh external/); // Single use; cannot replay.
  selected.cheapPolicy = p.grant();
  const observation = { ...f.observation, run: { ...f.observation.run, head_branch: manifest.branch } };
  assert.equal(verifyTrainRun(manifest, observation).integrationRun, 123);
  selected.cheapPolicy = p.grant();
  assert.throws(() => verifyTrainRun(manifest, { ...observation, run: { ...observation.run, conclusion: 'failure' } }), /successful exhaustive Integration/);
});

for (const result of ['failure', 'cancelled', 'skipped', 'timed_out', null]) {
  test(`cheap quick refuses ${result}`, async t => {
    const f = await fixture(t); const s = cheap(f.selections[0]); s.candidateJobs.jobs[0].conclusion = result;
    assert.throws(() => candidateAdmission(s.candidateRun, s.candidateJobs), /Candidate admission/);
  });
}
for (const [name, mutate] of [
  ['wrong attempt', s => { s.candidateJobs.jobs[0].run_attempt++; }],
  ['wrong run', s => { s.candidateJobs.jobs[0].run_id++; }],
  ['wrong head', s => { s.candidateJobs.jobs[0].head_sha = 'a'.repeat(40); }],
  ['running quick', s => { s.candidateJobs.jobs[0].status = 'in_progress'; }],
  ['missing aggregate', s => { s.candidateJobs.jobs.pop(); s.candidateJobs.total_count--; }],
  ['truncated inventory', s => { s.candidateJobs.total_count++; }],
  ['duplicate ID', s => { s.candidateJobs.jobs[1].id = s.candidateJobs.jobs[0].id; }],
  ['duplicate name', s => { s.candidateJobs.jobs[1].name = s.candidateJobs.jobs[0].name; }],
  ['executed full proof', s => { s.candidateJobs.jobs[1].conclusion = 'success'; }],
  ['unknown job', s => { s.candidateJobs.jobs[1].name = 'hidden bypass'; }],
  ['failed aggregate', s => { s.candidateJobs.jobs[2].conclusion = 'failure'; }],
]) test(`cheap custody refuses ${name}`, async t => {
  const f = await fixture(t); const s = cheap(f.selections[0]); mutate(s);
  assert.throws(() => candidateAdmission(s.candidateRun, s.candidateJobs), /Candidate admission/);
});

test('external observation refuses writer/proof bypass and forged or copied grants', () => {
  const p = policy('example/conduit'); const grant = p.grant();
  assert.equal(p.calls.length, 7);
  assert.deepEqual(p.calls.find(([, endpoint]) => endpoint.endsWith('/actions/variables/CONDUIT_PR_ADMISSION_TIER'))?.[0], 'policy');
  assert.throws(() => requireCheapTrainPolicy(JSON.parse(JSON.stringify(grant)), 'example/conduit'), /fresh external/);
  assert.throws(() => requireCheapTrainPolicy(grant, 'other/conduit'), /fresh external/);
  requireCheapTrainPolicy(grant, 'example/conduit');
  assert.throws(() => requireCheapTrainPolicy(grant, 'example/conduit'), /fresh external/);
  p.protection.required_status_checks.contexts = ['candidate']; assert.throws(p.grant, /required checks/);
});

test('legacy successful Candidate accepts newly skipped cheap job without changing its tier', async t => {
  const f = await fixture(t); const s = f.selections[0];
  s.candidateJobs.jobs.push({ ...s.candidateJobs.jobs[0], id: 999999, name: 'cheap-input/v1', conclusion: 'skipped' });
  s.candidateJobs.total_count++;
  assert.equal(candidateAdmission(s.candidateRun, s.candidateJobs).tier, 'complete-candidate');
  assert.equal(selectTrainInputs({ repository: f.manifest.repository, base: f.base, selections: f.selections }).inputs.length, 2);
});

test('workflow defaults remain full and cheap execution has no writer credentials', () => {
  const source = readFileSync('.github/workflows/candidate.yml', 'utf8');
  assert.match(source, /if: vars\.CONDUIT_PR_ADMISSION_TIER != 'cheap-input-v1'\n    uses: \.\/\.github\/workflows\/ci.yml/);
  assert.match(source, /if: vars\.CONDUIT_PR_ADMISSION_TIER == 'cheap-input-v1'/);
  assert.match(source, /node tools\/ci\/pipeline\/cli.mjs preflight "\$BASE"/);
  assert.match(source, /needs: \[proof, cheap\]/);
  assert.doesNotMatch(source, /secrets:|JWT|ADMIN_TOKEN|pull_request_target|permissions:[\s\S]*contents: write/);
});

test('actual Candidate aggregate refuses every unhealthy cheap/default route', async () => {
  const { spawnSync } = await import('node:child_process');
  const source = readFileSync('.github/workflows/candidate.yml', 'utf8');
  const script = source.split('\n  candidate:\n')[1].split('        run: |\n')[1].replace(/^          /gm, '');
  const passes = env => spawnSync('bash', ['-e', '-c', script], { env: { ...process.env, ...env } }).status === 0;
  for (const TIER of ['', 'unknown', 'cheap-input-v1']) {
    for (const RESULT of ['success', 'skipped', 'failure', 'cancelled', '']) {
      for (const CHEAP of ['success', 'skipped', 'failure', 'cancelled', '']) {
        assert.equal(passes({ TIER, RESULT, CHEAP }), TIER === 'cheap-input-v1'
          ? RESULT === 'skipped' && CHEAP === 'success'
          : RESULT === 'success' && CHEAP === 'skipped');
      }
    }
  }
});

test('trusted controller callback reobserves on every live capture and dispatch', async t => {
  const { commands } = await import('./helpers/train-command-fixture.mjs');
  const { createTrainCommands } = await import('../../tools/ci/pipeline/train-command.mjs');
  const f = await commands(t); cheap(f.selections[0]);
  await assert.rejects(f.api.capture('example/conduit', f.source, [1, 2], f.filename), /external cheap policy observer/);
  const p = policy('example/conduit'); let observations = 0;
  const controller = createTrainCommands({ command: f.command, directory: f.directory,
    observeCheapPolicy: ({ repository }) => { assert.equal(repository, 'example/conduit'); observations++; return p.grant(); } });
  const manifest = await controller.capture('example/conduit', f.source, [1, 2], f.filename);
  assert.equal(observations, 1);
  assert.deepEqual(await controller.prepareDispatch(f.filename), manifest);
  assert.equal(observations, 2);
  // Verify reconstructs admission and then checks product custody: each internal
  // validation must reobserve rather than replay an already consumed grant.
  const { mkdir, writeFile } = await import('node:fs/promises');
  const path = await import('node:path');
  const { sealTarget } = await import('../../tools/ci/pipeline/receipts.mjs');
  const { TARGETS } = await import('../../tools/ci/pipeline/targets.mjs');
  const products = path.join(f.output, 'products'); await mkdir(products);
  for (const target of TARGETS) {
    const directory = path.join(products, target.id); await mkdir(directory);
    await writeFile(path.join(directory, 'fixture'), 'inert fixture');
    await sealTarget({ directory, target: target.id, sha: f.source, proofClass: target.proofClass });
  }
  f.observation.run.head_branch = manifest.branch;
  assert.equal((await controller.verify(f.filename, 123, products)).source, f.source);
  assert.equal(observations, 4);
  const forged = createTrainCommands({ command: f.command, directory: f.directory, observeCheapPolicy: () => ({}) });
  await assert.rejects(forged.prepareDispatch(f.filename), /fresh external/);
  p.protection.enforce_admins.enabled = false;
  await assert.rejects(controller.prepareDispatch(f.filename), /administrators/);
});

for (const [name, mutate] of [
  ['unset activation', p => { delete p.activation.value; }],
  ['unknown activation', p => { p.activation.value = 'cheap'; }],
  ['writer disabled', p => { p.f.config.enabled = false; }],
  ['force update enabled', p => { p.protection.allow_force_pushes.enabled = true; }],
  ['deletion enabled', p => { p.protection.allow_deletions.enabled = true; }],
  ['ordinary Candidate check', p => { p.protection.required_status_checks.contexts = ['candidate']; }],
]) test(`cheap policy refuses ${name}`, () => {
  const p = policy('example/conduit'); mutate(p); assert.throws(p.grant);
});

test('stale external observation cannot admit later cheap input', t => {
  let now = 100;
  t.mock.method(performance, 'now', () => now);
  const p = policy('example/conduit'); const grant = p.grant();
  now += 60_001;
  assert.throws(() => requireCheapTrainPolicy(grant, 'example/conduit'), /fresh external/);
});
