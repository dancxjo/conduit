import test from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, verify } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, rm, chmod, symlink, link } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { withTrainAppSession, TrainAppSessionError } from '../../tools/ci/pipeline/train-app-session.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';

// Actual ephemeral RSA fixtures. Never search for or load host credentials.
const pair = generateKeyPairSync('rsa', { modulusLength: 2048 });
const pem = pair.privateKey.export({ type: 'pkcs8', format: 'pem' });
const START = Date.parse('2026-10-26T10:00:00Z');
const TOKEN = `ghs_202_${'x'.repeat(700)}.opaque-stateless-format`;
const json = (data, status = 200) => new Response(JSON.stringify(data), { status, headers: { 'content-type': 'application/json' } });

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'train-session-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const sourceDirectory = path.join(root, 'source'); await mkdir(sourceDirectory);
  const privateKeyFile = path.join(root, 'key.pem'); await writeFile(privateKeyFile, pem, { mode: 0o600 });
  const f = machinePolicyFixture();
  const protection = { required_status_checks: { contexts: ['train-proof'], checks: [{ context: 'train-proof', app_id: 15368 }], strict: false },
    required_pull_request_reviews: null, enforce_admins: { enabled: true }, allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  const state = { now: START, calls: [], ownerCalls: 0, callbacks: 0, revocations: 0,
    tokenRequests: 0, callbackFailure: false, revokeFailure: false, scopeDelay: false,
    malformed: null, oversize: null, redirect: null, stall: null,
    minted: { token: TOKEN, expires_at: new Date(START + 3600000).toISOString(),
      permissions: { ...f.app.permissions }, repository_selection: 'selected', repositories: [{ id: 101, full_name: f.repository }] } };
  const policyApi = async endpoint => {
    if (endpoint === 'user') { state.ownerCalls++; return f.observerUser; }
    if (endpoint.endsWith('/rulesets/404')) return f.ruleset;
    if (endpoint.endsWith('/rules/branches/dev?per_page=100')) return f.branchRules;
    if (endpoint.endsWith('/branches/dev/protection')) return protection;
    throw new Error('Unexpected policy endpoint');
  };
  const transport = async (url, options) => {
    const parsed = new URL(url), endpoint = parsed.pathname + parsed.search;
    assert.ok(parsed.origin === 'https://api.github.com', 'fixed API origin');
    assert.ok(options.redirect === 'error', 'redirects must refuse');
    assert.ok(options.signal instanceof AbortSignal, 'bounded request signal');
    assert.ok(options.headers.Authorization.startsWith('Bearer '), 'bearer credential required');
    state.calls.push({ method: options.method, endpoint });
    if (options.method === 'DELETE') state.revocations++;
    if (options.method === 'POST') state.tokenRequests++;
    if (state.stall === endpoint) return await new Promise(() => {});
    if (state.malformed === endpoint) return new Response(`invalid-json-${TOKEN}`, { status: 200 });
    if (state.oversize === endpoint) return new Response('x'.repeat(128 * 1024 + 1), { status: 200 });
    if (state.redirect === endpoint) return new Response(null, { status: 302, headers: { location: 'https://untrusted.invalid/' } });
    if (endpoint === '/app') {
      const jwt = options.headers.Authorization.slice(7), parts = jwt.split('.');
      assert.ok(parts.length === 3, 'three JWT components');
      const header = JSON.parse(Buffer.from(parts[0], 'base64url'));
      const claims = JSON.parse(Buffer.from(parts[1], 'base64url'));
      assert.deepEqual(header, { alg: 'RS256', typ: 'JWT' });
      assert.equal(claims.iss, f.config.appId);
      assert.equal(claims.iat, START / 1000 - 60); assert.equal(claims.exp, START / 1000 + 540);
      assert.ok(verify('RSA-SHA256', Buffer.from(`${parts[0]}.${parts[1]}`), pair.publicKey, Buffer.from(parts[2], 'base64url')), 'actual RSA signature verifies');
      return json(f.app);
    }
    if (endpoint === '/app/installations/303') return json(f.installation);
    if (options.method === 'POST' && endpoint === '/app/installations/303/access_tokens') {
      const body = JSON.parse(options.body);
      assert.deepEqual(body, { repository_ids: [101], permissions: f.app.permissions });
      return json(state.minted, state.mintStatus ?? 201);
    }
    assert.ok(options.headers.Authorization === `Bearer ${TOKEN}`, 'only scoped installation credential used');
    if (endpoint === '/installation/repositories?per_page=100') return json(f.repositories);
    if (endpoint === '/repos/fixture/repository/rulesets/404') {
      if (state.scopeDelay) state.now += 3600001;
      return json(f.machineRuleset);
    }
    if (options.method === 'DELETE' && endpoint === '/installation/token') {
      if (state.revokeFailure) throw new Error(`transport-secret-${TOKEN}`);
      return new Response(null, { status: 204 });
    }
    throw new Error('Unexpected transport endpoint');
  };
  const options = { config: f.config, privateKeyFile, policyApi, sourceDirectory,
    transport, clock: () => state.now, timeoutMs: 1000,
    async callback(credentials) {
      state.callbacks++;
      assert.ok(credentials.installationToken === TOKEN && credentials.installationToken.length > 40, 'opaque long token retained only in memory');
      assert.equal(credentials.identity.appId, 202);
      if (state.callbackFailure) throw new Error(`callback-secret-${TOKEN}`);
      return { completed: true };
    } };
  const run = () => withTrainAppSession(options);
  return { ...f, root, privateKeyFile, options, state, protection, run };
}

