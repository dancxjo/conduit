import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstat, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fixture } from './helpers/train-fixture.mjs';
import { stageTrainPublication } from '../../tools/ci/pipeline/train-publication.mjs';

const hash = value => createHash('sha256').update(value).digest('hex');
async function setup(t, changeProducer = () => {}, ordinary = false) {
  const f = await fixture(t); const repository = f.manifest.repository;
  const repo = { full_name: repository, id: 11 };
  const run = { ...f.observation.run, repository: repo, head_repository: repo, run_attempt: 2 };
  const pushRun = { ...run, id: 555, run_attempt: 1, event: 'push', head_branch: 'dev' };
  const proofRun = ordinary ? pushRun : run;
  const acquisitionReport = { target: 'hosted-linux', sourceSha: f.source, outcome: 'success' };
  const producer = { schema: 'conduit.pipeline-producer/v1', source: f.source, tree: f.manifest.tree,
    target: 'hosted-linux', manifestDigest: ordinary ? null : f.manifest.digest,
    run: { repository, repositoryId: 11, id: proofRun.id, attempt: proofRun.run_attempt, source: f.source,
      workflowSource: f.source, workflow: `${repository}/.github/workflows/integration.yml@refs/heads/${proofRun.head_branch}`, job: 'target' },
    toolchain: { rustc: 'rustc pinned', cargo: 'cargo pinned', node: 'node pinned',
      acquisition: { report: acquisitionReport, sha256: hash(JSON.stringify(acquisitionReport, null, 2) + '\n') } },
    environment: { sha256: 'a'.repeat(64), platform: 'linux', arch: 'x64', kernel: 'test', image: { os: 'test', version: '1' } },
    selection: { policyTree: f.manifest.tree, inventory: 'source-bound-command-contract', commands: [{
      program: 'cargo', args: ['build'], cwd: '/source', environmentSha256: 'a'.repeat(64), outcome: 'success', durationMs: 1 }] } };
  changeProducer(producer);
  const files = { product: 'verified executable', 'producer.json': JSON.stringify(producer) };
  const receipt = { schema: 'conduit.pipeline-target/v1', target: 'hosted-linux', sha: f.source,
    proofClass: 'executable', files: Object.entries(files).map(([name, data]) => ({ path: name,
      size: Buffer.byteLength(data), sha256: hash(data) })) };
  files['receipt.json'] = JSON.stringify(receipt);
  const script = `import io,json,sys,tarfile,zipfile
p=json.loads(sys.stdin.read()); tar=io.BytesIO()
with tarfile.open(fileobj=tar,mode='w',format=tarfile.USTAR_FORMAT) as t:
 for name,data in p.items():
  i=tarfile.TarInfo('hosted-linux/'+name); raw=data.encode(); i.size=len(raw); t.addfile(i,io.BytesIO(raw))
b=io.BytesIO()
with zipfile.ZipFile(b,'w',compression=zipfile.ZIP_DEFLATED) as z: z.writestr('hosted-linux.tar',tar.getvalue())
sys.stdout.buffer.write(b.getvalue())`;
  const zip = spawnSync('python3', ['-c', script], { input: JSON.stringify(files) }); assert.equal(zip.status, 0);
  const bytes = zip.stdout;
  const artifact = { id: 456, name: 'product-hosted-linux', expired: false, size_in_bytes: bytes.length,
    digest: `sha256:${hash(bytes)}`, workflow_run: { id: proofRun.id, head_sha: f.source, repository_id: 11, head_repository_id: 11 } };
  const state = { adoption: !ordinary, changedDiscovery: false, discoveries: 0, downloads: 0, actualPush: pushRun };
  const observed = { manifest: f.manifest, run, integrationRun: run.id, runAttempt: run.run_attempt,
    manifestDigest: f.manifest.digest, acceptedSource: f.source, development: f.source };
  const metadata = endpoint => {
    if (endpoint.endsWith('/actions/runs/555')) return state.actualPush;
    if (endpoint.endsWith(`/actions/runs/${run.id}`)) return run;
    if (endpoint.includes('/artifacts?')) return { total_count: 1, artifacts: [artifact] };
    if (endpoint.endsWith('/actions/artifacts/456')) return artifact;
    throw new Error(`Unexpected endpoint ${endpoint}`);
  };
  const command = (program, args) => {
    assert.equal(program, 'gh'); return { status: 0, stdout: JSON.stringify(metadata(args[1])) };
  };
  const artifactCommand = async (program, args, options) => {
    if (args[1].endsWith('/zip')) { state.downloads++; await options.output.write(bytes); return { status: 0 }; }
    return command(program, args, options);
  };
  const discovery = async () => {
    state.discoveries++;
    if (!state.adoption) return null;
    return state.changedDiscovery && state.discoveries > 1 ? { ...observed, runAttempt: 99 } : observed;
  };
  const directory = path.join(f.directory, 'publication-stage');
  return { ...f, state, artifact, producer, directory, run, pushRun,
    stage: () => stageTrainPublication({ pushRun, repository, directory, sourceDirectory: f.directory,
      command, artifactCommand, discovery, targets: [{ id: 'hosted-linux', proofClass: 'executable' }] }) };
}

