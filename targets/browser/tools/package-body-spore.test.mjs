import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { packageBrowserBodySpore } from './package-body-spore.mjs';
import { readBodyBoundZip } from '../workspace/creche-native-zip.mjs';
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
function fixture() {
  const runtime = new Uint8Array([0,97,115,109,1,0,0,0]);
  const file = { path:'runtime.wasm', bytes:runtime.length, sha256:sha(runtime), media_type:'application/wasm' };
  const bundle = sha(`conduit.release/host-bundle-content@1\0${file.path}\0${file.sha256}\n`);
  const make = {make_package_id:'browser-wasm@1', output:'browser-bundle', builder_adapter:'conduit-host-browser/build-wasm@1', deployment_adapter:'conduit-host-browser/load@1'};
  const manifest = {schema:'conduit.body/spore-manifest@2',spore_id:'spore/test',body_id:'body/test',target:'browser/wasm32/page',output:'browser-bundle',binding:{mode:'self-joining',invitation_id:'invitation/test'},make,image_content_digest:bundle};
  const prepared = {spore_manifest:structuredClone(manifest),spore_id:manifest.spore_id,body_id:manifest.body_id,image_content_digest:bundle,invitation_id:'invitation/test',invitation_nonce:Array(32).fill(1),invitation_secret:Array(32).fill(2),invitation_expires_at_millis:2000};
  const release = {schema:'conduit.release/host-bundle@1',target_id:manifest.target,make_package_id:make.make_package_id,output:make.output,builder_adapter:make.builder_adapter,deployment_adapter:make.deployment_adapter,source_identity:'fixture-only',bundle_sha256:bundle,files:[file]};
  let requests = 0;
  const options = {manifest,prepared,releaseUrl:'https://release.invalid/browser-page.json',filename:'browser.zip',nowMillis:1000,fetcher:async url => {requests++; return new Response(String(url).endsWith('runtime.wasm')?runtime:JSON.stringify(release));}};
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
