import { promises as fs, constants as fsConstants } from 'node:fs';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { CacheRefusal } from './refusal.mjs';
import { hashFile, snapshotInventory, assertSupportedPackage } from './cargo-snapshot.mjs';
import { SCCACHE_ENGINE } from './engine.mjs';
import { HOSTED_SNAPSHOT_LIMITS, proceduralMacroBypassNames } from './hosted-snapshot.mjs';
import { sharedCompatibilityInventory, verifyHostedAdmission } from './hosted-admission.mjs';

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
// Macro reexports can hide their dynamic artifact behind an ordinary rlib.
// Use the full resolver dependency closure, never only --extern suffixes.
export const HOSTED_RUSTC_WRAPPER = `#!/bin/sh
set -eu
compiler="$1"; shift
for arg in "$@"; do
  case "$arg" in @*) echo 'compiler-cache refuses response files' >&2; exit 64;; esac
done
for arg in "$@"; do
  case "$arg" in --print=*|-vV|--version) exec "$compiler" "$@";; esac
done
while IFS= read -r name; do
  if [ "$name" = "\${CARGO_PKG_NAME:-}" ]; then
    printf '%s\n' "$name" >> /task/proc-macro-bypass.log
    exec "$compiler" "$@"
  fi
done < /closure/proc-macro-bypass.txt
for arg in "$@"; do
  case "$arg" in proc-macro|--crate-type=*proc-macro*|*.so) echo 'unclassified procedural macro input' >&2; exit 64;; esac
done
exec /engine/sccache "$compiler" "$@"
`;
export const HOSTED_SHARED_RUSTC_WRAPPER = HOSTED_RUSTC_WRAPPER.replace('exec /engine/sccache', '. /closure/shared-admission.sh\nexec /engine/sccache');
export const HOSTED_METADATA_SCRIPT = '#!/bin/sh\nset -eu\n/toolchain/bin/cargo metadata --frozen --offline --config /closure/cargo-config.toml --manifest-path /source/Cargo.toml --format-version 1 > /task/resolved-metadata.json\n';
export const HOSTED_CARGO_SANDBOX_SCRIPT = CARGO_SANDBOX_SCRIPT.replace(
  '/toolchain/bin/cargo build --frozen --offline --manifest-path /source/Cargo.toml --target-dir /task/target --lib -j 1',
  '/toolchain/bin/cargo build --locked --frozen --offline --config /closure/cargo-config.toml --manifest-path /source/Cargo.toml --target-dir /task/target --release -p conduit --bin conduit -j 2');
