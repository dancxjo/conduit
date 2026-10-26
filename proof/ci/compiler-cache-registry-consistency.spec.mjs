import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import * as module from '../../tools/ci/compiler-cache/hosted-snapshot.mjs';

// Deterministically replace the cache file after its first read. Production
// extraction must retain the bytes whose checksum it actually authenticated.
test('registry extraction retains authenticated bytes across cache replacement', async () => {
const root=await fs.mkdtemp(path.join(os.tmpdir(),'authenticated-crate-race-'));const archive=path.join(root,'cache/index/race-fixture-0.1.0.crate');const evil=path.join(root,'evil.tar.gz');
try{
 await fs.mkdir(path.dirname(archive),{recursive:true});await fs.mkdir(root+'/vendor');await fs.mkdir(root+'/python');
 const generate=spawnSync('/usr/bin/python3',['-c',`import tarfile,io,sys
for file,value in [(sys.argv[1],1),(sys.argv[2],2)]:
 with tarfile.open(file,'w:gz')as tar:
  data=('pub const VALUE: usize = '+str(value)+';\\n').encode(); member=tarfile.TarInfo('race-fixture-0.1.0/src/lib.rs');member.size=len(data);member.mode=0o644;tar.addfile(member,io.BytesIO(data))
`,archive,evil],{encoding:'utf8'});if(generate.status!==0)throw Error(generate.stderr);
 const expected=createHash('sha256').update(await fs.readFile(archive)).digest('hex');
 await fs.writeFile(root+'/Cargo.lock',`version = 3\n[[package]]\nname = "race-fixture"\nversion = "0.1.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${expected}"\n`);
 await fs.writeFile(root+'/python/sitecustomize.py',`import pathlib,os
base_open=pathlib.Path.open
wanted=pathlib.Path(os.environ['CONDUIT_TEST_ARCHIVE']);evil=pathlib.Path(os.environ['CONDUIT_TEST_EVIL']);seen=False
class RaceReader:
 def __init__(self,file): self.file=file
 def __enter__(self): self.file.__enter__();return self
 def __exit__(self,*args):return self.file.__exit__(*args)
 def read(self,*args):
  global seen
  result=self.file.read(*args)
  if not seen:
   seen=True
   with base_open(evil,'rb')as source: data=source.read()
   with base_open(wanted,'wb')as target: target.write(data)
  return result
 def __getattr__(self,key):return getattr(self.file,key)
def race_open(self,*args,**kwargs):
 file=base_open(self,*args,**kwargs);mode=args[0]if args else kwargs.get('mode','r')
 return RaceReader(file)if self==wanted and mode=='rb'and not seen else file
pathlib.Path.open=race_open
`);
 const prior={PYTHONPATH:process.env.PYTHONPATH,CONDUIT_TEST_ARCHIVE:process.env.CONDUIT_TEST_ARCHIVE,CONDUIT_TEST_EVIL:process.env.CONDUIT_TEST_EVIL};Object.assign(process.env,{PYTHONPATH:root+'/python',CONDUIT_TEST_ARCHIVE:archive,CONDUIT_TEST_EVIL:evil});
 try{await module.extractAuthenticatedRegistry({lockFile:root+'/Cargo.lock',vendorRoot:root+'/vendor',archiveCache:root+'/cache'});}finally{for(const[key,value]of Object.entries(prior))if(value===undefined)delete process.env[key];else process.env[key]=value;}
 const extracted=await fs.readFile(root+'/vendor/race-fixture-0.1.0/src/lib.rs','utf8');const after=createHash('sha256').update(await fs.readFile(archive)).digest('hex');if(after===expected)throw Error('race hook did not execute');
 const receipt={cacheChangedAfterVerifiedRead:true,extractedAuthenticatedOriginal:extracted.includes('VALUE: usize = 1'),extracted};assert.equal(receipt.extractedAuthenticatedOriginal, true, 'extraction must use the authenticated buffer despite a later cache change');
}finally{await fs.rm(root,{recursive:true});}

});
