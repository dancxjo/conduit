import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { assertSupportedPackage, snapshotInventory, prepareCompositorSnapshot } from '../../tools/ci/compiler-cache/cargo-snapshot.mjs';
import { cargoSandboxArgs, runCompositorCargoCache, proveOutsideReadRefused, clearCompositorCargoCache } from '../../tools/ci/compiler-cache/cargo-sandbox.mjs';
const rejected=(action,code)=>assert.rejects(action,error=>error.code===code);
async function temp(t){const root=await fs.mkdtemp(path.join(os.tmpdir(),'conduit-cargo-cache-'));t.after(()=>fs.rm(root,{recursive:true,force:true}));return root;}
const actualPackage=new URL('../../targets/conduitos/compositor-core/',import.meta.url);

test('only the actual reviewed dependency-free package contract is admitted',async t=>{
  const root=await temp(t);const manifest=await fs.readFile(new URL('Cargo.toml',actualPackage),'utf8');await fs.writeFile(path.join(root,'Cargo.toml'),manifest);
  await assertSupportedPackage(root);
  await fs.writeFile(path.join(root,'Cargo.toml'),manifest+'\n[dependencies]\nproc-macro-fixture = "1"\n');
  await rejected(()=>assertSupportedPackage(root),'unsupported-cargo-package-contract');
  await fs.writeFile(path.join(root,'Cargo.toml'),manifest);await fs.writeFile(path.join(root,'build.rs'),'fn main() {}');
  await rejected(()=>assertSupportedPackage(root),'unsupported-cargo-build-script');
});
test('snapshot inventories exact bytes and refuses symlink escape',async t=>{
  const root=await temp(t);await fs.writeFile(path.join(root,'source'),'one');const first=await snapshotInventory(root);
  await fs.writeFile(path.join(root,'source'),'two');assert.notEqual((await snapshotInventory(root)).sha256,first.sha256);
  await fs.symlink('/etc',path.join(root,'outside'));await rejected(()=>snapshotInventory(root),'cargo-snapshot-unsupported-file');
});
test('compiler server and Cargo launch are contained in the same finite mount plan',()=>{
  const args=cargoSandboxArgs({snapshotRoot:'/private/snapshot',taskRoot:'/private/task',cacheObjects:'/private/cache',namespace:'exact-inputs',maxCacheBytes:1024});
  assert.ok(args.includes('--unshare-all'));assert.ok(args.includes('--clearenv'));assert.ok(args.includes('--die-with-parent'));
  assert.deepEqual(args.slice(-2),['/bin/sh','/task/run.sh']);
  assert.ok(!args.includes('/etc'));assert.ok(!args.includes('/home'));assert.ok(!args.includes('--ro-bind-try'));
  assert.ok(args.indexOf('/private/snapshot/toolchain')>0);assert.ok(args.indexOf('/private/task')>0);
});
test('reserve refusal occurs before creating task output or spawning Cargo',async t=>{
  const root=await temp(t);const snapshotRoot=path.join(root,'snapshot');await fs.mkdir(snapshotRoot);await fs.mkdir(path.join(snapshotRoot,'source'));
  await fs.copyFile(new URL('Cargo.toml',actualPackage),path.join(snapshotRoot,'source/Cargo.toml'));
  const snapshot={schema:'compositor-cargo-snapshot-v1',root:snapshotRoot,...await snapshotInventory(snapshotRoot)};
  await rejected(()=>runCompositorCargoCache({snapshot,cacheRoot:path.join(root,'cache'),taskRoot:path.join(root,'task'),reserveBytes:Number.MAX_SAFE_INTEGER}),'cargo-reserve-admission');
  assert.equal((await fs.readdir(root)).includes('task'),false);assert.equal((await fs.readdir(path.join(root,'cache'))).includes('.cargo-lease'),false);
  await fs.writeFile(path.join(root,'cache/retirement.json'),'interrupted');
  await rejected(()=>runCompositorCargoCache({snapshot,cacheRoot:path.join(root,'cache'),taskRoot:path.join(root,'task'),reserveBytes:Number.MAX_SAFE_INTEGER}),'cargo-retirement-interrupted');
});
test('real Conduit Cargo cold/warm builds and compiler outside-read refusal',{
  skip:!process.env.CONDUIT_CARGO_CACHE_SNAPSHOT,
},async t=>{
  const snapshot=JSON.parse(await fs.readFile(process.env.CONDUIT_CARGO_CACHE_SNAPSHOT,'utf8'));
  const root=await temp(t);const cacheRoot=path.join(root,'cache');
  const cold=await runCompositorCargoCache({snapshot,cacheRoot,taskRoot:path.join(root,'cold')});
  const warm=await runCompositorCargoCache({snapshot,cacheRoot,taskRoot:path.join(root,'warm')});
  assert.equal(cold.after.stats.cache_misses.counts.Rust,1);assert.equal(warm.after.stats.cache_hits.counts.Rust,1);
  assert.deepEqual(warm.outputs,cold.outputs);assert.equal(warm.namespace,cold.namespace);
  assert.ok(warm.validatedWallMs>=warm.wallMs);assert.ok(cold.validatedWallMs>=cold.wallMs);
  assert.equal(warm.policy.bwrapSha256,warm.bwrapSha256);
  for(const field of ['sandboxArgsSha256','runnerSourceSha256'])assert.match(warm.policy[field],/^[a-f0-9]{64}$/);
  // An unleased server must still block retirement: no open object fd is
  // required while the engine is idle, so audit its actual cache identity.
  const socket=path.join(root,'unleased-server.sock');
  const config=path.join(root,'unleased-server.toml');await fs.writeFile(config,`[cache.disk]\ndir = ${JSON.stringify(path.join(cacheRoot,'objects'))}\nsize = 16777216\n`);
  const server=spawn(path.join(snapshot.root,'engine/sccache'),[],{env:{PATH:'/usr/bin:/bin',HOME:root,SCCACHE_CONF:config,SCCACHE_DIR:path.join(cacheRoot,'objects'),SCCACHE_SERVER_UDS:socket,SCCACHE_START_SERVER:'1',SCCACHE_NO_DAEMON:'1',SCCACHE_IDLE_TIMEOUT:'0'},stdio:'ignore'});
  const exited=new Promise(resolve=>{server.once('error',resolve);server.once('close',resolve);});
  try{
    let ready=false;for(let attempt=0;attempt<100;attempt++){try{await fs.lstat(socket);ready=true;break;}catch(error){if(error.code!=='ENOENT')throw error;}await new Promise(resolve=>setTimeout(resolve,20));}
    assert.equal(ready,true);
    await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace:warm.namespace}),'cargo-cache-server-active');
  }finally{if(server.exitCode===null&&server.signalCode===null)server.kill('SIGKILL');await exited;}
  const cleared=await clearCompositorCargoCache({cacheRoot,expectedNamespace:warm.namespace});
  assert.ok(cleared.files.length>0);assert.ok(cleared.payloadBytes>0);
  assert.ok((await fs.stat(path.join(root,'warm/cargo-cache-receipt.json'))).isFile());
  const afterClear=await runCompositorCargoCache({snapshot,cacheRoot,taskRoot:path.join(root,'after-clear')});
  assert.equal(afterClear.after.stats.cache_misses.counts.Rust,1);assert.deepEqual(afterClear.outputs,cold.outputs);
  const hostOnlyFile=path.join(root,'host-only');await fs.writeFile(hostOnlyFile,'existing host data');
  const refusal=await proveOutsideReadRefused({snapshot,cacheRoot,taskRoot:path.join(root,'negative'),hostOnlyFile});assert.notEqual(refusal.code,0);
});

