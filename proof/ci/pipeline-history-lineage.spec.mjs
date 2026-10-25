import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { prepareCaptureLineagePolicy } from '../../tools/ci/pipeline/history-lineage-create.mjs';
import { CAPTURE_LINEAGE_POLICY, createCaptureLineagePolicy, historyMap, requireCaptureLineage } from '../../tools/ci/pipeline/history-lineage.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'capture-lineage-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const environment = { ...process.env, GIT_AUTHOR_NAME: 'Capture', GIT_AUTHOR_EMAIL: 'capture@example.invalid',
    GIT_COMMITTER_NAME: 'Capture', GIT_COMMITTER_EMAIL: 'capture@example.invalid',
    GIT_AUTHOR_DATE: '2020-01-01T00:00:00Z', GIT_COMMITTER_DATE: '2020-01-01T00:00:00Z' };
  function repo(name) {
    const cwd = path.join(root, name); mkdirSync(cwd);
    const git = (args, input) => execFileSync('git', args, { cwd, env: environment, input, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }).trim();
    git(['init', '-q']);
    const blob = value => git(['hash-object', '-w', '--stdin'], value);
    const empty = git(['mktree'], '');
    const tree = (name, contents) => git(['mktree'], `100644 blob ${blob(contents)}\t${name}\n`);
    const commit = (tree, parents = [], message = 'capture\n') => git(['commit-tree', tree, ...parents.flatMap(parent => ['-p', parent])], message);
    return { cwd, git, empty, tree, commit };
  }
  const old = repo('original'), mapped = repo('rewritten');
  const originalBase = old.commit(old.tree('historical-evidence.bin', 'original preserved evidence'));
  const mappedBase = mapped.commit(mapped.empty);
  const original = old.commit(old.tree('source.txt', 'same full source'), [originalBase]);
  const rewritten = mapped.commit(mapped.tree('source.txt', 'same full source'), [mappedBase]);
  mapped.git(['update-ref', 'refs/heads/main', rewritten]); mapped.git(['symbolic-ref', 'HEAD', 'refs/heads/main']);
  const mapBytes = Buffer.from(`old                                      new\n${originalBase} ${mappedBase}\n${original} ${rewritten}\n`);
  const locator = (name, bytes, sha256, assetId) => ({ releaseId: 123, assetId, bytes, sha256,
    url: `https://github.com/dancxjo/conduit-evidence/releases/download/migration/${name}` });
  const options = { mapBytes, captures: [original], originalDirectory: old.cwd, mappedDirectory: mapped.cwd,
    mapAsset: locator('commit-map.txt', mapBytes.length, hash(mapBytes), 1),
    originalBackup: locator('original.bundle', 1000, 'a'.repeat(64), 2) };
  const policy = createCaptureLineagePolicy(options);
  const policyFile = path.join(mapped.cwd, CAPTURE_LINEAGE_POLICY);
  function install(value = policy) {
    mkdirSync(path.dirname(policyFile), { recursive: true }); writeFileSync(policyFile, JSON.stringify(value) + '\n');
    mapped.git(['add', CAPTURE_LINEAGE_POLICY]);
    mapped.git(['-c', 'user.name=Capture', '-c', 'user.email=capture@example.invalid', 'commit', '-qm', 'Pin historical migration custody']);
    return mapped.git(['rev-parse', 'HEAD']);
  }
  return { root, old, mapped, originalBase, mappedBase, original, rewritten, options, policy, policyFile, install,
    check: publication => requireCaptureLineage(original, publication, { cwd: mapped.cwd }) };
}

test('historical capture bridges through committed map inclusion, exact source and actual mapped ancestry', t => {
  const f = fixture(t); const publication = f.install();
  assert.throws(() => f.mapped.git(['cat-file', '-t', f.original])); // Original history is absent from clean rewritten clone.
  const result = f.check(publication);
  assert.equal(result.relation, 'mapped-history');
  assert.equal(result.captureSource, f.original);
  assert.equal(result.mappedCaptureSource, f.rewritten);
  assert.equal(result.publicationSource, publication);
  assert.equal(result.mapAsset.sha256, hash(f.options.mapBytes));
  assert.equal(result.policySha256, hash(readFileSync(f.policyFile)));
});

test('without a committed migration policy only actual complete ancestry is accepted', t => {
  const f = fixture(t);
  assert.throws(() => f.check(f.rewritten), /Exact ancestry Git failed/);
  const actual = requireCaptureLineage(f.mappedBase, f.rewritten, { cwd: f.mapped.cwd });
  assert.equal(actual.relation, 'actual-ancestry');
  mkdirSync(path.dirname(f.policyFile), { recursive: true }); writeFileSync(f.policyFile, JSON.stringify(f.policy));
  assert.throws(() => f.check(f.rewritten), /uncommitted migration policy/);
});

