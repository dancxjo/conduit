// Layered writer authority. The app bypasses only the update restriction;
// independent classic protection still requires exact-source Integration proof.
import { spawnSync } from 'node:child_process';
import { constants } from 'node:fs';
import { open, realpath } from 'node:fs/promises';
import path from 'node:path';

const fail = reason => { throw new Error(`Machine writer policy: ${reason}`); };
const positive = value => Number.isSafeInteger(value) && value > 0;
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const keys = (value, expected) => value && typeof value === 'object' && !Array.isArray(value)
  && same(Object.keys(value).sort(), [...expected].sort());
export const TRAIN_APP_PERMISSIONS = Object.freeze({
  actions: 'write', administration: 'read', checks: 'read', contents: 'write', metadata: 'read', pull_requests: 'read',
});
const permissions = value => keys(value, Object.keys(TRAIN_APP_PERMISSIONS))
  && Object.entries(TRAIN_APP_PERMISSIONS).every(([key, mode]) => value[key] === mode);
const update = value => keys(value, ['type', 'parameters']) && value.type === 'update'
  && keys(value.parameters, ['update_allows_fetch_and_merge']) && value.parameters.update_allows_fetch_and_merge === false;

export function validateMachineWriterConfiguration(config, repository) {
  if (!keys(config, ['schema', 'enabled', 'repository', 'repositoryId', 'appId', 'appSlug', 'installationId', 'rulesetId', 'rulesetName'])
      || config.schema !== 'conduit.train-machine-writer/v1' || config.enabled !== true
      || !/^[\w.-]+\/[\w.-]+$/.test(config.repository ?? '') || config.repository !== repository
      || !['repositoryId', 'appId', 'installationId', 'rulesetId'].every(key => positive(config[key]))
      || config.appId === 15368 || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(config.appSlug ?? '')
      || config.appSlug === 'github-actions' || config.rulesetName !== 'conduit-dev-machine-writer') {
    fail('explicit reviewed dedicated App and ruleset identities required; activation is off');
  }
  return config;
}

export function validateMachineWriterObservation({ config, repository, app, installation, repositories, ruleset, branchRules }) {
  validateMachineWriterConfiguration(config, repository);
  const owner = repository.split('/')[0];
  if (app?.id !== config.appId || app.slug !== config.appSlug || app.owner?.login !== owner
      || app.owner?.type !== 'User' || !permissions(app.permissions) || !same(app.events, [])) fail('authenticated App identity or least privilege differs');
  if (installation?.id !== config.installationId || installation.app_id !== config.appId
      || installation.app_slug !== config.appSlug || installation.account?.login !== owner
      || installation.account?.type !== 'User' || installation.repository_selection !== 'selected'
      || installation.suspended_at !== null || !permissions(installation.permissions)
      || !same(installation.events, [])) fail('selected repository installation identity differs');
  if (repositories?.total_count !== 1 || !Array.isArray(repositories.repositories) || repositories.repositories.length !== 1) fail('installation token must expose exactly the selected repository');
  const selected = repositories.repositories[0];
  if (selected?.id !== config.repositoryId || selected.full_name !== repository || selected.owner?.login !== owner
      || selected.owner?.type !== 'User' || selected.fork !== false || selected.archived !== false) fail('installation token repository identity differs');
  const allowed = ['id', 'name', 'target', 'source_type', 'source', 'enforcement', 'bypass_actors', 'conditions', 'rules',
    'node_id', '_links', 'created_at', 'updated_at', 'current_user_can_bypass'];
  if (!ruleset || Object.keys(ruleset).some(key => !allowed.includes(key))
      || ruleset.id !== config.rulesetId || ruleset.name !== config.rulesetName || ruleset.target !== 'branch'
      || ruleset.source_type !== 'Repository' || ruleset.source !== repository || ruleset.enforcement !== 'active'
      || !keys(ruleset.conditions, ['ref_name']) || !keys(ruleset.conditions.ref_name, ['include', 'exclude'])
      || !same(ruleset.conditions.ref_name.include, ['refs/heads/dev']) || !same(ruleset.conditions.ref_name.exclude, [])
      || !Array.isArray(ruleset.bypass_actors) || ruleset.bypass_actors.length !== 1
      || !keys(ruleset.bypass_actors[0], ['actor_id', 'actor_type', 'bypass_mode'])
      || ruleset.bypass_actors[0].actor_id !== config.appId || ruleset.bypass_actors[0].actor_type !== 'Integration'
      || ruleset.bypass_actors[0].bypass_mode !== 'always'
      || !Array.isArray(ruleset.rules) || ruleset.rules.length !== 1 || !update(ruleset.rules[0])) fail('exact active dev-only update ruleset differs');
  if (!Array.isArray(branchRules) || branchRules.length !== 1) fail('unknown or additional applicable branch rules');
  const rule = branchRules[0];
  if (!keys(rule, ['type', 'parameters', 'ruleset_source_type', 'ruleset_source', 'ruleset_id'])
      || !update({ type: rule.type, parameters: rule.parameters }) || rule.ruleset_source_type !== 'Repository'
      || rule.ruleset_source !== repository || rule.ruleset_id !== config.rulesetId) fail('applicable branch rule does not match reviewed writer ruleset');
  return { schema: 'conduit.train-machine-writer-observation/v1', repository, repositoryId: config.repositoryId,
    appId: config.appId, appSlug: config.appSlug, installationId: config.installationId, rulesetId: config.rulesetId };
}

