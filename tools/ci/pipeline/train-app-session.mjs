// Trusted-controller building block. No CLI wiring, secret output, environment
// mutation, child processes, token persistence, refresh, or mutation retries.
import { createPrivateKey, sign } from 'node:crypto';
import { constants } from 'node:fs';
import { open, realpath } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { TRAIN_APP_PERMISSIONS, validateMachineWriterConfiguration,
  validateMachineWriterObservation } from './train-machine-policy.mjs';
import { validateLandingProtection } from './train-land.mjs';

const ORIGIN = 'https://api.github.com';
const KEY_LIMIT = 32768;
const RESPONSE_LIMIT = 128 * 1024;
const TOKEN_LIMIT = 16384;
const inside = (root, file) => file === root || file.startsWith(`${root}${path.sep}`);
const permissionMatch = value => value && typeof value === 'object' && !Array.isArray(value)
  && Object.keys(value).length === Object.keys(TRAIN_APP_PERMISSIONS).length
  && Object.entries(TRAIN_APP_PERMISSIONS).every(([key, mode]) => value[key] === mode);
const opaqueToken = value => typeof value === 'string' && value.length > 0 && value.length <= TOKEN_LIMIT && /^[\x21-\x7e]+$/.test(value);
const CODES = new Set(['configuration-refused', 'private-key-refused', 'clock-refused', 'observer-refused',
  'app-identity-refused', 'installation-refused', 'policy-refused', 'mint-failed', 'mint-response-refused',
  'token-expiry-refused', 'token-scope-refused', 'callback-failed', 'revocation-failed', 'session-failed']);

export class TrainAppSessionError extends Error {
  constructor(code, revocation = 'not-attempted') {
    code = CODES.has(code) ? code : 'session-failed';
    super(`Train App session refused (${code}).`);
    this.name = 'TrainAppSessionError'; this.code = code;
    this.revocation = ['not-attempted', 'succeeded', 'failed'].includes(revocation) ? revocation : 'not-attempted';
  }
}
const refuse = code => { throw new TrainAppSessionError(code); };

async function readSigningKey(filename, sourceDirectory) {
  let bytes, fd;
  try {
    const controller = await realpath(fileURLToPath(new URL('../../../', import.meta.url)));
    const source = await realpath(sourceDirectory);
    if (typeof process.getuid !== 'function' || typeof filename !== 'string' || !path.isAbsolute(filename)
        || path.resolve(filename) !== filename || await realpath(path.dirname(filename)) !== path.dirname(filename)
        || inside(source, filename) || inside(controller, filename) || filename.split(path.sep).includes('.git')) refuse('private-key-refused');
    fd = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    const before = await fd.stat();
    if (!before.isFile() || before.nlink !== 1 || before.uid !== process.getuid()
        || (before.mode & 0o077) !== 0 || (before.mode & 0o400) === 0
        || before.size < 32 || before.size > KEY_LIMIT) refuse('private-key-refused');
    bytes = Buffer.alloc(before.size + 1); let count = 0;
    while (count < bytes.length) {
      const read = await fd.read(bytes, count, bytes.length - count, null);
      if (!read.bytesRead) break;
      count += read.bytesRead;
    }
    const after = await fd.stat();
    if (count !== before.size || after.size !== before.size || after.mtimeMs !== before.mtimeMs
        || after.ctimeMs !== before.ctimeMs) refuse('private-key-refused');
    const key = createPrivateKey({ key: bytes.subarray(0, count), format: 'pem' });
    const bits = key.asymmetricKeyDetails?.modulusLength;
    if (key.asymmetricKeyType !== 'rsa' || !Number.isSafeInteger(bits) || bits < 2048 || bits > 8192) refuse('private-key-refused');
    return key;
  } catch { refuse('private-key-refused'); }
  finally { bytes?.fill(0); await fd?.close().catch(() => {}); }
}

function milliseconds(clock) {
  let value;
  try { value = clock(); } catch { refuse('clock-refused'); }
  if (!Number.isSafeInteger(value) || value < 60000) refuse('clock-refused');
  return value;
}
function jwtFor(key, appId, time) {
  const now = Math.floor(time / 1000);
  const encode = value => Buffer.from(JSON.stringify(value)).toString('base64url');
  const payload = `${encode({ alg: 'RS256', typ: 'JWT' })}.${encode({ iat: now - 60, exp: now + 540, iss: appId })}`;
  try { return { jwt: `${payload}.${sign('RSA-SHA256', Buffer.from(payload), key).toString('base64url')}`, expires: (now + 540) * 1000 }; }
  catch { refuse('private-key-refused'); }
}

