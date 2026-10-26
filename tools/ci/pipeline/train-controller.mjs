// Trusted orchestration only. Proposed source is Git/data, never imported code.
import { spawnSync } from 'node:child_process';
import { constants } from 'node:fs';
import { open } from 'node:fs/promises';
import path from 'node:path';
import { withTrainAppSession } from './train-app-session.mjs';
import { createTrainCommands } from './train-command.mjs';
import { createTrainDispatcher } from './train-dispatch.mjs';
import { createTrainLander, assertSeparateTrainController } from './train-land.mjs';
import { observeCheapTrainPolicy } from './train-cheap-policy.mjs';
import { createPolicyObserverApi, validateMachineWriterConfiguration } from './train-machine-policy.mjs';

const fail = reason => { throw new Error(`Train controller: ${reason}`); };
const shape = (value, fields) => value && typeof value === 'object' && !Array.isArray(value)
  && Object.keys(value).sort().join() === [...fields].sort().join();
const absolute = value => typeof value === 'string' && value.length <= 4096 && !value.includes('\0')
  && path.isAbsolute(value) && path.resolve(value) === value;
const positive = value => Number.isSafeInteger(value) && value > 0;
const fields = {
  assemble: ['prs', 'worktree', 'manifest'], capture: ['prs', 'source', 'manifest'],
  dispatch: ['manifest'], acquire: ['manifest', 'runId', 'archives'],
  land: ['manifest', 'runId', 'archives', 'evidence'],
};

export function validateTrainControllerRequest(value) {
  const extra = Object.hasOwn(fields, value?.operation) ? fields[value.operation] : null;
  if (!extra || !shape(value, ['schema', 'operation', 'targetSource', ...extra])
      || value.schema !== 'conduit.train-controller-request/v1'
      || !absolute(value.targetSource) || !absolute(value.manifest)
      || ['worktree', 'archives', 'evidence'].some(key => key in value && !absolute(value[key]))
      || ('runId' in value && !positive(value.runId))
      || ('source' in value && !/^[a-f0-9]{40}$/.test(value.source ?? ''))
      || ('prs' in value && (!Array.isArray(value.prs) || !value.prs.length || value.prs.length > 32
        || !value.prs.every(positive) || new Set(value.prs).size !== value.prs.length))) {
    fail('bounded explicit operation and source paths required');
  }
  return Object.freeze({ ...value, ...('prs' in value ? { prs: Object.freeze([...value.prs]) } : {}) });
}

export async function readTrainControllerRequest(filename) {
  if (!absolute(filename)) fail('absolute request file required');
  const fd = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const stat = await fd.stat();
    if (!stat.isFile() || stat.nlink !== 1 || stat.size < 2 || stat.size > 65536) fail('bounded regular request required');
    const bytes = Buffer.alloc(stat.size + 1); let length = 0;
    while (length < bytes.length) {
      const read = await fd.read(bytes, length, bytes.length - length, null);
      if (!read.bytesRead) break;
      length += read.bytesRead;
    }
    const after = await fd.stat();
    if (length !== stat.size || after.size !== stat.size || after.mtimeMs !== stat.mtimeMs
        || after.ctimeMs !== stat.ctimeMs) fail('request changed during read');
    let value;
    try { value = JSON.parse(bytes.subarray(0, length).toString()); }
    catch { fail('request JSON invalid'); }
    return validateTrainControllerRequest(value);
  } finally { await fd.close(); }
}

// Credentials go only to the fixed trusted child programs. In particular Git
// reads/merge commands do not inherit the observer, JWT or installation token.
export function createTrainSourceEnvironment(environment = process.env, credentialReferences = []) {
  const env = { ...environment };
  for (const name of ['GH_TOKEN', 'GITHUB_TOKEN', ...credentialReferences]) if (name) delete env[name];
  for (const name of Object.keys(env)) if (name.startsWith('GIT_')) delete env[name];
  Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_CONFIG_SYSTEM: '/dev/null', GIT_TERMINAL_PROMPT: '0', GIT_NO_REPLACE_OBJECTS: '1',
    GIT_NO_LAZY_FETCH: '1' });
  return Object.freeze(env);
}