test('active cache lease and changed source are refused without disturbing evidence',async t=>{
  const root=await temp(t);const snapshotRoot=path.join(root,'snapshot');await fs.mkdir(snapshotRoot);await fs.mkdir(path.join(snapshotRoot,'source'));
  await fs.copyFile(new URL('Cargo.toml',actualPackage),path.join(snapshotRoot,'source/Cargo.toml'));
  const snapshot={schema:'compositor-cargo-snapshot-v1',root:snapshotRoot,...await snapshotInventory(snapshotRoot)};
  const cacheRoot=path.join(root,'cache');await fs.mkdir(cacheRoot);const lease=path.join(cacheRoot,'.cargo-lease');await fs.mkdir(lease);await fs.writeFile(path.join(lease,'owner-proof'),'active owner');
  await rejected(()=>runCompositorCargoCache({snapshot,cacheRoot,taskRoot:path.join(root,'task')}),'cargo-cache-busy');
  assert.equal(await fs.readFile(path.join(lease,'owner-proof'),'utf8'),'active owner');
  await fs.writeFile(path.join(snapshotRoot,'source/README.md'),'changed snapshot');
  await rejected(()=>runCompositorCargoCache({snapshot,cacheRoot:path.join(root,'other-cache'),taskRoot:path.join(root,'other-task')}),'cargo-snapshot-mismatch');
  assert.equal((await fs.readdir(root)).includes('other-cache'),false);
});

test('snapshot preparation refuses recursively copying into its own source',async t=>{
  const root=await temp(t);const source=path.join(root,'source');await fs.mkdir(source);await fs.mkdir(path.join(source,'src'));await fs.mkdir(path.join(root,'toolchain'));await fs.writeFile(path.join(root,'engine'),'placeholder');
  await fs.copyFile(new URL('Cargo.toml',actualPackage),path.join(source,'Cargo.toml'));
  await rejected(()=>prepareCompositorSnapshot({packageRoot:source,toolchainRoot:path.join(root,'toolchain'),engine:path.join(root,'engine'),destination:path.join(source,'src','snapshot')}),'overlapping-cargo-snapshot');
  assert.deepEqual(await fs.readdir(path.join(source,'src')),[]);
});

