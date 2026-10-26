import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { decodePublicationReceiptArchive } from '../../tools/ci/pipeline/train-publication-receipt-decode.mjs';
import { createHash } from 'node:crypto';
import { observeTrainDownstream } from '../../tools/ci/pipeline/train-supervisor-downstream.mjs';

function fixture() {
  const expected = { repository: 'owner/conduit', repositoryId: 11, source: 'a'.repeat(40),
    tree: 'b'.repeat(40), proofRunId: 77, proofRunAttempt: 2, manifestDigest: 'c'.repeat(64) };
  const repo = { full_name: expected.repository, id: 11 };
  const integration = { id: 100, run_attempt: 1, repository: repo, head_repository: repo,
    head_sha: expected.source, head_branch: 'dev', event: 'push', path: '.github/workflows/integration.yml',
    status: 'completed', conclusion: 'success' };
  const publisher = { ...integration, id: 200, run_attempt: 3, head_branch: 'main',
    head_sha: 'd'.repeat(40), event: 'workflow_run', path: '.github/workflows/publish.yml' };
  const manifest = { schema: 'conduit.pipeline-release/v1', repository: expected.repository,
    testedSourceSha: expected.source, treeSha: expected.tree, acceptedMainSha: 'e'.repeat(40),
    integrationRun: '100', adoptedProof: { runId: 77, runAttempt: 2, acceptanceRunId: 100,
      acceptanceRunAttempt: 1, manifestDigest: expected.manifestDigest },
    };
  const adopted = { integrationRun: 77, runAttempt: 2, acceptedSource: expected.source,
    manifestDigest: expected.manifestDigest, manifest: { repository: expected.repository,
      source: expected.source, tree: expected.tree, digest: expected.manifestDigest }, run: { id: 77, run_attempt: 2 } };
  const release = { id: 300, tag_name: `release-${expected.source}`, draft: false, prerelease: false,
    published_at: '2026-10-26T12:00:00Z', assets: [] };
  const state = { integration, publisher, manifest, adopted, release, missingIntegration: false,
    missingRelease: false, partial: false, duplicates: false, newer: false, discoveryCalls: 0,
    reads: new Map(), changeAttempt: false, changeRelease: false };
  const refresh = () => {
    const raw = Buffer.from(JSON.stringify(manifest));
    release.assets = [{ id: 400, name: 'manifest.json', size: raw.length,
      digest: `sha256:${createHash('sha256').update(raw).digest('hex')}` }];
    return raw;
  };
  const proof = { schema: 'conduit.train-publication-receipt/v1', repository: expected.repository,
    testedSourceSha: expected.source, acceptedMainSha: manifest.acceptedMainSha, treeSha: expected.tree,
    integrationRun: 100, acceptanceRunAttempt: 1, adoptedProof: structuredClone(manifest.adoptedProof),
    publication: { runId: 200, runAttempt: 3, workflow: '.github/workflows/publish.yml' },
    releaseTag: release.tag_name, releaseManifestSha256: '' };
  const artifact = { id: 500, name: 'publication-receipt-3', expired: false,
    workflow_run: { id: 200, head_sha: publisher.head_sha, repository_id: 11, head_repository_id: 11 } };
  let archive;
  const refreshReceipt = () => {
    proof.releaseManifestSha256 = release.assets[0]?.digest.slice(7);
    archive = Buffer.from(JSON.stringify(proof));
    artifact.size_in_bytes = archive.length;
    artifact.digest = `sha256:${createHash('sha256').update(archive).digest('hex')}`;
  };
  refresh(); refreshReceipt();
  state.artifact = artifact; state.proof = proof;
  const api = async endpoint => {
    state.reads.set(endpoint, (state.reads.get(endpoint) ?? 0) + 1);
    if (endpoint.includes('/workflows/integration.yml/runs?')) {
      let runs = state.missingIntegration ? [] : [integration];
      if (state.duplicates) runs.push(integration);
      if (state.newer) runs.push({ ...integration, id: 101, status: 'in_progress', conclusion: null });
      return structuredClone({ total_count: runs.length + (state.partial ? 1 : 0), workflow_runs: runs });
    }
    if (endpoint.endsWith('/actions/runs/100') || endpoint.endsWith('/actions/runs/101')) {
      const value = structuredClone(endpoint.endsWith('/101') ? { ...integration, id: 101, status: 'in_progress', conclusion: null } : integration);
      if (state.changeAttempt && state.reads.get(endpoint) > 1) value.run_attempt++;
      return value;
    }
    if (endpoint.includes('/workflows/publish.yml/runs?')) return { total_count: 1, workflow_runs: [structuredClone(publisher)] };
    if (endpoint.endsWith('/actions/runs/200/artifacts?per_page=100')) return { total_count: 1, artifacts: [artifact] };
    if (endpoint.endsWith('/actions/artifacts/500')) return artifact;
    if (endpoint.endsWith('/actions/runs/200')) return structuredClone(publisher);
    if (endpoint.includes('/releases/tags/')) {
      if (state.missingRelease) return null;
      const value = structuredClone(release);
      if (state.changeRelease && state.reads.get(endpoint) > 1) value.assets[0].digest = `sha256:${'f'.repeat(64)}`;
      return value;
    }
    if (endpoint.includes('/git/ref/tags/')) return { object: { type: 'commit', sha: manifest.acceptedMainSha } };
    if (endpoint.includes('/git/commits/')) return { sha: manifest.acceptedMainSha, tree: { sha: expected.tree } };
    throw new Error(`Unexpected read ${endpoint}`);
  };
  const discovery = async () => { state.discoveryCalls++; return structuredClone(adopted); };
  const readManifest = async request => {
    assert.deepEqual(request, { repository: expected.repository, releaseId: 300, assetId: 400, limit: 1024 * 1024 });
    return Buffer.from(JSON.stringify(manifest));
  };
  return { ...state, state, expected, integration, publisher, manifest, release, adopted, api, refresh,
    proof, artifact, refreshReceipt,
    observe: options => observeTrainDownstream({ expected, api, discovery, readManifest,
      readReceiptArchive: async () => archive, decodeReceiptArchive: async raw => raw, ...options }) };
}

