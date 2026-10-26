import test from 'node:test';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {fixture} from './helpers/train-fixture.mjs';
import {machinePolicyFixture} from './helpers/train-machine-policy-fixture.mjs';
import {runTrainServiceTick,readTrainServiceConfiguration} from '../../tools/ci/pipeline/train-service.mjs';
const hash=x=>createHash('sha256').update(JSON.stringify(x)).digest('hex');
async function setup(t,maxBatches=2){
 const f=await fixture(t),dir=await mkdtemp(path.join(os.tmpdir(),'train-service-'));
 t.after(()=>rm(dir,{recursive:true,force:true}));
 const serviceRoot=path.join(dir,'service'); await mkdir(serviceRoot,{mode:0o700});
 const config=machinePolicyFixture(f.manifest.repository).config;
 const service={schema:'conduit.train-service/v1',repository:f.manifest.repository,repositoryId:config.repositoryId,policy:'all-ready-same-repository-dev-prs',sourceObjectStore:f.directory,serviceRoot,maxBatches};
 const configurationFile=path.join(dir,'config.json');await writeFile(configurationFile,JSON.stringify(service),{mode:0o600});
 let selectionIndex=0,activeManifest; const operations=[];
 const selectReady=async()=>{
  const inputs=f.manifest.inputs.map((input,i)=>({...input,number:input.number+selectionIndex*100}));selectionIndex++;
  const identity={repository:f.manifest.repository,base:f.manifest.base,inputs};
  const selection={...identity,branch:`train/${hash(identity)}`};
  const payload={schema:f.manifest.schema,...selection,source:f.manifest.source,tree:f.manifest.tree};
  activeManifest={...payload,digest:hash(payload)};
  return {kind:'selected',selection,development:{source:selection.base,runId:7,runAttempt:1}};
 };
 const controller=async({request})=>{
  operations.push(request.operation);
  if(request.operation==='assemble'){await writeFile(request.manifest,JSON.stringify(activeManifest),{mode:0o600});return activeManifest;}
  if(request.operation==='dispatch')return {source:activeManifest.source,branch:activeManifest.branch,runId:null,dispatched:true};
  if(request.operation==='acquire')return {source:activeManifest.source,manifestDigest:activeManifest.digest,runId:123,runAttempt:1};
  return {landedDev:activeManifest.source,tree:activeManifest.tree,manifestDigest:activeManifest.digest,runId:123,runAttempt:1};
 };
 const {runTrainSupervisorTick}=await import('../../tools/ci/pipeline/train-supervisor.mjs');
 let now=0;
 const supervisor=options=>runTrainSupervisorTick({...options,identity:async()=>'a'.repeat(64),controller,
  observer:async()=>({kind:'success',binding:{id:123,attempt:1}}),
  downstreamObserver:async()=>({kind:'complete',binding:{repository:config.repository,repositoryId:config.repositoryId,
   source:activeManifest.source,tree:activeManifest.tree,manifestDigest:activeManifest.digest,proofRunId:123,proofRunAttempt:1,
   acceptanceRunId:124,acceptanceRunAttempt:1,publicationRunId:125,publicationRunAttempt:1,publicationReceiptArtifactId:126}})});
 const options={configurationFile,config,privateKeyFile:'/unused-fixture',observerEnvironmentReference:'FIXTURE_OWNER',identity:async()=>'a'.repeat(64),selectReady,supervisor,clock:()=>now+=300001};
 return {service,config,options,operations,tick:overrides=>runTrainServiceTick({...options,...overrides}),journal:async()=>JSON.parse(await readFile(path.join(serviceRoot,'journal.json'),'utf8'))};
}
test('actual supervisor progresses two isolated batches across fresh ticks and retains all evidence',async t=>{
 const f=await setup(t);
 for(let batch=0;batch<2;batch++){
  assert.equal((await f.tick()).stage,'selected');
  for(let i=0;i<5;i++) assert.equal((await f.tick()).stage,'current');
  assert.equal((await f.tick()).batchComplete,true);
 }
 assert.equal((await f.tick()).capacityReached,true);
 const journal=await f.journal();assert.equal(journal.completed.length,2);
 assert.notEqual(journal.completed[0].batchRoot,journal.completed[1].batchRoot);
 for(const batch of journal.completed){
  assert.equal(JSON.parse(await readFile(path.join(batch.batchRoot,'core','journal.json'),'utf8')).stage,'complete');
  assert.equal(JSON.parse(await readFile(path.join(batch.batchRoot,'queue.json'),'utf8')).repository,f.service.repository);
 }
 assert.deepEqual(f.operations,['assemble','dispatch','acquire','land','assemble','dispatch','acquire','land']);
});
test('maintenance never selects or mutates; blocked core cannot advance to another batch',async t=>{
 const f=await setup(t);let selected=0;
 assert.equal((await f.tick({maintenance:true,selectReady:async()=>{selected++;}})).paused,true);assert.equal(selected,0);
 await f.tick();assert.equal((await f.tick({supervisor:async options=>{assert.equal(options.maintenance,true);return {stage:'new',paused:true};},maintenance:true})).stage,'current');
 assert.equal((await f.tick({supervisor:async()=>({inspectionRequired:true})})).inspectionRequired,true);
 assert.equal((await f.tick({selectReady:async()=>{selected++;}})).inspectionRequired,true);assert.equal(selected,0);
});
test('crash-retained lock and allocation intent are never silently removed or replayed',async t=>{
 const f=await setup(t);const lock=path.join(f.service.serviceRoot,'controller.lock');
 await writeFile(lock,'fixture abandoned lock',{mode:0o600});await assert.rejects(f.tick(),/lock exists/);
 await rm(lock);await f.tick();
 const journal=await f.journal();journal.current.stage='allocation-intent';
 await writeFile(path.join(f.service.serviceRoot,'journal.json'),JSON.stringify(journal),{mode:0o600});
 assert.equal((await f.tick()).gate,'allocation-outcome-unknown');assert.equal(f.operations.length,0);
});
test('config, runtime, queue and source identities cannot be substituted on restart',async t=>{
 const f=await setup(t);await f.tick();
 await assert.rejects(f.tick({identity:async()=>'b'.repeat(64)}),/binding changed/);
 const journal=await f.journal();const queue=path.join(journal.current.batchRoot,'queue.json');
 const value=JSON.parse(await readFile(queue,'utf8'));value.prs.reverse();value.prs.push(999);
 await writeFile(queue,JSON.stringify(value),{mode:0o600});await assert.rejects(f.tick(),/queue changed/);
});
test('selector session errors are inspection gates and never enter core mutation',async t=>{
 const f=await setup(t);let calls=0;
 assert.equal((await f.tick({selectReady:async()=>{throw new Error('fixture secret-shaped error');},supervisor:async()=>{calls++;}})).gate,'selection-session-refused');
 assert.equal((await f.tick()).inspectionRequired,true);assert.equal(calls,0);
});
test('configuration refuses permissive modes and unbounded capacity',async t=>{
 const f=await setup(t);const value={...f.service,maxBatches:129};
 await writeFile(f.options.configurationFile,JSON.stringify(value),{mode:0o600});
 await assert.rejects(readTrainServiceConfiguration(f.options.configurationFile),/invalid reviewed configuration/);
});

