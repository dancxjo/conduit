// Finite authenticated observations, never artifact authority or a writer.
// api/readManifest/discovery are trusted, read-only adapters owned by the
// caller's short credential session. No credential, sleep or retry lives here.
import { createHash } from 'node:crypto';
import { validateIntegrationRun } from './publish.mjs';

const positive = value => Number.isSafeInteger(value) && value > 0;
const sha = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
const hex = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const pending = new Set(['queued', 'requested', 'waiting', 'pending', 'in_progress']);
class DownstreamRefusal extends Error {}
const fail = message => { throw new DownstreamRefusal(message); };
function inventory(value, key) {
  const values = value?.[key];
  if (!Number.isSafeInteger(value?.total_count) || value.total_count < 0 || value.total_count > 100
      || !Array.isArray(values) || values.length !== value.total_count
      || values.some(item => !positive(item?.id)) || new Set(values.map(item => item.id)).size !== values.length) {
    fail(`Incomplete or ambiguous ${key} inventory`);
  }
  return values;
}
function sameRun(a, b) {
  if (a.id !== b.id || a.run_attempt !== b.run_attempt || a.head_sha !== b.head_sha
      || a.status !== b.status || a.conclusion !== b.conclusion) fail('Downstream run changed during observation');
}
function runIdentity(run, expected, event, workflow) {
  if (!positive(run?.id) || !positive(run.run_attempt)
      || run.repository?.full_name !== expected.repository || run.head_repository?.full_name !== expected.repository
      || run.repository?.id !== expected.repositoryId || run.head_repository?.id !== expected.repositoryId
      || run.event !== event || run.path !== `.github/workflows/${workflow}.yml`
      || !sha(run.head_sha) || (run.status !== 'completed' && !pending.has(run.status))) fail('Foreign downstream run identity');
}
function receipt(expected, run) {
  return { repository: expected.repository, repositoryId: expected.repositoryId,
    source: expected.source, tree: expected.tree,
    proofRunId: expected.proofRunId, proofRunAttempt: expected.proofRunAttempt,
    manifestDigest: expected.manifestDigest,
    acceptanceRunId: run.id, acceptanceRunAttempt: run.run_attempt };
}
function validateExpected(expected) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(expected?.repository ?? '') || expected.repository.length > 512
      || !positive(expected.repositoryId) || !sha(expected.source) || !sha(expected.tree)
      || !positive(expected.proofRunId) || !positive(expected.proofRunAttempt) || !hex(expected.manifestDigest)) fail('Invalid landed train identity');
}

/** Metadata completion proves publication custody, not independent product
 * extraction, target revalidation or live Pages availability. Those remain
 * the publisher's owning verifiers. Binding pins prevent adopting a rerun. */