test('stages authenticated accepted manual products through real bounded acquisition/restoration', async t => {
  const f = await setup(t); const result = await f.stage();
  assert.equal(result.adopted, true); assert.equal(result.proofRunId, f.run.id);
  assert.equal(result.acceptanceRunId, f.pushRun.id); assert.equal(f.state.downloads, 1);
  assert.equal(await readFile(path.join(result.bundle, 'hosted-linux/product'), 'utf8'), 'verified executable');
});
test('ordinary dev proof authenticates its own archives and producer custody', async t => {
  const f = await setup(t, () => {}, true);
  const result = await f.stage();
  assert.equal(result.adopted, false); assert.equal(result.proofRunId, f.pushRun.id);
  assert.equal(result.manifestDigest, null); assert.equal(f.state.downloads, 1);
  assert.equal(await readFile(path.join(result.bundle, 'hosted-linux/product'), 'utf8'), 'verified executable');
});
for (const status of ['in_progress', 'failure']) test(`refuses actual ${status} dev acceptance before downloading`, async t => {
  const f = await setup(t); f.state.actualPush = { ...f.pushRun,
    status: status === 'in_progress' ? status : 'completed', conclusion: status === 'failure' ? status : null };
  await assert.rejects(f.stage()); assert.equal(f.state.downloads, 0);
});
for (const [name, alter] of [
  ['foreign producer attempt', producer => { producer.run.attempt++; }],
  ['foreign manifest digest', producer => { producer.manifestDigest = 'b'.repeat(64); }],
  ['missing acquisition custody', producer => { delete producer.toolchain.acquisition; }],
]) test(`refuses authenticated archive with ${name}`, async t => {
  const f = await setup(t, alter); await assert.rejects(f.stage());
  await assert.rejects(lstat(path.join(f.directory, 'publication-staging.json')), { code: 'ENOENT' });
});
test('changed adoption proof after staging fails without fallback', async t => {
  const f = await setup(t); f.state.changedDiscovery = true;
  await assert.rejects(f.stage(), /proof changed/);
  await assert.rejects(lstat(path.join(f.directory, 'publication-staging.json')), { code: 'ENOENT' });
});
test('altered API archive digest refuses without product substitution', async t => {
  const f = await setup(t); f.artifact.digest = `sha256:${'0'.repeat(64)}`;
  await assert.rejects(f.stage(), /digest mismatch/);
});

for (const [name, alter] of [
  ['foreign ordinary attempt', producer => { producer.run.attempt++; }],
  ['invented ordinary train digest', producer => { producer.manifestDigest = 'b'.repeat(64); }],
]) test(`ordinary proof refuses ${name}`, async t => {
  const f = await setup(t, alter, true);
  await assert.rejects(f.stage());
  await assert.rejects(lstat(path.join(f.directory, 'publication-staging.json')), { code: 'ENOENT' });
});