for (const [name, mutate, message] of [
  ['changed map count', p => { p.map.count = 1; }, /inclusion proof length|authenticated map/],
  ['changed map proof', p => { p.captures[0].proof[0].hash = 'f'.repeat(64); }, /authenticated map/],
  ['changed original bytes', p => { p.captures[0].originalCommit = Buffer.from('invented original').toString('base64'); }, /witness hash differs/],
  ['missing parent witness', p => { p.captures[0].parents = []; }, /parent inventory differs/],
  ['changed parent identity', p => { p.captures[0].parents[0].original = 'f'.repeat(40); }, /parent order differs/],
  ['changed mapped parent', p => { p.captures[0].parents[0].mapped = 'e'.repeat(40); }, /authenticated map/],
  ['duplicate capture', p => { p.captures.push(p.captures[0]); }, /invalid committed/],
  ['untrusted asset location', p => { p.map.asset.url = 'https://example.invalid/map'; }, /custody URL/],
  ['invalid custody digest', p => { p.originalBackup.sha256 = 'short'; }, /custody locator/],
]) test(`refuses ${name} even in a committed policy`, t => {
  const f = fixture(t); const policy = structuredClone(f.policy); mutate(policy);
  const publication = f.install(policy); assert.throws(() => f.check(publication), message);
});

test('dirty policy cannot replace the committed migration pin', t => {
  const f = fixture(t); const publication = f.install();
  writeFileSync(f.policyFile, JSON.stringify({ ...f.policy, repository: 'attacker/repo' }));
  assert.throws(() => f.check(publication), /differs from committed/);
});

test('source, message and parent changes are refused while constructing witnesses', t => {
  const f = fixture(t);
  for (const changed of [
    f.mapped.commit(f.mapped.tree('source.txt', 'changed code'), [f.mappedBase]),
    f.mapped.commit(f.mapped.tree('source.txt', 'same full source'), [f.mappedBase], 'different message\n'),
    f.mapped.commit(f.mapped.tree('source.txt', 'same full source'), []),
  ]) {
    const mapBytes = Buffer.from(f.options.mapBytes.toString().replace(f.rewritten, changed));
    assert.throws(() => createCaptureLineagePolicy({ ...f.options, mapBytes,
      mapAsset: { ...f.options.mapAsset, bytes: mapBytes.length, sha256: hash(mapBytes) } }), /changes tree, topology, authorship or message/);
  }
});

test('frozen map bytes, completeness and unique original identities are enforced', t => {
  const f = fixture(t);
  assert.throws(() => createCaptureLineagePolicy({ ...f.options, mapBytes: Buffer.from('different map') }), /differs from frozen/);
  const mapBytes = Buffer.from(`old new\n${f.original} ${f.rewritten}\n`);
  assert.throws(() => createCaptureLineagePolicy({ ...f.options, mapBytes,
    mapAsset: { ...f.options.mapAsset, bytes: mapBytes.length, sha256: hash(mapBytes) } }), /parent absent/);
  assert.throws(() => historyMap([{ original: f.original, mapped: f.rewritten }, { original: f.original, mapped: f.rewritten }]), /duplicate/);
});

test('missing mapped history, grafts, replacement refs and shallow history cannot forge publication lineage', t => {
  const f = fixture(t); const publication = f.install();
  const unrelated = f.mapped.commit(f.mapped.empty, [], 'unrelated\n');
  const forged = f.mapped.commit(f.mapped.empty, [f.rewritten], 'forged\n');
  f.mapped.git(['replace', unrelated, forged]);
  assert.throws(() => f.check(unrelated), /policy checkout|not in publication/);
  mkdirSync(path.join(f.mapped.cwd, '.git/info'), { recursive: true });
  writeFileSync(path.join(f.mapped.cwd, '.git/info/grafts'), '');
  assert.throws(() => f.check(publication), /graft/);
  rmSync(path.join(f.mapped.cwd, '.git/info/grafts'));
  writeFileSync(path.join(f.mapped.cwd, '.git/shallow'), publication + '\n');
  assert.throws(() => f.check(publication), /non-shallow/);
  rmSync(path.join(f.mapped.cwd, '.git/shallow'));
  rmSync(path.join(f.mapped.cwd, '.git/objects', f.mappedBase.slice(0, 2), f.mappedBase.slice(2)));
  assert.throws(() => f.check(publication), /Exact ancestry Git failed/);
});


