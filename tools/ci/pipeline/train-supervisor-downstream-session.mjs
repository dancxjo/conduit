// Credentials end with this finite observation; Git/Python receive none.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { withTrainAppSession } from './train-app-session.mjs';
import { createPolicyObserverApi } from './train-machine-policy.mjs';
import { createTrainSessionCommand, createTrainMetadataApi, createTrainSourceEnvironment } from './train-controller.mjs';
import { discoverTrainAdoption } from './train-adoption.mjs';
import { observeTrainDownstream } from './train-supervisor-downstream.mjs';
import { decodePublicationReceiptArchive } from './train-publication-receipt-decode.mjs';

export async function observeSupervisorDownstream({ queue, train, config, privateKeyFile,
  observerEnvironmentReference, environment = process.env, command = spawnSync,
  proofBinding, binding, session = withTrainAppSession }) {
  const directory = fileURLToPath(new URL('../../../', import.meta.url));
  const policyApi = createPolicyObserverApi(observerEnvironmentReference, config, { env: environment, command });
  const sourceEnvironment = createTrainSourceEnvironment(environment, [observerEnvironmentReference]);
  return session({ config, privateKeyFile, policyApi, sourceDirectory: queue.worktree,
    callback: async ({ installationToken }) => {
      const missing = new Set();
      const releaseEndpoint = `repos/${queue.repository}/releases/tags/release-${train.source}`;
      const observedCommand = (program, args, options) => {
        const result = command(program, args, options);
        if (program === 'gh' && args[0] === 'api' && args[1] === releaseEndpoint
            && Number.isInteger(result?.status) && result.status > 0 && !result.error && !result.signal
            && /\bHTTP 404\b/.test(result.stderr ?? '')) missing.add(releaseEndpoint);
        return result;
      };
      const child = createTrainSessionCommand({ installationToken, observerEnvironmentReference,
        environment, command: observedCommand });
      const metadata = createTrainMetadataApi(child, directory);
      const api = endpoint => {
        try { return metadata(endpoint); }
        catch (error) { if (endpoint === releaseEndpoint && missing.has(endpoint)) return null; throw error; }
      };
      const sourceCommand = (program, args, options = {}) => {
        if (program === 'gh') return child(program, args, options);
        if (program !== 'python3') throw new Error('Unsupported downstream decoder');
        return command(program, ['-I', ...args], { ...options, cwd: directory, env: sourceEnvironment });
      };
      const bytes = (endpoint, limit, accept) => {
        const result = child('gh', ['api', endpoint, ...(accept ? ['-H', `Accept: ${accept}`] : [])],
          { cwd: directory, encoding: null, timeout: 60000, maxBuffer: limit });
        if (result?.status !== 0 || !Buffer.isBuffer(result.stdout) || result.stdout.length > limit) {
          throw new Error('Downstream archive acquisition refused');
        }
        return result.stdout;
      };
      return observeTrainDownstream({ expected: { repository: queue.repository, repositoryId: config.repositoryId,
        source: train.source, tree: train.tree, proofRunId: proofBinding.id,
        proofRunAttempt: proofBinding.attempt, manifestDigest: train.digest }, binding, api,
      discovery: options => discoverTrainAdoption({ ...options, directory: queue.worktree,
        command: sourceCommand, sourceEnvironment }),
      readManifest: ({ assetId, limit }) => bytes(`repos/${queue.repository}/releases/assets/${assetId}`, limit, 'application/octet-stream'),
      readReceiptArchive: ({ artifactId, limit }) => bytes(`repos/${queue.repository}/actions/artifacts/${artifactId}/zip`, limit),
      decodeReceiptArchive: archive => decodePublicationReceiptArchive(archive, sourceCommand) });
    } });
}
