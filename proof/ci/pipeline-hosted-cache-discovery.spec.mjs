import test from 'node:test';
import assert from 'node:assert/strict';
import { discoverAcceptedHostedSeeds } from '../../tools/ci/pipeline/hosted-cache-discovery.mjs';
import { hostedCacheSeedName } from '../../tools/ci/pipeline/hosted-cache-seed.mjs';

function fixture({ adopted = false, lookback = 8 } = {}) {
  const repository = 'owner/repository', namespace = 'a'.repeat(64);
  const source = '1'.repeat(40), tree = '2'.repeat(40);
  const repo = { full_name: repository, id: 11 };
  const acceptanceRun = { id: 200, run_attempt: 2, head_sha: source, head_branch: 'dev',
    path: '.github/workflows/integration.yml', repository: repo, head_repository: repo,
    event: 'push', status: 'completed', conclusion: 'success' };
  const proofRun = { ...acceptanceRun, id: 100, run_attempt: 3, event: 'workflow_dispatch', head_branch: `train/${'b'.repeat(64)}` };
  const adoption = { run: proofRun, integrationRun: proofRun.id, runAttempt: proofRun.run_attempt,
    acceptedSource: source, manifest: { tree } };
  const artifact = { id: 500, name: hostedCacheSeedName(namespace, adopted ? 3 : 2), expired: false };
  const state = { inventory: { total_count: 1, workflow_runs: [{ ...acceptanceRun }] },
    artifacts: { total_count: 1, artifacts: [artifact] }, adoption: adopted ? adoption : null,
    development: source, comparison: { status: 'identical', merge_base_commit: { sha: source } },
    tree, calls: [], discoveries: [], discoveryError: null };
  const command = (program, args, options) => {
    assert.equal(program, 'gh'); assert.equal(args[0], 'api');
    assert.equal(options.timeout, 60000); assert.equal(options.maxBuffer, 2 * 1024 ** 2);
    const endpoint = args[1]; state.calls.push(endpoint); let value;
    if (endpoint.includes('/workflows/integration.yml/runs?')) {
      assert.equal(endpoint, `repos/${repository}/actions/workflows/integration.yml/runs?branch=dev&event=push&status=success&per_page=${lookback}`);
      value = state.inventory;
    } else if (endpoint.endsWith('/actions/runs/200')) value = acceptanceRun;
    else if (endpoint.endsWith('/git/ref/heads/dev')) value = { object: { sha: state.development } };
    else if (endpoint.includes('/compare/')) value = state.comparison;
    else if (endpoint.includes('/git/commits/')) value = { tree: { sha: state.tree } };
    else if (endpoint.endsWith('/artifacts?per_page=100')) value = state.artifacts;
    else throw new Error(`Unexpected endpoint ${endpoint}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  const discovery = async options => {
    assert.equal(options.repository, repository); assert.equal(options.command, command);
    assert.deepEqual(options.pushRun, acceptanceRun);
    state.discoveries.push(options.pushRun.id);
    if (state.discoveryError) throw state.discoveryError;
    return state.adoption;
  };
  const discover = () => discoverAcceptedHostedSeeds({ repository, namespace, lookback, directory: '/fixture', command, discovery });
  return { repository, namespace, source, tree, acceptanceRun, proofRun, adoption, artifact, state, discover, command, discovery };
}

test('ordinary accepted dev source selects its exact attempt artifact', async () => {
  const f = fixture(); const result = await f.discover();
  assert.deepEqual(result, [{ artifactId: 500, accepted: { repository: f.repository, source: f.source,
    tree: f.tree, run: f.acceptanceRun, acceptanceRun: f.acceptanceRun } }]);
  assert.deepEqual(f.state.discoveries, [200]);
  assert.ok(f.state.calls.includes(`repos/${f.repository}/actions/runs/200/artifacts?per_page=100`));
});
test('adopted dev acceptance delegates verifier and selects original manual producer artifact', async () => {
  const f = fixture({ adopted: true }); const [result] = await f.discover();
  assert.equal(result.accepted.run.id, 100); assert.equal(result.accepted.run.run_attempt, 3);
  assert.equal(result.accepted.acceptanceRun.id, 200);
  assert.deepEqual(f.state.discoveries, [200]);
  assert.ok(f.state.calls.includes(`repos/${f.repository}/actions/runs/100/artifacts?per_page=100`));
  assert.ok(!f.state.calls.some(call => call.includes('/actions/runs/200/artifacts')));
});
test('finite lookback accepts a complete bounded page without searching all history', async () => {
  const f = fixture({ lookback: 1 }); f.state.inventory.total_count = 1000;
  assert.equal((await f.discover()).length, 1);
  assert.equal(f.state.calls.filter(call => call.includes('/workflows/integration.yml/runs?')).length, 1);
  assert.ok(!f.state.calls.some(call => call.includes('page=2')));
});
test('empty accepted history gives cold result without manual or PR discovery', async () => {
  const f = fixture(); f.state.inventory = { total_count: 0, workflow_runs: [] };
  assert.deepEqual(await f.discover(), []); assert.deepEqual(f.state.discoveries, []);
  assert.equal(f.state.calls.length, 1);
});
for (const variant of ['expired', 'missing', 'other-namespace', 'other-attempt']) test(`${variant} seed gives explicit cold result`, async () => {
  const f = fixture();
  if (variant === 'expired') f.artifact.expired = true;
  if (variant === 'missing') f.state.artifacts = { total_count: 0, artifacts: [] };
  if (variant === 'other-namespace') f.artifact.name = hostedCacheSeedName('c'.repeat(64), 2);
  if (variant === 'other-attempt') f.artifact.name = hostedCacheSeedName(f.namespace, 1);
  assert.deepEqual(await f.discover(), []);
});
for (const [name, mutate] of [
  ['wrong listed source', f => { f.state.inventory.workflow_runs[0].head_sha = '3'.repeat(40); }],
  ['changed run attempt', f => { f.acceptanceRun.run_attempt++; }],
  ['incomplete accepted window', f => { f.state.inventory.total_count = 8; }],
  ['incomplete artifact inventory', f => { f.state.artifacts.total_count = 2; }],
  ['duplicate seed artifact', f => { f.state.artifacts.artifacts.push({ ...f.artifact, id: 501 }); f.state.artifacts.total_count++; }],
  ['accepted source excluded from dev', f => { f.state.comparison = { status: 'diverged', merge_base_commit: { sha: '3'.repeat(40) } }; }],
  ['wrong ancestry identity', f => { f.state.comparison.merge_base_commit.sha = '3'.repeat(40); }],
  ['old successful manual without dev acceptance', f => { Object.assign(f.acceptanceRun, { event: 'workflow_dispatch', head_branch: `train/${'b'.repeat(64)}` }); Object.assign(f.state.inventory.workflow_runs[0], f.acceptanceRun); }],
  ['successful PR without dev acceptance', f => { Object.assign(f.acceptanceRun, { event: 'pull_request', head_branch: 'feature' }); Object.assign(f.state.inventory.workflow_runs[0], f.acceptanceRun); }],
  ['forked producer', f => { f.acceptanceRun.head_repository = { full_name: 'fork/repository', id: 12 }; }],
  ['wrong workflow', f => { f.acceptanceRun.path = '.github/workflows/candidate.yml'; }],
  ['invalid seed ID', f => { f.artifact.id = 0; }],
  ['duplicate acceptance run', f => { f.state.inventory.workflow_runs.push({ ...f.state.inventory.workflow_runs[0] }); f.state.inventory.total_count++; }],
]) test(`refuses ${name}`, async () => {
  const f = fixture(); mutate(f); await assert.rejects(f.discover());
});
for (const [name, mutate] of [
  ['producer source', f => { f.proofRun.head_sha = '3'.repeat(40); }],
  ['producer attempt', f => { f.adoption.runAttempt++; }],
  ['producer run ID', f => { f.adoption.integrationRun++; }],
  ['accepted source', f => { f.adoption.acceptedSource = '3'.repeat(40); }],
  ['captured tree', f => { f.adoption.manifest.tree = '3'.repeat(40); }],
]) test(`refuses mismatched adopted ${name}`, async () => {
  const f = fixture({ adopted: true }); mutate(f); await assert.rejects(f.discover());
});
test('adoption verifier refusal propagates and cannot fall back to ordinary source', async () => {
  const f = fixture({ adopted: true }); f.state.discoveryError = new Error('Strict Git reconstruction refused');
  await assert.rejects(f.discover(), /Strict Git reconstruction refused/);
  assert.ok(!f.state.calls.some(call => call.includes('/artifacts?')));
});
for (const lookback of [0, 21, 1.5]) test(`invalid lookback ${lookback} refuses before API calls`, async () => {
  const f = fixture();
  await assert.rejects(discoverAcceptedHostedSeeds({ repository: f.repository, namespace: f.namespace,
    lookback, command: f.command, discovery: f.discovery }));
  assert.equal(f.state.calls.length, 0);
});

test('first matching accepted seed stops before irrelevant older producer', async () => {
  const f = fixture();
  f.state.inventory.total_count = 2;
  f.state.inventory.workflow_runs.push({ ...f.acceptanceRun, id: 199 });
  const selected = await discoverAcceptedHostedSeeds({ repository: f.repository, namespace: f.namespace,
    command: f.command, discovery: f.discovery, firstOnly: true });
  assert.equal(selected.length, 1);
  assert.deepEqual(f.state.discoveries, [200]);
  assert.ok(!f.state.calls.some(call => call.endsWith('/actions/runs/199')));
});
test('invalid first-only option refuses before observing GitHub', async () => {
  const f = fixture();
  await assert.rejects(discoverAcceptedHostedSeeds({ repository: f.repository, namespace: f.namespace,
    command: f.command, discovery: f.discovery, firstOnly: 'true' }), /selection mode/);
  assert.equal(f.state.calls.length, 0);
});
