// Compiler acceleration comes from an already accepted producer. This finite
// lookback is cache eligibility, not a claim to have searched all history.
import { spawnSync } from 'node:child_process';
import { discoverTrainAdoption } from './train-adoption.mjs';
import { validateIntegrationRun } from './publish.mjs';
import { hostedCacheSeedName } from './hosted-cache-seed.mjs';
const fail = message => { throw new Error(message); };
const oid = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
const positive = value => Number.isSafeInteger(value) && value > 0;
export function createHostedSeedObserver({ repository, directory = process.cwd(), command = spawnSync }) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '')) fail('Invalid compiler seed repository');
  const api = endpoint => {
    const result = command('gh', ['api', `repos/${repository}/${endpoint}`], {
      cwd: directory, encoding: 'utf8', timeout: 60000, maxBuffer: 2 * 1024 ** 2,
    });
    if (result.error || result.status !== 0) fail('Compiler seed discovery API failed');
    return JSON.parse(result.stdout);
  };
  const accepted = source => {
    const development = api('git/ref/heads/dev').object?.sha;
    if (!oid(source) || !oid(development)) fail('Invalid accepted compiler source');
    const comparison = api(`compare/${source}...${development}`);
    if (!['identical', 'ahead'].includes(comparison.status) || comparison.merge_base_commit?.sha !== source) {
      fail('Compiler seed source is no longer accepted in development');
    }
    return development;
  };
  return { api, accepted };
}

export async function discoverAcceptedHostedSeeds({ repository, namespace,
  directory = process.cwd(), lookback = 8, command = spawnSync,
  discovery = discoverTrainAdoption, firstOnly = false }) {
  hostedCacheSeedName(namespace, 1);
  if (!Number.isSafeInteger(lookback) || lookback < 1 || lookback > 20) fail('Bounded compiler seed lookback required');
  if (typeof firstOnly !== 'boolean') fail('Invalid compiler seed selection mode');
  const { api, accepted } = createHostedSeedObserver({ repository, directory, command });
  const inventory = api(`actions/workflows/integration.yml/runs?branch=dev&event=push&status=success&per_page=${lookback}`);
  const runs = inventory.workflow_runs;
  if (!Number.isSafeInteger(inventory.total_count) || inventory.total_count < 0 ||
      !Array.isArray(runs) || runs.length !== Math.min(lookback, inventory.total_count) ||
      runs.some(run => !positive(run.id)) || new Set(runs.map(run => run.id)).size !== runs.length) {
    fail('Invalid bounded accepted Integration window');
  }
  const result = [], ids = new Set();
  for (const listed of [...runs].sort((a, b) => b.id - a.id)) {
    const acceptanceRun = api(`actions/runs/${listed.id}`);
    validateIntegrationRun(acceptanceRun, repository, listed.head_sha);
    if (acceptanceRun.id !== listed.id || acceptanceRun.run_attempt !== listed.run_attempt ||
        !positive(acceptanceRun.run_attempt) || !positive(acceptanceRun.repository?.id) ||
        acceptanceRun.head_repository?.id !== acceptanceRun.repository.id) fail('Accepted compiler run changed');
    accepted(acceptanceRun.head_sha);
    // Publication's verifier authenticates the exact manual producer and
    // reconstructs its captured merge tree when the dev run adopted proof.
    const adopted = await discovery({ pushRun: acceptanceRun, repository, directory, command });
    const run = adopted?.run ?? acceptanceRun;
    if (adopted && (run.id !== adopted.integrationRun || run.run_attempt !== adopted.runAttempt ||
        run.head_sha !== acceptanceRun.head_sha || adopted.acceptedSource !== acceptanceRun.head_sha)) {
      fail('Compiler seed adopted producer differs');
    }
    const tree = api(`git/commits/${acceptanceRun.head_sha}`).tree?.sha;
    if (!oid(tree) || (adopted && adopted.manifest.tree !== tree)) fail('Compiler seed accepted tree differs');
    const artifacts = api(`actions/runs/${run.id}/artifacts?per_page=100`);
    if (!Number.isSafeInteger(artifacts.total_count) || artifacts.total_count < 0 || artifacts.total_count > 100 ||
        !Array.isArray(artifacts.artifacts) || artifacts.artifacts.length !== artifacts.total_count) fail('Incomplete compiler seed artifact inventory');
    const matches = artifacts.artifacts.filter(artifact => artifact.name === hostedCacheSeedName(namespace, run.run_attempt));
    if (matches.length > 1) fail('Duplicate accepted compiler seed');
    for (const artifact of matches) {
      if (!positive(artifact.id) || artifact.expired !== false) {
        if (artifact.expired === true) continue;
        fail('Invalid accepted compiler seed ID');
      }
      if (ids.has(artifact.id)) continue;
      ids.add(artifact.id);
      result.push({ artifactId: artifact.id,
        accepted: { repository, source: acceptanceRun.head_sha, tree, run, acceptanceRun } });
      if (firstOnly) return result;
    }
  }
  return result;
}
