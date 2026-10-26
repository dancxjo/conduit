import { TRAIN_APP_PERMISSIONS } from '../../../tools/ci/pipeline/train-machine-policy.mjs';

// Invented identities are fixture-only, never registration or activation data.
export function machinePolicyFixture(repository = 'fixture/repository') {
  const owner = repository.split('/')[0];
  const config = { schema: 'conduit.train-machine-writer/v1', enabled: true, repository,
    repositoryId: 101, appId: 202, appSlug: 'fixture-integration', installationId: 303,
    rulesetId: 404, rulesetName: 'conduit-dev-machine-writer' };
  const app = { id: 202, slug: config.appSlug, owner: { login: owner, type: 'User' },
    permissions: { ...TRAIN_APP_PERMISSIONS }, events: [] };
  const installation = { id: 303, app_id: 202, app_slug: config.appSlug, account: { login: owner, type: 'User' },
    repository_selection: 'selected', suspended_at: null, permissions: { ...TRAIN_APP_PERMISSIONS }, events: [] };
  const repositories = { total_count: 1, repositories: [{ id: 101, full_name: repository,
    owner: { login: owner, type: 'User' }, fork: false, archived: false }] };
  const rule = { type: 'update', parameters: { update_allows_fetch_and_merge: false } };
  const ruleset = { id: 404, name: config.rulesetName, target: 'branch', source_type: 'Repository', source: repository,
    enforcement: 'active', conditions: { ref_name: { include: ['refs/heads/dev'], exclude: [] } },
    bypass_actors: [{ actor_id: 202, actor_type: 'Integration', bypass_mode: 'always' }], rules: [rule] };
  const branchRules = [{ ...rule, ruleset_source_type: 'Repository', ruleset_source: repository, ruleset_id: 404 }];
  return { config, repository, app, installation, repositories, ruleset, branchRules };
}
