import { promises as fs } from 'node:fs';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { CacheRefusal } from './refusal.mjs';
import { hashFile, snapshotInventory, assertSupportedPackage } from './cargo-snapshot.mjs';
import { SCCACHE_ENGINE } from './engine.mjs';

const fail = code => { throw new CacheRefusal(code); };
const sha = value => createHash('sha256').update(value).digest('hex');
export const CARGO_SANDBOX_SCRIPT = `#!/bin/sh
set -eu
export SCCACHE_NO_DAEMON=1 SCCACHE_SERVER_UDS=/task/sccache.sock SCCACHE_DIR=/cache SCCACHE_CONF=/task/sccache.toml SCCACHE_DIRECT=false SCCACHE_IDLE_TIMEOUT=0
SCCACHE_START_SERVER=1 /engine/sccache &
server=$!
trap 'kill -KILL "$server" 2>/dev/null || :; wait "$server" 2>/dev/null || :' EXIT HUP INT TERM
n=0
while [ ! -S /task/sccache.sock ]; do
  kill -0 "$server"
  n=$((n+1)); [ "$n" -lt 100 ]; /usr/bin/sleep 0.05
done
/engine/sccache --show-stats --stats-format json > /task/before.json
/toolchain/bin/cargo build --frozen --offline --manifest-path /source/Cargo.toml --target-dir /task/target --lib -j 1
/engine/sccache --show-stats --stats-format json > /task/after.json
/engine/sccache --stop-server > /task/stopped.txt
wait "$server"
trap - EXIT HUP INT TERM
`;
async function canonicalDestination(value) {
  let existing = path.resolve(value); const missing=[];
  for (;;) { try { return path.join(await fs.realpath(existing),...missing.reverse()); }
    catch(error) { if(error.code!=='ENOENT')throw error; const parent=path.dirname(existing); if(parent===existing)throw error; missing.push(path.basename(existing)); existing=parent; }
  }
}
async function boundedBytes(root) {
  let total=0,count=0;
  async function walk(directory) { for(const item of await fs.readdir(directory,{withFileTypes:true})) {
    if(++count>100000)fail('cargo-runtime-file-bound'); const file=path.join(directory,item.name);
    if(item.isDirectory())await walk(file);
    else if(item.isFile())total+=(await fs.lstat(file)).size;
    else if(!item.isSocket())fail('cargo-runtime-unsupported-file');
  } }
  await walk(root); return total;
}
function runSandbox(args,{onTick,timeoutMs=120000}={}) {
  return new Promise((resolve,reject)=>{
    const child=spawn('/usr/bin/bwrap',args,{detached:true,stdio:['ignore','pipe','pipe'],env:{PATH:'/usr/bin:/bin',LANG:'C'}});
    const started=performance.now(); let failure; let logBytes=0;const stdout=[],stderr=[];let checking=false;
    const kill=()=>{if(child.exitCode===null&&child.signalCode===null){try{process.kill(-child.pid,'SIGKILL');}catch{}}};
    const timer=setTimeout(()=>{failure=new CacheRefusal('cargo-timeout');kill();},timeoutMs);
    const monitor=setInterval(()=>{if(checking||!onTick)return;checking=true;onTick().catch(error=>{failure=error;kill();}).finally(()=>{checking=false;});},100);
    for(const [stream,chunks]of [[child.stdout,stdout],[child.stderr,stderr]])stream.on('data',bytes=>{logBytes+=bytes.length;if(logBytes>1024*1024){failure=new CacheRefusal('cargo-log-bound');kill();}else chunks.push(bytes);});
    child.once('error',error=>{failure=error;});
    child.once('close',async(code,signal)=>{
      clearInterval(monitor);clearTimeout(timer);
      while(checking)await new Promise(resolve=>setTimeout(resolve,10));
      const result={code,signal,wallMs:performance.now()-started,stdout:Buffer.concat(stdout).toString(),stderr:Buffer.concat(stderr).toString()};
      if(failure)reject(Object.assign(failure,{result}));
      else if(code!==0)reject(Object.assign(new CacheRefusal('cargo-sandbox-failed'),{result}));
      else resolve(result);
    });
  });
}
export function cargoSandboxArgs({snapshotRoot,taskRoot,cacheObjects,namespace,maxCacheBytes,mode='cargo'}) {
  const args=['--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv',
    '--ro-bind',path.join(snapshotRoot,'source'),'/source',
    '--ro-bind',path.join(snapshotRoot,'toolchain'),'/toolchain',
    '--ro-bind',path.join(snapshotRoot,'engine'),'/engine',
    '--ro-bind',path.join(snapshotRoot,'runtime/usr'),'/usr',
    '--ro-bind',path.join(snapshotRoot,'runtime/bin'),'/bin',
    '--symlink','usr/lib','/lib','--symlink','usr/lib64','/lib64',
    '--proc','/proc','--dev','/dev','--bind',taskRoot,'/task','--bind',cacheObjects,'/cache',
    '--chdir','/source',
    '--setenv','PATH','/toolchain/bin:/usr/bin:/bin','--setenv','HOME','/task/home','--setenv','CARGO_HOME','/task/home',
    '--setenv','TMPDIR','/task/tmp','--setenv','LANG','C','--setenv','CARGO_INCREMENTAL','0',
    '--setenv','RUSTC','/toolchain/bin/rustc','--setenv','RUSTC_WRAPPER','/engine/sccache',
    '--setenv','CARGO_CONDUIT_CACHE_INPUTS_SHA256',namespace,'--setenv','SCCACHE_CACHE_SIZE',String(maxCacheBytes)];
  if(mode==='cargo')return [...args,'/bin/sh','/task/run.sh'];
  if(mode==='negative-read')return [...args,'/toolchain/bin/rustc','/task/negative.rs','--crate-name','outside_read','--crate-type=rlib','--out-dir','/task/negative-output'];
  fail('unsupported-cargo-mode');
}