async function canonicalDestination(value) {
  let existing = path.resolve(value); const missing=[];
  for (;;) { try { return path.join(await fs.realpath(existing),...missing.reverse()); }
    catch(error) { if(error.code!=='ENOENT')throw error; const parent=path.dirname(existing); if(parent===existing)throw error; missing.push(path.basename(existing)); existing=parent; }
  }
}
// Observe compiler temporaries without mistaking a normal unlink for a failed
// build. Directory FDs + O_NOFOLLOW also prevent following a swapped symlink
// out of the owned task/cache while accounting runs outside the sandbox.
export async function measureCargoOutputBytes(root) {
  let total=0,count=0;
  async function walk(directory,outer=false) {
    let fd;
    try {fd=await fs.open(directory,fsConstants.O_RDONLY|fsConstants.O_DIRECTORY|fsConstants.O_NOFOLLOW);}
    catch(error){if(error.code==='ENOENT'&&!outer)return;if(error.code==='ELOOP')fail('cargo-runtime-unsupported-file');throw error;}
    try{
      let entries;
      try{entries=await fs.readdir(`/proc/self/fd/${fd.fd}`,{withFileTypes:true});}
      catch(error){if(error.code==='ENOENT'&&!outer)return;throw error;}
      for(const item of entries){
        if(++count>100000)fail('cargo-runtime-file-bound');
        const file=`/proc/self/fd/${fd.fd}/${item.name}`;let stat;
        try{stat=await fs.lstat(file);}catch(error){if(error.code==='ENOENT')continue;throw error;}
        if(stat.isDirectory())await walk(file);
        else if(stat.isFile())total+=stat.size;
        else if(!stat.isSocket())fail('cargo-runtime-unsupported-file');
      }
    }finally{await fd.close();}
  }
  await walk(root,true);return total;
}
export function runSandbox(args,{onTick,timeoutMs=120000,monitorMs=100,resourcesFile,observeCommand}={}) {
  if(observeCommand!==undefined&&typeof observeCommand!=='function')fail('invalid-command-observer');
  return new Promise((resolve,reject)=>{
    const program=resourcesFile?'/usr/bin/time':'/usr/bin/bwrap';
    const actualArgs=resourcesFile?['-v','-o',resourcesFile,'/usr/bin/bwrap',...args]:[...args];
    const environment={PATH:'/usr/bin:/bin',LANG:'C'},cwd=process.cwd();
    const child=spawn(program,actualArgs,{detached:true,stdio:['ignore','pipe','pipe'],env:environment,cwd});
    const started=performance.now(); let failure; let logBytes=0;const stdout=[],stderr=[];let checking=false;
    const kill=()=>{if(child.exitCode===null&&child.signalCode===null){try{process.kill(-child.pid,'SIGKILL');}catch{}}};
    const timer=setTimeout(()=>{failure=new CacheRefusal('cargo-timeout');kill();},timeoutMs);
    const monitor=setInterval(()=>{if(checking||!onTick)return;checking=true;onTick().catch(error=>{failure=error;kill();}).finally(()=>{checking=false;});},monitorMs);
    for(const [stream,chunks]of [[child.stdout,stdout],[child.stderr,stderr]])stream.on('data',bytes=>{logBytes+=bytes.length;if(logBytes>1024*1024){failure=new CacheRefusal('cargo-log-bound');kill();}else chunks.push(bytes);});
    child.once('error',error=>{failure=error;});
    child.once('close',async(code,signal)=>{
      clearInterval(monitor);clearTimeout(timer);
      while(checking)await new Promise(resolve=>setTimeout(resolve,10));
      const result={code,signal,wallMs:performance.now()-started,stdout:Buffer.concat(stdout).toString(),stderr:Buffer.concat(stderr).toString()};
      if(observeCommand)try{
        const observation=observeCommand({program,args:actualArgs,cwd,environment,durationMs:result.wallMs,outcome:!failure&&code===0?'success':'failed',code,signal,...(failure?{failureCode:failure.code??'command-failed'}:{})});
        if(observation&&typeof observation.then==='function')throw new CacheRefusal('async-command-observer');
      }catch(error){failure=error;}
      if(failure)reject(Object.assign(failure,{result}));
      else if(code!==0)reject(Object.assign(new CacheRefusal('cargo-sandbox-failed'),{result}));
      else resolve(result);
    });
  });
}
export function cargoSandboxArgs({snapshotRoot,taskRoot,cacheObjects,namespace,maxCacheBytes,mode='cargo',hosted=false,sharedAdmission=false}) {
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
  if(hosted){
    args.push('--ro-bind',path.join(snapshotRoot,'vendor'),'/vendor',
      '--ro-bind',path.join(snapshotRoot,'git-vendor'),'/git-vendor',
      '--ro-bind',path.join(snapshotRoot,'cargo-config.toml'),'/closure/cargo-config.toml',
      '--ro-bind',path.join(snapshotRoot,'proc-macro-bypass.txt'),'/closure/proc-macro-bypass.txt',
      '--setenv','RUSTC_WRAPPER','/task/rustc-wrapper.sh');
  }
  if(sharedAdmission){if(!hosted)fail('unsupported-shared-cache-profile');args.push('--ro-bind',path.join(snapshotRoot,'shared-admission.sh'),'/closure/shared-admission.sh');}
  if(mode==='hosted-metadata'&&hosted)return [...args,'/bin/sh','/task/metadata.sh'];
  if(mode==='cargo')return [...args,'/bin/sh','/task/run.sh'];
  if(mode==='negative-read')return [...args,'/toolchain/bin/rustc','/task/negative.rs','--crate-name','outside_read','--crate-type=rlib','--out-dir','/task/negative-output'];
  fail('unsupported-cargo-mode');
}