async function responseBody(response) {
  if (!response || !Number.isInteger(response.status) || response.status < 100 || response.status > 599
      || response.redirected || (response.url && new URL(response.url).origin !== ORIGIN)) throw new Error('response');
  const declared = response.headers?.get('content-length');
  if (declared !== null && declared !== undefined && (!/^[0-9]+$/.test(declared) || Number(declared) > RESPONSE_LIMIT)) throw new Error('bound');
  let total = 0; const chunks = [];
  for await (const chunk of response.body ?? []) {
    total += chunk.length;
    if (total > RESPONSE_LIMIT) throw new Error('bound');
    chunks.push(chunk);
  }
  const text = new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks));
  return { status: response.status, data: text.length ? JSON.parse(text) : null };
}

export async function withTrainAppSession({ config, privateKeyFile, policyApi, callback,
  sourceDirectory = process.cwd(), transport = globalThis.fetch, clock = Date.now, timeoutMs = 30000 }) {
  let installationToken, jwt, failure, result, revoke;
  let revocation = 'not-attempted';
  let scopedConfig;
  try {
    try {
      validateMachineWriterConfiguration(config, config?.repository);
      if (config.repository.split('/').some(part => part === '.' || part === '..')
          || typeof policyApi !== 'function' || typeof callback !== 'function' || typeof transport !== 'function'
          || !Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 30000) refuse('configuration-refused');
      scopedConfig = Object.freeze({ ...config });
    } catch { refuse('configuration-refused'); }
    const key = await readSigningKey(privateKeyFile, sourceDirectory);
    const signed = jwtFor(key, scopedConfig.appId, milliseconds(clock)); jwt = signed.jwt;
    const allowed = new Set([`GET /app`, `GET /app/installations/${scopedConfig.installationId}`,
      `POST /app/installations/${scopedConfig.installationId}/access_tokens`, 'GET /installation/repositories?per_page=100',
      `GET /repos/${scopedConfig.repository}/rulesets/${scopedConfig.rulesetId}`, 'DELETE /installation/token']);
    const request = async (method, endpoint, credential, code, body) => {
      if (!allowed.has(`${method} ${endpoint}`)) refuse(code);
      if (credential === jwt && milliseconds(clock) >= signed.expires) refuse(code);
      const controller = new AbortController(); let timer;
      try {
        return await Promise.race([
          (async () => {
            const response = await transport(`${ORIGIN}${endpoint}`, { method, redirect: 'error', signal: controller.signal,
              headers: { Accept: 'application/vnd.github+json', Authorization: `Bearer ${credential}`,
                'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'conduit-train-app-session',
                ...(body ? { 'Content-Type': 'application/json' } : {}) }, ...(body ? { body: JSON.stringify(body) } : {}) });
            if (response.status >= 300 && response.status < 400) throw new Error('redirect');
            return await responseBody(response);
          })(),
          new Promise((_, reject) => { timer = setTimeout(() => { controller.abort(); reject(new Error('timeout')); }, timeoutMs); }),
        ]);
      } catch { refuse(code); }
      finally { clearTimeout(timer); }
    };
    revoke = () => request('DELETE', '/installation/token', installationToken, 'revocation-failed');
    const observe = async endpoint => {
      let timer;
      try {
        return await Promise.race([Promise.resolve().then(() => policyApi(endpoint)),
          new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('observer timeout')), timeoutMs); })]);
      } finally { clearTimeout(timer); }
    };
    const owner = async () => {
      let user;
      try { user = await observe('user'); } catch { refuse('observer-refused'); }
      if (user?.login !== scopedConfig.observerLogin || user.id !== scopedConfig.observerId || user.type !== 'User') refuse('observer-refused');
      return user;
    };
    const observerUser = await owner();
    const appResponse = await request('GET', '/app', jwt, 'app-identity-refused');
    const app = appResponse.data;
    if (appResponse.status !== 200 || app?.id !== scopedConfig.appId || app.slug !== scopedConfig.appSlug
        || app.owner?.id !== scopedConfig.observerId || app.owner?.login !== scopedConfig.observerLogin
        || app.owner?.type !== 'User' || !permissionMatch(app.permissions) || !Array.isArray(app.events) || app.events.length) refuse('app-identity-refused');
    const installResponse = await request('GET', `/app/installations/${scopedConfig.installationId}`, jwt, 'installation-refused');
    const installation = installResponse.data;
    if (installResponse.status !== 200 || installation?.id !== scopedConfig.installationId
        || installation.app_id !== scopedConfig.appId || installation.app_slug !== scopedConfig.appSlug
        || installation.account?.id !== scopedConfig.observerId || installation.account?.login !== scopedConfig.observerLogin
        || installation.account?.type !== 'User' || installation.repository_selection !== 'selected'
        || installation.suspended_at !== null || !permissionMatch(installation.permissions)
        || !Array.isArray(installation.events) || installation.events.length) refuse('installation-refused');
    let ruleset, branchRules, protection;
    try {
      ruleset = await observe(`repos/${scopedConfig.repository}/rulesets/${scopedConfig.rulesetId}`);
      branchRules = await observe(`repos/${scopedConfig.repository}/rules/branches/dev?per_page=100`);
      protection = await observe(`repos/${scopedConfig.repository}/branches/dev/protection`);
      validateLandingProtection(protection, []);
      if (['allow_force_pushes', 'allow_deletions'].some(key => protection[key]?.enabled !== false)) refuse('policy-refused');
    } catch { refuse('policy-refused'); }
    // Capture a usable opaque token immediately. Even a rejected scope/expiry
    // response must reach the one revocation attempt below.
    const minted = await request('POST', `/app/installations/${scopedConfig.installationId}/access_tokens`, jwt, 'mint-failed',
      { repository_ids: [scopedConfig.repositoryId], permissions: { ...TRAIN_APP_PERMISSIONS } });
    if (opaqueToken(minted.data?.token)) installationToken = minted.data.token;
    if (minted.status !== 201 || !installationToken) refuse('mint-response-refused');
    const expiresAt = Date.parse(minted.data.expires_at), now = milliseconds(clock);
    if (!Number.isSafeInteger(expiresAt) || expiresAt <= now || expiresAt > now + 3600000) refuse('token-expiry-refused');
    if (!permissionMatch(minted.data.permissions) || (minted.data.repository_selection !== undefined && minted.data.repository_selection !== 'selected')) refuse('token-scope-refused');
    if (minted.data.repositories !== undefined && (!Array.isArray(minted.data.repositories)
        || minted.data.repositories.length !== 1 || minted.data.repositories[0]?.id !== scopedConfig.repositoryId
        || minted.data.repositories[0]?.full_name !== scopedConfig.repository)) refuse('token-scope-refused');
    const repositories = await request('GET', '/installation/repositories?per_page=100', installationToken, 'token-scope-refused');
    const machineRuleset = await request('GET', `/repos/${scopedConfig.repository}/rulesets/${scopedConfig.rulesetId}`, installationToken, 'token-scope-refused');
    if (repositories.status !== 200 || machineRuleset.status !== 200) refuse('token-scope-refused');
    try { validateMachineWriterObservation({ config: scopedConfig, repository: scopedConfig.repository,
      app, installation, observerUser, repositories: repositories.data, ruleset, branchRules, machineRuleset: machineRuleset.data }); }
    catch { refuse('token-scope-refused'); }
    await owner();
    if (milliseconds(clock) >= expiresAt) refuse('token-expiry-refused');
    if (milliseconds(clock) >= signed.expires) refuse('app-identity-refused');
    try { result = await callback({ jwt, installationToken, expiresAt,
      identity: Object.freeze({ repository: scopedConfig.repository, repositoryId: scopedConfig.repositoryId,
        appId: scopedConfig.appId, appSlug: scopedConfig.appSlug, installationId: scopedConfig.installationId }) }); }
    catch { refuse('callback-failed'); }
    // The callback receives in-memory credentials only; it is separately trusted
    // controller code, not proposed-source code or a sandboxed hostile callback.
  } catch (error) {
    failure = new TrainAppSessionError(error instanceof TrainAppSessionError ? error.code : 'session-failed');
  } finally {
    if (installationToken && revocation === 'not-attempted') {
      try {
        const response = await revoke();
        revocation = response.status === 204 ? 'succeeded' : 'failed';
      } catch { revocation = 'failed'; }
    }
    jwt = undefined; installationToken = undefined;
  }
  if (failure) throw new TrainAppSessionError(failure.code, revocation);
  if (revocation !== 'succeeded') throw new TrainAppSessionError('revocation-failed', revocation);
  return result;
}