// Supported vertical slice: actual dependency-free compositor-core Cargo build.
// The entire compiler server and its children are inside the same bwrap root;
// no host daemon can silently perform compilation outside the snapshot.
export async function runCompositorCargoCache({snapshot,cacheRoot,taskRoot,maxCacheBytes=16*1024*1024,maxTaskBytes=64*1024*1024,reserveBytes=1024*1024*1024}) {
  const started=performance.now();
  if(snapshot?.schema!=='compositor-cargo-snapshot-v1'||![snapshot.root,cacheRoot,taskRoot].every(value=>typeof value==='string'&&path.isAbsolute(value))||[maxCacheBytes,maxTaskBytes,reserveBytes].some(value=>!Number.isSafeInteger(value)||value<=0))fail('invalid-cargo-cache-policy');
  const [sourceRoot,cache,task]=await Promise.all([fs.realpath(snapshot.root),canonicalDestination(cacheRoot),canonicalDestination(taskRoot)]);
  if([sourceRoot,cache,task].some((first,index,roots)=>roots.some((second,other)=>index!==other&&(first===second||first.startsWith(`${second}/`)))))fail('overlapping-cargo-roots');
  await assertSupportedPackage(path.join(sourceRoot,'source'));
  const input=await snapshotInventory(sourceRoot);if(input.sha256!==snapshot.sha256)fail('cargo-snapshot-mismatch');
  const policy={schema:2,snapshotSha256:input.sha256,runnerSha256:sha(CARGO_SANDBOX_SCRIPT),sandboxArgsSha256:sha(cargoSandboxArgs.toString()),runnerSourceSha256:sha(runSandbox.toString()),bwrapSha256:await hashFile('/usr/bin/bwrap'),maxCacheBytes};
  const namespace=sha(JSON.stringify(policy));
  await fs.mkdir(cache,{recursive:true,mode:0o700});
  const lease=path.join(cache,'.cargo-lease');
  try{await fs.mkdir(lease,{mode:0o700});}catch(error){if(error.code==='EEXIST')fail('cargo-cache-busy');throw error;}
  try {
    await fs.writeFile(path.join(lease,'owner.json'),JSON.stringify({pid:process.pid,bootId:(await fs.readFile('/proc/sys/kernel/random/boot_id','utf8')).trim(),procStat:await fs.readFile(`/proc/${process.pid}/stat`,'utf8')}),{flag:'wx',mode:0o600});
    const marker=path.join(cache,'policy.json');
    try{if(await fs.readFile(marker,'utf8')!==JSON.stringify(policy))fail('cargo-cache-policy-mismatch');}
    catch(error){if(error.code!=='ENOENT')throw error;if((await fs.readdir(cache)).some(name=>name!=='.cargo-lease'))fail('unmanaged-cargo-cache');await fs.writeFile(marker,JSON.stringify(policy),{flag:'wx',mode:0o600});}
    const names=await fs.readdir(cache);
    if(names.some(name=>name==='retirement.json'||name.startsWith('.retiring-')))fail('cargo-retirement-interrupted');
    if(names.some(name=>!['.cargo-lease','policy.json','objects'].includes(name)))fail('unmanaged-cargo-cache');
    const objects=path.join(cache,'objects');await fs.mkdir(objects,{recursive:true,mode:0o700});
    if((await fs.lstat(objects)).isSymbolicLink())fail('unsafe-cargo-cache');
    const free=await fs.statfs(cache,{bigint:true});if(free.bavail*free.bsize<BigInt(reserveBytes)+BigInt(maxCacheBytes)+BigInt(maxTaskBytes))fail('cargo-reserve-admission');
    if(await hashFile(path.join(sourceRoot,'engine/sccache'))!==SCCACHE_ENGINE.binarySha256)fail('engine-binary-mismatch');
    await fs.mkdir(task,{mode:0o700});for(const name of ['home','tmp'])await fs.mkdir(path.join(task,name));
    await fs.writeFile(path.join(task,'run.sh'),CARGO_SANDBOX_SCRIPT,{flag:'wx',mode:0o400});
    await fs.writeFile(path.join(task,'sccache.toml'),`[cache.disk]\ndir = "/cache"\nsize = ${maxCacheBytes}\n`,{flag:'wx',mode:0o400});
    const check=async()=>{
      const stat=await fs.statfs(cache,{bigint:true});if(stat.bavail*stat.bsize<BigInt(reserveBytes))fail('cargo-reserve-lost');
      if(await boundedBytes(objects)>maxCacheBytes)fail('cargo-cache-bound');if(await boundedBytes(task)>maxTaskBytes)fail('cargo-task-bound');
    };
    await check();
    const args=cargoSandboxArgs({snapshotRoot:sourceRoot,taskRoot:task,cacheObjects:objects,namespace,maxCacheBytes});
    let result;
    try{result=await runSandbox(args,{onTick:check});}
    catch(error){await fs.writeFile(path.join(task,'failure.json'),JSON.stringify({code:error.code,result:error.result},null,2),{flag:'wx',mode:0o600});throw error;}
    await check();if((await snapshotInventory(sourceRoot)).sha256!==input.sha256)fail('cargo-snapshot-mutated');
    const before=JSON.parse(await fs.readFile(path.join(task,'before.json'),'utf8'));
    const after=JSON.parse(await fs.readFile(path.join(task,'after.json'),'utf8'));
    const outputs=[];const dependencies=path.join(task,'target/debug/deps');
    for(const name of (await fs.readdir(dependencies)).sort())if(name.endsWith('.rlib')||name.endsWith('.rmeta'))outputs.push({name,sha256:await hashFile(path.join(dependencies,name)),bytes:(await fs.stat(path.join(dependencies,name))).size});
    const receipt={schema:1,scope:'Actual compositor-core Cargo --lib build only; no full workspace, link/test/release or arbitrary procedural-macro proof',namespace,snapshotSha256:input.sha256,generatedInputs:{lockfileSha256:await hashFile(path.join(sourceRoot,'source/Cargo.lock')),buildScriptOutputs:[]},policy,compilerBoundary:'Owned server and rustc children inside bwrap --unshare-all with snapshot mounts read-only',args,environment:CARGO_SANDBOX_SCRIPT,bwrapSha256:policy.bwrapSha256,before,after,outputs,cacheBytes:await boundedBytes(objects),taskBytes:await boundedBytes(task),validatedWallMs:performance.now()-started,...result};
    await fs.writeFile(path.join(task,'cargo-cache-receipt.json'),JSON.stringify(receipt,null,2),{flag:'wx',mode:0o600});return receipt;
  } finally{await fs.rm(lease,{recursive:true});}
}

