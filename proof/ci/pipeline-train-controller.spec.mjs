import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, writeFile, symlink, link } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateKeyPairSync, verify, createHash } from 'node:crypto';
import { commands } from './helpers/train-command-fixture.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';
import { runTrainController, createTrainSessionCommand, validateTrainControllerRequest,
  readTrainControllerRequest } from '../../tools/ci/pipeline/train-controller.mjs';
import { withTrainAppSession } from '../../tools/ci/pipeline/train-app-session.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';

const TOKEN = 'fixture-installation-token', JWT = 'fixture-app-jwt';
const protection = { required_status_checks: { contexts: ['train-accepted'], strict: false,
  checks: [{ context: 'train-accepted', app_id: 15368 }] }, required_pull_request_reviews: null,
  enforce_admins: { enabled: true }, allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };

async function fixture(t) {
  const f = await commands(t), machine = machinePolicyFixture('example/conduit');
  const selected = f.selections[0];
  selected.candidateJobs.jobs = [
    { id: 1101, name: 'cheap-input/v1', status: 'completed', conclusion: 'success' },
    { id: 1102, name: 'candidate', status: 'completed', conclusion: 'success' },
    { id: 1103, name: 'proof', status: 'completed', conclusion: 'skipped' },
  ];
  selected.candidateJobs.total_count = 3;
  let policyReads = 0, sessions = 0, activation = 'cheap-input-v1', currentJwt = JWT;
  const policyApi = endpoint => {
    policyReads++;
    if (endpoint === 'user') return machine.observerUser;
    if (endpoint.includes('/variables/')) return { name: 'CONDUIT_PR_ADMISSION_TIER', value: activation };
    if (endpoint.includes('/protection')) return protection;
    if (endpoint.includes('/rulesets/')) return machine.ruleset;
    if (endpoint.includes('/rules/branches/')) return machine.branchRules;
    assert.fail('Unexpected fixture policy endpoint');
  };
  const command = (program, args, options) => {
    assert.ok(!Object.hasOwn(options.env, 'FIXTURE_OBSERVER'), 'observer excluded from source operations');
    if (program === 'git') {
      assert.ok(!Object.hasOwn(options.env, 'GH_TOKEN'), 'Git reads have no installation credential');
      return f.command(program, args, options);
    }
    let data;
    if (args[1] === 'app') { assert.ok(options.env.GH_TOKEN === currentJwt); data = machine.app; }
    else if (args[1] === 'app/installations/303') { assert.ok(options.env.GH_TOKEN === currentJwt); data = machine.installation; }
    else {
      assert.ok(options.env.GH_TOKEN === TOKEN, 'metadata uses scoped App token');
      if (args[1].startsWith('installation/')) data = machine.repositories;
      else if (args[1].includes('/rulesets/')) data = machine.machineRuleset;
      else return f.command(program, args, options);
    }
    return { status: 0, stdout: JSON.stringify(data) };
  };
  // Issuer/real RSA/revocation have their own actual-crypto fixture corpus.
  // This composition fixture executes the real Git/admission controller path.
  const session = async options => {
    sessions++; assert.equal(options.sourceDirectory, f.directory);
    return options.callback({ jwt: JWT, installationToken: TOKEN });
  };
  const request = { schema: 'conduit.train-controller-request/v1', operation: 'capture',
    targetSource: f.directory, prs: [1, 2], source: f.source, manifest: f.filename };
  const options = { request, config: machine.config, privateKeyFile: path.join(f.output, 'unused-fixture.pem'),
    observerEnvironmentReference: 'FIXTURE_OBSERVER', environment: { ...process.env,
      FIXTURE_OBSERVER: 'fixture-owner', GH_TOKEN: 'fixture-human',
    GIT_DIR: '/fixture/untrusted', GIT_CONFIG_COUNT: '1', GIT_CONFIG_KEY_0: 'http.extraHeader',
    GIT_CONFIG_VALUE_0: 'fixture-header', GIT_SSL_NO_VERIFY: '1', GITHUB_TOKEN: 'fixture-workflow' },
    policyApi, command, session };
  return { ...f, options, request, machine, setJwt: value => { currentJwt = value; },
    sessions: () => sessions, policyReads: () => policyReads,
    disable: () => { activation = ''; } };
}

