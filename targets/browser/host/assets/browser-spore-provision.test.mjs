import assert from 'node:assert/strict';
import test from 'node:test';
import { parseBrowserSporeProvision } from './browser-spore-provision.mjs';

const encode = value => new TextEncoder().encode(JSON.stringify(value));
function fixture() {
  return { schema: 'conduit.spore/native-package-provision@1', spore: {
    schema: 'conduit.body/spore-manifest@2', spore_id: 'spore/test', image_id: 'image/test',
    profile_id: 'profile/test', body_id: 'body/test', target: 'browser/wasm32/page',
    output: 'browser-bundle', image_content_digest: `sha256:${'a'.repeat(64)}`,
    binding: { mode: 'self-joining', invitation_id: 'invitation/test' },
    make: { make_package_id: 'browser-wasm@1', output: 'browser-bundle',
      builder_adapter: 'conduit-host-browser/build-wasm@1', deployment_adapter: 'conduit-host-browser/load@1' },
  }, invitation_provision: { invitation_id: 'invitation/test', nonce: Array(32).fill(1),
    secret: Array(32).fill(2), expires_at_millis: 2000,
    rendezvous: { protocol: 1, body_id: 'body/test', invitation_id: 'invitation/test',
      candidates: [{ candidate_id: 'owner-selected-route' }] } } };
}
const parse = value => parseBrowserSporeProvision(encode(value), { nowMillis: 1000, expectedBodyId: 'body/test' });

test('returns canonical detached claim and erasable secret without asserting admission', () => {
  const source = fixture(), bytes = encode(source), result = parseBrowserSporeProvision(bytes, { nowMillis: 1000 });
  assert.deepEqual(result.claim, { invitation_id: 'invitation/test', body_id: 'body/test',
    nonce: Array(32).fill(1), expires_at_millis: 2000 });
  assert.deepEqual(result.rendezvous, source.invitation_provision.rendezvous);
  assert.equal(Object.isFrozen(result.manifest.binding), true);
  bytes.fill(0); source.invitation_provision.nonce.fill(9);
  assert.equal(result.claim.nonce[0], 1);
  result.secret.fill(0);
  assert.equal(source.invitation_provision.secret[0], 2);
  assert.equal('membership_admitted' in result, false);
});

test('rejects foreign scope, wrong platform, expired and malformed authority', () => {
  for (const change of [
    p => p.schema = 'wrong', p => p.spore.schema = 'wrong',
    p => p.spore.target = 'linux/aarch64/host', p => p.spore.output = 'native-bundle',
    p => p.spore.binding.mode = 'unbound', p => p.spore.body_id = 'body/foreign',
    p => p.spore.binding.invitation_id = 'invitation/foreign',
    p => p.spore.image_content_digest = 'sha256:wrong',
    p => p.spore.make.output = 'native-bundle', p => p.spore.profile_id = '',
    p => p.invitation_provision.nonce[0] = 256, p => p.invitation_provision.secret[0] = -1,
    p => p.invitation_provision.nonce.fill(0), p => p.invitation_provision.secret.fill(0),
    p => p.invitation_provision.expires_at_millis = 1000,
    p => p.invitation_provision.expires_at_millis = 1.5,
  ]) { const p = fixture(); change(p); assert.throws(() => parse(p)); }
});

test('retains opaque bounded routes but refuses changed route binding or capacity', () => {
  for (const change of [
    r => r.body_id = 'body/foreign', r => r.invitation_id = 'invitation/foreign',
    r => r.protocol = 2, r => r.candidates = [], r => r.candidates = Array(5).fill({}),
    r => r.candidates = [{ payload: 'x'.repeat(17000) }],
  ]) { const p = fixture(); change(p.invitation_provision.rendezvous); assert.throws(() => parse(p)); }
  const p = fixture(); delete p.invitation_provision.rendezvous;
  assert.equal(parse(p).rendezvous, null);
});

test('applies existing finite authenticated candidate contract without selecting transport', () => {
  const p = fixture();
  p.invitation_provision.rendezvous_candidates = [{ schema: 'conduit.body/rendezvous-candidate@1',
    body_id: 'body/test', line_family: 'websocket', locator: 'wss://owner.invalid/line',
    rendezvous_identity: 'route/test', expires_at_millis: 1500, maximum_attempts: 1,
    connection_timeout_millis: 500, authentication: { mode: 'conduit-authenticated-line', server_identity: 'owner/test' } }];
  assert.equal(parse(p).rendezvousCandidates.length, 1);
  p.invitation_provision.rendezvous_candidates[0].expires_at_millis = 2001;
  assert.throws(() => parse(p));
});

test('refuses invalid UTF-8, oversized input and unprovided or nonfinite clock', () => {
  for (const bytes of [new Uint8Array(), new Uint8Array(256 * 1024 + 1), new Uint8Array([255]), encode(null)]) {
    assert.throws(() => parseBrowserSporeProvision(bytes, { nowMillis: 1000 }));
  }
  for (const nowMillis of [undefined, -1, NaN, Infinity, 0.1]) {
    assert.throws(() => parseBrowserSporeProvision(encode(fixture()), { nowMillis }));
  }
});

test('consumes the existing Body-bound ZIP producer provision unchanged', async () => {
  const { createBodyBoundZip, readBodyBoundZip } = await import('../../workspace/creche-native-zip.mjs');
  const p = fixture(), invitation = p.invitation_provision;
  const artifact = await createBodyBoundZip({ filename: 'browser.zip', prepared: {
    spore_manifest: p.spore, spore_id: p.spore.spore_id,
    image_content_digest: p.spore.image_content_digest, body_id: p.spore.body_id,
    invitation_id: invitation.invitation_id, invitation_nonce: invitation.nonce,
    invitation_secret: invitation.secret, invitation_expires_at_millis: invitation.expires_at_millis,
    rendezvous: invitation.rendezvous,
  }, release: { payloads: [{ path: 'runtime.wasm', bytes: new Uint8Array([0]), media_type: 'application/wasm' }] } });
  const entries = readBodyBoundZip(artifact.bytes).entries;
  const result = parseBrowserSporeProvision(entries.get('conduit-spore.json'), { nowMillis: 1000 });
  assert.equal(result.manifest.body_id, p.spore.body_id);
  assert.deepEqual(result.secret, invitation.secret);
});

test('returns bounded exact Body Root provenance inertly and rejects secret extensions',()=>{
  const p=fixture();p.invitation_provision.root_producer={host_id:'host/owner',boot_id:'boot/owner'};p.invitation_provision.root_authority={schema:'conduit.body/root-authority@1',body_id:'body/test',owner_host_id:'host/owner',public_key:Array(32).fill(3)};
  assert.deepEqual(parse(p).rootAuthority,p.invitation_provision.root_authority);
  assert.deepEqual(parse(p).rootProducer,p.invitation_provision.root_producer);
  for(const change of [r=>r.body_id='foreign',r=>r.public_key.fill(0),r=>r.seed=[1],r=>r.private_record_sha256='forbidden']){
    const copy=structuredClone(p);change(copy.invitation_provision.root_authority);assert.throws(()=>parse(copy));
  }
});
