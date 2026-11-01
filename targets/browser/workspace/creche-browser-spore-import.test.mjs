import assert from 'node:assert/strict';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { createBodyBoundZip } from './creche-native-zip.mjs';
import { importBrowserSpore } from './creche-browser-spore-import.mjs';
import { Conduit, admitBrowserAssetMap } from '../sdk/browser-sdk.mjs';
const encode = value => new TextEncoder().encode(JSON.stringify(value));
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const inventory = files => sha('conduit.release/host-bundle-content@1\0' + files.map(f => `${f.path}\0${f.sha256}\n`).join(''));
async function fixture() {
  const runtime = new Uint8Array([0,97,115,109,1,0,0,0]);
  const file = { path: 'runtime.wasm', bytes: runtime.length, sha256: sha(runtime), media_type: 'application/wasm' };
  const pkg = { schema:'conduit.browser/sdk-package@1', runtime_abi:'conduit.browser/runtime-abi@1', package_id:'@conduit/browser',package_version:'0.1.0',target_id:'browser/wasm32/page',image_id:`image:sha256:${'b'.repeat(64)}`,profile_id:`sha256:${'c'.repeat(64)}` };
  const distribution = { schema:'conduit.release/host-bundle@1', target_id:pkg.target_id,make_package_id:'browser-wasm@1',output:'browser-bundle',files:[file],bundle_sha256:inventory([file]),reviewed_distribution:{schema:'conduit.browser/reviewed-distribution@1',runtime_abi:pkg.runtime_abi,distribution_id:'fixture'} };
  const image = {schema:'conduit.browser/bundle-image@1',image_id:pkg.image_id,profile_id:pkg.profile_id,reviewed_distribution:{runtime_abi:pkg.runtime_abi,distribution_digest:distribution.bundle_sha256},files:[file]};
  const imageBytes=encode(image);
  const files=[file,{path:'conduit-browser-image.json',bytes:imageBytes.length,sha256:sha(imageBytes),media_type:'application/json'}];
  const release={...distribution, files,bundle_sha256:inventory(files), distribution_id:'fixture',distribution_sha256:distribution.bundle_sha256,browser_image_id:pkg.image_id,browser_profile_id:pkg.profile_id};
  Object.assign(pkg,{files,bundle_sha256:release.bundle_sha256});
  const manifest={schema:'conduit.body/spore-manifest@2',spore_id:'spore/test',image_id:pkg.image_id,profile_id:pkg.profile_id,body_id:'body/test',target:pkg.target_id,output:'browser-bundle',image_content_digest:release.bundle_sha256,binding:{mode:'self-joining',invitation_id:'invite/test'},make:{make_package_id:'browser-wasm@1',output:'browser-bundle',builder_adapter:'builder/test',deployment_adapter:'deploy/test'}};
  const prepared={spore_manifest:manifest,spore_id:manifest.spore_id,body_id:manifest.body_id,image_content_digest:release.bundle_sha256,invitation_id:'invite/test',invitation_nonce:Array(32).fill(1),invitation_secret:Array(32).fill(2),invitation_expires_at_millis:2000};
  const payloads=[{...file,bytes:runtime},{...files[1],bytes:imageBytes}];
  const zip=await createBodyBoundZip({prepared,release:{payloads},filename:'browser.zip'});
  return {zip,prepared,payloads,policy:{package:pkg,distribution,release}};
}
test('imports exact reviewed closure with detached erasable provision, no execution or identity creation',async()=>{
  const f=await fixture();
  const result=await importBrowserSpore(f.zip.bytes,{reviewedRelease:f.policy,nowMillis:1000,expectedBodyId:'body/test'});
  assert.deepEqual([...result.browserOptions.assetMap.keys()],f.payloads.map(p=>p.path));
  assert.equal(result.browserOptions.assetMap.has('conduit-spore.json'),false);
  assert.equal(result.provision.secret[0],2);
  result.provision.secret.fill(0);
  assert.equal(f.prepared.invitation_secret[0],2);
  assert.equal('hostId' in result,false);
});
test('refuses foreign or expired provision and independently selected policy mismatches',async()=>{
  for(const change of [o=>o.expectedBodyId='foreign',o=>o.nowMillis=2000,o=>o.reviewedRelease.package.profile_id='foreign',o=>o.reviewedRelease.package.bundle_sha256=sha('foreign'),o=>o.reviewedRelease=undefined]) {
    const f=await fixture(),options={reviewedRelease:f.policy,nowMillis:1000}; change(options);
    await assert.rejects(importBrowserSpore(f.zip.bytes,options));
  }
});
test('asset admission refuses extra/missing/changed/oversized assets and snapshots caller bytes',async()=>{
  for(const change of [m=>m.set('conduit-spore.json',encode({secret:'forbidden'})),m=>m.delete('runtime.wasm'),m=>m.get('runtime.wasm').fill(1),m=>m.set('x.mjs',new Uint8Array(256*1024+1))]) {
    const f=await fixture(),map=new Map(f.payloads.map(p=>[p.path,p.bytes.slice()]));change(map);
    await assert.rejects(admitBrowserAssetMap(map,f.policy));
  }
  const f=await fixture(),map=new Map(f.payloads.map(p=>[p.path,p.bytes.slice()]));
  const pending=admitBrowserAssetMap(map,f.policy);map.get('runtime.wasm').fill(9);
  assert.equal((await pending).payloads.get('runtime.wasm')[0],0);
});
test('archive corruption and file count bounds refuse before release admission',async()=>{
  const f=await fixture(),broken=f.zip.bytes.slice();broken[0]=0;
  await assert.rejects(importBrowserSpore(broken,{reviewedRelease:f.policy,nowMillis:1000}));
  await assert.rejects(admitBrowserAssetMap(new Map(Array.from({length:17},(_,i)=>[`${i}.mjs`,new Uint8Array([1])])),f.policy));
});

test('SDK map entrance reaches existing Boot requirements without ambient fetch',async()=>{
  const f=await fixture(),map=new Map(f.payloads.map(p=>[p.path,p.bytes]));
  const old=globalThis.fetch;globalThis.fetch=()=>{throw new Error('unexpected ambient fetch');};
  try {await assert.rejects(Conduit.browser({assetMap:map,reviewedRelease:f.policy}),{code:'ArtifactMissing'});}
  finally {globalThis.fetch=old;}
});