test('exact adopted Integration and named publisher complete only authenticated metadata custody', async () => {
  const f = fixture(); const result = await f.observe();
  assert.equal(result.kind, 'complete');
  assert.equal(result.binding.acceptanceRunId, 100);
  assert.equal(result.binding.proofRunAttempt, 2);
  assert.equal(result.binding.publicationRunId, 200);
  assert.equal(result.binding.publicationRunAttempt, 3);
  assert.match(result.binding.proofBoundary, /not independently revalidated/);
});
test('landed without dev-push is waiting; it does not invoke adoption', async () => {
  const f = fixture(); f.state.missingIntegration = true;
  assert.equal((await f.observe()).reason, 'exact-dev-push-not-observed');
  assert.equal(f.state.discoveryCalls, 0);
});
test('pending Integration pins its actual attempt and remains waiting', async () => {
  const f = fixture(); f.integration.status = 'in_progress'; f.integration.conclusion = null;
  const result = await f.observe(); assert.equal(result.kind, 'waiting');
  assert.equal(result.binding.acceptanceRunAttempt, 1); assert.equal(f.state.discoveryCalls, 0);
});
test('newer pending Integration is never replaced by older success', async () => {
  const f = fixture(); f.state.newer = true;
  const result = await f.observe(); assert.equal(result.kind, 'waiting');
  assert.equal(result.binding.acceptanceRunId, 101);
});
test('failed Integration remains an actual failure, not an observation refusal', async () => {
  const f = fixture(); f.integration.conclusion = 'failure';
  assert.equal((await f.observe()).kind, 'failed'); assert.equal(f.state.discoveryCalls, 0);
});
for (const [name, mutate] of [
  ['partial inventory', f => { f.state.partial = true; }],
  ['duplicate inventory', f => { f.state.duplicates = true; }],
  ['wrong repo numeric identity', f => { f.integration.repository = { ...f.integration.repository, id: 12 }; }],
  ['foreign head repository', f => { f.integration.head_repository = { id: 11, full_name: 'fork/conduit' }; }],
  ['wrong Integration event', f => { f.integration.event = 'workflow_dispatch'; }],
  ['wrong Integration workflow', f => { f.integration.path = '.github/workflows/candidate.yml'; }],
  ['wrong source', f => { f.integration.head_sha = '0'.repeat(40); }],
  ['stale acceptance attempt', f => { f.state.changeAttempt = true; }],
  ['no adoption', f => { f.adopted.integrationRun = 78; }],
  ['foreign adopted source', f => { f.adopted.acceptedSource = '0'.repeat(40); }],
  ['foreign adopted tree', f => { f.adopted.manifest.tree = '0'.repeat(40); }],
  ['stale manual attempt', f => { f.adopted.runAttempt++; }],
  ['different manifest digest', f => { f.adopted.manifestDigest = '0'.repeat(64); }],
  ['release wrong source', f => { f.manifest.testedSourceSha = '0'.repeat(40); f.refresh(); }],
  ['release wrong acceptance', f => { f.manifest.adoptedProof.acceptanceRunId++; f.refresh(); }],
  ['release wrong producer', f => { f.manifest.adoptedProof.runId++; f.refresh(); }],
  ['release stale producer attempt', f => { f.manifest.adoptedProof.runAttempt++; f.refresh(); }],
  ['release wrong tree', f => { f.manifest.treeSha = '0'.repeat(40); f.refresh(); }],
  ['manifest changed bytes', f => { f.manifest.integrationRun = '101'; }],
  ['duplicate manifest assets', f => { f.release.assets.push({ ...f.release.assets[0], id: 401 }); }],
  ['draft release', f => { f.release.draft = true; }],
  ['wrong Publish event', f => { f.publisher.event = 'push'; }],
  ['wrong Publish workflow', f => { f.publisher.path = '.github/workflows/candidate.yml'; }],
  ['stale Publish attempt', f => { f.publisher.run_attempt++; f.artifact.name = 'publication-receipt-4'; }],
  ['foreign Publish repository', f => { f.publisher.repository = { id: 11, full_name: 'fork/conduit' }; }],
  ['changed publication asset', f => { f.state.changeRelease = true; }],
]) test(`refuses ${name}`, async () => {
  const f = fixture(); mutate(f); assert.equal((await f.observe()).kind, 'refused');
});
test('missing exact release remains waiting after real adoption', async () => {
  const f = fixture(); f.state.missingRelease = true;
  const result = await f.observe(); assert.equal(result.reason, 'exact-release-not-observed');
  assert.equal(f.state.discoveryCalls, 1);
});
test('legacy release without producer run binding never completes', async () => {
  const f = fixture();
  const result = await f.observe({ readReceiptArchive: undefined }); assert.equal(result.kind, 'waiting');
  assert.equal(result.reason, 'publish-receipt-observer-unavailable');
});
test('published release before Publish job completion remains waiting with pinned attempt', async () => {
  const f = fixture(); f.publisher.status = 'in_progress'; f.publisher.conclusion = null;
  const result = await f.observe(); assert.equal(result.kind, 'waiting');
  assert.equal(result.binding.publicationRunId, 200);
});
test('actual failed Publish cannot be replaced by existing release success', async () => {
  const f = fixture(); f.publisher.conclusion = 'failure';
  const result = await f.observe(); assert.equal(result.kind, 'failed'); assert.equal(result.phase, 'publication');
});
test('pinned acceptance and publication attempts cannot silently advance', async () => {
  const f = fixture();
  assert.equal((await f.observe({ binding: { acceptanceRunId: 100, acceptanceRunAttempt: 2 } })).kind, 'refused');
  assert.equal((await f.observe({ binding: { acceptanceRunId: 100, acceptanceRunAttempt: 1,
    publicationRunId: 200, publicationRunAttempt: 2 } })).kind, 'refused');
});