async function deriveCargoPolicy(snapshot,input,{maxCacheBytes,cacheMode}) {
  const hosted=snapshot.schema==='hosted-linux-cargo-snapshot-v1';
  const frozenMode=hosted?(snapshot.cacheMode??'exact-snapshot'):'exact-snapshot';
  if(cacheMode!==frozenMode||!['exact-snapshot','audited-dependencies'].includes(frozenMode))fail('hosted-cache-mode-mismatch');
  const shared=hosted&&frozenMode==='audited-dependencies';
  if(shared)await verifyHostedAdmission(snapshot.root);
  const script=hosted?HOSTED_CARGO_SANDBOX_SCRIPT:CARGO_SANDBOX_SCRIPT;
  const wrapper=shared?HOSTED_SHARED_RUSTC_WRAPPER:HOSTED_RUSTC_WRAPPER;
  const policy={schema:shared?3:2,...(shared?{cacheMode:frozenMode,compatibilitySha256:sha(JSON.stringify(sharedCompatibilityInventory(input.files)))}:{snapshotSha256:input.sha256}),runnerSha256:sha(script+(hosted?wrapper+HOSTED_METADATA_SCRIPT:'')),sandboxArgsSha256:sha(cargoSandboxArgs.toString()),runnerSourceSha256:sha(runSandbox.toString()),bwrapSha256:await hashFile('/usr/bin/bwrap'),maxCacheBytes};
  return {policy,namespace:sha(JSON.stringify(policy))};
}
// Read-only pre-restore identity. Exact same derivation is used by the build;
// cache restoration never authorizes a different snapshot/task/source receipt.
export async function hostedCachePolicy(snapshot,{maxCacheBytes=4*1024**3,cacheMode=snapshot?.cacheMode??'exact-snapshot'}={}) {
  if(snapshot?.schema!=='hosted-linux-cargo-snapshot-v1'||!path.isAbsolute(snapshot.root??'')||!Number.isSafeInteger(maxCacheBytes)||maxCacheBytes<=0)fail('invalid-hosted-cache-policy');
  const input=await snapshotInventory(snapshot.root,HOSTED_SNAPSHOT_LIMITS);if(input.sha256!==snapshot.sha256)fail('cargo-snapshot-mismatch');
  return deriveCargoPolicy(snapshot,input,{maxCacheBytes,cacheMode});
}

