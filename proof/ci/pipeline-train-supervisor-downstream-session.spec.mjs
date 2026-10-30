import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash, generateKeyPairSync, verify } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fixture } from './helpers/train-fixture.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';
import { observeSupervisorDownstream } from '../../tools/ci/pipeline/train-supervisor-downstream-session.mjs';
import { withTrainAppSession, TrainAppSessionError } from '../../tools/ci/pipeline/train-app-session.mjs';
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
function zip(name, bytes) {
  const script = `import io,sys,zipfile\nb=io.BytesIO()\nwith zipfile.ZipFile(b,'w',compression=zipfile.ZIP_DEFLATED) as z:z.writestr(sys.argv[1],sys.stdin.buffer.read())\nsys.stdout.buffer.write(b.getvalue())`;
  const result = spawnSync('python3', ['-I', '-c', script, name], { input: bytes, timeout: 10000, maxBuffer: 262144 });
  assert.ok(result.status === 0, 'actual finite fixture ZIP creation succeeds'); return result.stdout;
}
async function setup(t) {
  const f = await fixture(t), machine = machinePolicyFixture(f.manifest.repository);
  const directory = await mkdtemp(path.join(os.tmpdir(), 'downstream-session-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const key = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const privateKeyFile = path.join(directory, 'ephemeral-fixture.pem');
  await writeFile(privateKeyFile, key.privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  const repo = { id: 101, full_name: f.manifest.repository };
  const manual = { ...f.observation.run, id: 77, run_attempt: 2, check_suite_id: 44, repository: repo, head_repository: repo };
  const push = { ...manual, id: 100, run_attempt: 1, event: 'push', head_branch: 'dev' };
  const publisher = { ...push, id: 200, run_attempt: 3, event: 'workflow_run', path: '.github/workflows/publish.yml', head_sha: 'd'.repeat(40), head_branch: 'main' };
  const releaseManifest = { schema: 'conduit.pipeline-release/v1', repository: repo.full_name,
    testedSourceSha: f.source, acceptedMainSha: 'e'.repeat(40), treeSha: f.manifest.tree, integrationRun: '100',
    adoptedProof: { runId: 77, runAttempt: 2, acceptanceRunId: 100, acceptanceRunAttempt: 1, manifestDigest: f.manifest.digest } };
  const releaseBytes = Buffer.from(JSON.stringify(releaseManifest));
  const proof = { schema: 'conduit.train-publication-receipt/v1', repository: repo.full_name,
    testedSourceSha: f.source, acceptedMainSha: releaseManifest.acceptedMainSha, treeSha: f.manifest.tree,
    integrationRun: 100, acceptanceRunAttempt: 1, adoptedProof: releaseManifest.adoptedProof,
    publication: { runId: 200, runAttempt: 3, workflow: '.github/workflows/publish.yml' },
    releaseTag: `release-${f.source}`, releaseManifestSha256: sha(releaseBytes).slice(7) };
  const trainZip = zip('train-manifest.json', Buffer.from(JSON.stringify(f.manifest)));
  let receiptZip = zip('publication-receipt.json', Buffer.from(JSON.stringify(proof)));
  const artifact = (id, name, bytes, owner) => ({ id, name, size_in_bytes: bytes.length,
    digest: sha(bytes), expired: false, workflow_run: { id: owner.id, head_sha: owner.head_sha, repository_id: 101, head_repository_id: 101 } });
  const trainArtifact = artifact(456, 'train-manifest', trainZip, manual);
  const publicationArtifact = artifact(500, 'publication-receipt-3', receiptZip, publisher);
  const release = { id: 300, tag_name: `release-${f.source}`, draft: false, prerelease: false,
    published_at: '2026-10-26T12:00:00Z', assets: [{ id: 400, name: 'manifest.json', size: releaseBytes.length, digest: sha(releaseBytes) }] };
  const protection = { required_status_checks: { contexts: ['train-proof'], strict: false,
    checks: [{ context: 'train-proof', app_id: 15368 }] }, required_pull_request_reviews: null,
    enforce_admins: { enabled: true }, allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  const state = { releaseFailure: null, binaryFailure: false, oversizedBinary: false, malformedMetadata: false,
    revokeFailure: false, minted: 0, revoked: 0, python: 0, acquisitions: [], metadata: 0 };
  const binary = endpoint => endpoint.endsWith('/artifacts/456/zip') ? trainZip
    : endpoint.endsWith('/artifacts/500/zip') ? receiptZip : endpoint.endsWith('/assets/400') ? releaseBytes : null;
  const metadata = endpoint => {
    if (endpoint === 'user') return machine.observerUser;
    if (endpoint.includes('/protection')) return protection;
    if (endpoint.includes('/rules/branches/')) return machine.branchRules;
    if (endpoint.includes('/rulesets/')) return machine.ruleset;
    if (endpoint.includes('/workflows/integration.yml/runs?')) return { total_count: 1, workflow_runs: [endpoint.includes('event=push') ? push : manual] };
    if (endpoint.includes('/workflows/publish.yml/runs?')) return { total_count: 1, workflow_runs: [publisher] };
    if (endpoint.endsWith('/actions/runs/100')) return push;
    if (endpoint.endsWith('/actions/runs/77')) return manual;
    if (endpoint.endsWith('/actions/runs/200')) return publisher;
    if (endpoint.includes('/git/ref/heads/')) return { object: { sha: f.source } };
    if (endpoint.includes('/compare/')) return { status: 'identical', merge_base_commit: { sha: f.source } };
    if (endpoint.includes('/check-suites/')) return { total_count: 1, check_runs: [{ name: 'train-accepted', app: { id: 15368 }, check_suite: { id: 44 }, head_sha: f.source, status: 'completed', conclusion: 'success' }] };
    if (endpoint.endsWith('/actions/runs/77/artifacts?per_page=100')) return { total_count: 1, artifacts: [trainArtifact] };
    if (endpoint.endsWith('/actions/runs/200/artifacts?per_page=100')) return { total_count: 1, artifacts: [publicationArtifact] };
    if (endpoint.endsWith('/actions/artifacts/456')) return trainArtifact;
    if (endpoint.endsWith('/actions/artifacts/500')) return publicationArtifact;
    if (endpoint.includes('/releases/tags/')) return release;
    if (endpoint.includes('/git/ref/tags/')) return { object: { type: 'commit', sha: releaseManifest.acceptedMainSha } };
    if (endpoint.includes('/git/commits/')) return { sha: endpoint.endsWith(f.source) ? f.source : releaseManifest.acceptedMainSha, tree: { sha: f.manifest.tree } };
    assert.fail('unexpected fixture metadata endpoint');
  };
  const gitLog = path.join(directory, 'git-child-observations');
  const proxy = path.join(directory, 'bin'); await mkdir(proxy);
  const realGit = spawnSync('which', ['git'], { encoding: 'utf8' }).stdout.trim();
  await writeFile(path.join(proxy, 'git'), `#!${process.execPath}\nconst fs=require('node:fs'),cp=require('node:child_process');fs.appendFileSync(${JSON.stringify(gitLog)},JSON.stringify({clean:!['GH_TOKEN','GITHUB_TOKEN','FIXTURE_OBSERVER'].some(k=>Object.hasOwn(process.env,k))})+'\\n');const r=cp.spawnSync(${JSON.stringify(realGit)},process.argv.slice(2),{env:process.env,stdio:'inherit'});process.exit(r.status??1);`, { mode: 0o700 });
  const poison = path.join(directory, 'python-poison'); await mkdir(poison);
  await writeFile(path.join(poison, 'zipfile.py'), "raise RuntimeError('fixture Python path must be isolated')\n");
  const environment = { PATH: `${proxy}:${process.env.PATH}`, FIXTURE_OBSERVER: 'fixture-owner',
    GH_TOKEN: 'fixture-human', GITHUB_TOKEN: 'fixture-actions', PYTHONPATH: poison, PYTHONSTARTUP: '/fixture/must-not-read' };
  const command = (program, args, options) => {
    assert.ok(!['FIXTURE_OBSERVER', 'GITHUB_TOKEN'].some(name => Object.hasOwn(options.env, name)), 'child excludes other credential roles');
    if (program === 'python3') {
      state.python++; assert.equal(args[0], '-I');
      assert.ok(!Object.hasOwn(options.env, 'GH_TOKEN'), 'real Python has no mutation credential');
      assert.ok(Number.isFinite(options.timeout) && Number.isFinite(options.maxBuffer));
      return spawnSync(program, args, options);
    }
    assert.equal(program, 'gh'); assert.equal(args[0], 'api');
    const endpoint = args[1];
    assert.ok(options.env.GH_TOKEN === (endpoint === 'user' || endpoint.includes('/protection') || endpoint.includes('/rulesets/') || endpoint.includes('/rules/branches/') ? 'fixture-owner' : 'fixture-installation'), 'exact read channel uses its own credential');
    if (endpoint.includes('/releases/tags/') && state.releaseFailure) return state.releaseFailure;
    const bytes = binary(endpoint);
    if (bytes) {
      state.acquisitions.push({ endpoint, limit: options.maxBuffer });
      if (state.binaryFailure) return { status: 1, stderr: 'fixture-installation' };
      return { status: 0, stdout: state.oversizedBinary ? Buffer.alloc(options.maxBuffer + 1) : bytes };
    }
    state.metadata++;
    return { status: 0, stdout: state.malformedMetadata && endpoint.includes('/workflows/integration.yml/runs?') ? 'fixture-installation invalid JSON' : JSON.stringify(metadata(endpoint)) };
  };
  const transport = async (url, options) => {
    const endpoint = new URL(url).pathname + new URL(url).search;
    let data;
    if (endpoint === '/app') {
      const parts = options.headers.Authorization.slice(7).split('.');
      assert.ok(verify('RSA-SHA256', Buffer.from(`${parts[0]}.${parts[1]}`), key.publicKey, Buffer.from(parts[2], 'base64url')));
      data = machine.app;
    } else if (endpoint === '/app/installations/303') data = machine.installation;
    else if (options.method === 'POST') {
      state.minted++; assert.deepEqual(JSON.parse(options.body), { repository_ids: [101], permissions: machine.app.permissions });
      data = { token: 'fixture-installation', permissions: machine.app.permissions,
        expires_at: new Date(Date.now() + 3500000).toISOString(), repository_selection: 'selected' };
    } else if (options.method === 'DELETE') { state.revoked++; return new Response(null, { status: state.revokeFailure ? 500 : 204 }); }
    else if (endpoint.startsWith('/installation/')) data = machine.repositories;
    else data = machine.machineRuleset;
    return new Response(JSON.stringify(data), { status: options.method === 'POST' ? 201 : 200 });
  };
  const options = { queue: { repository: repo.full_name, worktree: f.directory },
    train: { source: f.source, tree: f.manifest.tree, branch: f.manifest.branch, digest: f.manifest.digest },
    config: machine.config, privateKeyFile, observerEnvironmentReference: 'FIXTURE_OBSERVER', environment, command,
    proofBinding: { id: 77, attempt: 2 }, session: settings => withTrainAppSession({ ...settings, transport }) };
  return { state, observe: () => observeSupervisorDownstream(options), gitLog,
    corruptReceipt() { receiptZip = zip('publication-receipt.json', Buffer.from('invalid fixture JSON')); Object.assign(publicationArtifact, artifact(500, publicationArtifact.name, receiptZip, publisher)); } };
}

test('real RSA session composes authenticated adoption, bounded archives and isolated Python with exact downstream completion', async t => {
  const f = await setup(t), result = await f.observe();
  assert.equal(result.kind, 'complete'); assert.equal(result.binding.publicationRunId, 200);
  assert.equal(f.state.minted, 1); assert.equal(f.state.revoked, 1);
  assert.ok(f.state.python >= 2 && f.state.acquisitions.length >= 3);
  assert.ok(f.state.acquisitions.every(item => item.limit <= 1024 * 1024));
  const observations = (await readFile(f.gitLog, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(observations.length > 3 && observations.every(value => value.clean));
  assert.ok(!JSON.stringify(result).includes('fixture-installation'));
});
test('only exact missing release HTTP404 becomes waiting; unknown HTTP errors refuse', async t => {
  const f = await setup(t);
  f.state.releaseFailure = { status: 1, stderr: 'gh: Not Found (HTTP 404)' };
  assert.equal((await f.observe()).reason, 'exact-release-not-observed');
  f.state.releaseFailure = { status: 1, stderr: 'gh: Forbidden (HTTP 403)' };
  assert.equal((await f.observe()).kind, 'refused');
  assert.equal(f.state.minted, 2); assert.equal(f.state.revoked, 2);
});
test('transport failures and lookalike404 strings cannot masquerade as an absent release', async t => {
  const f = await setup(t);
  for (const failure of [
    { status: null, signal: 'SIGTERM', stderr: 'gh: Not Found (HTTP 404)' },
    { status: null, error: Object.assign(new Error('fixture timeout'), { code: 'ETIMEDOUT' }), stderr: 'HTTP 404' },
    { status: 1, stderr: 'HTTP 4040' },
  ]) {
    f.state.releaseFailure = failure;
    assert.equal((await f.observe()).kind, 'refused');
  }
});
test('binary transport, oversize and real decoder corruption refuse without leaking credentials', async t => {
  const f = await setup(t); f.state.binaryFailure = true;
  assert.equal((await f.observe()).kind, 'refused');
  f.state.binaryFailure = false; f.state.oversizedBinary = true;
  assert.equal((await f.observe()).kind, 'refused');
  f.state.oversizedBinary = false; f.corruptReceipt();
  const result = await f.observe(); assert.equal(result.kind, 'refused');
  assert.ok(!JSON.stringify(result).includes('fixture-installation'));
  assert.equal(f.state.minted, 3); assert.equal(f.state.revoked, 3);
});
test('malformed metadata remains refused and actual failed revocation propagates as a session failure', async t => {
  const f = await setup(t); f.state.malformedMetadata = true;
  const result = await f.observe(); assert.equal(result.kind, 'refused');
  assert.ok(!JSON.stringify(result).includes('fixture-installation'));
  f.state.malformedMetadata = false; f.state.revokeFailure = true;
  await assert.rejects(f.observe(), error => error instanceof TrainAppSessionError && error.revocation === 'failed'
    && error.code === 'revocation-failed' && !error.message.includes('fixture-installation'));
});
