import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { packageBrowserBodySpore } from './package-body-spore.mjs';
import { readBodyBoundZip } from '../workspace/creche-native-zip.mjs';
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
function fixture() {
  const runtime = new Uint8Array([0,97,115,109,1,0,0,0]);
  const file = { path:'runtime.wasm', bytes:runtime.length, sha256:sha(runtime), media_type:'application/wasm' };
  const extra = ['conduit-browser-image.json','browser-boot-profile.mjs'].map(path => ({path,bytes:1,sha256:sha('x'),media_type:'text/plain'}));
  const files=[file,...extra];
  const bundle = sha('conduit.release/host-bundle-content@1\0'+files.map(f=>`${f.path}\0${f.sha256}\n`).join(''));
  const make = {make_package_id:'browser-wasm@1', output:'browser-bundle', builder_adapter:'conduit-host-browser/build-wasm@1', deployment_adapter:'conduit-host-browser/load@1'};
  const manifest = {schema:'conduit.body/spore-manifest@2',spore_id:'spore/test',image_id:'image/test',profile_id:'profile/test',body_id:'body/test',target:'browser/wasm32/page',output:'browser-bundle',binding:{mode:'self-joining',invitation_id:'invitation/test'},make,image_content_digest:bundle};
  const prepared = {spore_manifest:structuredClone(manifest),spore_id:manifest.spore_id,body_id:manifest.body_id,image_content_digest:bundle,invitation_id:'invitation/test',invitation_nonce:Array(32).fill(1),invitation_secret:Array(32).fill(2),invitation_expires_at_millis:2000};
  prepared.root_producer={host_id:'host/owner',boot_id:'boot/owner'};
  prepared.root_authority={schema:'conduit.body/root-authority@1',body_id:manifest.body_id,owner_host_id:'host/owner',public_key:Array(32).fill(3)};
  const release = {schema:'conduit.release/host-bundle@1',target_id:manifest.target,make_package_id:make.make_package_id,output:make.output,builder_adapter:make.builder_adapter,deployment_adapter:make.deployment_adapter,source_identity:'fixture-only',bundle_sha256:bundle,browser_image_id:'image/test',browser_profile_id:'profile/test',files};
  let requests = 0;
  const options = {manifest,prepared,releaseUrl:'https://release.invalid/browser-page.json',filename:'browser.zip',nowMillis:1000,fetcher:async url => {requests++; return new Response(String(url).endsWith('runtime.wasm')?runtime:String(url).endsWith('browser-page.json')?JSON.stringify(release):'x');}};
  return {options,release,runtime,requests:()=>requests};
}
test('packages the exact owner-prepared Spore using the existing ZIP consumer',async()=>{
  const {options}=fixture();
  const artifact = await packageBrowserBodySpore(options);
  const decoded = readBodyBoundZip(artifact.bytes);
  assert.equal(decoded.provision.spore.body_id,'body/test');
  assert.deepEqual(decoded.provision.invitation_provision.secret,Array(32).fill(2));
  assert.equal(artifact.image_content_digest, options.manifest.image_content_digest);
});
test('foreign, expired and malformed authority refuses before acquiring release',async()=>{
  for(const change of [o=>o.prepared.body_id='body/foreign',o=>o.prepared.invitation_id='invitation/foreign',o=>o.prepared.invitation_secret[0]=256,o=>o.prepared.invitation_nonce.fill(0),o=>o.prepared.invitation_expires_at_millis=1000,o=>o.manifest.target='linux/aarch64/host']) {
    const f=fixture();change(f.options);await assert.rejects(packageBrowserBodySpore(f.options));assert.equal(f.requests(),0);
  }
});
test('stale bytes and a different reviewed release cannot replace the bound image',async()=>{
  const f=fixture();f.runtime[7]=1;await assert.rejects(packageBrowserBodySpore(f.options),/exact identity/);
  const g=fixture();g.options.manifest.image_content_digest=sha('other');g.options.prepared.image_content_digest=g.options.manifest.image_content_digest;g.options.prepared.spore_manifest=structuredClone(g.options.manifest);
  await assert.rejects(packageBrowserBodySpore(g.options),/exact reviewed browser release/);
});
test('filesystem publication is private and cannot overwrite an existing package',async()=>{
  const {mkdtemp,writeFile,readFile,stat,rm}=await import('node:fs/promises');
  const {tmpdir}=await import('node:os');
  const path=await import('node:path');
  const {publishBrowserBodySpore}=await import('./package-body-spore.mjs');
  const dir=await mkdtemp(path.join(tmpdir(),'conduit-browser-spore-'));
  try {
    const f=fixture();
    for(const [name,value] of [['manifest.json',f.options.manifest],['prepared.json',f.options.prepared],['browser-page.json',f.release]]) await writeFile(path.join(dir,name),JSON.stringify(value),{mode:0o600});
    await writeFile(path.join(dir,'runtime.wasm'),f.runtime);
    for(const name of ['conduit-browser-image.json','browser-boot-profile.mjs'])await writeFile(path.join(dir,name),'x');
    const options={manifestPath:path.join(dir,'manifest.json'),preparedPath:path.join(dir,'prepared.json'),releasePath:path.join(dir,'browser-page.json'),outputPath:path.join(dir,'browser.zip'),nowMillis:1000};
    const result=await publishBrowserBodySpore(options);
    assert.equal(result.membership_admitted,false);
    assert.equal((await stat(options.outputPath)).mode&0o777,0o600);
    const original=await readFile(options.outputPath);
    await assert.rejects(publishBrowserBodySpore(options),{code:'EEXIST'});
    assert.deepEqual(await readFile(options.outputPath),original);
  } finally {await rm(dir,{recursive:true,force:true});}
});
test('retains the owner routed invitation and rejects a foreign route scope',async()=>{
  const f=fixture();
  const route={protocol:1,body_id:'body/test',invitation_id:'invitation/test',candidates:[{candidate_id:'canonical-owner-route'}]};
  f.options.prepared.rendezvous=route;
  const artifact=await packageBrowserBodySpore(f.options);
  assert.deepEqual(readBodyBoundZip(artifact.bytes).provision.invitation_provision.rendezvous,route);
  f.options.prepared.rendezvous={...route,body_id:'body/foreign'};
  await assert.rejects(packageBrowserBodySpore(f.options),/routed invitation/);
});
test('an unconfigured runtime distribution is not a browser Host package',async()=>{
  const f=fixture();delete f.release.browser_image_id;
  await assert.rejects(packageBrowserBodySpore(f.options),/configured BrowserBundle/);
});
test('release acquisition cannot replace the retained Spore or invitation',async()=>{
  const f=fixture(),fetcher=f.options.fetcher;
  f.options.fetcher=async url=>{f.options.manifest.body_id='body/replaced';f.options.prepared.body_id='body/replaced';f.options.prepared.invitation_secret.fill(9);return fetcher(url);};
  const artifact=await packageBrowserBodySpore(f.options),provision=readBodyBoundZip(artifact.bytes).provision;
  assert.equal(provision.spore.body_id,'body/test');
  assert.deepEqual(provision.invitation_provision.secret,Array(32).fill(2));
});

test('missing, foreign or secret-bearing Root provenance refuses before acquiring release',async()=>{
  for(const change of [p=>delete p.root_authority,p=>delete p.root_producer,p=>p.root_producer.host_id='foreign',p=>p.root_authority.body_id='foreign',p=>p.root_authority.public_key.fill(0),p=>p.root_authority.seed=Array(32).fill(9)]){
    const f=fixture();change(f.options.prepared);await assert.rejects(packageBrowserBodySpore(f.options));assert.equal(f.requests(),0);
  }
  const f=fixture(),artifact=await packageBrowserBodySpore(f.options);
  assert.deepEqual(readBodyBoundZip(artifact.bytes).provision.invitation_provision.root_authority,f.options.prepared.root_authority);
});
