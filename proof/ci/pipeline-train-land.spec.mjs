import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, readFile, lstat, mkdtemp, rm, symlink } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { fixture } from './helpers/train-fixture.mjs';
import { createTrainLander, assertSeparateTrainController } from '../../tools/ci/pipeline/train-land.mjs';
import { proofStatusIdentity } from '../../tools/ci/pipeline/train-proof-status.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';

async function setup(t, { configured = true, identity = true, observer = true } = {}) {
  const f = await fixture(t);
  const run = { ...f.observation.run, run_attempt: 1, check_suite_id: 777 };
  const machine = machinePolicyFixture(f.manifest.repository);
  const protection = { required_status_checks: { contexts: ['train-proof'], checks: [{ context: 'train-proof', app_id: 15368 }], strict: false },
    required_pull_request_reviews: null, enforce_admins: { enabled: true },
    allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  const check = { name: 'train-accepted', app: { id: 15368 }, head_sha: f.source,
    check_suite: { id: 777 }, status: 'completed', conclusion: 'success' };
  const proofStatus = { id: 888, url: `https://api.github.com/repos/${f.manifest.repository}/statuses/${f.source}`,
    creator: { id: 41898282, login: 'github-actions[bot]', type: 'Bot' },
    ...proofStatusIdentity(f.manifest.repository, f.source, run.id, run.run_attempt) };
  const inventory = { total_count: 1, workflow_runs: [run] };
  const state = { dev: f.base, rules: machine.branchRules, head: f.source, tree: f.manifest.tree, reject: false,
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
    else if (endpoint.includes('/rules/branches/dev')) value = state.rules;
    else if (endpoint.endsWith('/rulesets/404')) value = machine.machineRuleset;
    else if (endpoint === 'installation/repositories?per_page=100') value = machine.repositories;
    else if (endpoint.includes('/workflows/integration.yml/runs?')) value = inventory;
    else if (endpoint.includes('/actions/runs/')) value = run;
    else if (endpoint.includes('/statuses?')) value = [proofStatus];
    else if (endpoint.includes('/check-suites/')) value = { total_count: 1, check_runs: [check] };
    else if (endpoint.endsWith('/git/ref/heads/dev')) value = { object: { sha: state.dev } };
    else if (endpoint.includes('/git/ref/heads/train/')) value = { object: { sha: state.head } };
    else if (endpoint.includes('/git/commits/')) value = { tree: { sha: state.tree } };
    else throw new Error(`Unexpected endpoint ${endpoint}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  const appIdentityApi = endpoint => endpoint === 'app' ? machine.app : machine.installation;
  const policyObserverApi = endpoint => {
    if (endpoint === 'user') return machine.observerUser;
    if (endpoint.endsWith('/rulesets/404')) return machine.ruleset;
    if (endpoint.endsWith('/branches/dev/protection')) return protection;
    if (endpoint.endsWith('/rules/branches/dev?per_page=100')) return state.rules;
    throw new Error(`Unexpected observer endpoint ${endpoint}`);
  };
  const land = createTrainLander({ command, commands, directory: f.directory,
    machineWriter: configured ? machine.config : undefined, appIdentityApi: identity ? appIdentityApi : undefined,
    policyObserverApi: observer ? policyObserverApi : undefined });
  return { ...f, run, protection, check, proofStatus, inventory, state, evidence, machine,
    land: () => land('manifest.json', run.id, 'archives', evidence) };
}

test('lands exactly authenticated source through one non-forced protected ref update', async t => {
  const f = await setup(t); const result = await f.land();
  assert.equal(result.landedDev, f.source); assert.equal(f.state.mutations, 1);
  assert.equal(f.state.restores, 1); assert.equal(f.state.prepares, 2);
  const intent = JSON.parse(await readFile(path.join(f.evidence, 'landing-intent.json'), 'utf8'));
  assert.equal(intent.force, false); assert.equal(intent.priorDev, f.base);
  assert.equal(intent.machineWriter.appId, f.machine.config.appId);
  assert.equal(intent.machineWriter.rulesetId, f.machine.config.rulesetId);
  await assert.rejects(f.land(), /previous intent/); assert.equal(f.state.mutations, 1);
});
for (const options of [{ configured: false }, { identity: false }, { observer: false }]) test(`refuses missing machine setup ${JSON.stringify(options)}`, async t => {
  const f = await setup(t, options);
  await assert.rejects(f.land(), /Machine writer policy/);
  assert.equal(f.state.mutations, 0); assert.equal(f.state.restores, 0);
});
for (const [name, mutate] of [
  ['newer failed Integration', f => { f.inventory.workflow_runs.push({ ...f.run, id: 999, conclusion: 'failure' }); f.inventory.total_count++; }],
  ['writer-authored proof status', f => { f.proofStatus.creator.id = 202; }],
  ['stale proof status attempt', f => { f.proofStatus.description = 'Integration 1 attempt 99'; }],
  ['failed proof status', f => { f.proofStatus.state = 'failure'; }],
  ['ordinary Candidate green', f => { f.check.name = 'candidate'; }],
  ['ordinary Candidate protection policy', f => { f.protection.required_status_checks.contexts = ['candidate']; f.protection.required_status_checks.checks[0].context = 'candidate'; }],
  ['missing train acceptance', f => { f.check.name = 'unrelated'; }],
  ['failed train acceptance aggregate', f => { f.check.conclusion = 'failure'; }],
  ['skipped train acceptance aggregate', f => { f.check.conclusion = 'skipped'; }],
  ['foreign checks app', f => { f.check.app.id = 42; }],
  ['foreign checks suite', f => { f.check.check_suite.id++; }],
  ['changed train ref', f => { f.state.head = 'a'.repeat(40); }],
  ['changed source tree', f => { f.state.tree = 'a'.repeat(40); }],
  ['advanced dev', f => { f.state.dev = 'a'.repeat(40); }],
  ['zero-review PR requirement', f => { f.protection.required_pull_request_reviews = { required_approving_review_count: 0, require_code_owner_reviews: false }; }],
  ['review requirement', f => { f.protection.required_pull_request_reviews = { required_approving_review_count: 1 }; }],
  ['administrator check bypass', f => { f.protection.enforce_admins.enabled = false; }],
  ['unknown administrator enforcement', f => { delete f.protection.enforce_admins; }],
  ['disabled machine activation', f => { f.machine.config.enabled = false; }],
  ['stale machine identity', f => { f.machine.app.id++; }],
  ['wrong observer identity', f => { f.machine.observerUser.id++; }],
  ['hidden bypass list', f => { delete f.machine.ruleset.bypass_actors; }],
  ['unproven mutation token authority', f => { f.machine.machineRuleset.current_user_can_bypass = 'never'; }],
  ['machine proof bypass', f => { f.machine.ruleset.rules.push({ type: 'required_status_checks' }); }],
  ['force push permission', f => { f.protection.allow_force_pushes.enabled = true; }],
  ['branch deletion permission', f => { f.protection.allow_deletions.enabled = true; }],
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

test('refuses writer policy changes during restoration before writing an intent', async t => {
  const f = await setup(t);
  f.state.onRestore = () => { f.machine.ruleset.bypass_actors.push({ actor_type: 'User', actor_id: 55, bypass_mode: 'always' }); };
  await assert.rejects(f.land(), /exact active dev-only update ruleset differs/);
  assert.equal(f.state.mutations, 0);
  await assert.rejects(lstat(path.join(f.evidence, 'landing-intent.json')), { code: 'ENOENT' });
});

test('privileged controller and proposed source must be disjoint even through symlinks', async t => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'separate-controller-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const controller = path.join(root, 'controller'), source = path.join(root, 'source');
  await mkdir(controller); await mkdir(source); await mkdir(path.join(controller, 'nested'));
  await assertSeparateTrainController(source, controller);
  await assert.rejects(assertSeparateTrainController(controller, controller), /separately trusted/);
  await assert.rejects(assertSeparateTrainController(path.join(controller, 'nested'), controller), /separately trusted/);
  const alias = path.join(root, 'alias'); await symlink(controller, alias);
  await assert.rejects(assertSeparateTrainController(alias, controller), /separately trusted/);
});