for (const [name, mutate] of [
  ['foreign artifact owner', f => { f.artifact.workflow_run.id++; }],
  ['foreign artifact head repository', f => { f.artifact.workflow_run.head_repository_id++; }],
  ['expired publication receipt', f => { f.artifact.expired = true; }],
  ['wrong archive digest', f => { f.artifact.digest = `sha256:${'0'.repeat(64)}`; }],
  ['wrong receipt acceptance attempt', f => { f.proof.acceptanceRunAttempt++; f.refreshReceipt(); }],
  ['wrong receipt producer attempt', f => { f.proof.publication.runAttempt++; f.refreshReceipt(); }],
  ['wrong receipt release tag', f => { f.proof.releaseTag = 'release-other'; f.refreshReceipt(); }],
  ['wrong receipt proof identity', f => { f.proof.adoptedProof.manifestDigest = '0'.repeat(64); f.refreshReceipt(); }],
]) test(`publication receipt refuses ${name}`, async () => {
  const f = fixture(); mutate(f); assert.equal((await f.observe()).kind, 'refused');
});

test('known Publish binding observes its exact run outside recent window', async () => {
  const f = fixture(); let inventories = 0;
  const api = endpoint => {
    if (endpoint.includes('/workflows/publish.yml/runs?')) { inventories++; throw new Error('must use pinned run'); }
    return f.api(endpoint);
  };
  const result = await f.observe({ api, binding: { acceptanceRunId: 100, acceptanceRunAttempt: 1,
    publicationRunId: 200, publicationRunAttempt: 3 } });
  assert.equal(result.kind, 'complete'); assert.equal(inventories, 0);
});

