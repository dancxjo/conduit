import test from 'node:test';
import assert from 'node:assert/strict';
import {promises as fs} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {AUDITED_DEPENDENCIES,buildHostedAdmission,sharedCompatibilityInventory} from '../../tools/ci/compiler-cache/hosted-admission.mjs';
import {hostedCachePolicy,cargoSandboxArgs} from '../../tools/ci/compiler-cache/cargo-sandbox.mjs';
const sha=value=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
test('changing unrelated workspace source preserves shared compatibility while toolchain/vendor/config changes rotate it',()=>{
 const files=['toolchain/bin/rustc','engine/sccache','runtime/usr/bin/cc','vendor/syn/src/lib.rs','source/.cargo/config.toml','native-policy.json','cargo-config.toml','shared-admission.json','shared-admission.sh','source/products/conduit/src/main.rs','metadata.json'].map((file,index)=>({path:file,sha256:String(index).padStart(64,'0'),bytes:index,executable:false}));
 const key=sha(sharedCompatibilityInventory(files)),changed=structuredClone(files);changed.find(file=>file.path==='source/products/conduit/src/main.rs').sha256='f'.repeat(64);
 assert.equal(sha(sharedCompatibilityInventory(changed)),key);assert.notEqual(sha(changed),sha(files));
 for(const name of ['toolchain/bin/rustc','vendor/syn/src/lib.rs','source/.cargo/config.toml','shared-admission.sh']){
 const changed=structuredClone(files);changed.find(file=>file.path===name).sha256='e'.repeat(64);assert.notEqual(sha(sharedCompatibilityInventory(changed)),key);
 }
});
test('injecting hidden assembly into an audited package cannot inherit immutable source admission',async()=>{
 const root=await fs.mkdtemp(path.join(os.tmpdir(),'audited-input-refusal-'));const reviewed=AUDITED_DEPENDENCIES.crates[0];
 try{
 const directory=path.join(root,'vendor',reviewed.name+'-'+reviewed.version);await fs.mkdir(directory,{recursive:true});
 await fs.writeFile(path.join(directory,'lib.rs'),'core::arch::global_asm!(".incbin \\"/source/private-input\\"");\n');
 await fs.writeFile(path.join(directory,'.cargo-checksum.json'),JSON.stringify({package:reviewed.archiveSha256,files:{}}));
 const metadata={packages:[{name:reviewed.name,version:reviewed.version,source:'registry+https://github.com/rust-lang/crates.io-index',targets:[{kind:['lib']}]}]};
 await assert.rejects(buildHostedAdmission({root,metadata,bypassNames:[]}),{code:'audited-dependency-source-mismatch'});
 metadata.packages[0].targets.push({kind:['custom-build']});await assert.rejects(buildHostedAdmission({root,metadata,bypassNames:[]}),{code:'unsupported-audited-dependency'});
 }finally{await fs.rm(root,{recursive:true});}
});
test('pre-restore API refuses mode confusion and shared sandbox mounts only the sealed admission script',async()=>{
 await assert.rejects(hostedCachePolicy({schema:'other'}),{code:'invalid-hosted-cache-policy'});
 const args=cargoSandboxArgs({snapshotRoot:'/immutable',taskRoot:'/private',cacheObjects:'/managed',namespace:'a'.repeat(64),maxCacheBytes:1024,hosted:true,sharedAdmission:true});
 assert.ok(args.includes('/immutable/shared-admission.sh'));assert.ok(args.includes('/closure/shared-admission.sh'));
 assert.equal(args.filter(value=>value==='--bind').length,2);
});
