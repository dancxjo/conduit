import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { acquirePinnedEngine, SCCACHE_ENGINE } from '../../tools/ci/compiler-cache/engine.mjs';
import { CacheRefusal } from '../../tools/ci/compiler-cache/refusal.mjs';

async function fixture(t) {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'conduit-engine-acquire-'));
  t.after(()=>fs.rm(root,{recursive:true,force:true}));
  return {root,destination:path.join(root,'new-engine')};
}
async function withFetch(fetcher,action) {
  const original=globalThis.fetch;globalThis.fetch=fetcher;
  try{return await action();}finally{globalThis.fetch=original;}
}
const refuses=(action,code)=>assert.rejects(action,error=>error instanceof CacheRefusal&&error.code===code);

test('acquisition refuses relative destinations before network access',async()=>{
  await withFetch(()=>{throw new Error('network must not be called');},()=>refuses(()=>acquirePinnedEngine('relative-engine'),'unsupported-engine-platform'));
});
test('acquisition preserves an existing destination and never overwrites its files',async t=>{
  const {destination}=await fixture(t);await fs.mkdir(destination);await fs.writeFile(path.join(destination,'proof'),'unique evidence');
  await withFetch(()=>{throw new Error('network must not be called');},()=>assert.rejects(()=>acquirePinnedEngine(destination),error=>error.code==='EEXIST'));
  assert.equal(await fs.readFile(path.join(destination,'proof'),'utf8'),'unique evidence');
  assert.deepEqual(await fs.readdir(destination),['proof']);
});
test('HTTP failure refuses before archive or executable publication',async t=>{
  const {destination}=await fixture(t);
  await withFetch(async(url,options)=>{
    assert.equal(url,SCCACHE_ENGINE.url);assert.ok(options.signal instanceof AbortSignal);
    return new Response('unavailable',{status:503});
  },()=>refuses(()=>acquirePinnedEngine(destination),'engine-download-failed'));
  assert.deepEqual(await fs.readdir(destination),[]);
});
test('oversized downloads refuse without extracting or publishing executable bytes',async t=>{
  const {destination}=await fixture(t);
  await withFetch(async()=>new Response(new ReadableStream({start(controller){controller.enqueue(new Uint8Array(16*1024*1024+1));controller.close();}})),()=>refuses(()=>acquirePinnedEngine(destination),'engine-download-bound'));
  assert.deepEqual(await fs.readdir(destination),[]);
});
test('archive checksum mismatch refuses before extraction and executable publication',async t=>{
  const {destination}=await fixture(t);
  await withFetch(async()=>new Response('wrong archive bytes'),()=>refuses(()=>acquirePinnedEngine(destination),'engine-archive-mismatch'));
  assert.deepEqual(await fs.readdir(destination),[]);
});
test('already acquired pinned archive extracts exact executable without network download',{
  skip:!process.env.CONDUIT_COMPILER_CACHE_ENGINE_ARCHIVE,
},async t=>{
  const {root,destination}=await fixture(t);
  const archive=await fs.readFile(process.env.CONDUIT_COMPILER_CACHE_ENGINE_ARCHIVE);
  const binary=await withFetch(async()=>new Response(archive),()=>acquirePinnedEngine(destination));
  assert.equal(createHash('sha256').update(await fs.readFile(binary)).digest('hex'),SCCACHE_ENGINE.binarySha256);
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(destination,'acquisition.json'),'utf8')),SCCACHE_ENGINE);
  const result=spawnSync(binary,['--version'],{env:{PATH:'/usr/bin:/bin',HOME:root},encoding:'utf8',maxBuffer:4096,timeout:10000});
  assert.equal(result.status,0);assert.equal(result.stdout.trim(),`sccache ${SCCACHE_ENGINE.version}`);
});
