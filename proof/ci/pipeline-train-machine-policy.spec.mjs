import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm, symlink } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { validateMachineWriterConfiguration, validateMachineWriterObservation,
  observeMachineWriterPolicy, readMachineWriterConfiguration, createAppIdentityApi,
  TRAIN_APP_PERMISSIONS, createPolicyObserverApi, createTrainCredentialApis } from '../../tools/ci/pipeline/train-machine-policy.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';

test('accepts only independent dev update authority for reviewed dedicated App', () => {
  const f = machinePolicyFixture();
  assert.deepEqual(validateMachineWriterObservation(f), { schema: 'conduit.train-machine-writer-observation/v1',
    repository: f.repository, repositoryId: 101, appId: 202, appSlug: 'fixture-integration', installationId: 303, rulesetId: 404,
    observerLogin: 'fixture', observerId: 606 });
});
for (const [name, mutate] of [
  ['activation off', f => { f.config.enabled = false; }],
  ['missing real App ID', f => { f.config.appId = null; }],
  ['GitHub Actions app', f => { f.config.appId = 15368; }],
  ['GitHub Actions slug', f => { f.config.appSlug = 'github-actions'; }],
  ['unreviewed config field', f => { f.config.bypass = true; }],
  ['unpinned observer', f => { f.config.observerId = null; }],
  ['nonowner observer configuration', f => { f.config.observerLogin = 'administrator'; }],
  ['wrong observer identity', f => { f.observerUser.id++; }],
  ['wrong observer login', f => { f.observerUser.login = 'administrator'; }],
  ['nonhuman owner observer', f => { f.observerUser.type = 'Bot'; }],
  ['stale App ID', f => { f.app.id++; }],
  ['stale App slug', f => { f.app.slug = 'other-app'; }],
  ['wrong App owner', f => { f.app.owner.login = 'other-owner'; }],
  ['stale numeric owner identity', f => { f.app.owner.id++; }],
  ['App administration write', f => { f.app.permissions.administration = 'write'; }],
  ['App check forgery permission', f => { f.app.permissions.checks = 'write'; }],
  ['missing workflow update permission', f => { f.app.permissions.workflows = 'read'; }],
  ['unrequested issue write permission', f => { f.app.permissions.issues = 'write'; }],
  ['App webhook subscription', f => { f.app.events.push('push'); }],
  ['stale installation ID', f => { f.installation.id++; }],
  ['installation from another App', f => { f.installation.app_id++; }],
  ['stale installation slug', f => { f.installation.app_slug = 'old-app'; }],
  ['installation to all repositories', f => { f.installation.repository_selection = 'all'; }],
  ['suspended installation', f => { f.installation.suspended_at = '2026-10-25T00:00:00Z'; }],
  ['installation write administration', f => { f.installation.permissions.administration = 'write'; }],
  ['installation webhook subscription', f => { f.installation.events.push('pull_request'); }],
  ['incomplete installation repository inventory', f => { f.repositories.total_count = 2; }],
  ['additional token repository', f => { f.repositories.repositories.push({ ...f.repositories.repositories[0], id: 102 }); f.repositories.total_count++; }],
  ['wrong repository ID', f => { f.repositories.repositories[0].id++; }],
  ['fork installation', f => { f.repositories.repositories[0].fork = true; }],
  ['stale ruleset ID', f => { f.ruleset.id++; }],
  ['unreviewed ruleset name', f => { f.ruleset.name = 'other'; }],
  ['disabled ruleset', f => { f.ruleset.enforcement = 'disabled'; }],
  ['evaluate ruleset', f => { f.ruleset.enforcement = 'evaluate'; }],
  ['tag scope', f => { f.ruleset.target = 'tag'; }],
  ['all branch scope', f => { f.ruleset.conditions.ref_name.include = ['~ALL']; }],
  ['default branch alias', f => { f.ruleset.conditions.ref_name.include = ['~DEFAULT_BRANCH']; }],
  ['broad wildcard scope', f => { f.ruleset.conditions.ref_name.include = ['refs/heads/dev*']; }],
  ['additional branch scope', f => { f.ruleset.conditions.ref_name.include.push('refs/heads/main'); }],
  ['excluded dev', f => { f.ruleset.conditions.ref_name.exclude.push('refs/heads/dev'); }],
  ['unknown condition', f => { f.ruleset.conditions.repository_name = { include: ['*'] }; }],
  ['foreign ruleset source', f => { f.ruleset.source = 'other/repository'; }],
  ['inherited organization ruleset', f => { f.ruleset.source_type = 'Organization'; }],
  ['unknown ruleset gate', f => { f.ruleset.unknown_gate = true; }],
  ['additional rule bypassing proof', f => { f.ruleset.rules.push({ type: 'required_status_checks' }); }],
  ['omitted bypass visibility', f => { delete f.ruleset.bypass_actors; }],
  ['mutation token denied', f => { f.machineRuleset.current_user_can_bypass = 'never'; }],
  ['mutation token bypass omitted', f => { delete f.machineRuleset.current_user_can_bypass; }],
  ['mutation token exempt rather than audited bypass', f => { f.machineRuleset.current_user_can_bypass = 'exempt'; }],
  ['stale mutation token ruleset', f => { f.machineRuleset.id++; }],
  ['upstream fetch-and-merge bypass', f => { f.ruleset.rules[0].parameters.update_allows_fetch_and_merge = true; }],
  ['missing rule parameters', f => { delete f.ruleset.rules[0].parameters; }],
  ['missing applicable update rule', f => { f.branchRules = []; }],
  ['additional applicable rule', f => { f.branchRules.push({ ...f.branchRules[0], ruleset_id: 999 }); }],
  ['stale applicable ruleset', f => { f.branchRules[0].ruleset_id++; }],
  ['foreign applicable source', f => { f.branchRules[0].ruleset_source = 'other/repository'; }],
]) test(`refuses ${name}`, () => {
  const f = machinePolicyFixture(); mutate(f);
  assert.throws(() => validateMachineWriterObservation(f));
});
for (const actor_type of ['OrganizationAdmin', 'RepositoryRole', 'Team', 'User', 'DeployKey']) {
  test(`refuses ${actor_type} actor bypass`, () => {
    const f = machinePolicyFixture(); f.ruleset.bypass_actors[0].actor_type = actor_type;
    assert.throws(() => validateMachineWriterObservation(f));
  });
}
for (const bypass_mode of ['pull_request', 'exempt']) test(`refuses ${bypass_mode} App bypass mode`, () => {
  const f = machinePolicyFixture(); f.ruleset.bypass_actors[0].bypass_mode = bypass_mode;
  assert.throws(() => validateMachineWriterObservation(f));
});
test('refuses extra admin bypass even when reviewed App is present', () => {
  const f = machinePolicyFixture(); f.ruleset.bypass_actors.push({ actor_type: 'OrganizationAdmin', actor_id: null, bypass_mode: 'always' });
  assert.throws(() => validateMachineWriterObservation(f));
});
test('observer keeps JWT, installation token and trusted policy requests on distinct channels', () => {
  const f = machinePolicyFixture(), jwt = [], token = [], policy = [];
  const result = observeMachineWriterPolicy({ config: f.config, repository: f.repository,
    appApi(endpoint) { jwt.push(endpoint); return endpoint === 'app' ? f.app : f.installation; },
    api(endpoint) {
      token.push(endpoint);
      if (endpoint.startsWith('installation/')) return f.repositories;
      if (endpoint.includes('/rulesets/')) return f.machineRuleset;
      throw new Error(`Unexpected installation endpoint ${endpoint}`);
    },
    policyApi(endpoint) {
      policy.push(endpoint);
      if (endpoint === 'user') return f.observerUser;
      if (endpoint.includes('/rulesets/')) return f.ruleset;
      if (endpoint.endsWith('/protection')) return { fixture: true };
      return f.branchRules;
    } });
  assert.equal(result.identity.appId, 202); assert.deepEqual(result.protection, { fixture: true });
  assert.deepEqual(jwt, ['app', 'app/installations/303']);
  assert.deepEqual(token, ['installation/repositories?per_page=100', 'repos/fixture/repository/rulesets/404']);
  assert.deepEqual(policy, ['user', 'repos/fixture/repository/rulesets/404', 'repos/fixture/repository/rules/branches/dev?per_page=100', 'repos/fixture/repository/branches/dev/protection']);
});
test('missing identity channel refuses; no human-token fallback', () => {
  const f = machinePolicyFixture(); let calls = 0;
  assert.throws(() => observeMachineWriterPolicy({ config: f.config, repository: f.repository, api() { calls++; } }), /separate authenticated/);
  assert.equal(calls, 0);
});
test('JWT reference passes secret only to read-only identity command and errors do not expose it', () => {
  const secret = 'fixture-JWT-never-print';
  const api = createAppIdentityApi('CONDUIT_TRAIN_APP_JWT', { env: { CONDUIT_TRAIN_APP_JWT: secret },
    command(program, args, options) {
      assert.equal(program, 'python3'); assert.ok(args[0].endsWith('/train-app-identity.py')); assert.equal(args[1], 'app');
      assert.equal(options.env.GH_TOKEN, secret); return { status: 1, stderr: secret };
    } });
  assert.throws(() => api('app'), error => !error.message.includes(secret) && /observation failed/.test(error.message));
  assert.throws(() => api('app/installations/303/access_tokens'), /unsupported/);
  assert.throws(() => createAppIdentityApi('GH_TOKEN', { env: { GH_TOKEN: secret } }));
});
test('trusted observer credential is explicit and cannot call mutation or unrelated endpoints', () => {
  const f = machinePolicyFixture(), calls = [], secret = 'fixture-owner-observer-secret';
  const api = createPolicyObserverApi('CONDUIT_POLICY_OBSERVER', f.config, {
    env: { CONDUIT_POLICY_OBSERVER: secret, GH_TOKEN: 'different-machine-token' },
    command(program, args, options) {
      assert.equal(program, 'gh'); assert.equal(options.env.GH_TOKEN, secret);
      calls.push(args); return { status: 0, stdout: JSON.stringify(args[1] === 'user' ? f.observerUser : {}) };
    } });
  assert.deepEqual(api('user'), f.observerUser);
  api('repos/fixture/repository/rulesets/404');
  api('repos/fixture/repository/actions/variables/CONDUIT_PR_ADMISSION_TIER');
  assert.throws(() => api('repos/fixture/repository/actions/variables/OTHER_VARIABLE'), /unsupported/);
  assert.throws(() => api('repos/fixture/repository/git/refs/heads/dev'), /unsupported/);
  assert.throws(() => api('repos/fixture/repository/rulesets'), /unsupported/);
  assert.throws(() => api('repos/other/repository/rulesets/404'), /unsupported/);
  assert.throws(() => createPolicyObserverApi('GH_TOKEN', f.config, { env: { GH_TOKEN: secret } }));
  assert.throws(() => createPolicyObserverApi('CONDUIT_POLICY_OBSERVER', f.config, { env: { GH_TOKEN: secret } }));
  assert.ok(calls.every(args => args.length === 2 && args[0] === 'api'));
});
test('JWT and observer named references are distinct and never use mutation-token references', () => {
  const f = machinePolicyFixture(), env = { JWT: 'jwt', OBSERVER: 'observer', GH_TOKEN: 'machine', GITHUB_TOKEN: 'actions' };
  assert.throws(() => createTrainCredentialApis({ machineWriter: f.config, appJwtEnvironmentReference: 'JWT', policyObserverEnvironmentReference: 'JWT', env }), /distinct named/);
  for (const reserved of ['GH_TOKEN', 'GITHUB_TOKEN']) {
    assert.throws(() => createTrainCredentialApis({ machineWriter: f.config, appJwtEnvironmentReference: reserved, policyObserverEnvironmentReference: 'OBSERVER', env }));
    assert.throws(() => createTrainCredentialApis({ machineWriter: f.config, appJwtEnvironmentReference: 'JWT', policyObserverEnvironmentReference: reserved, env }));
  }
});
test('each credential child receives only its own authority and the fixed GitHub host', () => {
  const f = machinePolicyFixture(), seen = [];
  const channels = createTrainCredentialApis({ machineWriter: f.config,
    appJwtEnvironmentReference: 'JWT', policyObserverEnvironmentReference: 'OBSERVER',
    env: { JWT: 'fixture-jwt', OBSERVER: 'fixture-owner', GH_TOKEN: 'fixture-machine',
      GITHUB_TOKEN: 'fixture-actions', GH_HOST: 'untrusted.invalid' },
    command(program, args, options) {
      seen.push({ endpoint: args[1], env: options.env }); return { status: 0, stdout: '{}' };
    } });
  channels.appIdentityApi('app'); channels.policyObserverApi('user');
  assert.ok(seen[0].env.GH_TOKEN === 'fixture-jwt' && seen[1].env.GH_TOKEN === 'fixture-owner');
  assert.ok(seen.every(item => item.env.GH_HOST === 'github.com'
    && ['JWT', 'OBSERVER', 'GITHUB_TOKEN'].every(name => !Object.hasOwn(item.env, name))));
});
for (const mode of ['throw', 'error-result', 'malformed-json']) test(`credential channels sanitize ${mode} errors`, () => {
  const f = machinePolicyFixture(), secret = 'fixture-secret-never-in-error';
  const channels = createTrainCredentialApis({ machineWriter: f.config,
    appJwtEnvironmentReference: 'JWT', policyObserverEnvironmentReference: 'OBSERVER', env: { JWT: secret, OBSERVER: secret },
    command() {
      if (mode === 'throw') throw new Error(secret);
      if (mode === 'error-result') return { status: 1, stderr: secret };
      return { status: 0, stdout: `invalid JSON ${secret}` };
    } });
  for (const [api, endpoint] of [[channels.appIdentityApi, 'app'], [channels.policyObserverApi, 'user']]) {
    assert.throws(() => api(endpoint), error => !error.message.includes(secret) && /observation failed|response JSON invalid/.test(error.message));
  }
});
test('example remains disabled with no invented real identity and manifest has least privilege', async () => {
  const { readFile } = await import('node:fs/promises');
  const config = JSON.parse(await readFile('tools/ci/pipeline/train-machine-writer.example.json'));
  assert.equal(config.enabled, false); assert.equal(config.appId, null); assert.equal(config.installationId, null);
  assert.throws(() => validateMachineWriterConfiguration(config, config.repository), /activation is off/);
  const manifest = JSON.parse(await readFile('tools/ci/pipeline/train-app-manifest.json'));
  assert.deepEqual(manifest.default_permissions, TRAIN_APP_PERMISSIONS);
  assert.equal(manifest.default_permissions.workflows, 'write');
  assert.equal(manifest.default_permissions.administration, 'read');
  assert.equal(manifest.default_permissions.checks, 'read');
  assert.deepEqual(manifest.default_events, []); assert.equal(manifest.hook_attributes.active, false);
  assert.equal(manifest.public, false); assert.equal(manifest.request_oauth_on_install, false);
});
test('configuration reads are external, bounded and do not follow symlinks', async t => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'machine-policy-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = path.join(root, 'source'); await mkdir(source);
  const external = path.join(root, 'config.json'), inside = path.join(source, 'config.json'), linked = path.join(root, 'linked.json');
  const f = machinePolicyFixture();
  await writeFile(external, JSON.stringify(f.config)); await writeFile(inside, JSON.stringify(f.config));
  await symlink(external, linked);
  assert.deepEqual(await readMachineWriterConfiguration(external, source), f.config);
  await assert.rejects(readMachineWriterConfiguration(inside, source));
  await assert.rejects(readMachineWriterConfiguration(linked, source));
  await writeFile(external, 'x'.repeat(65537));
  await assert.rejects(readMachineWriterConfiguration(external, source), /bounded/);
  await writeFile(external, 'secret-not-valid-json');
  await assert.rejects(readMachineWriterConfiguration(external, source), error => /JSON invalid/.test(error.message) && !error.message.includes('secret-not-valid-json'));
});
