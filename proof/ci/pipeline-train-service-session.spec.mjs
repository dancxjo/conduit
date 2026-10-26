import test from 'node:test';
import assert from 'node:assert/strict';
import {generateKeyPairSync,verify} from 'node:crypto';
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {runTrainServiceTick} from '../../tools/ci/pipeline/train-service.mjs';
import {fixture} from './helpers/train-fixture.mjs';
import {machinePolicyFixture} from './helpers/train-machine-policy-fixture.mjs';
import {selectTrainServiceReadyInputs} from '../../tools/ci/pipeline/train-service-session.mjs';
import {withTrainAppSession,TrainAppSessionError} from '../../tools/ci/pipeline/train-app-session.mjs';
async function setup(t){
 const f=await fixture(t),machine=machinePolicyFixture(f.manifest.repository),scratch=await mkdtemp(path.join(os.tmpdir(),'ready-session-'));
 t.after(()=>rm(scratch,{recursive:true,force:true}));
 const key=generateKeyPairSync('rsa',{modulusLength:2048}),privateKeyFile=path.join(scratch,'fixture.pem');
 await writeFile(privateKeyFile,key.privateKey.export({type:'pkcs8',format:'pem'}),{mode:0o600});
 const repo={id:101,full_name:f.manifest.repository};
 for(const selected of f.selections){
  for(const object of [selected.pr.head.repo,selected.pr.base.repo,selected.candidateRun.repository,selected.candidateRun.head_repository])object.id=101;
  const run=selected.candidateRun;
  selected.candidateJobs={total_count:3,jobs:[['cheap-input/v1','success'],['candidate','success'],['proof','skipped']].map(([name,conclusion],i)=>({id:run.id*100+i,run_id:run.id,run_attempt:run.run_attempt,head_sha:run.head_sha,status:'completed',name,conclusion}))};
 }
 const integration={id:1000,run_attempt:1,repository:repo,head_repository:repo,head_sha:f.base,head_branch:'dev',path:'.github/workflows/integration.yml',event:'push',status:'completed',conclusion:'success'};
 const protection={required_status_checks:{contexts:['train-accepted'],checks:[{context:'train-accepted',app_id:15368}],strict:false},required_pull_request_reviews:null,enforce_admins:{enabled:true},allow_force_pushes:{enabled:false},allow_deletions:{enabled:false}};
 const state={minted:0,revoked:0,appReads:0,ownerReads:0,machineReads:0,revokeFailure:false,metadataFailure:false};
 const environment={PATH:'/fixture/no-executables',GH_TOKEN:'fixture-human',GITHUB_TOKEN:'fixture-actions',FIXTURE_OWNER:'fixture-owner',GH_HOST:'untrusted.invalid'};
 const metadata=endpoint=>{
  if(endpoint==='user')return machine.observerUser;
  if(endpoint==='app')return machine.app;
  if(endpoint==='app/installations/303')return machine.installation;
  if(endpoint==='installation/repositories?per_page=100')return machine.repositories;
  if(endpoint.endsWith('/actions/variables/CONDUIT_PR_ADMISSION_TIER'))return {name:'CONDUIT_PR_ADMISSION_TIER',value:'cheap-input-v1'};
  if(endpoint.includes('/branches/dev/protection'))return protection;
  if(endpoint.includes('/rules/branches/dev'))return machine.branchRules;
  if(endpoint.includes('/rulesets/'))return null;
  if(endpoint.endsWith('/git/ref/heads/dev'))return {object:{sha:f.base}};
  if(endpoint.includes('/workflows/integration.yml/runs?'))return {total_count:1,workflow_runs:[integration]};
  if(endpoint.endsWith('/actions/runs/1000'))return integration;
  if(endpoint.includes('/pulls?'))return f.selections.map(s=>({number:s.pr.number}));
  const pr=/\/pulls\/(\d+)$/.exec(endpoint);if(pr)return f.selections.find(s=>s.pr.number===Number(pr[1])).pr;
  const selected=f.selections.find(s=>endpoint.includes(s.pr.head.sha)||endpoint.includes(`/runs/${s.candidateRun.id}`));
  if(selected){if(endpoint.includes('/workflows/candidate.yml/runs?'))return {total_count:1,workflow_runs:[selected.candidateRun]};if(endpoint.endsWith(`/runs/${selected.candidateRun.id}`))return selected.candidateRun;if(endpoint.includes('/jobs?'))return selected.candidateJobs;}
  assert.fail('unexpected fixture read');
 };
 const command=(program,args,options)=>{
  assert.equal(program,'gh','selector executes no target Git/Python/build code');assert.equal(args[0],'api');
  assert.ok(!args.includes('POST')&&!args.includes('PATCH')&&!args.includes('DELETE'),'metadata channel remains read-only');
  assert.ok(!Object.hasOwn(options.env,'FIXTURE_OWNER')&&!Object.hasOwn(options.env,'GITHUB_TOKEN'));
  assert.equal(options.env.GH_HOST,'github.com');assert.ok(!args.some(a=>a.includes('fixture-owner')||a.includes('fixture-installation')));
  assert.ok(Number.isFinite(options.timeout)&&Number.isFinite(options.maxBuffer));
  assert.notEqual(options.cwd,f.directory,'metadata runs fixed controller code');
  const endpoint=args[1],owner=(endpoint.includes('/rulesets/')&&options.env.GH_TOKEN==='fixture-owner')||endpoint==='user'||endpoint.includes('/protection')||endpoint.includes('/rules/branches/')||endpoint.includes('/actions/variables/');
  const token=options.env.GH_TOKEN;
  if(owner){assert.ok(token==='fixture-owner');state.ownerReads++;}
  else if(endpoint==='app'||endpoint.startsWith('app/')){assert.ok(typeof token==='string'&&token.split('.').length===3);state.appReads++;}
  else{assert.ok(token==='fixture-installation');state.machineReads++;}
  if(state.metadataFailure)return {status:1,stderr:'fixture-installation'};
  let value=metadata(endpoint);if(endpoint.includes('/rulesets/'))value=token==='fixture-owner'?machine.ruleset:machine.machineRuleset;
  return {status:0,stdout:JSON.stringify(value)};
 };
 const transport=async(url,options)=>{
  const endpoint=new URL(url).pathname;let value;
  if(endpoint==='/app'){
   const parts=options.headers.Authorization.slice(7).split('.');assert.ok(verify('RSA-SHA256',Buffer.from(`${parts[0]}.${parts[1]}`),key.publicKey,Buffer.from(parts[2],'base64url')));value=machine.app;
  }else if(endpoint==='/app/installations/303')value=machine.installation;
  else if(options.method==='POST'){state.minted++;assert.deepEqual(JSON.parse(options.body),{repository_ids:[101],permissions:machine.app.permissions});value={token:'fixture-installation',permissions:machine.app.permissions,expires_at:new Date(Date.now()+3500000).toISOString(),repository_selection:'selected'};}
  else if(options.method==='DELETE'){state.revoked++;return new Response(null,{status:state.revokeFailure?500:204});}
  else if(endpoint.startsWith('/installation/'))value=machine.repositories;else value=machine.machineRuleset;
  return new Response(JSON.stringify(value),{status:options.method==='POST'?201:200});
 };
 const options={service:{repository:repo.full_name,repositoryId:101,sourceObjectStore:f.directory,policy:'all-ready-same-repository-dev-prs'},config:machine.config,privateKeyFile,observerEnvironmentReference:'FIXTURE_OWNER',environment,command,session:settings=>withTrainAppSession({...settings,transport})};
 return {state,options,integration,scratch,select:()=>selectTrainServiceReadyInputs(options)};
}
test('real RSA selection uses three credential roles, fresh cheap grants and revokes before delegation',async t=>{
 const f=await setup(t),selected=await f.select();
 assert.equal(selected.kind,'selected');assert.equal(selected.selection.inputs.length,2);
 assert.ok(selected.selection.inputs.every(s=>s.admission.tier==='cheap-input-v1'));
 assert.equal(f.state.minted,1);assert.equal(f.state.revoked,1);
 assert.ok(f.state.appReads>=4&&f.state.ownerReads>=8&&f.state.machineReads>0);
 const serviceRoot=path.join(f.scratch,'service');await mkdir(serviceRoot,{mode:0o700});
 const configurationFile=path.join(f.scratch,'service.json');
 await writeFile(configurationFile,JSON.stringify({schema:'conduit.train-service/v1',...f.options.service,serviceRoot,maxBatches:2}),{mode:0o600});
 let delegated=0;
 const tick=()=>runTrainServiceTick({...f.options,configurationFile,identity:async()=>'a'.repeat(64),
   selectReady:()=>f.select(),supervisor:async()=>{delegated++;assert.equal(f.state.revoked,f.state.minted);return {stage:'new',paused:true};}});
 assert.equal((await tick()).stage,'selected');assert.equal(delegated,0);
 assert.equal((await tick()).stage,'current');assert.equal(delegated,1);
 assert.ok(!JSON.stringify(selected).includes('fixture-installation'));
});
test('pending development returns waiting and red development blocks, with revocation on both',async t=>{
 const f=await setup(t);f.integration.status='in_progress';f.integration.conclusion=null;
 assert.equal((await f.select()).kind,'waiting');assert.equal(f.state.revoked,1);
 f.integration.status='completed';f.integration.conclusion='failure';assert.equal((await f.select()).kind,'blocked');assert.equal(f.state.revoked,2);
});
test('session metadata errors and failed revocation propagate without credential diagnostics',async t=>{
 const f=await setup(t);f.state.metadataFailure=true;
 await assert.rejects(f.select(),error=>error instanceof TrainAppSessionError&&!error.message.includes('fixture-installation'));
 assert.equal(f.state.revoked,0,'owner identity failure refuses before mint');
 f.state.metadataFailure=false;f.state.revokeFailure=true;
 await assert.rejects(f.select(),error=>error instanceof TrainAppSessionError&&error.revocation==='failed'&&!error.message.includes('fixture-installation'));
 assert.equal(f.state.revoked,1);
});
test('missing observer refuses before stored human credentials or sessions',async t=>{
 const f=await setup(t);delete f.options.environment.FIXTURE_OWNER;
 await assert.rejects(f.select());assert.equal(f.state.minted,0);
});

test('target controller checkout cannot become source-object execution context',async t=>{
 const f=await setup(t);f.options.service.sourceObjectStore=fileURLToPath(new URL('../../',import.meta.url)).replace(/\/$/,'');
 await assert.rejects(f.select(),/separate/i);assert.equal(f.state.minted,0);assert.equal(f.state.ownerReads,0);
});