test('symlink cannot stand in for committed source', t => {
  const f = fixture(t); const publication = f.install();
  const retained = path.join(f.root, 'retained-policy.json'); writeFileSync(retained, readFileSync(f.policyFile));
  rmSync(f.policyFile); symlinkSync(retained, f.policyFile);
  assert.throws(() => f.check(publication), /unsafe committed/);
});

async function preparationFixture(t) {
  const f = fixture(t);
  const mapFile = path.join(f.root, 'commit-map'); writeFileSync(mapFile, f.options.mapBytes);
  const capturesFile = path.join(f.root, 'captures.json'); writeFileSync(capturesFile, JSON.stringify([f.original]));
  const custodyFile = path.join(f.root, 'custody.json'); writeFileSync(custodyFile, JSON.stringify({
    mapAsset: f.options.mapAsset, originalBackup: f.options.originalBackup,
    captures: ['f'.repeat(40)], // Unrecognized input cannot replace the separate capture inventory.
  }));
  const release = { id: 123, draft: false, immutable: true, assets: [f.options.mapAsset, f.options.originalBackup].map(asset => ({
    id: asset.assetId, size: asset.bytes, digest: `sha256:${asset.sha256}`, state: 'uploaded', browser_download_url: asset.url,
  })) };
  const outputFile = path.join(f.root, 'new-policy.json');
  let calls = 0;
  const command = (program, args) => {
    assert.equal(program, 'gh'); assert.deepEqual(args, ['api', 'repos/dancxjo/conduit-evidence/releases/123']); calls++;
    return { status: 0, stdout: JSON.stringify(release) };
  };
  return { ...f, release, outputFile, calls: () => calls, prepare: (overrides = {}) => prepareCaptureLineagePolicy({
    originalDirectory: f.old.cwd, mappedDirectory: f.mapped.cwd, mapFile, capturesFile, custodyFile, outputFile, command, ...overrides,
  }) };
}

test('supported preparation authenticates immutable asset custody and emits a separate uninstalled policy', async t => {
  const f = await preparationFixture(t); const head = f.mapped.git(['rev-parse', 'HEAD']);
  const result = await f.prepare();
  assert.equal(result.installed, false); assert.equal(result.captures, 1); assert.equal(f.calls(), 1);
  assert.deepEqual(JSON.parse(readFileSync(f.outputFile)), f.policy);
  assert.equal(f.mapped.git(['rev-parse', 'HEAD']), head);
  await assert.rejects(f.prepare(), /already exists/);
  assert.equal(f.calls(), 1);
});

for (const [name, mutate] of [
  ['mutable release', release => { release.immutable = false; }],
  ['draft release', release => { release.draft = true; }],
  ['wrong release', release => { release.id++; }],
  ['asset digest mismatch', release => { release.assets[0].digest = 'sha256:' + 'f'.repeat(64); }],
  ['duplicate asset ID', release => { release.assets.push(release.assets[0]); }],
  ['missing original backup', release => { release.assets.pop(); }],
]) test(`preparation refuses ${name} without output`, async t => {
  const f = await preparationFixture(t); mutate(f.release);
  await assert.rejects(f.prepare(), /immutable release|asset identity/);
  assert.throws(() => readFileSync(f.outputFile), { code: 'ENOENT' });
});


test('preparation refuses output in a third live Git checkout', async t => {
  const f = await preparationFixture(t);
  const third = path.join(f.root, 'third-source'); mkdirSync(third);
  execFileSync('git', ['init', '-q', third]);
  await assert.rejects(f.prepare({ outputFile: path.join(third, 'policy.json') }), /outside every Git/);
  assert.equal(f.calls(), 0);
});

test('committed symlink blob cannot be replaced by equal regular working bytes', t => {
  const f = fixture(t);
  mkdirSync(path.dirname(f.policyFile), { recursive: true });
  const bytes = JSON.stringify(f.policy) + '\n';
  const blob = f.mapped.git(['hash-object', '-w', '--stdin'], bytes);
  f.mapped.git(['update-index', '--add', '--cacheinfo', `120000,${blob},${CAPTURE_LINEAGE_POLICY}`]);
  f.mapped.git(['-c', 'user.name=Capture', '-c', 'user.email=capture@example.invalid', 'commit', '-qm', 'Invalid policy mode']);
  writeFileSync(f.policyFile, bytes);
  assert.throws(() => f.check(f.mapped.git(['rev-parse', 'HEAD'])), /must be a regular blob/);
});