export async function proveOutsideReadRefused({snapshot,taskRoot,cacheRoot,hostOnlyFile}) {
  const sourceRoot=await fs.realpath(snapshot.root);const task=await canonicalDestination(taskRoot);
  if(!path.isAbsolute(hostOnlyFile)||hostOnlyFile.includes('"')||hostOnlyFile.includes('\\'))fail('invalid-outside-read-path');
  await fs.mkdir(task,{mode:0o700});await fs.mkdir(path.join(task,'negative-output'));await fs.mkdir(path.join(task,'home'));await fs.mkdir(path.join(task,'tmp'));
  const secret=await fs.readFile(hostOnlyFile);if(!secret.length)fail('missing-outside-read-fixture');
  await fs.writeFile(path.join(task,'negative.rs'),`pub const OUTSIDE: &str = include_str!("${hostOnlyFile}");\n`,{flag:'wx',mode:0o400});
  const args=cargoSandboxArgs({snapshotRoot:sourceRoot,taskRoot:task,cacheObjects:path.join(cacheRoot,'objects'),namespace:snapshot.sha256,maxCacheBytes:1024*1024,mode:'negative-read'});
  try{await runSandbox(args);fail('outside-read-unexpected-success');}
  catch(error){if(error.code!=='cargo-sandbox-failed'||!error.result?.stderr.includes('No such file or directory'))throw error;const receipt={schema:1,scope:'Actual rustc include_str! cannot read existing host-only file outside sandbox mounts',hostOnlyFileSha256:sha(secret),args,...error.result};await fs.writeFile(path.join(task,'outside-read-refusal.json'),JSON.stringify(receipt,null,2),{flag:'wx',mode:0o600});return receipt;}
}

