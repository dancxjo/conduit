import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, lstat, symlink } from 'node:fs/promises';
import path from 'node:path';
import { fixture } from './helpers/train-fixture.mjs';
import { restoreTrainArtifacts } from '../../tools/ci/pipeline/train-restore.mjs';
import { producerFixture } from './helpers/producer-fixture.mjs';

async function setup(t, variation = '') {
  const f = await fixture(t); const run = f.observation.run;
  run.run_attempt = 1; run.repository = { ...run.repository, id: 11 };
  run.head_repository = { ...run.head_repository, id: 11 };
  const acquisition = path.join(f.directory, 'acquired'); await mkdir(acquisition);
  const archive = path.join(acquisition, 'product-hosted-linux.zip');
  const data = Buffer.from('verified product');
  const producer = producerFixture({ source: f.source, tree: f.manifest.tree, target: 'hosted-linux', run });
  if (variation === 'old-producer-attempt') producer.run.attempt++;
  const producerBytes = JSON.stringify(producer);
  const productReceipt = { schema: 'conduit.pipeline-target/v1', target: 'hosted-linux', sha: f.source,
    proofClass: 'executable', files: [{ path: 'product', size: data.length,
      sha256: createHash('sha256').update(data).digest('hex') },
    { path: 'producer.json', size: Buffer.byteLength(producerBytes),
      sha256: createHash('sha256').update(producerBytes).digest('hex') }] };
  const python = `import io,json,sys,tarfile,zipfile
p=json.loads(sys.stdin.read())
b=io.BytesIO()
with tarfile.open(fileobj=b,mode='w',format=tarfile.USTAR_FORMAT) as t:
 for name,data in [('hosted-linux/product','verified product'),('hosted-linux/receipt.json',json.dumps(p['receipt'])),('hosted-linux/producer.json',p['producer'])]:
  i=tarfile.TarInfo(name); raw=data.encode(); i.size=len(raw); t.addfile(i,io.BytesIO(raw))
 v=p['variation']
 if v and v!='old-producer-attempt':
  i=tarfile.TarInfo({'traversal':'hosted-linux/../outside','extra':'foreign/file','duplicate':'hosted-linux/product','link':'hosted-linux/link','special':'hosted-linux/device'}.get(v,'hosted-linux/unused'))
  if v=='link': i.type=tarfile.SYMTYPE; i.linkname='/tmp'
  if v=='special': i.type=tarfile.CHRTYPE
  if v=='oversized-tar': i.size=4*1024**3+1
  if v=='oversized-extension': i.type=b'x'; i.size=65537
  if v=='oversized-tar': t.fileobj.write(i.tobuf()); t.offset+=512
  else: t.addfile(i)
with zipfile.ZipFile(p['archive'],'w',compression=zipfile.ZIP_DEFLATED) as z:
 z.writestr('wrong.tar' if p['variation']=='wrong-zip' else 'hosted-linux.tar',b.getvalue())
 if p['variation']=='extra-zip': z.writestr('extra','x')
`;
  const made = spawnSync('python3', ['-c', python], { input: JSON.stringify({ archive, receipt: productReceipt, variation, producer: producerBytes }), encoding: 'utf8' });
  assert.equal(made.status, 0, made.stderr);
  const bytes = await readFile(archive);
  const artifact = { id: 456, name: 'product-hosted-linux', expired: false,
    size_in_bytes: bytes.length, digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}`,
    workflow_run: { id: run.id, head_sha: f.source, repository_id: 11, head_repository_id: 11 } };
  const receipt = { schema: 'conduit.train-artifact-acquisition/v1', repository: f.manifest.repository,
    repositoryId: 11, manifestDigest: f.manifest.digest, source: f.source, tree: f.manifest.tree,
    runId: run.id, runAttempt: 1, artifacts: [{ id: 456, name: artifact.name,
      archive: 'product-hosted-linux.zip', bytes: bytes.length, digest: artifact.digest }] };
  const save = () => writeFile(path.join(acquisition, 'acquisition.json'), JSON.stringify(receipt)); await save();
  let refreshes = 0; let finalRun = run;
  const metadata = async endpoint => {
    if (endpoint.includes('/actions/artifacts/')) return artifact;
    refreshes++; return refreshes > 1 ? finalRun : run;
  };
  const directory = path.join(f.directory, 'restored');
  return { ...f, acquisition, archive, artifact, receipt, save, directory,
    final: value => { finalRun = value; },
    restore: overrides => restoreTrainArtifacts({ manifest: f.manifest, observation: f.observation,
      acquisition, directory, metadata, targets: [{ id: 'hosted-linux', proofClass: 'executable' }], ...overrides }) };
}

test('restores live authenticated archives and verifies exact target bundle', async t => {
  const f = await setup(t); const result = await f.restore();
  assert.equal(result.runAttempt, 1);
  assert.equal(await readFile(path.join(result.bundle, 'hosted-linux/product'), 'utf8'), 'verified product');
  assert.equal(result.receipts[0].sha, f.source);
});
for (const variation of ['traversal', 'extra', 'duplicate', 'link', 'special', 'wrong-zip', 'extra-zip', 'oversized-tar', 'oversized-extension']) {
  test(`refuses ${variation} archive while preserving failed evidence`, async t => {
    const f = await setup(t, variation); await assert.rejects(f.restore());
    await assert.rejects(lstat(path.join(f.directory, 'restoration.json')), { code: 'ENOENT' });
    assert.equal((await lstat(f.archive)).isFile(), true);
  });
}
for (const [name, mutate] of [
  ['foreign run', f => { f.artifact.workflow_run.id++; }],
  ['foreign source', f => { f.artifact.workflow_run.head_sha = 'a'.repeat(40); }],
  ['expired artifact', f => { f.artifact.expired = true; }],
  ['fake acquisition digest', f => { f.receipt.artifacts[0].digest = `sha256:${'0'.repeat(64)}`; }],
  ['foreign name', f => { f.artifact.name = 'product-other'; }],
]) test(`refuses ${name} from live custody`, async t => {
  const f = await setup(t); mutate(f); await f.save(); await assert.rejects(f.restore());
  await assert.rejects(lstat(f.directory), { code: 'ENOENT' });
});
test('refuses changed attempt after verified extraction', async t => {
  const f = await setup(t); f.final({ ...f.observation.run, run_attempt: 2 });
  await assert.rejects(f.restore(), /attempt changed/);
  await assert.rejects(lstat(path.join(f.directory, 'restoration.json')), { code: 'ENOENT' });
});
test('an authentic old-attempt artifact cannot be adopted as current proof', async t => {
  const f = await setup(t, 'old-producer-attempt');
  await assert.rejects(f.restore(), /producer run identity mismatch/);
  await assert.rejects(lstat(path.join(f.directory, 'restoration.json')), { code: 'ENOENT' });
});
test('refuses altered archive bytes even with original acquisition', async t => {
  const f = await setup(t); const bytes = await readFile(f.archive); bytes[0] ^= 1; await writeFile(f.archive, bytes);
  await assert.rejects(f.restore(), /digest mismatch/);
});
test('refuses symbolic archive paths', async t => {
  const f = await setup(t); const replacement = f.archive + '-replacement';
  await writeFile(replacement, await readFile(f.archive));
  const { unlink } = await import('node:fs/promises'); await unlink(f.archive); await symlink(replacement, f.archive);
  await assert.rejects(f.restore(), /type mismatch/);
});
