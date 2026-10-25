import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fixture } from './helpers/train-fixture.mjs';
import { discoverTrainAdoption } from '../../tools/ci/pipeline/train-adoption.mjs';

async function setup(t, zipVariation = '') {
  const f = await fixture(t);
  const repository = f.manifest.repository;
  const repo = { full_name: repository, id: 11 };
  const pushRun = { id: 555, run_attempt: 1, head_sha: f.source, repository: repo, head_repository: repo,
    event: 'push', head_branch: 'dev', path: '.github/workflows/integration.yml', status: 'in_progress', conclusion: null };
  const run = { ...f.observation.run, run_attempt: 2, check_suite_id: 777, repository: repo, head_repository: repo };
  const checks = { total_count: 1, check_runs: [{ name: 'candidate', app: { id: 15368 },
    check_suite: { id: 777 }, head_sha: f.source, status: 'completed', conclusion: 'success' }] };
  const state = { inventory: [run], accepted: f.source, mergeBase: f.source,
    artifactList: true, tree: f.manifest.tree, zipVariation, latestChange: false, reads: 0 };
  let zip; let artifact;
  const makeZip = (manifest = f.manifest) => {
    const script = `import io,json,sys,zipfile
p=json.loads(sys.stdin.read()); b=io.BytesIO()
with zipfile.ZipFile(b,'w',compression=zipfile.ZIP_DEFLATED) as z:
 z.writestr('../train-manifest.json' if p['variation']=='traversal' else 'train-manifest.json',json.dumps(p['manifest']))
 if p['variation']=='extra': z.writestr('extra','x')
sys.stdout.buffer.write(b.getvalue())`;
    const result = spawnSync('python3', ['-c', script], { input: JSON.stringify({ manifest, variation: state.zipVariation }) });
    assert.equal(result.status, 0);
    zip = result.stdout;
    artifact = { id: 456, name: 'train-manifest', expired: false, size_in_bytes: zip.length,
      digest: `sha256:${createHash('sha256').update(zip).digest('hex')}`,
      workflow_run: { id: run.id, head_sha: f.source, repository_id: 11, head_repository_id: 11 } };
  };
  makeZip();
  const command = (program, args, options) => {
    if (program !== 'gh') return spawnSync(program, args, options);
    const endpoint = args[1]; let value;
    if (endpoint.endsWith('/zip')) return { status: 0, stdout: zip };
    if (endpoint.endsWith('/actions/artifacts/456')) value = artifact;
    else if (endpoint.endsWith('/actions/runs/555')) value = pushRun;
    else if (endpoint.endsWith(`/actions/runs/${run.id}`)) {
      state.reads++;
      value = state.latestChange && state.reads > 1 ? { ...run, run_attempt: run.run_attempt + 1 } : run;
    } else if (endpoint.includes('/workflows/integration.yml/runs?')) value = { total_count: state.inventory.length, workflow_runs: state.inventory };
    else if (endpoint.includes('/check-suites/')) value = checks;
    else if (endpoint.includes('/artifacts?')) value = { total_count: state.artifactList ? 1 : 0, artifacts: state.artifactList ? [artifact] : [] };
    else if (endpoint.endsWith('/git/ref/heads/dev')) value = { object: { sha: state.accepted } };
    else if (endpoint.includes('/compare/')) value = { status: state.accepted === f.source ? 'identical' : 'ahead', merge_base_commit: { sha: state.mergeBase } };
    else if (endpoint.includes('/git/commits/')) value = { tree: { sha: state.tree } };
    else throw new Error(`Unexpected endpoint ${endpoint}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  return { ...f, pushRun, run, state, checks, artifact: () => artifact, makeZip,
    adopt: () => discoverTrainAdoption({ pushRun, repository, directory: f.directory, command }) };
}

test('adopts authenticated exact manual proof after source accepted on dev', async t => {
  const f = await setup(t); const result = await f.adopt();
  assert.equal(result.integrationRun, f.run.id); assert.equal(result.runAttempt, 2);
  assert.deepEqual(result.manifest, f.manifest); assert.equal(result.acceptedSource, f.source);
});
test('no manual train gives no adoption without inventing success', async t => {
  const f = await setup(t); f.state.inventory = []; assert.equal(await f.adopt(), null);
});
test('advanced development retains exact accepted ancestor', async t => {
  const f = await setup(t); f.state.accepted = 'a'.repeat(40);
  assert.equal((await f.adopt()).development, f.state.accepted);
});
for (const [name, mutate] of [
  ['failed latest run', f => { f.run.conclusion = 'failure'; }],
  ['newer failed run', f => { f.state.inventory.push({ ...f.run, id: 999, conclusion: 'failure' }); }],
  ['pending latest run', f => { f.run.status = 'in_progress'; }],
  ['missing manifest', f => { f.state.artifactList = false; }],
  ['expired manifest', f => { f.artifact().expired = true; }],
  ['foreign manifest source', f => { f.artifact().workflow_run.head_sha = 'a'.repeat(40); }],
  ['foreign artifact run', f => { f.artifact().workflow_run.id++; }],
  ['wrong ZIP digest', f => { f.artifact().digest = `sha256:${'0'.repeat(64)}`; }],
  ['failed candidate', f => { f.checks.check_runs[0].conclusion = 'failure'; }],
  ['foreign candidate app', f => { f.checks.check_runs[0].app.id = 42; }],
  ['unaccepted source', f => { f.state.mergeBase = 'a'.repeat(40); }],
  ['wrong accepted tree', f => { f.state.tree = 'a'.repeat(40); }],
  ['changed run attempt', f => { f.state.latestChange = true; }],
  ['malformed train branch', f => { f.run.head_branch = 'train/invalid'; }],
]) test(`refuses ${name}, never falls back to no adoption`, async t => {
  const f = await setup(t); mutate(f); await assert.rejects(f.adopt());
});
for (const variation of ['extra', 'traversal']) test(`refuses ${variation} manifest ZIP`, async t => {
  const f = await setup(t, variation); await assert.rejects(f.adopt());
});
test('authenticated but altered manifest cannot invent selected merge tree', async t => {
  const f = await setup(t); f.makeZip({ ...f.manifest, tree: 'a'.repeat(40) });
  await assert.rejects(f.adopt(), /digest mismatch/);
});
test('rehashed foreign tree still fails actual Git reconstruction', async t => {
  const f = await setup(t);
  const altered = { ...f.manifest, tree: 'a'.repeat(40) };
  const payload = { ...altered }; delete payload.digest;
  altered.digest = createHash('sha256').update(JSON.stringify(payload)).digest('hex');
  f.makeZip(altered);
  await assert.rejects(f.adopt(), /tree|source/i);
});
