// Grants exist only in a trusted controller's memory. Serialized observations,
// repository variables and PR-supplied JSON never authorize cheap train inputs.
import { observeMachineWriterPolicy } from './train-machine-policy.mjs';
import { validateLandingProtection } from './train-land.mjs';
const grants = new WeakMap();
const fail = reason => { throw new Error(`Cheap train policy: ${reason}`); };

export function observeCheapTrainPolicy({ config, repository, api, appApi, policyApi }) {
  const { identity: writer, protection } = observeMachineWriterPolicy({ config, repository, api, appApi, policyApi });
  // Actions write permission does not grant variables read. Activation is
  // observed only through the separately authenticated owner-policy channel.
  const activation = policyApi(`repos/${repository}/actions/variables/CONDUIT_PR_ADMISSION_TIER`);
  if (activation?.name !== 'CONDUIT_PR_ADMISSION_TIER' || activation.value !== 'cheap-input-v1') {
    fail('reviewed repository cheap-input-v1 activation required');
  }
  validateLandingProtection(protection, []);
  if (protection.allow_force_pushes?.enabled !== false || protection.allow_deletions?.enabled !== false) {
    fail('force updates and deletion must remain disabled');
  }
  const grant = Object.freeze({});
  grants.set(grant, { repository, writer, observed: performance.now() });
  return grant;
}

export function requireCheapTrainPolicy(grant, repository) {
  const observation = grant && typeof grant === 'object' ? grants.get(grant) : null;
  if (!observation || observation.repository !== repository || performance.now() - observation.observed > 60_000) {
    fail('fresh external machine-writer and exhaustive-proof policy observation required');
  }
  grants.delete(grant); // One selection only; every refresh must reobserve.
}