function safeError(code, revocation) {
  return error => {
    assert.ok(error instanceof TrainAppSessionError, 'typed public session error');
    assert.equal(error.code, code); assert.equal(error.revocation, revocation);
    assert.ok(error.message.includes(`revocation=${revocation}`), 'CLI-visible error retains cleanup outcome');
    assert.ok(!String(error).includes(TOKEN) && !JSON.stringify(error).includes(TOKEN), 'error excludes credential');
    assert.ok(!String(error).includes('PRIVATE KEY'), 'error excludes key material');
    return true;
  };
}

test('real RSA JWT signs exact claims, scoped mint runs callback, finally revokes once', async t => {
  const f = await fixture(t), beforeEnvironment = { ...process.env }, beforeKey = await readFile(f.privateKeyFile);
  assert.deepEqual(await f.run(), { completed: true });
  assert.equal(f.state.tokenRequests, 1); assert.equal(f.state.callbacks, 1); assert.equal(f.state.revocations, 1);
  assert.equal(f.state.ownerCalls, 2);
  assert.equal(f.state.calls.at(-1).method, 'DELETE');
  assert.ok(Object.keys(process.env).length === Object.keys(beforeEnvironment).length
    && Object.entries(beforeEnvironment).every(([key, value]) => process.env[key] === value), 'environment remains unchanged');
  assert.ok((await readFile(f.privateKeyFile)).equals(beforeKey), 'input key is never rewritten');
});
test('optional mint repository fields still require actual installation scope verification', async t => {
  const f = await fixture(t); delete f.state.minted.repositories; delete f.state.minted.repository_selection;
  assert.deepEqual(await f.run(), { completed: true });
  assert.ok(f.state.calls.some(call => call.endpoint === '/installation/repositories?per_page=100'));
});
for (const variant of ['permissions', 'repositories', 'selection', 'expired', 'too-long', 'malformed-expiry', 'actual-scope', 'machine-bypass', 'expired-before-callback']) {
  test(`minted token ${variant} refuses callback and still revokes`, async t => {
    const f = await fixture(t);
    if (variant === 'permissions') f.state.minted.permissions.administration = 'write';
    if (variant === 'repositories') f.state.minted.repositories.push({ id: 999, full_name: 'other/repository' });
    if (variant === 'selection') f.state.minted.repository_selection = 'all';
    if (variant === 'expired') f.state.minted.expires_at = new Date(START - 1).toISOString();
    if (variant === 'too-long') f.state.minted.expires_at = new Date(START + 3600001).toISOString();
    if (variant === 'malformed-expiry') f.state.minted.expires_at = TOKEN;
    if (variant === 'actual-scope') { f.repositories.repositories.push({ ...f.repositories.repositories[0], id: 999 }); f.repositories.total_count++; }
    if (variant === 'machine-bypass') f.machineRuleset.current_user_can_bypass = 'never';
    if (variant === 'expired-before-callback') f.state.scopeDelay = true;
    const code = ['expired', 'too-long', 'malformed-expiry', 'expired-before-callback'].includes(variant) ? 'token-expiry-refused' : 'token-scope-refused';
    await assert.rejects(f.run(), safeError(code, 'succeeded'));
    assert.equal(f.state.callbacks, 0); assert.equal(f.state.revocations, 1); assert.equal(f.state.tokenRequests, 1);
  });
}
test('unexpected mint status with usable token still triggers revocation', async t => {
  const f = await fixture(t); f.state.mintStatus = 500;
  await assert.rejects(f.run(), safeError('mint-response-refused', 'succeeded'));
  assert.equal(f.state.revocations, 1); assert.equal(f.state.callbacks, 0);
});
for (const [callbackFailure, revokeFailure, code] of [[true, false, 'callback-failed'], [false, true, 'revocation-failed'], [true, true, 'callback-failed']]) {
  test(`callbackFailure=${callbackFailure} revokeFailure=${revokeFailure} preserves distinct outcome`, async t => {
    const f = await fixture(t); Object.assign(f.state, { callbackFailure, revokeFailure });
    await assert.rejects(f.run(), safeError(code, revokeFailure ? 'failed' : 'succeeded'));
    assert.equal(f.state.revocations, 1); assert.equal(f.state.callbacks, 1);
  });
}
for (const variant of ['permissions', 'symlink', 'hardlink', 'inside-source', 'oversized', 'invalid', 'non-rsa']) {
  test(`private key ${variant} refuses before any transport`, async t => {
    const f = await fixture(t);
    if (variant === 'permissions') await chmod(f.privateKeyFile, 0o644);
    if (variant === 'symlink') { const alias = path.join(f.root, 'alias.pem'); await symlink(f.privateKeyFile, alias); f.options.privateKeyFile = alias; }
    if (variant === 'hardlink') await link(f.privateKeyFile, path.join(f.root, 'other.pem'));
    if (variant === 'inside-source') { f.options.privateKeyFile = path.join(f.options.sourceDirectory, 'key.pem'); await writeFile(f.options.privateKeyFile, pem, { mode: 0o600 }); }
    if (variant === 'oversized') await writeFile(f.privateKeyFile, Buffer.alloc(32769));
    if (variant === 'invalid') await writeFile(f.privateKeyFile, 'invalid-private-key-never-echo');
    if (variant === 'non-rsa') await writeFile(f.privateKeyFile, generateKeyPairSync('ec', { namedCurve: 'prime256v1' }).privateKey.export({ type: 'pkcs8', format: 'pem' }));
    await assert.rejects(f.run(), safeError('private-key-refused', 'not-attempted'));
    assert.equal(f.state.calls.length, 0); assert.equal(f.state.callbacks, 0);
  });
}
for (const variant of ['owner', 'app', 'installation', 'policy']) test(`${variant} identity refusal cannot mint`, async t => {
  const f = await fixture(t);
  if (variant === 'owner') f.observerUser.id++;
  if (variant === 'app') f.app.id++;
  if (variant === 'installation') f.installation.app_id++;
  if (variant === 'policy') f.protection.enforce_admins.enabled = false;
  const code = { owner: 'observer-refused', app: 'app-identity-refused', installation: 'installation-refused', policy: 'policy-refused' }[variant];
  await assert.rejects(f.run(), safeError(code, 'not-attempted'));
  assert.equal(f.state.tokenRequests, 0); assert.equal(f.state.callbacks, 0);
});
for (const variant of ['malformed', 'oversize', 'redirect', 'stall']) test(`${variant} API response refuses without secret detail or retry`, async t => {
  const f = await fixture(t); f.state[variant] = '/app'; f.options.timeoutMs = 10;
  await assert.rejects(f.run(), safeError('app-identity-refused', 'not-attempted'));
  assert.equal(f.state.tokenRequests, 0); assert.equal(f.state.calls.length, 1);
});
test('cleanup timeout is reported without retry after callback failure', async t => {
  const f = await fixture(t); f.options.timeoutMs = 10; f.state.stall = '/installation/token'; f.state.callbackFailure = true;
  await assert.rejects(f.run(), safeError('callback-failed', 'failed'));
  assert.equal(f.state.revocations, 1);
});
test('post-mint trusted observer timeout still revokes the scoped token', async t => {
  const f = await fixture(t), original = f.options.policyApi; let owners = 0;
  f.options.timeoutMs = 10;
  f.options.policyApi = endpoint => endpoint === 'user' && ++owners > 1 ? new Promise(() => {}) : original(endpoint);
  await assert.rejects(f.run(), safeError('observer-refused', 'succeeded'));
  assert.equal(f.state.callbacks, 0); assert.equal(f.state.revocations, 1);
});
test('secret-bearing malformed response never appears in errors or starts callback', async t => {
  const f = await fixture(t); f.state.malformed = '/app/installations/303/access_tokens';
  await assert.rejects(f.run(), safeError('mint-failed', 'not-attempted'));
  assert.equal(f.state.callbacks, 0); assert.equal(f.state.tokenRequests, 1);
  assert.equal(f.state.revocations, 0); // No trustworthy token can be parsed.
});
test('disabled setup and invalid clock refuse before minting', async t => {
  const f = await fixture(t); f.config.enabled = false;
  await assert.rejects(f.run(), safeError('configuration-refused', 'not-attempted'));
  assert.equal(f.state.calls.length, 0);
  f.config.enabled = true; f.options.clock = () => NaN;
  await assert.rejects(f.run(), safeError('clock-refused', 'not-attempted'));
  assert.equal(f.state.calls.length, 0);
});
