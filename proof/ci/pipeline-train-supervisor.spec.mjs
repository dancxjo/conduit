import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, chmod, unlink } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { fixture } from './helpers/train-fixture.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';
import { runTrainSupervisorTick, readTrainSupervisorQueue, inspectSupervisorRun, trainSupervisorRuntimeIdentity } from '../../tools/ci/pipeline/train-supervisor.mjs';
import { withSupervisorStore, readSupervisorJson } from '../../tools/ci/pipeline/train-supervisor-store.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url));
async function setup(t) {
  const f = await fixture(t), directory = await mkdtemp(path.join(os.tmpdir(), 'supervisor-fixture-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const stateDirectory = path.join(directory, 'state'); await mkdir(stateDirectory, { mode: 0o700 });
  const queue = { schema: 'conduit.train-supervisor-queue/v1', id: 'fixture', repository: f.manifest.repository,
    prs: f.manifest.inputs.map(input => input.number), targetSource: f.directory,
    worktree: path.join(directory, 'train'), manifest: path.join(directory, 'manifest.json'),
    archives: path.join(directory, 'archives'), evidence: path.join(directory, 'evidence') };
  const queueFile = path.join(directory, 'queue.json');
  const saveQueue = () => writeFile(queueFile, JSON.stringify(queue), { mode: 0o600 }); await saveQueue();
  const config = machinePolicyFixture(queue.repository).config;
  const options = { queueFile, stateDirectory, config, privateKeyFile: '/fixture/not-read', observerEnvironmentReference: 'FIXTURE_OBSERVER' };
  const calls = []; let now = Date.parse('2026-10-26T00:00:00Z');
  const controller = async ({ request }) => {
    calls.push(request.operation);
    if (request.operation === 'assemble') { await writeFile(queue.manifest, JSON.stringify(f.manifest), { mode: 0o600 }); return f.manifest; }
    if (request.operation === 'dispatch') return { source: f.source, branch: f.manifest.branch, runId: null, dispatched: true };
    if (request.operation === 'acquire') return { source: f.source, manifestDigest: f.manifest.digest, runId: 123, runAttempt: 1 };
    return { landedDev: f.source, tree: f.manifest.tree, manifestDigest: f.manifest.digest, runId: 123, runAttempt: 1 };
  };
  return { ...f, directory, queue, saveQueue, options, controller, calls,
    tick: overrides => runTrainSupervisorTick({ ...options, controller, downstreamObserver: async () => ({ kind: 'waiting' }), identity: async () => 'a'.repeat(64), clock: () => now += 300001, ...overrides }) };
}
test('finite ticks progress once through explicit operations and never claim downstream completion', async t => {
  const f = await setup(t); const observer = async () => ({ kind: 'success', binding: { id: 123, attempt: 1 } });
  for (const stage of ['assembled', 'awaiting-run', 'proof-ready', 'acquired', 'awaiting-downstream']) {
    assert.equal((await f.tick({ observer })).stage, stage);
  }
  assert.deepEqual(f.calls, ['assemble', 'dispatch', 'acquire', 'land']);
  const result = await f.tick({ observer }); assert.equal(result.stage, 'awaiting-downstream'); assert.equal(result.complete, undefined);
  assert.equal(f.calls.length, 4);
});
test('waiting only observes; failures and attempts cannot redispatch', async t => {
  const f = await setup(t); await f.tick(); await f.tick();
  let calls = 0;
  const observer = async () => { calls++; return { kind: 'waiting', binding: { id: 123, attempt: 1 } }; };
  assert.equal((await f.tick({ observer })).stage, 'awaiting-run');
  assert.equal((await f.tick({ observer: async () => ({ kind: 'transient' }) })).observationFailed, true);
  assert.equal((await f.tick({ observer })).stage, 'awaiting-run');
  assert.equal(calls, 2); assert.deepEqual(f.calls, ['assemble', 'dispatch']);
  assert.equal((await f.tick({ observer: async () => ({ kind: 'failed' }) })).stage, 'blocked');
  assert.equal((await f.tick()).inspectionRequired, true);
});
test('drain pauses every mutation and preserves current evidence while allowing finite read observation', async t => {
  const f = await setup(t);
  assert.equal((await f.tick({ maintenance: true })).paused, true); assert.equal(f.calls.length, 0);
  await f.tick(); assert.equal((await f.tick({ maintenance: true })).paused, true);
  await f.tick();
  assert.equal((await f.tick({ maintenance: true, observer: async () => ({ kind: 'waiting' }) })).stage, 'awaiting-run');
  assert.deepEqual(f.calls, ['assemble', 'dispatch']);
});
test('intent survives real process interruption and crash lock requires explicit inspection', async t => {
  const f = await setup(t);
  const script = `import {runTrainSupervisorTick} from './tools/ci/pipeline/train-supervisor.mjs';
await runTrainSupervisorTick({...${JSON.stringify(f.options)},identity:async()=> 'a'.repeat(64),controller:async()=>process.exit(23)});`;
  const child = spawnSync(process.execPath, ['--input-type=module', '-e', script], { cwd: root, encoding: 'utf8', timeout: 30000 });
  assert.ok(child.status === 23, 'actual child interrupts after durable intent');
  const journal = await readSupervisorJson(path.join(f.options.stateDirectory, 'journal.json'));
  assert.equal(journal.stage, 'intent'); assert.equal(journal.operation, 'assemble');
  await assert.rejects(f.tick(), /lock exists/);
  // Fixture operator inspection removes only the verified crash lock, never an automatic PID heuristic.
  await unlink(path.join(f.options.stateDirectory, 'controller.lock'));
  assert.equal((await f.tick()).inspectionRequired, true); assert.equal(f.calls.length, 0);
});
test('queue/config/manifest substitution and reordered inputs refuse before further mutation', async t => {
  const f = await setup(t); await f.tick();
  f.queue.prs.reverse(); await f.saveQueue(); await assert.rejects(f.tick(), /journal identity changed/);
  f.queue.prs.reverse(); await f.saveQueue();
  await assert.rejects(f.tick({ config: { ...f.options.config, appId: 999 } }), /journal identity changed/);
  await writeFile(f.queue.manifest, JSON.stringify({ ...f.manifest, source: 'a'.repeat(40) }), { mode: 0o600 });
  await assert.rejects(f.tick()); assert.deepEqual(f.calls, ['assemble']);
});
test('same configured root excludes concurrent queue ticks; unsafe queue files refuse', async t => {
  const f = await setup(t); let release;
  const held = withSupervisorStore(f.options.stateDirectory, () => new Promise(resolve => { release = resolve; }));
  while (!release) await new Promise(resolve => setTimeout(resolve, 1));
  await assert.rejects(f.tick(), /lock exists/); release(); await held;
  await chmod(f.options.queueFile, 0o644); await assert.rejects(readTrainSupervisorQueue(f.options.queueFile), /private state refused/);
  await chmod(f.options.queueFile, 0o600); const link = path.join(f.directory, 'queue-link'); await symlink(f.options.queueFile, link);
  await assert.rejects(readTrainSupervisorQueue(link), /private state refused/);
});
function observationFixture(f) {
  const train = { source: f.source, tree: f.manifest.tree, branch: f.manifest.branch };
  const run = { id: 123, run_attempt: 1, path: '.github/workflows/integration.yml', event: 'workflow_dispatch',
    head_sha: f.source, head_branch: train.branch, status: 'completed', conclusion: 'success', check_suite_id: 44,
    repository: { id: 101, full_name: f.queue.repository }, head_repository: { id: 101, full_name: f.queue.repository } };
  const inventory = { total_count: 1, workflow_runs: [run] };
  const api = endpoint => endpoint.includes('/runs?') ? inventory : endpoint.includes('/git/ref/') ? { object: { sha: f.source } }
    : endpoint.includes('/git/commits/') ? { tree: { sha: train.tree } } : run;
  return { queue: f.queue, train, repositoryId: 101, api, run, inventory };
}
test('production exact-run observer validates bounded complete inventory and immutable custody', async t => {
  const f = await setup(t), o = observationFixture(f);
  assert.deepEqual(inspectSupervisorRun(o), { kind: 'success', binding: { id: 123, attempt: 1 } });
  for (const field of ['event', 'path', 'head_sha', 'head_branch']) {
    const old = o.run[field]; o.run[field] = 'foreign'; assert.equal(inspectSupervisorRun(o).kind, 'refused'); o.run[field] = old;
  }
  o.inventory.total_count = 2; assert.equal(inspectSupervisorRun(o).kind, 'refused');
  o.inventory.workflow_runs.push({ ...o.run }); assert.equal(inspectSupervisorRun(o).kind, 'refused');
  o.inventory.total_count = 1; o.inventory.workflow_runs.pop();
  assert.equal(inspectSupervisorRun({ ...o, binding: { id: 123, attempt: 2 } }).kind, 'refused');
  assert.equal(inspectSupervisorRun({ ...o, binding: { id: 123, attempt: null } }).kind, 'success');
});

async function cryptographicSession(t, f, { revokeFailure = false } = {}) {
  const { generateKeyPairSync, verify } = await import('node:crypto');
  const { withTrainAppSession } = await import('../../tools/ci/pipeline/train-app-session.mjs');
  const machine = machinePolicyFixture(f.queue.repository), pair = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const privateKeyFile = path.join(f.directory, 'ephemeral-fixture.pem');
  await writeFile(privateKeyFile, pair.privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  let minted = 0, revoked = 0;
  const protection = { required_status_checks: { contexts: ['train-proof'], strict: false,
    checks: [{ context: 'train-proof', app_id: 15368 }] }, required_pull_request_reviews: null,
    enforce_admins: { enabled: true }, allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  const policy = endpoint => endpoint === 'user' ? machine.observerUser : endpoint.includes('/protection') ? protection
    : endpoint.includes('/rulesets/') ? machine.ruleset : machine.branchRules;
  const transport = async (url, options) => {
    const endpoint = new URL(url).pathname + new URL(url).search;
    let data;
    if (endpoint === '/app') {
      const parts = options.headers.Authorization.slice(7).split('.');
      assert.ok(verify('RSA-SHA256', Buffer.from(`${parts[0]}.${parts[1]}`), pair.publicKey, Buffer.from(parts[2], 'base64url')), 'actual ephemeral JWT signature verifies');
      data = machine.app;
    } else if (endpoint === '/app/installations/303') data = machine.installation;
    else if (options.method === 'POST') {
      minted++; assert.deepEqual(JSON.parse(options.body), { repository_ids: [101], permissions: machine.app.permissions });
      data = { token: 'fixture-supervisor-token', permissions: machine.app.permissions,
        expires_at: new Date(Date.now() + 3500000).toISOString(), repository_selection: 'selected' };
    } else if (options.method === 'DELETE') { revoked++; return new Response(null, { status: revokeFailure ? 500 : 204 }); }
    else if (endpoint.startsWith('/installation/')) data = machine.repositories;
    else data = machine.machineRuleset;
    return new Response(JSON.stringify(data), { status: options.method === 'POST' ? 201 : 200 });
  };
  return { machine, policy, privateKeyFile, session: options => withTrainAppSession({ ...options, transport }),
    counts: () => ({ minted, revoked }) };
}
test('supervisor assembly composes actual RSA scoped session with real Git assembly and canonical admission', async t => {
  const f = await setup(t);
  const { commands } = await import('./helpers/train-command-fixture.mjs');
  const { runTrainController } = await import('../../tools/ci/pipeline/train-controller.mjs');
  const real = await commands(t);
  const added = spawnSync('git', ['remote', 'add', 'origin', real.directory], { cwd: real.directory, encoding: 'utf8' });
  assert.ok(added.status === 0);
  f.queue.targetSource = real.directory; await f.saveQueue();
  const h = await cryptographicSession(t, f);
  const result = await f.tick({ privateKeyFile: h.privateKeyFile,
    controller: options => runTrainController({ ...options, policyApi: h.policy, session: h.session,
      command: real.command, environment: { PATH: process.env.PATH } }) });
  assert.equal(result.stage, 'assembled');
  const manifest = await readSupervisorJson(f.queue.manifest);
  assert.equal(manifest.tree, real.manifest.tree);
  assert.deepEqual(manifest.inputs.map(input => input.number), f.queue.prs);
  assert.deepEqual(h.counts(), { minted: 1, revoked: 1 });
  assert.ok(!JSON.stringify(result).includes('fixture-supervisor-token'));
});
test('actual authenticated run observer blocks failed token revocation instead of reopening sessions', async t => {
  const f = await setup(t); await f.tick(); await f.tick();
  await mkdir(f.queue.worktree, { mode: 0o700 });
  const h = await cryptographicSession(t, f, { revokeFailure: true }), o = observationFixture(f);
  const command = (_program, args, options) => {
    const endpoint = args[1];
    assert.ok(!Object.hasOwn(options.env, 'FIXTURE_OBSERVER'));
    const data = ['user'].includes(endpoint) || endpoint.includes('/protection') || endpoint.includes('/rulesets/') || endpoint.includes('/rules/branches/')
      ? h.policy(endpoint) : o.api(endpoint);
    return { status: 0, stdout: JSON.stringify(data) };
  };
  const result = await f.tick({ privateKeyFile: h.privateKeyFile, session: h.session, command,
    environment: { PATH: process.env.PATH, FIXTURE_OBSERVER: 'fixture-only-owner' } });
  assert.equal(result.stage, 'blocked'); assert.equal(result.inspectionRequired, true);
  const journal = await readSupervisorJson(path.join(f.options.stateDirectory, 'journal.json'));
  assert.equal(journal.sessionFailure.revocation, 'failed');
  assert.deepEqual(h.counts(), { minted: 1, revoked: 1 });
  await f.tick(); assert.deepEqual(h.counts(), { minted: 1, revoked: 1 });
});

test('runtime identity Git children use supplied environment with all credential roles removed', async () => {
  let calls = 0;
  const result = await trainSupervisorRuntimeIdentity({ observerEnvironmentReference: 'FIXTURE_OBSERVER',
    environment: { PATH: process.env.PATH, FIXTURE_OBSERVER: 'fixture-owner', GH_TOKEN: 'fixture-human', GITHUB_TOKEN: 'fixture-actions' },
    command(_program, args, options) {
      const checked = spawnSync(process.execPath, ['-e', "process.exit(['FIXTURE_OBSERVER','GH_TOKEN','GITHUB_TOKEN'].some(k=>Object.hasOwn(process.env,k))?2:0)"],
        { env: options.env, encoding: 'utf8', timeout: 10000 });
      assert.ok(checked.status === 0, 'actual child has no credential role'); calls++;
      return { status: 0, stdout: args[0] === 'status' ? '' : 'a'.repeat(40) + '\n' };
    } });
  assert.ok(/^[a-f0-9]{64}$/.test(result) && calls >= 4);
});
test('read cadence refuses immediate repeated observation without opening a session', async t => {
  const f = await setup(t); await f.tick(); await f.tick(); let calls = 0;
  const observer = async () => { calls++; return { kind: 'waiting' }; };
  await f.tick({ clock: () => 900000, observer });
  assert.equal((await f.tick({ clock: () => 900001, observer })).observationDeferred, true);
  assert.equal(calls, 1);
});


test('downstream completion requires exact accepted publication custody and never repeats landing', async t => {
  const f = await setup(t), observer = async () => ({ kind: 'success', binding: { id: 123, attempt: 1 } });
  for (let n = 0; n < 5; n++) await f.tick({ observer });
  const binding = { repository: f.queue.repository, repositoryId: f.options.config.repositoryId,
    source: f.manifest.source, tree: f.manifest.tree, manifestDigest: f.manifest.digest,
    proofRunId: 123, proofRunAttempt: 1, acceptanceRunId: 456, acceptanceRunAttempt: 1,
    publicationRunId: 789, publicationRunAttempt: 1, publicationReceiptArtifactId: 8 };
  await assert.rejects(f.tick({ downstreamObserver: async () => ({ kind: 'complete', binding: { ...binding, source: 'f'.repeat(40) } }) }), /exact custody/);
  assert.equal((await f.tick({ downstreamObserver: async () => ({ kind: 'complete', binding }) })).stage, 'complete');
  assert.equal((await f.tick({ downstreamObserver: async () => { throw new Error('must not reobserve terminal batch'); } })).complete, true);
  assert.deepEqual(f.calls, ['assemble', 'dispatch', 'acquire', 'land']);
});
