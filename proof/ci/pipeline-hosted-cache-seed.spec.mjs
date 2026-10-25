import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, link } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { sealHostedCacheSeed, acquireHostedCacheSeed, retireHostedCacheSeeds, hostedCacheSeedName } from '../../tools/ci/pipeline/hosted-cache-seed.mjs';

const digest = data => createHash('sha256').update(data).digest('hex');
async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'hosted-seed-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const checkout = path.join(root, 'repo'); await mkdir(checkout);
  const git = args => {
    const result = spawnSync('git', args, { cwd: checkout, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr); return result.stdout.trim();
  };
  git(['init', '-q']); git(['config', 'user.name', 'Fixture']); git(['config', 'user.email', 'fixture@example.invalid']);
  await writeFile(path.join(checkout, 'source'), 'exact source');
  git(['add', '.']); git(['commit', '-qm', 'source']);
  const cacheRoot = path.join(root, 'cache'); await mkdir(cacheRoot);
  const policy = { schema: 3, maxCacheBytes: 1024 * 1024, compatibility: 'fixture' };
  const namespace = digest(JSON.stringify(policy));
  await writeFile(path.join(cacheRoot, 'policy.json'), JSON.stringify(policy));
  const object = path.join(cacheRoot, 'objects/a/b', 'ab' + '0'.repeat(62));
  await mkdir(path.dirname(object), { recursive: true }); await writeFile(object, 'compiler object');
  const repository = 'owner/repository', source = git(['rev-parse', 'HEAD']), tree = git(['rev-parse', 'HEAD^{tree}']);
  const repo = { full_name: repository, id: 11 };
  const run = { id: 123, run_attempt: 2, path: '.github/workflows/integration.yml', head_sha: source,
    repository: repo, head_repository: repo, event: 'workflow_dispatch', head_branch: `train/${'a'.repeat(64)}`,
    status: 'completed', conclusion: 'success' };
  const acceptanceRun = { ...run, id: 456, event: 'push', head_branch: 'dev' };
  const accepted = { repository, source, tree, run, acceptanceRun };
  const directory = path.join(root, 'sealed');
  const seal = () => sealHostedCacheSeed({ cacheRoot, policy, namespace, source, tree, run, directory, repositoryRoot: checkout });
  let zip, artifact, readCount = 0;
  const state = { wrongDigest: false, changedAttempt: false, deletes: 0, deleted: false, ambiguous: false, zipVariation: '' };
  async function zipSeed(variation = '') {
    const script = `import io,sys,zipfile,tarfile,json
p=json.loads(sys.stdin.read()); tar=open(p['tar'],'rb').read()
if p['variation'] in ['negative-tar-size','negative-object-size']:
 with tarfile.open(fileobj=io.BytesIO(tar),mode='r:') as old:
  item=next(m for m in old if (m.name=='seed.json' if p['variation']=='negative-tar-size' else m.name.startswith('objects/')))
 offset=item.offset; header=bytearray(tar[offset:offset+512])
 header[124:136]=bytes([255])*12
 header[148:156]=b'        '
 header[148:156]=('%06o' % sum(header)).encode()+bytes([0,32])
 tar=tar[:offset]+header+tar[offset+512:]
if p['variation'] in ['negative-receipt','fractional-receipt','boolean-receipt','negative-total']:
 b=io.BytesIO()
 with tarfile.open(fileobj=io.BytesIO(tar),mode='r:') as old, tarfile.open(fileobj=b,mode='w',format=tarfile.USTAR_FORMAT) as out:
  for item in old:
   data=old.extractfile(item).read()
   if item.name=='seed.json':
    receipt=json.loads(data)
    if p['variation']=='negative-total': receipt['bytes']=-1
    else: receipt['files'][0]['bytes']={'negative-receipt':-1,'fractional-receipt':0.5,'boolean-receipt':True}[p['variation']]
    data=json.dumps(receipt).encode(); item.size=len(data)
   out.addfile(item,io.BytesIO(data))
 tar=b.getvalue()
if p['variation']=='corrupt-object':
 b=io.BytesIO()
 with tarfile.open(fileobj=io.BytesIO(tar),mode='r:') as old, tarfile.open(fileobj=b,mode='w',format=tarfile.USTAR_FORMAT) as out:
  for item in old:
   data=old.extractfile(item).read()
   if item.name.startswith('objects/'): data=b'x'*len(data)
   out.addfile(item,io.BytesIO(data))
 tar=b.getvalue()
if p['variation'] in ['traversal','symlink','duplicate']:
 b=io.BytesIO()
 with tarfile.open(fileobj=b,mode='w',format=tarfile.USTAR_FORMAT) as out:
  info=tarfile.TarInfo('../escape' if p['variation']=='traversal' else 'seed.json')
  if p['variation']=='symlink': info.type=tarfile.SYMTYPE; info.linkname='/etc/passwd'; out.addfile(info)
  else:
   info.size=2; out.addfile(info,io.BytesIO(b'{}'))
   if p['variation']=='duplicate': out.addfile(info,io.BytesIO(b'{}'))
 tar=b.getvalue()
b=io.BytesIO()
with zipfile.ZipFile(b,'w',compression=zipfile.ZIP_DEFLATED) as out:
 out.writestr('seed.tar',tar)
 if p['variation']=='extra': out.writestr('extra','x')
sys.stdout.buffer.write(b.getvalue())`;
    const made = spawnSync('python3', ['-c', script], { input: JSON.stringify({ tar: path.join(directory, 'seed.tar'), variation }) });
    assert.equal(made.status, 0, made.stderr?.toString()); zip = made.stdout;
    artifact = { id: 789, name: hostedCacheSeedName(namespace, run.run_attempt), size_in_bytes: zip.length,
      digest: `sha256:${digest(zip)}`, expired: false,
      workflow_run: { id: run.id, head_sha: source, repository_id: 11, head_repository_id: 11 } };
  }
  const command = async (program, args, options) => {
    assert.equal(program, 'gh'); const endpoint = args[1]; let value;
    if (args.includes('DELETE')) {
      state.deletes++; state.deleted = true;
      return { status: state.ambiguous ? 1 : 0, stdout: '' };
    }
    if (args.includes('--include')) return { status: 1, stdout: 'HTTP/2.0 404 Not Found\n{}' };
    if (endpoint.endsWith('/zip')) { await options.output.write(zip); return { status: 0, stdout: '' }; }
    if (endpoint.endsWith('/actions/runs/123')) { readCount++; value = state.changedAttempt && readCount > 1 ? { ...run, run_attempt: 3 } : run; }
    else if (endpoint.endsWith('/actions/runs/456')) value = acceptanceRun;
    else if (endpoint.endsWith('/actions/artifacts/789')) value = state.wrongDigest ? { ...artifact, digest: `sha256:${'0'.repeat(64)}` } : artifact;
    else throw new Error(`Unexpected endpoint ${endpoint}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  const acquire = () => acquireHostedCacheSeed({ accepted, policy, namespace, artifactId: 789,
    directory: path.join(root, 'acquired'), repositoryRoot: checkout, command });
  const retire = () => retireHostedCacheSeeds({ repository, namespace, artifactIds: [789], directory: path.join(root, 'retirement'), command });
  return { root, cacheRoot, object, policy, namespace, run, accepted, state, seal, zipSeed, acquire, retire, artifact: () => artifact };
}

test('sealed managed objects restore from exact accepted artifact, without proof outputs', async t => {
  const f = await fixture(t); const sealed = await f.seal(); await f.zipSeed();
  const restored = await f.acquire();
  assert.equal(restored.receipt.source, f.accepted.source);
  assert.equal(await readFile(path.join(restored.cacheRoot, 'objects/a/b', path.basename(f.object)), 'utf8'), 'compiler object');
  assert.equal(sealed.receipt.files.length, 1);
  await assert.rejects(readFile(path.join(restored.cacheRoot, 'producer.json')), { code: 'ENOENT' });
});
for (const variation of ['lease', 'retirement', 'unknown', 'symlink', 'hardlink', 'policy']) test(`seal refuses ${variation} and preserves objects`, async t => {
  const f = await fixture(t);
  if (variation === 'lease') await mkdir(path.join(f.cacheRoot, '.cargo-lease'));
  if (variation === 'retirement') await writeFile(path.join(f.cacheRoot, 'retirement.json'), '{}');
  if (variation === 'unknown') await writeFile(path.join(f.cacheRoot, 'proof.json'), '{}');
  if (variation === 'symlink') { await rm(f.object); await symlink('/etc/passwd', f.object); }
  if (variation === 'hardlink') await link(f.object, path.join(f.root, 'other'));
  if (variation === 'policy') await writeFile(path.join(f.cacheRoot, 'policy.json'), '{}');
  await assert.rejects(f.seal());
  if (!['symlink'].includes(variation)) assert.equal(await readFile(f.object, 'utf8'), 'compiler object');
});
for (const variation of ['pr', 'fork', 'unaccepted', 'wrong-tree', 'wrong-digest', 'wrong-name', 'wrong-attempt', 'changed-attempt', 'extra', 'traversal', 'symlink', 'duplicate', 'corrupt-object']) {
  test(`acquisition refuses ${variation}`, async t => {
    const f = await fixture(t); await f.seal(); await f.zipSeed(variation);
    if (variation === 'pr') f.run.event = 'pull_request';
    if (variation === 'fork') f.run.head_repository = { full_name: 'fork/repository', id: 12 };
    if (variation === 'unaccepted') f.accepted.acceptanceRun = f.run;
    if (variation === 'wrong-tree') f.accepted.tree = '3'.repeat(40);
    if (variation === 'wrong-digest') f.state.wrongDigest = true;
    if (variation === 'wrong-name') f.artifact().name = 'product-hosted-linux';
    if (variation === 'wrong-attempt') f.artifact().name = hostedCacheSeedName(f.namespace, 1);
    if (variation === 'changed-attempt') f.state.changedAttempt = true;
    await assert.rejects(f.acquire());
    await assert.rejects(readFile(path.join(f.root, 'escape')), { code: 'ENOENT' });
  });
}
test('explicit retirement records exact supplied IDs and confirms404 without namespace-clear claim', async t => {
  const f = await fixture(t); await f.seal(); await f.zipSeed();
  const result = await f.retire();
  assert.equal(f.state.deletes, 1); assert.equal(result.completeNamespaceClear, false);
  assert.equal(JSON.parse(await readFile(path.join(f.root, 'retirement/intent.json'))).artifacts[0].id, 789);
});
for (const variation of ['negative-tar-size', 'negative-object-size', 'negative-receipt', 'fractional-receipt', 'boolean-receipt', 'negative-total']) {
  test(`acquisition refuses ${variation} before object extraction`, async t => {
    const f = await fixture(t); await f.seal(); await f.zipSeed(variation);
    await assert.rejects(f.acquire(), /Seed tar size bound|Seed object byte bound|Seed total bytes differs/);
    await assert.rejects(readFile(path.join(f.root, 'acquired/restored/cache/objects/a/b', path.basename(f.object))), { code: 'ENOENT' });
  });
}
test('retirement refuses product artifact before mutation', async t => {
  const f = await fixture(t); await f.seal(); await f.zipSeed(); f.artifact().name = 'product-hosted-linux';
  await assert.rejects(f.retire()); assert.equal(f.state.deletes, 0);
});
test('ambiguous delete retains intent, does not retry, and refuses existing destination', async t => {
  const f = await fixture(t); await f.seal(); await f.zipSeed(); f.state.ambiguous = true;
  await assert.rejects(f.retire()); assert.equal(f.state.deletes, 1);
  await assert.rejects(f.retire()); assert.equal(f.state.deletes, 1);
  assert.equal(JSON.parse(await readFile(path.join(f.root, 'retirement/intent.json'))).completeNamespaceClear, false);
});