async function retirementInventory(objects) {
  const entries=[];let payloadBytes=0,allocatedBytes=0;
  const rootStat=await fs.lstat(objects);
  if(!rootStat.isDirectory()||rootStat.isSymbolicLink()||rootStat.uid!==process.getuid())fail('unsafe-cargo-cache');
  const identities=new Set([`${rootStat.dev}:${rootStat.ino}`]);
  const directories=[];
  async function walk(relative='') {
    for(const name of(await fs.readdir(path.join(objects,relative))).sort()) {
      if(entries.length+directories.length>=100000)fail('cargo-retirement-inventory-bound');
      const rel=path.posix.join(relative,name);const file=path.join(objects,rel);const stat=await fs.lstat(file);
      if(stat.uid!==process.getuid())fail('unsafe-cargo-cache-owner');
      identities.add(`${stat.dev}:${stat.ino}`);
      if(stat.isDirectory()&&!stat.isSymbolicLink()) {
        if(!/^[a-f0-9](\/[a-f0-9])?$/.test(rel))fail('unmanaged-cargo-object-layout');
        directories.push({path:rel,dev:stat.dev,ino:stat.ino});await walk(rel);
      } else if(stat.isFile()&&stat.nlink===1) {
        const match=/^([a-f0-9])\/([a-f0-9])\/([a-f0-9]{64})$/.exec(rel);
        if(!match||!match[3].startsWith(match[1]+match[2]))fail('unmanaged-cargo-object-layout');
        payloadBytes+=stat.size;allocatedBytes+=stat.blocks*512;
        entries.push({path:rel,bytes:stat.size,allocatedBytes:stat.blocks*512,sha256:await hashFile(file),dev:stat.dev,ino:stat.ino});
      } else fail('unsafe-cargo-cache-object');
    }
  }
  await walk();return {entries,directories,payloadBytes,allocatedBytes,identities};
}
async function assertInactiveEngineObjects(objects,identities) {
  const root=await fs.lstat(objects);const opaqueNonEngineProcesses=[];let inspectedProcesses=0;
  for(const name of await fs.readdir('/proc')) {
    if(!/^[0-9]+$/.test(name))continue;
    const processRoot=`/proc/${name}`;
    try {
      if((await fs.stat(processRoot)).uid!==process.getuid())continue;
      const cmdline=await fs.readFile(`${processRoot}/cmdline`,'utf8');
      const engine=path.basename(cmdline.split('\0')[0])==='sccache';
      if(engine) {
        try {
          const mounted=await fs.stat(`${processRoot}/root/cache`);
          if(mounted.dev===root.dev&&mounted.ino===root.ino)fail('cargo-cache-server-active');
        } catch(error) { if(!['ENOENT','ESRCH'].includes(error.code))throw error; }
        const env=(await fs.readFile(`${processRoot}/environ`,'utf8')).split('\0');
        const setting=env.find(value=>value.startsWith('SCCACHE_DIR='))?.slice('SCCACHE_DIR='.length);
        if(setting&&path.isAbsolute(setting)) {
          try {const cache=await fs.stat(setting);if(cache.dev===root.dev&&cache.ino===root.ino)fail('cargo-cache-server-active');}
          catch(error){if(!['ENOENT','ESRCH'].includes(error.code))throw error;}
        }
      }
      let fds;
      try{fds=await fs.readdir(`${processRoot}/fd`);inspectedProcesses++;}
      catch(error){
        if(error.code==='EACCES'&&!engine&&!cmdline.includes(objects)){opaqueNonEngineProcesses.push(Number(name));continue;}
        if(error.code==='EACCES')fail('cargo-cache-owner-audit-incomplete');throw error;
      }
      for(const fd of fds) {
        try {const stat=await fs.stat(`${processRoot}/fd/${fd}`);if(identities.has(`${stat.dev}:${stat.ino}`))fail('cargo-cache-files-active');}
        catch(error){if(!['ENOENT','ESRCH','EACCES'].includes(error.code))throw error;}
      }
    } catch(error) {if(!['ENOENT','ESRCH'].includes(error.code))throw error;}
  }
  return {inspectedProcesses,opaqueNonEngineProcesses};
}
async function syncDirectory(directory) {const fd=await fs.open(directory,'r');try{await fd.sync();}finally{await fd.close();}}

