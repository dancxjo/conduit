// One finite trusted-host tick. All source/proof authority stays in the adapters.
import { createReadStream } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readdir, readFile, realpath, lstat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readSupervisorJson, withSupervisorStore } from './train-supervisor-store.mjs';
import { runTrainController, createTrainSessionCommand, createTrainMetadataApi, createTrainSourceEnvironment } from './train-controller.mjs';
import { withTrainAppSession, TrainAppSessionError } from './train-app-session.mjs';
import { createPolicyObserverApi, validateMachineWriterConfiguration } from './train-machine-policy.mjs';
import { verifyTrainManifest } from './train.mjs';
import { observeSupervisorDownstream } from './train-supervisor-downstream-session.mjs';
const digest = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const positive = value => Number.isSafeInteger(value) && value > 0;
const absolute = value => typeof value === 'string' && value.length <= 4096 && !/[\0\r\n]/.test(value)
  && path.isAbsolute(value) && path.resolve(value) === value;
const fail = reason => { throw new Error(`Train supervisor: ${reason}`); };
const fields = ['schema', 'id', 'repository', 'prs', 'targetSource', 'worktree', 'manifest', 'archives', 'evidence'];
export async function readTrainSupervisorQueue(filename) {
  if (!absolute(filename)) fail('absolute private queue required');
  const q = await readSupervisorJson(filename, 32768);
  if (!q || Object.keys(q).sort().join() !== [...fields].sort().join()
      || q.schema !== 'conduit.train-supervisor-queue/v1' || !/^[a-zA-Z0-9_-]{1,80}$/.test(q.id ?? '')
      || !/^[\w.-]+\/[\w.-]+$/.test(q.repository ?? '') || !Array.isArray(q.prs)
      || !q.prs.length || q.prs.length > 32 || !q.prs.every(positive) || new Set(q.prs).size !== q.prs.length
      || !['targetSource', 'worktree', 'manifest', 'archives', 'evidence'].every(name => absolute(q[name]))) fail('invalid explicit queue');
  return Object.freeze(Object.fromEntries(fields.map(key => [key, key === 'prs' ? Object.freeze([...q.prs]) : q[key]])));
}
const inside = (base, value) => { const relative = path.relative(base, value); return !relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative)); };
async function paths(queue, stateDirectory) {
  const controller = await realpath(fileURLToPath(new URL('../../../', import.meta.url)));
  const source = await realpath(queue.targetSource);
  if (source !== queue.targetSource || inside(source, controller) || inside(controller, source)) fail('separate canonical controller and source required');
  for (const candidate of [stateDirectory, queue.worktree, queue.manifest, queue.archives, queue.evidence]) {
    if (!absolute(candidate) || inside(source, candidate) || inside(controller, candidate)) fail('external canonical private outputs required');
    const parent = await realpath(path.dirname(candidate));
    const parentInfo = await lstat(parent);
    if (parentInfo.uid !== process.getuid() || parentInfo.mode & 0o077) fail('output parent must be owner-only');
    if (parent !== path.dirname(candidate)) fail('output parent must be canonical');
  }
  const outputs = [queue.worktree, queue.manifest, queue.archives, queue.evidence, stateDirectory];
  if (outputs.some((a, i) => outputs.some((b, j) => i !== j && (inside(a, b) || inside(b, a))))) fail('output paths overlap');
}
export async function trainSupervisorRuntimeIdentity({ environment = process.env, observerEnvironmentReference, command = spawnSync } = {}) {
  const root = fileURLToPath(new URL('./', import.meta.url));
  const controllerRoot = fileURLToPath(new URL('../../../', import.meta.url));
  const git = args => {
    const result = command('git', args, { cwd: controllerRoot, encoding: 'utf8', timeout: 10000, maxBuffer: 1048576,
      env: { ...createTrainSourceEnvironment(environment, [observerEnvironmentReference]), GIT_OPTIONAL_LOCKS: '0' } });
    if (result.error || result.signal || result.status !== 0) fail('controller Git identity unavailable');
    return result.stdout.trim();
  };
  const head = git(['rev-parse', 'HEAD']), tree = git(['rev-parse', 'HEAD^{tree}']);
  if (!/^[a-f0-9]{40}$/.test(head) || !/^[a-f0-9]{40}$/.test(tree)
      || git(['status', '--porcelain=v1', '--untracked-files=all'])) fail('pinned controller checkout must be clean');
  const executable = await realpath(process.execPath), executableInfo = await lstat(executable);
  if (!executableInfo.isFile() || executableInfo.size > 256 * 1024 * 1024) fail('runtime executable exceeds bound');
  const executableHash = createHash('sha256'); let executableBytes = 0;
  for await (const chunk of createReadStream(executable)) {
    executableBytes += chunk.length; if (executableBytes > executableInfo.size) fail('runtime executable changed'); executableHash.update(chunk);
  }
  const afterExecutable = await lstat(executable);
  if (executableBytes !== executableInfo.size || afterExecutable.mtimeMs !== executableInfo.mtimeMs
      || afterExecutable.ctimeMs !== executableInfo.ctimeMs) fail('runtime executable changed');
  const files = [];
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const filename = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) fail('controller code symlink refused');
      if (entry.isDirectory()) await visit(filename);
      else if (/\.(?:mjs|py|json)$/.test(entry.name)) files.push(filename);
      if (files.length > 1000) fail('controller inventory exceeds bound');
    }
  }
  await visit(root); let bytes = 0; const inventory = [];
  for (const filename of files.sort()) {
    const before = await lstat(filename);
    if (!before.isFile() || before.size > 4 * 1024 * 1024) fail('controller file exceeds bound');
    const data = await readFile(filename); const after = await lstat(filename);
    bytes += data.length;
    if (bytes > 32 * 1024 * 1024 || data.length !== before.size || after.mtimeMs !== before.mtimeMs || after.ctimeMs !== before.ctimeMs) fail('controller code changed during observation');
    inventory.push([path.relative(root, filename), createHash('sha256').update(data).digest('hex')]);
  }
  if (git(['rev-parse', 'HEAD']) !== head || git(['status', '--porcelain=v1', '--untracked-files=all'])) fail('controller changed during observation');
  return digest({ head, tree, inventory, executable, executableSha256: executableHash.digest('hex'), versions: process.versions, platform: process.platform, arch: process.arch });
}
const statuses = new Set(['queued', 'in_progress', 'requested', 'waiting', 'pending']);
export function inspectSupervisorRun({ queue, train, repositoryId, binding, api }) {
  const refuse = () => ({ kind: 'refused', reason: 'exact-run-custody' });
  const base = `repos/${queue.repository}`;
  const inventory = api(`${base}/actions/workflows/integration.yml/runs?branch=${encodeURIComponent(train.branch)}&head_sha=${train.source}&event=workflow_dispatch&per_page=100`);
  if (!Number.isSafeInteger(inventory?.total_count) || inventory.total_count < 0 || inventory.total_count > 100
      || !Array.isArray(inventory.workflow_runs) || inventory.workflow_runs.length !== inventory.total_count) return refuse();
  const valid = run => positive(run?.id) && positive(run.run_attempt) && run.path === '.github/workflows/integration.yml'
    && run.event === 'workflow_dispatch' && run.head_sha === train.source && run.head_branch === train.branch
    && run.repository?.full_name === queue.repository && run.head_repository?.full_name === queue.repository
    && run.repository.id === repositoryId && run.head_repository.id === repositoryId;
  if (inventory.workflow_runs.length > 1 || inventory.workflow_runs.some(run => !valid(run))) return refuse();
  if (!inventory.workflow_runs.length) return binding ? refuse() : { kind: 'waiting' };
  const listed = inventory.workflow_runs[0];
  if (binding && (listed.id !== binding.id || binding.attempt !== null && listed.run_attempt !== binding.attempt)) return refuse();
  if (api(`${base}/git/ref/heads/${train.branch}`).object?.sha !== train.source
      || api(`${base}/git/commits/${train.source}`).tree?.sha !== train.tree) return refuse();
  const run = api(`${base}/actions/runs/${listed.id}`);
  if (!valid(run) || run.id !== listed.id || run.run_attempt !== listed.run_attempt) return refuse();
  const observed = { id: run.id, attempt: run.run_attempt };
  if (run.status === 'completed') return run.conclusion === 'success' && positive(run.check_suite_id)
    ? { kind: 'success', binding: observed } : { kind: 'failed', binding: observed };
  return statuses.has(run.status) ? { kind: 'waiting', binding: observed } : refuse();
}
async function observe({ queue, train, config, privateKeyFile, observerEnvironmentReference, environment, command, binding, session = withTrainAppSession }) {
  const policyApi = createPolicyObserverApi(observerEnvironmentReference, config, { env: environment, command });
  return session({ config, privateKeyFile, policyApi, sourceDirectory: queue.worktree,
    callback: ({ installationToken }) => {
      let transient = false;
      const observedCommand = (program, args, options) => {
        const result = command(program, args, options);
        if (program === 'gh' && ['ETIMEDOUT', 'ECONNRESET', 'EAI_AGAIN'].includes(result?.error?.code)) transient = true;
        return result;
      };
      const child = createTrainSessionCommand({ installationToken, observerEnvironmentReference, environment, command: observedCommand });
      const api = createTrainMetadataApi(child, fileURLToPath(new URL('../../../', import.meta.url)));
      try { return inspectSupervisorRun({ queue, train, repositoryId: config.repositoryId, binding, api }); }
      catch (error) { if (transient) return { kind: 'transient' }; throw error; }
    } });
}
export async function runTrainSupervisorTick({ queueFile, stateDirectory, config, privateKeyFile,
  observerEnvironmentReference, maintenance = false, environment = process.env, command = spawnSync,
  controller = runTrainController, observer = observe, downstreamObserver = observeSupervisorDownstream,
  identity = trainSupervisorRuntimeIdentity, session = withTrainAppSession, clock = Date.now }) {
  const now = clock(); if (!Number.isSafeInteger(now) || now < 0) fail('invalid observation clock');
  const queue = await readTrainSupervisorQueue(queueFile);
  validateMachineWriterConfiguration(config, queue.repository);
  await paths(queue, stateDirectory);
  const queueDigest = digest(queue), configurationDigest = digest(config), runtimeDigest = await identity({ environment, observerEnvironmentReference });
  if (!/^[a-f0-9]{64}$/.test(runtimeDigest ?? '')) fail('controller identity invalid');
  return withSupervisorStore(stateDirectory, async store => {
    let state = await store.read();
    if (!state) state = { schema: 'conduit.train-supervisor-journal/v1', repository: queue.repository,
      queueDigest, configurationDigest, runtimeDigest, stage: 'new', sequence: 0 };
    if (state.schema !== 'conduit.train-supervisor-journal/v1' || state.repository !== queue.repository
        || state.queueDigest !== queueDigest || state.configurationDigest !== configurationDigest
        || state.runtimeDigest !== runtimeDigest || !Number.isSafeInteger(state.sequence) || state.sequence < 0
        || !['new', 'assembled', 'awaiting-run', 'proof-ready', 'acquired', 'awaiting-downstream', 'complete', 'intent', 'blocked'].includes(state.stage)) fail('journal identity changed; inspect recovery');
    if (['assembled', 'awaiting-run', 'proof-ready', 'acquired', 'awaiting-downstream', 'complete'].includes(state.stage) && !state.train) fail('journal train identity missing');
    if (['proof-ready', 'acquired', 'awaiting-downstream', 'complete'].includes(state.stage) && (!positive(state.binding?.id) || !positive(state.binding?.attempt))) fail('journal run identity missing');
    if (state.nextObservationAt !== undefined && (!Number.isSafeInteger(state.nextObservationAt) || state.nextObservationAt < 0)) fail('journal clock invalid');
    if (state.train && (!/^[a-f0-9]{40}$/.test(state.train.source ?? '') || !/^[a-f0-9]{40}$/.test(state.train.tree ?? '')
        || !/^train\/[a-f0-9]{64}$/.test(state.train.branch ?? '') || !/^[a-f0-9]{64}$/.test(state.train.digest ?? ''))) fail('journal train identity invalid');
    const status = () => ({ repository: state.repository, queue: queue.id, queueDigest, stage: state.stage,
      ...(state.train ? { source: state.train.source, manifestDigest: state.train.digest } : {}),
      ...(state.binding ? { runId: state.binding.id, runAttempt: state.binding.attempt } : {}),
      ...(state.gate ? { gate: state.gate } : {}), ...(state.sessionFailure ? { sessionFailure: state.sessionFailure } : {}) });
    const save = async changes => { state = { ...state, ...changes, sequence: state.sequence + 1 }; await store.write(state); };
    if (state.stage === 'intent' || state.stage === 'blocked') return { ...status(), inspectionRequired: true };
    if (state.stage !== 'new') {
      const manifest = await readSupervisorJson(queue.manifest, 65536); verifyTrainManifest(manifest);
      if (!state.train || manifest.repository !== queue.repository || ['source', 'tree', 'branch', 'digest'].some(key => manifest[key] !== state.train[key])) fail('recorded manifest changed');
    }
    if (state.stage === 'complete') return { ...status(), complete: true, publication: state.downstreamBinding };
    if (state.stage === 'awaiting-downstream') {
      if (state.nextObservationAt && now < state.nextObservationAt) return { ...status(), observationDeferred: true };
      let observation;
      try { observation = await downstreamObserver({ queue, train: state.train, config, privateKeyFile,
        observerEnvironmentReference, environment, command, proofBinding: state.binding,
        binding: state.downstreamBinding, session }); }
      catch (error) {
        await save({ stage: 'blocked', gate: 'downstream-session-refused', sessionFailure: {
          code: error instanceof TrainAppSessionError ? error.code : 'unclassified-observer-failure',
          revocation: error instanceof TrainAppSessionError ? error.revocation : 'unknown' } });
        return { ...status(), inspectionRequired: true };
      }
      if (!['waiting', 'failed', 'refused', 'complete'].includes(observation?.kind)) fail('unknown downstream observation');
      if (observation.kind === 'complete' && (!observation.binding
          || observation.binding.repository !== queue.repository || observation.binding.repositoryId !== config.repositoryId
          || observation.binding.source !== state.train.source || observation.binding.tree !== state.train.tree
          || observation.binding.manifestDigest !== state.train.digest
          || observation.binding.proofRunId !== state.binding.id || observation.binding.proofRunAttempt !== state.binding.attempt
          || !['acceptanceRunId', 'acceptanceRunAttempt', 'publicationRunId', 'publicationRunAttempt', 'publicationReceiptArtifactId']
            .every(key => positive(observation.binding[key])))) fail('downstream completion lacks exact custody');
      await save({ nextObservationAt: now + 300000,
        ...(observation.binding ? { downstreamBinding: observation.binding } : {}),
        ...(observation.kind === 'complete' ? { stage: 'complete', gate: null } : {}),
        ...(['failed', 'refused'].includes(observation.kind) ? { stage: 'blocked', gate: `downstream-${observation.kind}` } : {}) });
      return { ...status(), ...(state.stage === 'complete' ? { complete: true, publication: state.downstreamBinding } : {}) };
    }
    if (state.stage === 'awaiting-run') {
      if (state.nextObservationAt && now < state.nextObservationAt) return { ...status(), observationDeferred: true };
      let observation;
      try { observation = await observer({ queue, train: state.train, config, privateKeyFile,
        observerEnvironmentReference, environment, command, binding: state.binding, session }); }
      catch (error) {
        await save({ stage: 'blocked', gate: 'observer-session-refused', sessionFailure: {
          code: error instanceof TrainAppSessionError ? error.code : 'unclassified-observer-failure',
          revocation: error instanceof TrainAppSessionError ? error.revocation : 'unknown' } });
        return { ...status(), inspectionRequired: true };
      }
      if (observation?.kind === 'transient') {
        const failures = (state.transientObservations ?? 0) + 1;
        await save({ transientObservations: failures, nextObservationAt: now + 300000,
          ...(failures >= 6 ? { stage: 'blocked', gate: 'metadata-transient-limit' } : {}) });
        return { ...status(), observationFailed: true };
      }
      if (observation?.kind === 'refused' || observation?.kind === 'failed') {
        await save({ stage: 'blocked', gate: observation.kind === 'failed' ? 'proof-failed' : 'run-custody-refused' });
      } else if (observation?.kind === 'waiting' || observation?.kind === 'success') {
        if (observation.kind === 'success' && !observation.binding) fail('successful observer lacks run identity');
        if (observation.binding && (!positive(observation.binding.id) || !positive(observation.binding.attempt)
            || state.binding && (state.binding.id !== observation.binding.id || state.binding.attempt !== null && state.binding.attempt !== observation.binding.attempt))) fail('observer run identity changed');
        await save({ observationFailed: false, transientObservations: 0, nextObservationAt: now + 300000, ...(observation.binding ? { binding: observation.binding } : {}),
          ...(observation.kind === 'success' ? { stage: 'proof-ready' } : {}) });
      } else fail('unknown observer result');
      return status();
    }
    if (maintenance) return { ...status(), paused: true };
    const operation = { new: 'assemble', assembled: 'dispatch', 'proof-ready': 'acquire', acquired: 'land' }[state.stage];
    const request = { schema: 'conduit.train-controller-request/v1', operation,
      targetSource: operation === 'assemble' ? queue.targetSource : queue.worktree, manifest: queue.manifest,
      ...(operation === 'assemble' ? { prs: queue.prs, worktree: queue.worktree } : {}),
      ...(['acquire', 'land'].includes(operation) ? { runId: state.binding?.id, archives: queue.archives } : {}),
      ...(operation === 'land' ? { evidence: queue.evidence } : {}) };
    await save({ stage: 'intent', operation, nonce: randomUUID(), requestDigest: digest(request), previousStage: state.stage });
    let result;
    try { result = await controller({ request, config, privateKeyFile, observerEnvironmentReference, environment, command }); }
    catch (error) {
      await save({ sessionFailure: { code: error instanceof TrainAppSessionError ? error.code : 'operation-outcome-unknown',
        revocation: error instanceof TrainAppSessionError ? error.revocation : 'unknown' } });
      return { ...status(), inspectionRequired: true };
    }
    // Even a successful API result must match this exact recorded operation.
    if (operation === 'assemble') {
      verifyTrainManifest(result);
      if (result.repository !== queue.repository || result.inputs.map(input => input.number).sort((a,b)=>a-b).join() !== [...queue.prs].sort((a,b)=>a-b).join()) fail('assembled input differs');
      await save({ stage: 'assembled', train: { source: result.source, tree: result.tree, branch: result.branch, digest: result.digest } });
    } else if (operation === 'dispatch') {
      if (result?.source !== state.train.source || result.branch !== state.train.branch
          || !(result.runId === null || positive(result.runId)) || result.runId === null && result.dispatched !== true) fail('dispatch acknowledgement differs');
      await save({ stage: 'awaiting-run', ...(result.runId ? { binding: { id: result.runId, attempt: null } } : {}) });
    } else if (operation === 'acquire') {
      if (result?.source !== state.train.source || result.manifestDigest !== state.train.digest || result.runId !== state.binding.id || result.runAttempt !== state.binding.attempt) fail('acquisition differs');
      await save({ stage: 'acquired' });
    } else {
      if (result?.landedDev !== state.train.source || result.tree !== state.train.tree || result.manifestDigest !== state.train.digest
          || result.runId !== state.binding.id || result.runAttempt !== state.binding.attempt) fail('landing acknowledgement differs');
      await save({ stage: 'awaiting-downstream', gate: 'actual-dev-adoption-and-publication' });
    }
    return status();
  });
}
