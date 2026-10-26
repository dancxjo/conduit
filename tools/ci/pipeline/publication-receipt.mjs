// Run-bound evidence is separate from the stable, resumable release manifest.
import { createHash } from 'node:crypto';
import { mkdir, open, realpath } from 'node:fs/promises';
import path from 'node:path';

const positive = value => Number.isSafeInteger(value) && value > 0;
const fail = () => { throw new Error('Publication run identity refused'); };
export function publicationIdentityFromEnvironment(environment = process.env) {
  if (environment.GITHUB_ACTIONS !== 'true') return null;
  if (environment.GITHUB_EVENT_NAME !== 'workflow_run'
      || !/^[1-9][0-9]*$/.test(environment.GITHUB_RUN_ID ?? '')
      || !/^[1-9][0-9]*$/.test(environment.GITHUB_RUN_ATTEMPT ?? '')) fail();
  const identity = { runId: Number(environment.GITHUB_RUN_ID), runAttempt: Number(environment.GITHUB_RUN_ATTEMPT),
    workflow: '.github/workflows/publish.yml' };
  if (!positive(identity.runId) || !positive(identity.runAttempt)) fail();
  return Object.freeze(identity);
}

export function validatePublicationIdentity(identity, repository, integration, run) {
  if (!identity || Object.keys(identity).sort().join() !== 'runAttempt,runId,workflow'
      || !positive(identity.runId) || !positive(identity.runAttempt)
      || identity.workflow !== '.github/workflows/publish.yml'
      || run?.id !== identity.runId || run.run_attempt !== identity.runAttempt
      || run.path !== identity.workflow || run.event !== 'workflow_run'
      || run.status !== 'in_progress' || run.conclusion !== null
      || !positive(integration?.id) || !positive(integration.run_attempt)
      || !positive(integration.repository?.id)
      || integration.head_repository?.id !== integration.repository.id
      || [run.repository, run.head_repository].some(value => value?.full_name !== repository
        || value.id !== integration.repository.id)) fail();
  return Object.freeze({ ...identity });
}

export async function writePublicationReceipt({ filename, publication, repository, integration, manifest, manifestBytes }) {
  if (!path.isAbsolute(filename) || path.resolve(filename) !== filename
      || !Buffer.isBuffer(manifestBytes) || manifestBytes.length > 1048576
      || JSON.stringify(JSON.parse(manifestBytes)) !== JSON.stringify(manifest)
      || manifest.repository !== repository || manifest.integrationRun !== String(integration.id)) fail();
  await mkdir(path.dirname(filename), { recursive: true });
  if (await realpath(path.dirname(filename)) !== path.dirname(filename)) fail();
  const receipt = { schema: 'conduit.train-publication-receipt/v1', repository,
    testedSourceSha: manifest.testedSourceSha, acceptedMainSha: manifest.acceptedMainSha,
    treeSha: manifest.treeSha, integrationRun: integration.id, acceptanceRunAttempt: integration.run_attempt,
    publication, releaseTag: `release-${manifest.testedSourceSha}`,
    releaseManifestSha256: createHash('sha256').update(manifestBytes).digest('hex'),
    ...(manifest.adoptedProof ? { adoptedProof: manifest.adoptedProof } : {}) };
  const fd = await open(filename, 'wx', 0o600);
  try { await fd.writeFile(`${JSON.stringify(receipt, null, 2)}\n`); await fd.sync(); }
  finally { await fd.close(); }
  return receipt;
}
