import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { proceduralMacroBypassNames, prepareHostedSnapshot } from '../../tools/ci/compiler-cache/hosted-snapshot.mjs';
import { HOSTED_RUSTC_WRAPPER, cargoSandboxArgs, runHostedCargoCache } from '../../tools/ci/compiler-cache/cargo-sandbox.mjs';

test('procedural macro reexports and transitive dependencies all bypass cache', () => {
  const packages=[['derive','proc-macro'],['reexport','lib'],['consumer','lib'],['plain','lib']].map(([id,kind])=>({id,name:id,targets:[{kind:[kind]}]}));
  const nodes=[['derive',[]],['reexport',['derive']],['consumer',['reexport']],['plain',[]]].map(([id,deps])=>({id,deps:deps.map(pkg=>({pkg}))}));
  assert.deepEqual(proceduralMacroBypassNames({packages,resolve:{nodes}}),['consumer','derive','reexport']);
  assert.throws(()=>proceduralMacroBypassNames({packages,resolve:{}}),/unsupported-hosted-dependency-graph/);
});

test('hosted cache refuses other snapshot profiles before touching task or cache storage', async () => {
  await assert.rejects(runHostedCargoCache({snapshot:{schema:'anything'}}),/invalid-hosted-snapshot/);
  await assert.rejects(prepareHostedSnapshot({repositoryRoot:'relative'}),/unsupported-hosted-snapshot-policy/);
});

test('actual sandbox wrapper bypasses hidden macro consumers, caches plain crates and refuses response files', async t => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'hosted-wrapper-'));t.after(()=>fs.rm(root,{recursive:true,force:true}));
  for(const name of ['task','closure','engine'])await fs.mkdir(path.join(root,name));
  await fs.writeFile(path.join(root,'task/wrapper'),HOSTED_RUSTC_WRAPPER,{mode:0o500});
  await fs.writeFile(path.join(root,'task/compiler'),'#!/bin/sh\nprintf "direct\\n"\n',{mode:0o500});
  await fs.writeFile(path.join(root,'engine/sccache'),'#!/bin/sh\nprintf "cached\\n"\n',{mode:0o500});
  await fs.writeFile(path.join(root,'closure/proc-macro-bypass.txt'),'consumer\nderive\nreexport\n');
  const args=['--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv',
    '--ro-bind','/usr/bin/dash','/bin/sh','--ro-bind','/lib64/ld-linux-x86-64.so.2','/lib64/ld-linux-x86-64.so.2',
    '--ro-bind','/usr/lib/x86_64-linux-gnu/libc.so.6','/lib/x86_64-linux-gnu/libc.so.6',
    '--bind',path.join(root,'task'),'/task','--ro-bind',path.join(root,'closure'),'/closure','--ro-bind',path.join(root,'engine'),'/engine'];
  const run=(name,compilerArgs)=>spawnSync('/usr/bin/bwrap',[...args,'--setenv','CARGO_PKG_NAME',name,'/bin/sh','/task/wrapper','/task/compiler',...compilerArgs],{encoding:'utf8'});
  for(const name of ['consumer','derive','reexport']){
    const result=run(name,['--extern','ordinary=/task/ordinary.rlib']);assert.equal(result.status,0,result.stderr);assert.equal(result.stdout,'direct\n');
  }
  const plain=run('plain',['--crate-type','rlib']);assert.equal(plain.status,0,plain.stderr);assert.equal(plain.stdout,'cached\n');
  const response=run('plain',['@/task/args']);assert.equal(response.status,64);assert.match(response.stderr,/refuses response files/);
  const unknown=run('plain',['--extern','hidden=/task/unknown.so']);assert.equal(unknown.status,64);assert.match(unknown.stderr,/unclassified procedural macro/);
  const probe=run('plain',['--print=cfg','--crate-type','proc-macro']);assert.equal(probe.status,0,probe.stderr);assert.equal(probe.stdout,'direct\n');
});

test('hosted mount plan exposes immutable vendor/config and only private mutable task and managed objects', () => {
  const args=cargoSandboxArgs({snapshotRoot:'/snapshot',taskRoot:'/private-task',cacheObjects:'/managed-objects',namespace:'a'.repeat(64),maxCacheBytes:4096,hosted:true});
  const mutable=[];for(let i=0;i<args.length;i++)if(args[i]==='--bind')mutable.push(args[i+1]);
  assert.deepEqual(mutable,['/private-task','/managed-objects']);
  assert.ok(args.includes('/snapshot/vendor'));assert.ok(args.includes('/snapshot/proc-macro-bypass.txt'));
  assert.ok(args.includes('--unshare-all'));assert.ok(args.includes('--clearenv'));
});

