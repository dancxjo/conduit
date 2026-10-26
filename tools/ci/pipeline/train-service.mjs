// One finite multi-batch host tick; authority remains in selection and supervisor.
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { mkdir, realpath, lstat, open } from 'node:fs/promises';
import { readSupervisorJson } from './train-supervisor-store.mjs';
import { withTrainServiceStore } from './train-service-store.mjs';
import { runTrainSupervisorTick, trainSupervisorRuntimeIdentity } from './train-supervisor.mjs';
import { verifyTrainManifest } from './train.mjs';
import { validateMachineWriterConfiguration } from './train-machine-policy.mjs';
const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const fail = reason => { throw new Error(`Train service: ${reason}`); };
const absolute = value => typeof value === 'string' && value.length <= 4096 && !/[\0\r\n]/.test(value) && path.isAbsolute(value) && path.resolve(value) === value;
const inside = (a,b) => b === a || b.startsWith(a + path.sep);
export async function readTrainServiceConfiguration(filename) {
  if (!absolute(filename)) fail('absolute configuration required');
  const value = await readSupervisorJson(filename, 32768);
  const fields = ['schema','repository','repositoryId','policy','sourceObjectStore','serviceRoot','maxBatches'];
  if (!value || Object.keys(value).sort().join() !== fields.sort().join()
      || value.schema !== 'conduit.train-service/v1' || !/^[\w.-]+\/[\w.-]+$/.test(value.repository ?? '')
      || !Number.isSafeInteger(value.repositoryId) || value.repositoryId < 1
      || value.policy !== 'all-ready-same-repository-dev-prs'
      || !['sourceObjectStore','serviceRoot'].every(key => absolute(value[key]))
      || !Number.isSafeInteger(value.maxBatches) || value.maxBatches < 1 || value.maxBatches > 128) fail('invalid reviewed configuration');
  return Object.freeze(value);
}
async function privateDirectory(directory) {
  if (await realpath(directory) !== directory) fail('canonical directory required');
  const info = await lstat(directory);
  if (!info.isDirectory() || info.uid !== process.getuid() || info.mode & 0o077) fail('owner-only directory required');
}
async function writeFresh(filename,value) {
  const fd = await open(filename,'wx',0o600);
  try { await fd.writeFile(JSON.stringify(value)+'\n'); await fd.sync(); } finally { await fd.close(); }
  const parent = await open(path.dirname(filename),'r'); try { await parent.sync(); } finally { await parent.close(); }
}
function validateSelection(result, service) {
  const selection = result?.selection;
  if (result?.kind !== 'selected' || selection?.repository !== service.repository) fail('invalid selected repository');
  const {branch,...identity} = selection;
  if (branch !== `train/${hash(identity)}`) fail('selection digest differs');
  // Reuse the canonical admission schema, including all exact job receipts.
  const payload = {schema:'conduit.integration-train/v2',...selection,source:'0'.repeat(40),tree:'0'.repeat(40)};
  verifyTrainManifest({...payload,digest:hash(payload)});
  if (result.development?.source !== selection.base || !['runId','runAttempt'].every(key => Number.isSafeInteger(result.development[key]) && result.development[key]>0)) fail('development identity missing');
  return branch.slice(6);
}
export async function runTrainServiceTick({configurationFile,config,privateKeyFile,observerEnvironmentReference,
  environment = process.env, command, maintenance=false, selectReady,
  supervisor=runTrainSupervisorTick, identity=trainSupervisorRuntimeIdentity, clock=Date.now}) {
  const service = await readTrainServiceConfiguration(configurationFile);
  validateMachineWriterConfiguration(config,service.repository);
  if (config.repositoryId !== service.repositoryId) fail('repository identity differs');
  await privateDirectory(service.serviceRoot);
  const controllerRoot=await realpath(fileURLToPath(new URL('../../../',import.meta.url)));
  if (inside(controllerRoot,service.serviceRoot) || inside(service.serviceRoot,controllerRoot)) fail('service state must be outside controller');
  if (await realpath(service.sourceObjectStore) !== service.sourceObjectStore || inside(service.sourceObjectStore,service.serviceRoot)
      || inside(service.serviceRoot,service.sourceObjectStore)) fail('separate canonical source object store required');
  const runtime = await identity({environment,observerEnvironmentReference,command});
  if (!/^[a-f0-9]{64}$/.test(runtime ?? '')) fail('runtime identity invalid');
  const configurationDigest = hash({service,config});
  return withTrainServiceStore(service.serviceRoot,async store => {
    const now=clock();
    if (!Number.isSafeInteger(now) || now<0) fail('invalid service clock');
    let state = await store.read();
    if (!state) state = {schema:'conduit.train-service-journal/v1',repository:service.repository,configurationDigest,runtime,completed:[],current:null,sequence:0};
    if (state.schema !== 'conduit.train-service-journal/v1' || state.repository !== service.repository || state.configurationDigest !== configurationDigest || state.runtime !== runtime
        || !Number.isSafeInteger(state.sequence) || state.sequence<0 || !Array.isArray(state.completed) || state.completed.length>service.maxBatches
        || (state.nextReadyAt !== undefined && (!Number.isSafeInteger(state.nextReadyAt) || state.nextReadyAt<0))
        || state.completed.some((batch,index)=>!batch || !/^[a-f0-9]{64}$/.test(batch.selectionDigest ?? '')
          || batch.batchRoot !== path.join(service.serviceRoot,`batch-${String(index+1).padStart(3,'0')}-${batch.selectionDigest}`)
          || !/^[a-f0-9]{64}$/.test(batch.queueDigest ?? '') || batch.completion?.complete !== true)) fail('journal binding changed; inspect recovery');
    const save = async changes => { state={...state,...changes,sequence:state.sequence+1}; await store.write(state); };
    const result = extra => ({repository:service.repository,completedBatches:state.completed.length,...extra});
    if (state.blocked) return result({inspectionRequired:true,gate:state.blocked});
    if (!state.current) {
      if (maintenance) return result({paused:true});
      if (state.completed.length >= service.maxBatches) return result({capacityReached:true,inspectionRequired:true});
      if (state.nextReadyAt && now<state.nextReadyAt) return result({observationDeferred:true});
      if (typeof selectReady !== 'function') fail('trusted short-session selector required');
      let selected;
      try { selected=await selectReady({service,config,privateKeyFile,observerEnvironmentReference,environment,command}); }
      catch { await save({blocked:'selection-session-refused'}); return result({inspectionRequired:true,gate:state.blocked}); }
      if (selected?.kind === 'idle' || selected?.kind === 'waiting') {
        await save({nextReadyAt:now+300000});
        return result(selected.kind==='idle' ? {idle:true} : {waiting:true,development:selected.development});
      }
      if (selected?.kind !== 'selected') { await save({blocked:'selection-refused'}); return result({inspectionRequired:true,gate:state.blocked}); }
      const digest = validateSelection(selected,service);
      if (state.completed.some(batch=>batch.selectionDigest===digest)) fail('completed selection cannot be replayed');
      const batchRoot=path.join(service.serviceRoot,`batch-${String(state.completed.length+1).padStart(3,'0')}-${digest}`);
      const queue={schema:'conduit.train-supervisor-queue/v1',id:`batch-${digest}`,repository:service.repository,
        prs:selected.selection.inputs.map(input=>input.number),targetSource:service.sourceObjectStore,
        worktree:path.join(batchRoot,'source'),manifest:path.join(batchRoot,'manifest.json'),archives:path.join(batchRoot,'archives'),evidence:path.join(batchRoot,'evidence.json')};
      // Record allocation intent before filesystem effects. A crash never recreates it.
      await save({current:{stage:'allocation-intent',selectionDigest:digest,selection:selected.selection,development:selected.development,batchRoot,queue,queueDigest:hash(queue)}});
      await mkdir(batchRoot,{mode:0o700});
      await mkdir(path.join(batchRoot,'core'),{mode:0o700});
      await writeFresh(path.join(batchRoot,'queue.json'),queue);
      await save({current:{...state.current,stage:'active'}});
      return result({stage:'selected',selectionDigest:digest});
    }
    const batch=state.current;
    if (batch.stage !== 'active') return result({inspectionRequired:true,gate:'allocation-outcome-unknown'});
    if (batch.batchRoot !== path.join(service.serviceRoot,`batch-${String(state.completed.length+1).padStart(3,'0')}-${batch.selectionDigest}`)
        || hash(batch.queue)!==batch.queueDigest || batch.selectionDigest!==validateSelection({kind:'selected',selection:batch.selection,development:batch.development},service)) fail('batch binding changed');
    const queueFile=path.join(batch.batchRoot,'queue.json');
    if (hash(await readSupervisorJson(queueFile,32768)) !== batch.queueDigest) fail('private queue changed');
    let observation;
    try { observation=await supervisor({queueFile,stateDirectory:path.join(batch.batchRoot,'core'),config,privateKeyFile,
      observerEnvironmentReference,environment,command,maintenance,clock}); }
    catch { await save({blocked:'core-tick-refused'}); return result({inspectionRequired:true,gate:state.blocked}); }
    if (observation?.source) {
      const manifest=await readSupervisorJson(batch.queue.manifest,65536);
      verifyTrainManifest(manifest);
      const selectedIdentity={repository:manifest.repository,base:manifest.base,inputs:manifest.inputs};
      if (manifest.branch !== batch.selection.branch || hash(selectedIdentity) !== batch.selectionDigest) {
        await save({blocked:'assembled-selection-changed'});
        return result({inspectionRequired:true,gate:state.blocked});
      }
    }
    if (observation?.inspectionRequired) { await save({blocked:'core-inspection-required'}); return result({inspectionRequired:true,core:observation}); }
    if (observation?.complete === true) {
      // Core validates exact adoption/publication custody; retain its whole directory.
      if (observation.stage !== 'complete' || !observation.publication || !/^[a-f0-9]{40}$/.test(observation.source ?? '')) fail('core completion lacks custody');
      await save({completed:[...state.completed,{selectionDigest:batch.selectionDigest,batchRoot:batch.batchRoot,queueDigest:batch.queueDigest,completion:observation}],current:null});
      return result({batchComplete:true,core:observation});
    }
    return result({stage:'current',core:observation});
  });
}