test('map count is authenticated even within the same proof-depth interval', t => {
  const f = fixture(t);
  const originalExtra = f.old.commit(f.old.empty, [f.originalBase], 'extra\n');
  const mappedExtra = f.mapped.commit(f.mapped.empty, [f.mappedBase], 'extra\n');
  const mapBytes = Buffer.concat([f.options.mapBytes, Buffer.from(`${originalExtra} ${mappedExtra}\n`)]);
  const policy = createCaptureLineagePolicy({ ...f.options, mapBytes,
    mapAsset: { ...f.options.mapAsset, bytes: mapBytes.length, sha256: hash(mapBytes) } });
  assert.equal(policy.map.count, 3);
  policy.map.count = 4; // Both counts have a two-level inclusion path.
  assert.throws(() => f.install(policy) && f.check(f.mapped.git(['rev-parse', 'HEAD'])), /authenticated map/);
});

test('merge capture preserves both parent identities and their order', t => {
  const f = fixture(t);
  const otherOriginal = f.old.commit(f.old.empty, [], 'second root\n');
  const otherMapped = f.mapped.commit(f.mapped.empty, [], 'second root\n');
  const original = f.old.commit(f.old.tree('source.txt', 'same full source'), [f.originalBase, otherOriginal], 'merge capture\n');
  const mapped = f.mapped.commit(f.mapped.tree('source.txt', 'same full source'), [f.mappedBase, otherMapped], 'merge capture\n');
  const mapBytes = Buffer.from(`old new\n${f.originalBase} ${f.mappedBase}\n${otherOriginal} ${otherMapped}\n${original} ${mapped}\n`);
  const policy = createCaptureLineagePolicy({ ...f.options, captures: [original], mapBytes,
    mapAsset: { ...f.options.mapAsset, bytes: mapBytes.length, sha256: hash(mapBytes) } });
  f.mapped.git(['reset', '--hard', mapped]);
  const publication = f.install(policy);
  assert.equal(requireCaptureLineage(original, publication, { cwd: f.mapped.cwd }).mappedCaptureSource, mapped);
  policy.captures[0].parents.reverse();
  const invalid = f.install(policy);
  assert.throws(() => requireCaptureLineage(original, invalid, { cwd: f.mapped.cwd }), /parent order/);
});

test('original signature bytes remain in the witness without signing the rewritten commit', t => {
  const f = fixture(t);
  const originalRaw = execFileSync('git', ['cat-file', 'commit', f.original], { cwd: f.old.cwd });
  const signedRaw = Buffer.from(originalRaw.toString('latin1').replace('\n\n',
    '\ngpgsig -----BEGIN PGP SIGNATURE-----\n fixture: no signature validity claim\n -----END PGP SIGNATURE-----\n\n'), 'latin1');
  const signed = f.old.git(['hash-object', '-w', '-t', 'commit', '--stdin'], signedRaw);
  const mapBytes = Buffer.from(f.options.mapBytes.toString().replace(f.original, signed));
  const policy = createCaptureLineagePolicy({ ...f.options, captures: [signed], mapBytes,
    mapAsset: { ...f.options.mapAsset, bytes: mapBytes.length, sha256: hash(mapBytes) } });
  assert.equal(Buffer.from(policy.captures[0].originalCommit, 'base64').equals(signedRaw), true);
  const publication = f.install(policy);
  assert.equal(requireCaptureLineage(signed, publication, { cwd: f.mapped.cwd }).mappedCaptureSource, f.rewritten);
});


test('matching root OID cannot hide a missing nested source tree', t => {
  const f = fixture(t);
  const child = f.old.tree('file.txt', 'nested source');
  const oldTree = f.old.git(['mktree'], `040000 tree ${child}\tsrc\n`);
  const newChild = f.mapped.tree('file.txt', 'nested source');
  const newTree = f.mapped.git(['mktree'], `040000 tree ${newChild}\tsrc\n`);
  const original = f.old.commit(oldTree, [f.originalBase], 'nested capture\n');
  const mapped = f.mapped.commit(newTree, [f.mappedBase], 'nested capture\n');
  const mapBytes = Buffer.from(`old new\n${f.originalBase} ${f.mappedBase}\n${original} ${mapped}\n`);
  const policy = createCaptureLineagePolicy({ ...f.options, captures: [original], mapBytes,
    mapAsset: { ...f.options.mapAsset, bytes: mapBytes.length, sha256: hash(mapBytes) } });
  f.mapped.git(['reset', '--hard', mapped]);
  const publication = f.install(policy);
  rmSync(path.join(f.mapped.cwd, '.git/objects', child.slice(0, 2), child.slice(2)));
  assert.throws(() => requireCaptureLineage(original, publication, { cwd: f.mapped.cwd }), /Git refused ls-tree/);
});
