// Selection is read-only and revokes its short session before batch assembly.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { withTrainAppSession } from './train-app-session.mjs';
import { createTrainSessionCommand, createTrainMetadataApi } from './train-controller.mjs';
import { createPolicyObserverApi, validateMachineWriterConfiguration } from './train-machine-policy.mjs';
import { assertSeparateTrainController } from './train-land.mjs';
import { observeCheapTrainPolicy } from './train-cheap-policy.mjs';
import { discoverReadyTrainInputs } from './train-ready-selection.mjs';

export async function selectTrainServiceReadyInputs({ service, config, privateKeyFile,
  observerEnvironmentReference, environment = process.env, command = spawnSync,
  session = withTrainAppSession }) {
  validateMachineWriterConfiguration(config, service.repository);
  if (config.repositoryId !== service.repositoryId) throw new Error('Ready selector repository differs');
  await assertSeparateTrainController(service.sourceObjectStore);
  const directory = fileURLToPath(new URL('../../../', import.meta.url));
  const policyApi = createPolicyObserverApi(observerEnvironmentReference, config, { env: environment, command });
  return session({ config, privateKeyFile, policyApi, sourceDirectory: service.sourceObjectStore,
    callback: ({ jwt, installationToken }) => {
      const api = createTrainMetadataApi(createTrainSessionCommand({ installationToken,
        observerEnvironmentReference, environment, command }), directory);
      const appApi = createTrainMetadataApi(createTrainSessionCommand({ installationToken: jwt,
        observerEnvironmentReference, environment, command }), directory);
      return discoverReadyTrainInputs({ policy: service.policy, repository: service.repository,
        repositoryId: service.repositoryId, api,
        observeCheapPolicy: ({ repository }) => {
          if (repository !== config.repository) throw new Error('Ready selector policy repository differs');
          return observeCheapTrainPolicy({ config, repository, api, appApi, policyApi });
        } });
    } });
}