test('actual process interruption retains service lock and core mutation intent',async t=>{
 const f=await setup(t);await f.tick();
 const moduleRoot=fileURLToPath(new URL('../../tools/ci/pipeline/',import.meta.url));
 const script=`import {runTrainServiceTick} from ${JSON.stringify('file://'+moduleRoot+'train-service.mjs')};
 import {runTrainSupervisorTick} from ${JSON.stringify('file://'+moduleRoot+'train-supervisor.mjs')};
 await runTrainServiceTick({configurationFile:${JSON.stringify(f.options.configurationFile)},config:${JSON.stringify(f.config)},
 identity:async()=> 'a'.repeat(64), supervisor:options=>runTrainSupervisorTick({...options,identity:async()=> 'a'.repeat(64),controller:async()=>process.exit(42)})});`;
 const child=spawnSync(process.execPath,['--input-type=module','-e',script],{encoding:'utf8',timeout:10000,maxBuffer:32768});
 assert.equal(child.error,undefined);assert.equal(child.signal,null);assert.equal(child.status,42);
 const state=await f.journal();
 const core=JSON.parse(await readFile(path.join(state.current.batchRoot,'core','journal.json'),'utf8'));
 assert.equal(core.stage,'intent');assert.equal(core.operation,'assemble');
 await assert.rejects(f.tick(),/lock exists/);
});

test('changed admission between ready selection and assembly blocks before dispatch',async t=>{
 const f=await setup(t);await f.tick();
 const journal=await f.journal(),queue=journal.current.queue;
 const original=f.options.supervisor;
 const result=await f.tick({supervisor:async options=>{
  const observed=await original(options);
  const manifest=JSON.parse(await readFile(queue.manifest,'utf8'));
  manifest.inputs[0].number+=1000;manifest.inputs.sort((a,b)=>a.number-b.number);
  const selected={repository:manifest.repository,base:manifest.base,inputs:manifest.inputs};
  manifest.branch=`train/${hash(selected)}`;
  const {digest,...payload}=manifest;manifest.digest=hash(payload);
  await writeFile(queue.manifest,JSON.stringify(manifest),{mode:0o600});return observed;
 }});
 assert.equal(result.gate,'assembled-selection-changed');
 assert.equal((await f.tick()).inspectionRequired,true);assert.deepEqual(f.operations,['assemble']);
});


test('pending accepted-dev observation waits without mutation or tight selection polling',async t=>{
 const f=await setup(t);let calls=0;const now=Date.parse('2026-10-26T00:00:00Z');
 const selectReady=async()=>{calls++;return {kind:'waiting',development:{source:'a'.repeat(40),runId:7,runAttempt:1,status:'in_progress'}};};
 assert.equal((await f.tick({selectReady,clock:()=>now})).waiting,true);
 assert.equal((await f.tick({selectReady,clock:()=>now})).observationDeferred,true);
 assert.equal(calls,1);assert.deepEqual(f.operations,[]);
 assert.equal((await f.tick({clock:()=>now+300001})).stage,'selected');
});
test('unexpected core refusal is retained instead of automatically retried',async t=>{
 const f=await setup(t);await f.tick();let calls=0;
 const supervisor=async()=>{calls++;throw new Error('fixture unknown core outcome');};
 assert.equal((await f.tick({supervisor})).gate,'core-tick-refused');
 assert.equal((await f.tick({supervisor})).inspectionRequired,true);
 assert.equal(calls,1);assert.deepEqual(f.operations,[]);
 assert.ok((await f.journal()).current,'current batch and evidence remain retained');
});