// api uses the installation token, including /installation/repositories. appApi
// uses a separately supplied App JWT. Neither channel mints credentials here.
export function observeMachineWriterPolicy({ config, repository, api, appApi }) {
  validateMachineWriterConfiguration(config, repository);
  if (typeof api !== 'function' || typeof appApi !== 'function') fail('separate authenticated App JWT and installation channels required');
  return validateMachineWriterObservation({ config, repository,
    app: appApi('app'), installation: appApi(`app/installations/${config.installationId}`),
    repositories: api('installation/repositories?per_page=100'),
    ruleset: api(`repos/${repository}/rulesets/${config.rulesetId}`),
    branchRules: api(`repos/${repository}/rules/branches/dev?per_page=100`),
  });
}

export async function readMachineWriterConfiguration(filename, directory = process.cwd()) {
  const source = await realpath(directory);
  if (!path.isAbsolute(filename) || path.resolve(filename) !== filename
      || await realpath(path.dirname(filename)) !== path.dirname(filename)
      || filename === source || filename.startsWith(`${source}${path.sep}`)) fail('external canonical configuration path required');
  const fd = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const stat = await fd.stat();
    if (!stat.isFile() || stat.nlink !== 1 || stat.size < 2 || stat.size > 65536) fail('bounded regular configuration required');
    const bytes = Buffer.alloc(stat.size + 1); let count = 0;
    while (count < bytes.length) {
      const result = await fd.read(bytes, count, bytes.length - count, null);
      if (!result.bytesRead) break;
      count += result.bytesRead;
    }
    if (count !== stat.size) fail('configuration changed while reading');
    return JSON.parse(bytes.subarray(0, count).toString());
  } finally { await fd.close(); }
}

export function createAppIdentityApi(environmentReference, { command = spawnSync, directory = process.cwd(), env = process.env } = {}) {
  if (!/^[A-Z][A-Z0-9_]{0,127}$/.test(environmentReference ?? '')
      || ['GH_TOKEN', 'GITHUB_TOKEN', 'HOME', 'PATH'].includes(environmentReference)
      || typeof env[environmentReference] !== 'string' || !env[environmentReference]) fail('named external App JWT environment reference required');
  return endpoint => {
    if (endpoint !== 'app' && !/^app\/installations\/[1-9][0-9]*$/.test(endpoint)) fail('unsupported App identity endpoint');
    const result = command('gh', ['api', endpoint], { cwd: directory, encoding: 'utf8', timeout: 60000,
      maxBuffer: 2 * 1024 ** 2, env: { ...env, GH_TOKEN: env[environmentReference] } });
    if (result.error || result.status !== 0) fail('App identity observation failed');
    return JSON.parse(result.stdout);
  };
}