// Supported vertical slice: actual dependency-free compositor-core Cargo build.
// The entire compiler server and its children are inside the same bwrap root;
// no host daemon can silently perform compilation outside the snapshot.
export async function runCompositorCargoCache({snapshot,cacheRoot,taskRoot,maxCacheBytes=16*1024*1024,maxTaskBytes=64*1024*1024,reserveBytes=1024*1024*1024,observeCommand}) {
  const started=performance.now();
  const hosted=snapshot?.schema==='hosted-linux-cargo-snapshot-v1';
  const sharedAdmission=hosted&&snapshot.cacheMode==='audited-dependencies';
  const inventoryOptions=hosted?HOSTED_SNAPSHOT_LIMITS:undefined;
  const script=hosted?HOSTED_CARGO_SANDBOX_SCRIPT:CARGO_SANDBOX_SCRIPT;
  if((!hosted&&snapshot?.schema!=='compositor-cargo-snapshot-v1')||![snapshot.root,cacheRoot,taskRoot].every(value=>typeof value==='string'&&path.isAbsolute(value))||[maxCacheBytes,maxTaskBytes,reserveBytes].some(value=>!Number.isSafeInteger(value)||value<=0))fail('invalid-cargo-cache-policy');
  const [sourceRoot,cache,task]=await Promise.all([fs.realpath(snapshot.root),canonicalDestination(cacheRoot),canonicalDestination(taskRoot)]);
  if([sourceRoot,cache,task].some((first,index,roots)=>roots.some((second,other)=>index!==other&&(first===second||first.startsWith(`${second}/`)))))fail('overlapping-cargo-roots');
  if(!hosted)await assertSupportedPackage(path.join(sourceRoot,'source'));
  const input=await snapshotInventory(sourceRoot,inventoryOptions);if(input.sha256!==snapshot.sha256)fail('cargo-snapshot-mismatch');
  await fs.mkdir(cache,{recursive:true,mode:0o700});
  const lease=path.join(cache,'.cargo-lease');
  try{await fs.mkdir(lease,{mode:0o700});}catch(error){if(error.code==='EEXIST')fail('cargo-cache-busy');throw error;}
  try {
    await fs.writeFile(path.join(lease,'owner.json'),JSON.stringify({pid:process.pid,bootId:(await fs.readFile('/proc/sys/kernel/random/boot_id','utf8')).trim(),procStat:await fs.readFile(`/proc/${process.pid}/stat`,'utf8')}),{flag:'wx',mode:0o600});
    // Ownership, interrupted retirement and reserve admission precede any
    // sandbox/tool lookup. Refusal is meaningful even on a host without bwrap.
    const names=await fs.readdir(cache);
    if(names.some(name=>name==='retirement.json'||name.startsWith('.retiring-')))fail('cargo-retirement-interrupted');
    if(names.some(name=>!['.cargo-lease','policy.json','objects'].includes(name)))fail('unmanaged-cargo-cache');
    const free=await fs.statfs(cache,{bigint:true});if(free.bavail*free.bsize<BigInt(reserveBytes)+BigInt(maxCacheBytes)+BigInt(maxTaskBytes))fail('cargo-reserve-admission');
    const taskFree=await fs.statfs(path.dirname(task),{bigint:true});if(taskFree.bavail*taskFree.bsize<BigInt(reserveBytes)+BigInt(maxTaskBytes))fail('cargo-reserve-admission');
    const {policy,namespace}=await deriveCargoPolicy(snapshot,input,{maxCacheBytes,cacheMode:hosted?(snapshot.cacheMode??'exact-snapshot'):'exact-snapshot'});
    const marker=path.join(cache,'policy.json');
    try{if(await fs.readFile(marker,'utf8')!==JSON.stringify(policy))fail('cargo-cache-policy-mismatch');}
    catch(error){if(error.code!=='ENOENT')throw error;if(names.some(name=>name!=='.cargo-lease'))fail('unmanaged-cargo-cache');await fs.writeFile(marker,JSON.stringify(policy),{flag:'wx',mode:0o600});}
    const objects=path.join(cache,'objects');await fs.mkdir(objects,{recursive:true,mode:0o700});
    if((await fs.lstat(objects)).isSymbolicLink())fail('unsafe-cargo-cache');
    if(await hashFile(path.join(sourceRoot,'engine/sccache'))!==SCCACHE_ENGINE.binarySha256)fail('engine-binary-mismatch');
    await fs.mkdir(task,{mode:0o700});for(const name of ['home','tmp'])await fs.mkdir(path.join(task,name));
    if(hosted){
      await fs.writeFile(path.join(task,'rustc-wrapper.sh'),sharedAdmission?HOSTED_SHARED_RUSTC_WRAPPER:HOSTED_RUSTC_WRAPPER,{flag:'wx',mode:0o500});
      await fs.writeFile(path.join(task,'metadata.sh'),HOSTED_METADATA_SCRIPT,{flag:'wx',mode:0o400});
    }
    await fs.writeFile(path.join(task,'run.sh'),script,{flag:'wx',mode:0o400});
    await fs.writeFile(path.join(task,'sccache.toml'),`[cache.disk]\ndir = "/cache"\nsize = ${maxCacheBytes}\n`,{flag:'wx',mode:0o400});
    const check=async()=>{
      const stat=await fs.statfs(cache,{bigint:true});if(stat.bavail*stat.bsize<BigInt(reserveBytes))fail('cargo-reserve-lost');
      const taskSpace=await fs.statfs(task,{bigint:true});if(taskSpace.bavail*taskSpace.bsize<BigInt(reserveBytes))fail('cargo-reserve-lost');
      if(await measureCargoOutputBytes(objects)>maxCacheBytes)fail('cargo-cache-bound');if(await measureCargoOutputBytes(task)>maxTaskBytes)fail('cargo-task-bound');
    };
    await check();
    if(hosted){
      try{await runSandbox(cargoSandboxArgs({snapshotRoot:sourceRoot,taskRoot:task,cacheObjects:objects,namespace,maxCacheBytes,hosted,sharedAdmission,mode:'hosted-metadata'}),{onTick:check,observeCommand});
      const metadataFile=path.join(task,'resolved-metadata.json');if((await fs.stat(metadataFile)).size>32*1024**2)fail('hosted-metadata-bound');
      const actualNames=proceduralMacroBypassNames(JSON.parse(await fs.readFile(metadataFile,'utf8')));
      const allowed=new Set((await fs.readFile(path.join(sourceRoot,'proc-macro-bypass.txt'),'utf8')).trimEnd().split('\n'));
      if(actualNames.some(name=>!allowed.has(name)))fail('hosted-macro-closure-mismatch');
      }catch(error){await fs.writeFile(path.join(task,'failure.json'),JSON.stringify({stage:'offline-metadata',code:error.code,message:error.message,path:error.path,result:error.result},null,2),{flag:'wx',mode:0o600});throw error;}
    }
    const args=cargoSandboxArgs({snapshotRoot:sourceRoot,taskRoot:task,cacheObjects:objects,namespace,maxCacheBytes,hosted,sharedAdmission});
    let result;
    try{result=await runSandbox(args,{onTick:check,timeoutMs:hosted?3600000:120000,monitorMs:hosted?1000:100,resourcesFile:hosted?path.join(task,'resource-usage.txt'):undefined,observeCommand});}
    catch(error){await fs.writeFile(path.join(task,'failure.json'),JSON.stringify({code:error.code,message:error.message,path:error.path,result:error.result},null,2),{flag:'wx',mode:0o600});throw error;}
    await check();if((await snapshotInventory(sourceRoot,inventoryOptions)).sha256!==input.sha256)fail('cargo-snapshot-mutated');
    const before=JSON.parse(await fs.readFile(path.join(task,'before.json'),'utf8'));
    const after=JSON.parse(await fs.readFile(path.join(task,'after.json'),'utf8'));
    const outputs=[];const dependencies=path.join(task,hosted?'target/release/deps':'target/debug/deps');
    for(const name of (await fs.readdir(dependencies)).sort())if(name.endsWith('.rlib')||name.endsWith('.rmeta'))outputs.push({name,sha256:await hashFile(path.join(dependencies,name)),bytes:(await fs.stat(path.join(dependencies,name))).size});
    if(hosted)outputs.push({name:'conduit',sha256:await hashFile(path.join(task,'target/release/conduit')),bytes:(await fs.stat(path.join(task,'target/release/conduit'))).size});
    const buildScriptOutputs=[];
    if(hosted)for(const name of await fs.readdir(path.join(task,'target/release/build'))){
      const out=path.join(task,'target/release/build',name,'out');
      try{buildScriptOutputs.push({packageDirectory:name,...await snapshotInventory(out,HOSTED_SNAPSHOT_LIMITS)});}
      catch(error){if(error.code!=='ENOENT')throw error;}
    }
    const receipt={schema:1,bounds:{maxCacheBytes,maxTaskBytes,reserveBytes},scope:hosted?'Actual hosted Linux release Cargo build; transitive procedural-macro consumers bypass cache, behavioral proof executes separately':'Actual compositor-core Cargo --lib build only; no full workspace, link/test/release or arbitrary procedural-macro proof',namespace,snapshotSha256:input.sha256,generatedInputs:{lockfileSha256:await hashFile(path.join(sourceRoot,'source/Cargo.lock')),buildScriptOutputs},policy,compilerBoundary:'Owned server and rustc children inside bwrap --unshare-all with snapshot mounts read-only',args,environment:script,bwrapSha256:policy.bwrapSha256,before,after,outputs,...(hosted?{resourceUsage:await fs.readFile(path.join(task,'resource-usage.txt'),'utf8'),procMacroBypasses:(await fs.readFile(path.join(task,'proc-macro-bypass.log'),'utf8')).trimEnd().split('\n')}:{}),cacheBytes:await measureCargoOutputBytes(objects),taskBytes:await measureCargoOutputBytes(task),validatedWallMs:performance.now()-started,...result};
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
    const fields=policy.schema===3?['schema','cacheMode','compatibilitySha256','runnerSha256','sandboxArgsSha256','runnerSourceSha256','bwrapSha256','maxCacheBytes']:['schema','snapshotSha256','runnerSha256','sandboxArgsSha256','runnerSourceSha256','bwrapSha256','maxCacheBytes'];
    if(Object.keys(policy).sort().join()!==fields.sort().join()||![2,3].includes(policy.schema)||(policy.schema===3&&policy.cacheMode!=='audited-dependencies')||!Number.isSafeInteger(policy.maxCacheBytes)||policy.maxCacheBytes<=0||fields.filter(key=>key.endsWith('Sha256')).some(key=>!/^[a-f0-9]{64}$/.test(policy[key]))||sha(JSON.stringify(policy))!==expectedNamespace)fail('cargo-clear-policy-mismatch');
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

export async function runHostedCargoCache(options) {
  if(options.snapshot?.schema!=='hosted-linux-cargo-snapshot-v1')fail('invalid-hosted-snapshot');
  return runCompositorCargoCache({maxCacheBytes:4*1024**3,maxTaskBytes:12*1024**3,reserveBytes:10*1024**3,...options});
}