test('absence in a bounded window is waiting only while time coverage remains plausible', async () => {
  for (const outside of [false, true]) {
    const f = fixture(); f.integration.created_at = '2026-10-26T10:00:00Z';
    const api = endpoint => {
      if (endpoint.includes('/workflows/publish.yml/runs?')) return { total_count: 9,
        workflow_runs: Array.from({ length: 8 }, (_, i) => ({ ...f.publisher, id: 200 + i,
          created_at: outside ? '2026-10-26T11:00:00Z' : '2026-10-26T09:00:00Z' })) };
      if (/\/actions\/runs\/20[0-7]$/.test(endpoint)) return { ...f.publisher, id: Number(endpoint.split('/').at(-1)) };
      if (/\/actions\/runs\/20[0-7]\/artifacts\?/.test(endpoint)) return { total_count: 0, artifacts: [] };
      return f.api(endpoint);
    };
    const result = await f.observe({ api });
    assert.equal(result.kind, outside ? 'refused' : 'waiting');
  }
});

test('partial Publish window and duplicate receipt inventory refuse', async () => {
  for (const duplicate of [false, true]) {
    const f = fixture();
    const api = endpoint => {
      if (!duplicate && endpoint.includes('/workflows/publish.yml/runs?')) return { total_count: 2, workflow_runs: [f.publisher] };
      if (duplicate && endpoint.endsWith('/actions/runs/200/artifacts?per_page=100')) return {
        total_count: 2, artifacts: [f.artifact, { ...f.artifact, id: 501 }] };
      return f.api(endpoint);
    };
    assert.equal((await f.observe({ api })).kind, 'refused');
  }
});

function makeZip(name, data, extra = false) {
  const script = `import io,json,sys,zipfile
p=json.loads(sys.stdin.read()); b=io.BytesIO()
with zipfile.ZipFile(b,'w',compression=zipfile.ZIP_DEFLATED) as z:
 z.writestr(p['name'],p['data'])
 if p['extra']: z.writestr('extra','x')
sys.stdout.buffer.write(b.getvalue())`;
  const result = spawnSync('python3', ['-c', script], { input: JSON.stringify({ name, data, extra }) });
  assert.equal(result.status, 0); return result.stdout;
}
test('real fixed ZIP decoder composes with authenticated receipt observation', async () => {
  const f = fixture(); const archive = makeZip('publication-receipt.json', JSON.stringify(f.proof));
  f.artifact.size_in_bytes = archive.length;
  f.artifact.digest = `sha256:${createHash('sha256').update(archive).digest('hex')}`;
  const result = await f.observe({ readReceiptArchive: async () => archive,
    decodeReceiptArchive: raw => decodePublicationReceiptArchive(raw, spawnSync) });
  assert.equal(result.kind, 'complete');
});
for (const [name, filename, data, extra] of [
  ['traversal', '../publication-receipt.json', '{}', false],
  ['wrong member', 'manifest.json', '{}', false],
  ['multiple members', 'publication-receipt.json', '{}', true],
  ['oversized expansion', 'publication-receipt.json', 'x'.repeat(65537), false],
]) test(`fixed ZIP decoder refuses ${name}`, () => {
  assert.throws(() => decodePublicationReceiptArchive(makeZip(filename, data, extra), spawnSync), /refused/);
});

test('malformed persisted run IDs cannot select API paths; adapter errors are not disclosed', async () => {
  const f = fixture(); let calls = 0;
  const result = await f.observe({ binding: { acceptanceRunId: 100, acceptanceRunAttempt: 1,
    publicationRunId: '../private', publicationRunAttempt: 1 }, api: () => { calls++; } });
  assert.equal(result.kind, 'refused'); assert.equal(calls, 0);
  const secret = 'private-observer-credential';
  const failure = await f.observe({ api: () => { throw new Error(secret); } });
  assert.equal(failure.kind, 'refused'); assert.equal(JSON.stringify(failure).includes(secret), false);
});


test('older attempt artifacts have no authority; exact current attempt remains unique', async () => {
  const f = fixture();
  const api = endpoint => {
    if (endpoint.endsWith('/actions/runs/200/artifacts?per_page=100')) return { total_count: 2,
      artifacts: [{ ...f.artifact, id: 499, name: 'publication-receipt-2' }, f.artifact] };
    return f.api(endpoint);
  };
  assert.equal((await f.observe({ api })).kind, 'complete');
});
test('receipt acceptance run and attempt remain numeric identities', async () => {
  for (const field of ['integrationRun', 'acceptanceRunAttempt']) {
    const f = fixture(); f.proof[field] = String(f.proof[field]); f.refreshReceipt();
    assert.equal((await f.observe()).kind, 'refused');
  }
});


test('final snapshots revalidate repository, workflow and event rather than only run IDs', async () => {
  for (const target of [100, 200]) {
    const f = fixture(); let reads = 0;
    const api = async endpoint => {
      const value = await f.api(endpoint);
      if (endpoint.endsWith(`/actions/runs/${target}`) && ++reads > 1) value.event = 'pull_request';
      return value;
    };
    assert.equal((await f.observe({ api })).kind, 'refused');
  }
});