test('hosted source custody authenticates the raw Git tree and refuses changed, missing or extra copied bytes', async t => {
  const { verifyHostedSource }=await import('../../tools/ci/compiler-cache/hosted-snapshot.mjs');
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'hosted-source-'));t.after(()=>fs.rm(root,{recursive:true,force:true}));
  const repositoryRoot=path.join(root,'repository'),sourceRoot=path.join(root,'snapshot');
  await fs.mkdir(repositoryRoot);await fs.mkdir(sourceRoot);
  const git=args=>{const result=spawnSync('git',['-C',repositoryRoot,...args],{encoding:'utf8'});assert.equal(result.status,0,result.stderr);return result.stdout.trim();};
  git(['init','-b','source']);git(['config','user.name','Snapshot fixture']);git(['config','user.email','fixture@example.invalid']);
  await fs.writeFile(path.join(repositoryRoot,'input.rs'),'pub const VALUE:u8=7;\n');
  git(['add','input.rs']);git(['commit','-qm','source']);
  const sourceCommit=git(['rev-parse','HEAD']),sourceTree=git(['rev-parse','HEAD^{tree}']);
  await fs.copyFile(path.join(repositoryRoot,'input.rs'),path.join(sourceRoot,'input.rs'));
  const options={repositoryRoot,sourceRoot,sourceCommit,sourceTree};
  assert.equal((await verifyHostedSource(options)).files,1);
  await fs.writeFile(path.join(sourceRoot,'input.rs'),'pub const VALUE:u8=8;\n');
  await assert.rejects(verifyHostedSource(options),/hosted-source-blob-mismatch/);
  await fs.copyFile(path.join(repositoryRoot,'input.rs'),path.join(sourceRoot,'input.rs'));
  await fs.writeFile(path.join(sourceRoot,'extra'),'untracked');
  await assert.rejects(verifyHostedSource(options),/hosted-source-inventory-mismatch/);
  await fs.unlink(path.join(sourceRoot,'extra'));await fs.unlink(path.join(sourceRoot,'input.rs'));
  await assert.rejects(verifyHostedSource(options),/hosted-source-inventory-mismatch/);
});

test('registry acquisition verifies lock archive digests and refuses archive symlink escape', async t => {
  const {extractAuthenticatedRegistry}=await import('../../tools/ci/compiler-cache/hosted-snapshot.mjs');
  const {createHash}=await import('node:crypto');
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'hosted-vendor-'));t.after(()=>fs.rm(root,{recursive:true,force:true}));
  const cache=path.join(root,'cache'),registry=path.join(cache,'index');await fs.mkdir(registry,{recursive:true});
  const archive=path.join(registry,'example-1.0.0.crate');
  function makeArchive(kind){
    const result=spawnSync('/usr/bin/python3',['-c',`import tarfile,io,sys
with tarfile.open(sys.argv[1],'w:gz') as t:
 i=tarfile.TarInfo('example-1.0.0/src/lib.rs')
 if sys.argv[2]=='symlink': i.type=tarfile.SYMTYPE;i.linkname='/etc/passwd';t.addfile(i)
 else:
  b=b'pub const VALUE:u8=1;';i.size=len(b);t.addfile(i,io.BytesIO(b))
`,archive,kind],{encoding:'utf8'});assert.equal(result.status,0,result.stderr);
  }
  async function options(name,checksum){
    const lockFile=path.join(root,name+'.lock'),vendorRoot=path.join(root,name);await fs.mkdir(vendorRoot);
    await fs.writeFile(lockFile,`version = 4\n[[package]]\nname = "example"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${checksum}"\n`);
    return {lockFile,vendorRoot,archiveCache:cache};
  }
  makeArchive('regular');const checksum=createHash('sha256').update(await fs.readFile(archive)).digest('hex');
  const valid=await options('valid',checksum);assert.equal((await extractAuthenticatedRegistry(valid)).packages,1);
  assert.equal(await fs.readFile(path.join(valid.vendorRoot,'example-1.0.0/src/lib.rs'),'utf8'),'pub const VALUE:u8=1;');
  const mismatch=await options('mismatch','a'.repeat(64));
  await assert.rejects(extractAuthenticatedRegistry(mismatch),error=>error.code==='hosted-snapshot-command-failed'&&/missing authenticated crate archive/.test(error.diagnostic));
  assert.deepEqual(await fs.readdir(mismatch.vendorRoot),[]);
  makeArchive('symlink');const unsafe=await options('unsafe',createHash('sha256').update(await fs.readFile(archive)).digest('hex'));
  await assert.rejects(extractAuthenticatedRegistry(unsafe),error=>error.code==='hosted-snapshot-command-failed'&&/unsupported crate member/.test(error.diagnostic));
  assert.deepEqual(await fs.readdir(path.join(unsafe.vendorRoot,'example-1.0.0')),[]);
});