export function createTrainSessionCommand({ installationToken, observerEnvironmentReference,
  credentialReferences = [], environment = process.env, command = spawnSync }) {
  if (typeof installationToken !== 'string' || !/^[\x21-\x7e]{1,16384}$/.test(installationToken)) fail('scoped installation credential required');
  const inherited = { ...environment };
  const clean = extra => {
    return { ...createTrainSourceEnvironment({ ...inherited, ...extra },
      [observerEnvironmentReference, ...credentialReferences]) };
  };
  return (program, args, options = {}) => {
    if (!['gh', 'git'].includes(program) || !Array.isArray(args)
        || (program === 'gh' && args[0] !== 'api')) fail('unsupported controller child program');
    const env = clean(options.env);
    if (program === 'git') {
      // Never let ambient Git directory/config/transport overrides escape the
      // fresh push context. Synthetic commit identity is trusted caller data.
      for (const name of Object.keys(env)) if (name.startsWith('GIT_')) delete env[name];
      Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
        GIT_CONFIG_SYSTEM: '/dev/null', GIT_TERMINAL_PROMPT: '0', GIT_NO_REPLACE_OBJECTS: '1',
        GIT_NO_LAZY_FETCH: '1' });
      for (const name of ['GIT_AUTHOR_NAME', 'GIT_AUTHOR_EMAIL', 'GIT_AUTHOR_DATE',
        'GIT_COMMITTER_NAME', 'GIT_COMMITTER_EMAIL', 'GIT_COMMITTER_DATE']) {
        if (options.env && Object.hasOwn(options.env, name)) env[name] = options.env[name];
      }
    }
    if (program === 'gh' || args.includes('push')) env.GH_TOKEN = installationToken;
    env.GH_HOST = 'github.com';
    let result;
    try { result = command(program, args, { ...options, env }); }
    catch { return { status: 1, stderr: 'Controller child failed' }; }
    if (result?.error || result?.signal || result?.status !== 0) {
      return { status: Number.isInteger(result?.status) && result.status !== 0 ? result.status : 1,
        stderr: 'Controller child refused; inspect retained operation evidence' };
    }
    return result;
  };
}

export function createTrainMetadataApi(command, directory) {
  return endpoint => {
    const result = command('gh', ['api', endpoint], { cwd: directory,
      encoding: 'utf8', timeout: 60000, maxBuffer: 2 * 1024 ** 2 });
    if (result?.error || result?.signal || result?.status !== 0) fail('credential observation refused');
    try { return JSON.parse(result.stdout); }
    catch { fail('credential observation JSON invalid'); }
  };
}

export async function runTrainController({ request, config, privateKeyFile, observerEnvironmentReference,
  policyApi, environment = process.env, command = spawnSync, session = withTrainAppSession,
  artifactCommandFactory }) {
  request = validateTrainControllerRequest(request);
  validateMachineWriterConfiguration(config, config?.repository);
  config = Object.freeze({ ...config });
  environment = Object.freeze({ ...environment });
  const sourceEnvironment = createTrainSourceEnvironment(environment, [observerEnvironmentReference]);
  await assertSeparateTrainController(request.targetSource);
  policyApi ??= createPolicyObserverApi(observerEnvironmentReference, config, { env: environment, command });
  return session({ config, privateKeyFile, policyApi, sourceDirectory: request.targetSource,
    callback: async ({ jwt, installationToken }) => {
      const child = createTrainSessionCommand({ installationToken, observerEnvironmentReference, environment, command });
      const readApi = createTrainMetadataApi(child, request.targetSource);
      const appReadApi = createTrainMetadataApi(createTrainSessionCommand({ installationToken: jwt,
        observerEnvironmentReference, environment, command }), request.targetSource);
      const appIdentityApi = endpoint => {
        if (endpoint !== 'app' && endpoint !== `app/installations/${config.installationId}`) fail('unsupported App identity endpoint');
        return appReadApi(endpoint);
      };
      const observeCheapPolicy = ({ repository }) => {
        if (repository !== config.repository) fail('repository differs from session');
        return observeCheapTrainPolicy({ config, repository, api: readApi, appApi: appIdentityApi, policyApi });
      };
      let artifactCommand;
      if (request.operation === 'acquire') {
        artifactCommandFactory ??= (await import('./train-artifacts.mjs')).createBoundedTrainArtifactCommand;
        const env = { ...sourceEnvironment, GH_TOKEN: installationToken, GH_HOST: 'github.com' };
        artifactCommand = artifactCommandFactory({ env, directory: request.targetSource });
      }
      const commands = createTrainCommands({ command: child, directory: request.targetSource,
        artifactCommand, observeCheapPolicy, sourceEnvironment });
      switch (request.operation) {
        case 'assemble': return commands.assemble(config.repository, request.prs, request.worktree, request.manifest);
        case 'capture': return commands.capture(config.repository, request.source, request.prs, request.manifest);
        case 'dispatch': return createTrainDispatcher({ command: child, directory: request.targetSource, commands, sourceEnvironment })(request.manifest);
        case 'acquire': return commands.acquire(request.manifest, request.runId, request.archives);
        case 'land': return createTrainLander({ command: child, directory: request.targetSource, commands,
          machineWriter: config, appIdentityApi, policyObserverApi: policyApi })(request.manifest,
          request.runId, request.archives, request.evidence);
        default: fail('unknown controller operation');
      }
    },
  });
}
