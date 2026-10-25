// Stage authenticated ordinary or adopted products; publication retains its own verification.
import { spawnSync } from 'node:child_process';
import { lstat, mkdir, realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { discoverTrainAdoption } from './train-adoption.mjs';
import { acquireTrainArtifacts } from './train-artifacts.mjs';
import { restoreTrainArtifacts } from './train-restore.mjs';
import { verifyAcceptedTrainRun } from './train.mjs';
import { validateIntegrationRun } from './publish.mjs';
import { assertSourceCheckout } from './source.mjs';
import { TARGETS } from './targets.mjs';

const fail = reason => { throw new Error(reason); };

export async function stageTrainPublication({ pushRun, repository, directory,
  sourceDirectory = process.cwd(), command = spawnSync, artifactCommand,
  discovery = discoverTrainAdoption, targets = TARGETS }) {
  if (!Number.isSafeInteger(pushRun?.id) || pushRun.id <= 0 || !/^[\w.-]+\/[\w.-]+$/.test(repository ?? '')) fail('Invalid publication source observation');
  const base = `repos/${repository}`;
  const metadata = endpoint => {
    const result = command('gh', ['api', endpoint], { cwd: sourceDirectory, encoding: 'utf8',
      timeout: 60000, maxBuffer: 2 * 1024 ** 2 });
    if (result.error || result.status !== 0) fail('Publication run observation failed');
    return JSON.parse(result.stdout);
  };
  const actualPush = metadata(`${base}/actions/runs/${pushRun.id}`);
  validateIntegrationRun(actualPush, repository, pushRun.head_sha);
  if (actualPush.run_attempt !== pushRun.run_attempt || !Number.isSafeInteger(actualPush.run_attempt)
      || actualPush.run_attempt <= 0) fail('Publication acceptance run attempt changed');
  assertSourceCheckout(actualPush.head_sha, sourceDirectory);
  const adopted = await discovery({ pushRun: actualPush, repository, directory: sourceDirectory, command });
  const treeResult = spawnSync('git', ['rev-parse', `${actualPush.head_sha}^{tree}`],
    { cwd: sourceDirectory, encoding: 'utf8', maxBuffer: 1024 * 1024, timeout: 60000 });
  if (treeResult.error || treeResult.status !== 0 || !/^[a-f0-9]{40}$/.test(treeResult.stdout.trim())) fail('Cannot resolve publication source tree');
  const tree = treeResult.stdout.trim();
  const manifest = adopted?.manifest ?? { source: actualPush.head_sha, tree, digest: null, repository };
  const proofRun = adopted?.run ?? actualPush;
  const validate = adopted
    ? (value, observation) => verifyAcceptedTrainRun(value,
      { ...observation, acceptance: actualPush }, sourceDirectory)
    : (value, { run }) => {
      validateIntegrationRun(run, repository, actualPush.head_sha);
      if (run.id !== actualPush.id || run.run_attempt !== actualPush.run_attempt
          || value.source !== actualPush.head_sha || value.tree !== tree
          || value.digest !== null || value.repository !== repository) fail('Ordinary publication proof identity changed');
      return { repository, source: value.source, tree, integrationRun: run.id };
    };
  const observation = { run: proofRun, acceptance: actualPush };
  const verified = validate(manifest, observation);
  if (verified.source !== actualPush.head_sha || verified.tree !== tree
      || (adopted && (adopted.integrationRun !== proofRun.id
        || adopted.runAttempt !== proofRun.run_attempt || adopted.manifestDigest !== manifest.digest))) fail('Discovered proof differs from accepted publication source');
  if (!path.isAbsolute(directory) || await realpath(path.dirname(directory)) !== path.dirname(directory)) fail('Fresh absolute publication staging directory required');
  try { await lstat(directory); fail('Publication staging already exists; inspect retained evidence'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  await mkdir(directory);
  const acquisition = path.join(directory, 'archives');
  const acquired = await acquireTrainArtifacts({ manifest, observation,
    directory: acquisition, targets, validate, ...(artifactCommand ? { command: artifactCommand } : {}) });
  const restored = await restoreTrainArtifacts({ manifest, observation,
    acquisition, directory: path.join(directory, 'restored'), metadata, targets, validate });
  if (acquired.runId !== proofRun.id || restored.runId !== proofRun.id
      || acquired.runAttempt !== proofRun.run_attempt || restored.runAttempt !== proofRun.run_attempt) fail('Staged product attempt differs');
  const currentPush = metadata(`${base}/actions/runs/${actualPush.id}`);
  validateIntegrationRun(currentPush, repository, actualPush.head_sha);
  if (currentPush.run_attempt !== actualPush.run_attempt) fail('Acceptance run changed during staging');
  const current = await discovery({ pushRun: currentPush, repository, directory: sourceDirectory, command });
  if (Boolean(current) !== Boolean(adopted) || (adopted && (current.integrationRun !== adopted.integrationRun
      || current.runAttempt !== adopted.runAttempt || current.manifestDigest !== adopted.manifestDigest))) fail('Adopted proof changed during staging');
  const result = { schema: 'conduit.train-publication-staging/v1', adopted: Boolean(adopted),
    proofRunId: proofRun.id, runAttempt: proofRun.run_attempt,
    acceptanceRunId: actualPush.id, acceptanceRunAttempt: actualPush.run_attempt,
    source: actualPush.head_sha, tree, manifestDigest: manifest.digest,
    manifest, run: proofRun, bundle: restored.bundle };
  await writeFile(path.join(directory, 'publication-staging.json'), `${JSON.stringify(result, null, 2)}\n`,
    { flag: 'wx', mode: 0o600 });
  return result;
}