test('trusted session captures real cheap combined Git tree using fresh separate policy channels', async t => {
  const f = await fixture(t);
  const result = await runTrainController(f.options);
  assert.equal(result.source, f.source);
  assert.equal(result.inputs[0].admission.tier, 'cheap-input-v1');
  assert.deepEqual(JSON.parse(await readFile(f.filename)), result);
  assert.equal(f.sessions(), 1);
  assert.ok(f.policyReads() >= 4);
});

test('actual RSA session composes with real cheap Git capture and one final token revocation', async t => {
  const f = await fixture(t), pair = generateKeyPairSync('rsa', { modulusLength: 2048 });
  const privateKeyFile = path.join(f.output, 'ephemeral-fixture.pem');
  await writeFile(privateKeyFile, pair.privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  const now = Date.parse('2026-10-26T00:00:00Z'); let minted = 0, revoked = 0, actualJwt;
  const json = (value, status = 200) => new Response(JSON.stringify(value), { status });
  const transport = async (url, options) => {
    const endpoint = new URL(url).pathname + new URL(url).search;
    if (endpoint === '/app') {
      actualJwt = options.headers.Authorization.slice(7); f.setJwt(actualJwt);
      const parts = actualJwt.split('.');
      assert.ok(verify('RSA-SHA256', Buffer.from(`${parts[0]}.${parts[1]}`), pair.publicKey, Buffer.from(parts[2], 'base64url')));
      return json(f.machine.app);
    }
    if (endpoint === '/app/installations/303') return json(f.machine.installation);
    if (options.method === 'POST') {
      minted++;
      assert.deepEqual(JSON.parse(options.body), { repository_ids: [101], permissions: f.machine.app.permissions });
      return json({ token: TOKEN, permissions: f.machine.app.permissions,
        expires_at: new Date(now + 3600000).toISOString(), repository_selection: 'selected' }, 201);
    }
    assert.ok(options.headers.Authorization === `Bearer ${TOKEN}`);
    if (endpoint === '/installation/repositories?per_page=100') return json(f.machine.repositories);
    if (endpoint === '/repos/example/conduit/rulesets/404') return json(f.machine.machineRuleset);
    if (options.method === 'DELETE' && endpoint === '/installation/token') {
      revoked++; return new Response(null, { status: 204 });
    }
    assert.fail('Unexpected fixture session endpoint');
  };
  const result = await runTrainController({ ...f.options, privateKeyFile,
    session: options => withTrainAppSession({ ...options, transport, clock: () => now }) });
  assert.equal(result.source, f.source); assert.equal(minted, 1); assert.equal(revoked, 1);
  assert.ok(!JSON.stringify(result).includes(TOKEN) && !JSON.stringify(result).includes(actualJwt));
});

test('disabled cheap policy refuses before manifest creation', async t => {
  const f = await fixture(t); f.disable();
  await assert.rejects(runTrainController(f.options), /activation required/);
  await assert.rejects(readFile(f.filename), /ENOENT/);
});

test('controller acquisition uses explicit scoped downloader and authenticates full archive inventory', async t => {
  const f = await fixture(t), manifest = await runTrainController(f.options);
  Object.assign(f.observation.run, { run_attempt: 1, head_branch: manifest.branch });
  f.observation.run.repository.id = 101; f.observation.run.head_repository.id = 101;
  const bytes = Buffer.from('inert-acquisition-fixture');
  const artifacts = TARGETS.map((target, i) => ({ name: `product-${target.id}`, id: 200 + i,
    size_in_bytes: bytes.length, expired: false, digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}`,
    workflow_run: { id: 123, repository_id: 101, head_repository_id: 101, head_sha: f.source } }));
  let factories = 0, downloads = 0;
  const archives = path.join(f.output, 'new-archives');
  const result = await runTrainController({ ...f.options, request: {
    schema: f.request.schema, operation: 'acquire', targetSource: f.directory,
    manifest: f.filename, runId: 123, archives },
  artifactCommandFactory({ env, directory }) {
    factories++; assert.ok(env.GH_TOKEN === TOKEN);
    assert.ok(!Object.hasOwn(env, 'FIXTURE_OBSERVER') && !Object.hasOwn(env, 'GITHUB_TOKEN'));
    assert.equal(directory, f.directory);
    return async (program, args, options) => {
      assert.equal(program, 'gh'); assert.equal(args[0], 'api');
      if (args[1].endsWith('/zip')) { downloads++; await options.output.write(bytes); return { status: 0 }; }
      return { status: 0, stdout: JSON.stringify(args[1].includes('/artifacts?')
        ? { total_count: artifacts.length, artifacts } : f.observation.run) };
    };
  } });
  assert.equal(result.source, f.source); assert.equal(factories, 1); assert.equal(downloads, TARGETS.length);
  assert.deepEqual(JSON.parse(await readFile(path.join(archives, 'acquisition.json'))), result);
});

test('request shape and controller/source separation refuse before session issuance', async t => {
  const f = await fixture(t);
  for (const request of [ { ...f.request, operation: 'toString' }, { ...f.request, prs: [1, 1] },
    { ...f.request, targetSource: 'relative' }, { ...f.request, force: true } ]) {
    await assert.rejects(runTrainController({ ...f.options, request }));
  }
  await assert.rejects(runTrainController({ ...f.options, request: { ...f.request,
    targetSource: path.resolve(fileURLToPath(new URL('../../', import.meta.url))) } }), /separately trusted/);
  assert.equal(f.sessions(), 0);
});

test('bounded request loading refuses links and malformed secret-bearing JSON without echoing it', async t => {
  const f = await fixture(t), filename = path.join(f.output, 'request.json');
  await writeFile(filename, JSON.stringify(f.request));
  assert.deepEqual(await readTrainControllerRequest(filename), validateTrainControllerRequest(f.request));
  const alias = path.join(f.output, 'alias'); await symlink(filename, alias);
  await assert.rejects(readTrainControllerRequest(alias));
  const hardlink = path.join(f.output, 'hardlink'); await link(filename, hardlink);
  await assert.rejects(readTrainControllerRequest(hardlink), /bounded regular/);
  const malformed = path.join(f.output, 'malformed'); await writeFile(malformed, `invalid-${TOKEN}`);
  await assert.rejects(readTrainControllerRequest(malformed), error =>
    error.message.includes('JSON invalid') && !error.message.includes(TOKEN));
});

test('child bridge sends scoped token only to gh and exclusive Git push, with generic failures', () => {
  const environment = { PATH: process.env.PATH, FIXTURE_OBSERVER: 'fixture-owner', GH_TOKEN: 'fixture-human',
    GIT_DIR: '/fixture/untrusted', GIT_CONFIG_COUNT: '1', GIT_CONFIG_KEY_0: 'http.extraHeader',
    GIT_CONFIG_VALUE_0: 'fixture-header', GIT_SSL_NO_VERIFY: '1' };
  const seen = [];
  const invoke = createTrainSessionCommand({ installationToken: TOKEN, observerEnvironmentReference: 'FIXTURE_OBSERVER',
    environment, command(program, args, options) { seen.push({ program, args, env: options.env });
      return { status: 1, stderr: TOKEN, stdout: JWT }; } });
  environment.GH_TOKEN = 'changed-human';
  for (const [program, args] of [['git', ['rev-parse', 'HEAD']], ['git', ['push', '--porcelain']], ['gh', ['api', 'app']]]) {
    const result = invoke(program, args, { env: { FIXTURE_OBSERVER: 'injected-owner', GH_TOKEN: 'injected-human' } });
    assert.ok(!JSON.stringify(result).includes(TOKEN) && !JSON.stringify(result).includes(JWT));
  }
  assert.ok(!Object.hasOwn(seen[0].env, 'GH_TOKEN'));
  for (const item of seen.filter(item => item.program === 'git')) {
    assert.ok(['GIT_DIR', 'GIT_CONFIG_COUNT', 'GIT_CONFIG_KEY_0', 'GIT_CONFIG_VALUE_0',
      'GIT_SSL_NO_VERIFY'].every(name => !Object.hasOwn(item.env, name)));
    assert.equal(item.env.GIT_CONFIG_GLOBAL, '/dev/null');
    assert.equal(item.env.GIT_CONFIG_SYSTEM, '/dev/null');
    assert.equal(item.env.GIT_CONFIG_NOSYSTEM, '1');
    assert.equal(item.env.GIT_NO_REPLACE_OBJECTS, '1');
  }
  assert.ok(seen[1].env.GH_TOKEN === TOKEN && seen[2].env.GH_TOKEN === TOKEN);
  assert.ok(seen.every(item => !Object.hasOwn(item.env, 'FIXTURE_OBSERVER') && !item.args.includes(TOKEN)));
  assert.throws(() => invoke('cargo', ['build']), /unsupported/);
  assert.throws(() => invoke('gh', ['auth', 'token']), /unsupported/);
});