async function managedCacheFixture(t,{shared=false}={}) {
  const root=await temp(t);const cacheRoot=path.join(root,'cache');await fs.mkdir(cacheRoot);
  const policy={schema:shared?3:2,...(shared?{cacheMode:'audited-dependencies',compatibilitySha256:'a'.repeat(64)}:{snapshotSha256:'a'.repeat(64)}),runnerSha256:'b'.repeat(64),sandboxArgsSha256:'c'.repeat(64),runnerSourceSha256:'d'.repeat(64),bwrapSha256:'e'.repeat(64),maxCacheBytes:1024};
  await fs.writeFile(path.join(cacheRoot,'policy.json'),JSON.stringify(policy));
  const object='ab'+'0'.repeat(62);const directory=path.join(cacheRoot,'objects/a/b');await fs.mkdir(directory,{recursive:true});
  const objectPath=path.join(directory,object);await fs.writeFile(objectPath,'regenerable engine object');
  return {root,cacheRoot,objectPath,expectedNamespace:createHash('sha256').update(JSON.stringify(policy)).digest('hex')};
}
test('engine clear preserves active lease, unmanaged content and symlink objects',async t=>{
  const fixture=await managedCacheFixture(t);const {cacheRoot,expectedNamespace,objectPath}=fixture;
  const lease=path.join(cacheRoot,'.cargo-lease');await fs.mkdir(lease);await fs.writeFile(path.join(lease,'owner'),'active');
  await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace}),'cargo-cache-busy');
  assert.equal(await fs.readFile(path.join(lease,'owner'),'utf8'),'active');await fs.rm(lease,{recursive:true});
  await fs.writeFile(path.join(cacheRoot,'unique-proof'),'preserve');
  await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace}),'unmanaged-cargo-cache');
  assert.equal(await fs.readFile(path.join(cacheRoot,'unique-proof'),'utf8'),'preserve');await fs.unlink(path.join(cacheRoot,'unique-proof'));
  await fs.unlink(objectPath);await fs.symlink('/etc/passwd',objectPath);
  await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace}),'unsafe-cargo-cache-object');
  assert.ok((await fs.lstat(objectPath)).isSymbolicLink());
});
test('engine clear refuses open object descriptors and leaves task evidence intact',async t=>{
  const {root,cacheRoot,expectedNamespace,objectPath}=await managedCacheFixture(t);const proof=path.join(root,'task-proof');await fs.writeFile(proof,'unique output');
  const fd=await fs.open(objectPath,'r');
  try{await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace}),'cargo-cache-files-active');}finally{await fd.close();}
  const receipt=await clearCompositorCargoCache({cacheRoot,expectedNamespace});assert.equal(receipt.files.length,1);
  assert.deepEqual(await fs.readdir(path.join(cacheRoot,'objects')),[]);assert.equal(await fs.readFile(proof,'utf8'),'unique output');
});
test('interrupted retirement markers fail closed and wrong namespace cannot clear',async t=>{
  const {cacheRoot,expectedNamespace,objectPath}=await managedCacheFixture(t);
  await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace:'f'.repeat(64)}),'cargo-clear-policy-mismatch');
  await fs.writeFile(path.join(cacheRoot,'retirement.json'),'retained interrupted intent');
  await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace}),'cargo-retirement-interrupted');
  assert.equal(await fs.readFile(objectPath,'utf8'),'regenerable engine object');
  assert.equal(await fs.readFile(path.join(cacheRoot,'retirement.json'),'utf8'),'retained interrupted intent');
});

test('shared compatibility policy clears only managed objects and refuses an unknown cache mode',async t=>{
  const {cacheRoot,expectedNamespace,objectPath}=await managedCacheFixture(t,{shared:true});
  const marker=path.join(cacheRoot,'policy.json');const original=await fs.readFile(marker,'utf8');
  const unsupported=JSON.parse(original);unsupported.cacheMode='arbitrary-source';await fs.writeFile(marker,JSON.stringify(unsupported));
  await rejected(()=>clearCompositorCargoCache({cacheRoot,expectedNamespace:createHash('sha256').update(JSON.stringify(unsupported)).digest('hex')}),'cargo-clear-policy-mismatch');
  assert.equal(await fs.readFile(objectPath,'utf8'),'regenerable engine object');await fs.writeFile(marker,original);
  const receipt=await clearCompositorCargoCache({cacheRoot,expectedNamespace});assert.equal(receipt.policy.schema,3);assert.equal(receipt.files.length,1);
  assert.deepEqual(await fs.readdir(path.join(cacheRoot,'objects')),[]);
});