// Explicit retirement of regenerable engine objects only. The namespace from
// a prior build receipt prevents selecting a different managed cache by mistake.
// No stale/dead lease is broken. A persisted retirement intent blocks all later
// build/clear attempts after interruption; recovery requires an operator audit.
export async function clearCompositorCargoCache({cacheRoot,expectedNamespace}) {
  if(typeof cacheRoot!=='string'||!path.isAbsolute(cacheRoot)||!/^[a-f0-9]{64}$/.test(expectedNamespace))fail('invalid-cargo-clear-policy');
  const cache=await fs.realpath(cacheRoot);const lease=path.join(cache,'.cargo-lease');
  try{await fs.mkdir(lease,{mode:0o700});}catch(error){if(error.code==='EEXIST')fail('cargo-cache-busy');throw error;}
  try {
    await fs.writeFile(path.join(lease,'owner.json'),JSON.stringify({pid:process.pid,operation:'explicit-engine-clear',procStat:await fs.readFile(`/proc/${process.pid}/stat`,'utf8')}),{flag:'wx',mode:0o600});
    const names=await fs.readdir(cache);
    if(names.some(name=>name==='retirement.json'||name.startsWith('.retiring-')))fail('cargo-retirement-interrupted');
    if(names.some(name=>!['.cargo-lease','policy.json','objects'].includes(name))||!names.includes('objects'))fail('unmanaged-cargo-cache');
    const marker=path.join(cache,'policy.json');const stat=await fs.lstat(marker);
    if(!stat.isFile()||stat.nlink!==1||stat.uid!==process.getuid()||stat.size>16384)fail('unsafe-cargo-cache-policy');
    const policy=JSON.parse(await fs.readFile(marker,'utf8'));
    const fields=['schema','snapshotSha256','runnerSha256','sandboxArgsSha256','runnerSourceSha256','bwrapSha256','maxCacheBytes'];
    if(Object.keys(policy).sort().join()!==fields.sort().join()||policy.schema!==2||!Number.isSafeInteger(policy.maxCacheBytes)||policy.maxCacheBytes<=0||fields.filter(key=>key.endsWith('Sha256')).some(key=>!/^[a-f0-9]{64}$/.test(policy[key]))||sha(JSON.stringify(policy))!==expectedNamespace)fail('cargo-clear-policy-mismatch');
    const objects=path.join(cache,'objects');const inventory=await retirementInventory(objects);
    const processAudit=await assertInactiveEngineObjects(objects,inventory.identities);
    const tombstone=`.retiring-${randomUUID()}`;const intent=path.join(cache,'retirement.json');
    const receipt={schema:1,operation:'explicit-engine-object-clear',namespace:expectedNamespace,policy,processAudit,files:inventory.entries,payloadBytes:inventory.payloadBytes,allocatedBytes:inventory.allocatedBytes};
    const fd=await fs.open(intent,'wx',0o600);try{await fd.writeFile(JSON.stringify({schema:1,operation:'engine-retirement-in-progress',namespace:expectedNamespace,tombstone,inventorySha256:sha(JSON.stringify(inventory.entries)),fileCount:inventory.entries.length,payloadBytes:inventory.payloadBytes}));await fd.sync();}finally{await fd.close();}
    await syncDirectory(cache);
    const retired=path.join(cache,tombstone);await fs.rename(objects,retired);await syncDirectory(cache);
    const verified=await retirementInventory(retired);
    if(JSON.stringify(verified.entries)!==JSON.stringify(inventory.entries)||JSON.stringify(verified.directories)!==JSON.stringify(inventory.directories))fail('cargo-retirement-inventory-changed');
    await assertInactiveEngineObjects(retired,verified.identities);
    // Delete exactly the verified files and directories. Unexpected new content
    // makes rmdir fail and leaves the durable intent for audited recovery.
    for(const file of verified.entries) {
      const destination=path.join(retired,file.path);const current=await fs.lstat(destination);
      if(!current.isFile()||current.nlink!==1||current.dev!==file.dev||current.ino!==file.ino||current.size!==file.bytes||await hashFile(destination)!==file.sha256)fail('cargo-retirement-inventory-changed');
      await fs.unlink(destination);
    }
    for(const directory of [...verified.directories].sort((a,b)=>b.path.length-a.path.length||b.path.localeCompare(a.path)))await fs.rmdir(path.join(retired,directory.path));
    await fs.rmdir(retired);await fs.mkdir(objects,{mode:0o700});await syncDirectory(cache);
    await fs.unlink(intent);await syncDirectory(cache);return receipt;
  } finally {await fs.rm(lease,{recursive:true});}
}