export async function observeTrainDownstream({ expected, binding = null, api, discovery,
  readManifest, readReceiptArchive, decodeReceiptArchive, publicationWindow = 8 }) {
  let phase = 'integration';
  try {
    validateExpected(expected);
    if (binding && (!positive(binding.acceptanceRunId) || !positive(binding.acceptanceRunAttempt)
        || (binding.publicationRunId !== undefined && (!positive(binding.publicationRunId)
          || !positive(binding.publicationRunAttempt))))) fail('Invalid pinned downstream run identity');
    if (typeof api !== 'function' || typeof discovery !== 'function'
        || typeof readManifest !== 'function') fail('Explicit trusted read-only observation adapters required');
    const base = `repos/${expected.repository}`;
    const recheck = async (prior, event, workflow) => {
      const current = await api(`${base}/actions/runs/${prior.id}`);
      runIdentity(current, expected, event, workflow);
      if (workflow === 'integration' && (current.head_sha !== expected.source || current.head_branch !== 'dev')) fail('Acceptance source changed during observation');
      sameRun(prior, current);
    };
    const endpoint = `${base}/actions/workflows/integration.yml/runs?head_sha=${expected.source}&event=push&branch=dev&per_page=100`;
    const select = async () => {
      const runs = inventory(await api(endpoint), 'workflow_runs');
      for (const run of runs) {
        runIdentity(run, expected, 'push', 'integration');
        if (run.head_sha !== expected.source || run.head_branch !== 'dev') fail('Integration inventory differs from exact source');
      }
      return runs.sort((a, b) => b.id - a.id)[0] ?? null;
    };
    const listed = await select();
    if (!listed) {
      if (binding) fail('Pinned acceptance run disappeared');
      return { kind: 'waiting', phase, reason: 'exact-dev-push-not-observed' };
    }
    const run = await api(`${base}/actions/runs/${listed.id}`);
    runIdentity(run, expected, 'push', 'integration'); sameRun(listed, run);
    if (run.head_sha !== expected.source || run.head_branch !== 'dev') fail('Integration source changed');
    const bound = receipt(expected, run);
    if (binding && (binding.acceptanceRunId !== run.id || binding.acceptanceRunAttempt !== run.run_attempt)) fail('Pinned acceptance run or attempt changed');
    if (run.status !== 'completed') return { kind: 'waiting', phase, binding: bound, reason: 'integration-running' };
    if (run.conclusion !== 'success') return { kind: 'failed', phase, binding: bound, reason: 'integration-failed' };
    validateIntegrationRun(run, expected.repository, expected.source);
    const adopted = await discovery({ pushRun: run, repository: expected.repository });
    if (!adopted || adopted.integrationRun !== expected.proofRunId
        || adopted.runAttempt !== expected.proofRunAttempt || adopted.manifestDigest !== expected.manifestDigest
        || adopted.acceptedSource !== expected.source || adopted.manifest?.source !== expected.source
        || adopted.manifest?.tree !== expected.tree || adopted.manifest?.digest !== expected.manifestDigest
        || adopted.manifest?.repository !== expected.repository || adopted.run?.id !== expected.proofRunId
        || adopted.run?.run_attempt !== expected.proofRunAttempt) fail('Integration did not adopt the exact accepted train proof');
    const latest = await select();
    if (!latest) fail('Acceptance run disappeared after adoption');
    sameRun(run, latest);
    await recheck(run, 'push', 'integration');

    phase = 'publication';
    // Exact tag endpoint avoids guessing from a most-recent release window.
    // A missing release must be represented explicitly by the trusted adapter.
    const release = await api(`${base}/releases/tags/release-${expected.source}`);
    if (release === null) return { kind: 'waiting', phase, binding: bound, reason: 'exact-release-not-observed' };
    if (!positive(release?.id) || release.tag_name !== `release-${expected.source}`
        || release.draft !== false || release.prerelease !== false
        || !Number.isFinite(Date.parse(release.published_at)) || !Array.isArray(release.assets)
        || release.assets.length > 100 || new Set(release.assets.map(asset => asset.id)).size !== release.assets.length) fail('Invalid exact published release');
    const assets = release.assets.filter(asset => asset.name === 'manifest.json');
    if (assets.length !== 1) fail('Missing or duplicate publication manifest');
    const asset = assets[0];
    if (!positive(asset.id) || !positive(asset.size) || asset.size > 1024 * 1024
        || !/^sha256:[a-f0-9]{64}$/.test(asset.digest ?? '')) fail('Invalid bounded publication manifest asset');
    const raw = await readManifest({ repository: expected.repository, releaseId: release.id, assetId: asset.id, limit: 1024 * 1024 });
    if (!Buffer.isBuffer(raw) || raw.length !== asset.size
        || `sha256:${createHash('sha256').update(raw).digest('hex')}` !== asset.digest) fail('Publication manifest bytes differ from authenticated asset');
    const manifest = JSON.parse(raw.toString('utf8'));
    const adoptedProof = manifest.adoptedProof;
    if (manifest.schema !== 'conduit.pipeline-release/v1' || manifest.repository !== expected.repository
        || manifest.testedSourceSha !== expected.source || manifest.treeSha !== expected.tree
        || manifest.integrationRun !== String(run.id) || !sha(manifest.acceptedMainSha)
        || adoptedProof?.runId !== expected.proofRunId || adoptedProof.runAttempt !== expected.proofRunAttempt
        || adoptedProof.acceptanceRunId !== run.id || adoptedProof.acceptanceRunAttempt !== run.run_attempt
        || adoptedProof.manifestDigest !== expected.manifestDigest) fail('Published release differs from accepted train custody');
    const tag = await api(`${base}/git/ref/tags/release-${expected.source}`);
    if (tag?.object?.type !== 'commit' || tag.object.sha !== manifest.acceptedMainSha) fail('Publication tag differs from accepted main');
    const commit = await api(`${base}/git/commits/${manifest.acceptedMainSha}`);
    if (commit?.sha !== manifest.acceptedMainSha || commit.tree?.sha !== expected.tree) fail('Published accepted main tree differs');
    const result = { ...bound, releaseId: release.id, manifestAssetId: asset.id,
      publicationManifestDigest: asset.digest, acceptedMainSha: manifest.acceptedMainSha,
      proofBoundary: 'authenticated-release-metadata; products and live Pages not independently revalidated' };
    if (typeof readReceiptArchive !== 'function' || typeof decodeReceiptArchive !== 'function') return {
      kind: 'waiting', phase, binding: bound, publication: result, reason: 'publish-receipt-observer-unavailable' };
    if (!positive(publicationWindow) || publicationWindow > 20) fail('Invalid bounded Publish observation window');
    const publishedRuns = binding?.publicationRunId
      ? { total_count: 1, workflow_runs: [await api(`${base}/actions/runs/${binding.publicationRunId}`)] }
      : await api(`${base}/actions/workflows/publish.yml/runs?event=workflow_run&per_page=${publicationWindow}`);
    const window = publishedRuns?.workflow_runs;
    if (!Number.isSafeInteger(publishedRuns?.total_count) || publishedRuns.total_count < 0
        || !Array.isArray(window) || window.length !== Math.min(publicationWindow, publishedRuns.total_count)
        || window.some(value => !positive(value?.id)) || new Set(window.map(value => value.id)).size !== window.length) fail('Incomplete bounded Publish window');
    const matches = [];
    for (const listedPublish of [...window].sort((a, b) => b.id - a.id)) {
      runIdentity(listedPublish, expected, 'workflow_run', 'publish');
      const publishRun = await api(`${base}/actions/runs/${listedPublish.id}`);
      runIdentity(publishRun, expected, 'workflow_run', 'publish'); sameRun(listedPublish, publishRun);
      const artifacts = inventory(await api(`${base}/actions/runs/${publishRun.id}/artifacts?per_page=100`), 'artifacts');
      const receipts = artifacts.filter(value => value.name === `publication-receipt-${publishRun.run_attempt}`);
      if (receipts.length > 1) fail('Ambiguous publication receipt artifacts');
      if (!receipts.length) continue;
      const artifact = await api(`${base}/actions/artifacts/${receipts[0].id}`);
      if (artifact?.id !== receipts[0].id || artifact.name !== `publication-receipt-${publishRun.run_attempt}` || artifact.expired !== false
          || !positive(artifact.size_in_bytes) || artifact.size_in_bytes > 256 * 1024
          || !/^sha256:[a-f0-9]{64}$/.test(artifact.digest ?? '')
          || artifact.workflow_run?.id !== publishRun.id || artifact.workflow_run.head_sha !== publishRun.head_sha
          || artifact.workflow_run.repository_id !== expected.repositoryId
          || artifact.workflow_run.head_repository_id !== expected.repositoryId) fail('Invalid run-bound publication receipt artifact');
      const archive = await readReceiptArchive({ repository: expected.repository, artifactId: artifact.id, limit: 256 * 1024 });
      if (!Buffer.isBuffer(archive) || archive.length !== artifact.size_in_bytes
          || `sha256:${createHash('sha256').update(archive).digest('hex')}` !== artifact.digest) fail('Publication receipt archive differs');
      const decoded = await decodeReceiptArchive(archive);
      if (!Buffer.isBuffer(decoded) || !decoded.length || decoded.length > 65536) fail('Invalid bounded publication receipt JSON');
      const proof = JSON.parse(decoded.toString('utf8'));
      if (proof.schema !== 'conduit.train-publication-receipt/v1' || proof.repository !== expected.repository
          || proof.publication?.runId !== publishRun.id || proof.publication.runAttempt !== publishRun.run_attempt
          || proof.publication.workflow !== '.github/workflows/publish.yml') fail('Publication receipt producer differs');
      // An authenticated receipt for another source is not evidence for this train.
      if (proof.testedSourceSha !== expected.source) continue;
      const adoptedProof = proof.adoptedProof;
      if (proof.acceptedMainSha !== manifest.acceptedMainSha || proof.treeSha !== expected.tree
          || proof.integrationRun !== run.id || proof.acceptanceRunAttempt !== run.run_attempt
          || proof.releaseTag !== release.tag_name || proof.releaseManifestSha256 !== asset.digest.slice(7)
          || adoptedProof?.runId !== expected.proofRunId || adoptedProof.runAttempt !== expected.proofRunAttempt
          || adoptedProof.acceptanceRunId !== run.id || adoptedProof.acceptanceRunAttempt !== run.run_attempt
          || adoptedProof.manifestDigest !== expected.manifestDigest) fail('Publication receipt differs from accepted release custody');
      matches.push({ publishRun, artifact });
    }
    if (!matches.length && binding?.publicationRunId) fail('Pinned publication receipt disappeared');
    if (!matches.length && publishedRuns.total_count > publicationWindow) {
      const oldest = Math.min(...window.map(value => Date.parse(value.created_at)));
      const integrationCreated = Date.parse(run.created_at);
      if (!Number.isFinite(oldest) || !Number.isFinite(integrationCreated)) fail('Unknown bounded publication time coverage; inspect retained events');
      if (oldest > integrationCreated) fail('Exact publication receipt outside bounded window; inspect retained events');
    }
    if (!matches.length) return { kind: 'waiting', phase, binding: bound, publication: result,
      reason: 'exact-publish-receipt-not-in-bounded-window', publicationWindow };
    const { publishRun, artifact } = matches[0];
    if (binding?.publicationRunId && (binding.publicationRunId !== publishRun.id
        || binding.publicationRunAttempt !== publishRun.run_attempt)) fail('Pinned Publish run or attempt changed');
    const completeBinding = { ...result, publicationRunId: publishRun.id,
      publicationRunAttempt: publishRun.run_attempt, publicationReceiptArtifactId: artifact.id,
      publicationReceiptArchiveDigest: artifact.digest };
    if (publishRun.status !== 'completed') return { kind: 'waiting', phase, binding: completeBinding, reason: 'publish-running' };
    if (publishRun.conclusion !== 'success') return { kind: 'failed', phase, binding: completeBinding, reason: 'publish-failed' };
    await recheck(publishRun, 'workflow_run', 'publish');
    await recheck(run, 'push', 'integration');
    const currentRelease = await api(`${base}/releases/tags/release-${expected.source}`);
    const currentAsset = currentRelease?.assets?.filter(value => value.name === 'manifest.json');
    if (currentRelease?.id !== release.id || currentRelease.draft !== false || currentRelease.prerelease !== false
        || currentAsset?.length !== 1 || currentAsset[0].id !== asset.id
        || currentAsset[0].digest !== asset.digest || currentAsset[0].size !== asset.size) fail('Published release changed during observation');
    return { kind: 'complete', phase, binding: completeBinding };
  } catch (error) {
    return { kind: 'refused', phase, reason: error instanceof DownstreamRefusal ? error.message : 'downstream-adapter-or-decode-refused' };
  }
}
