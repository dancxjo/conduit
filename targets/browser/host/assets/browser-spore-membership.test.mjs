import test from 'node:test';
import assert from 'node:assert/strict';
import { createInvitedBrowserHandshake } from './browser-spore-membership.mjs';
function fixture() {
  let calls = 0;
  const input = { provision: { claim: { invitation_id: 'invitation/test', body_id: 'body/test',
    nonce: Array(32).fill(1), expires_at_millis: 2000 }, secret: Array(32).fill(2) },
    expectedBodyId: 'body/test', hostId: 'host/browser', bootId: 'boot/browser', nowMillis: 1000,
    advertisement: { host_id: 'host/browser', boot_id: 'boot/browser', offer_generation: 1 },
    verifyingKey: Array(32).fill(3), membership: { proveSpawn(claim, secret) {
      calls++; assert.deepEqual(secret, Array(32).fill(2));
      return { hostId: 'host/browser', bootId: 'boot/browser', signature: Array(64).fill(4) };
    } } };
  return { input, calls: () => calls };
}
test('preserves exact offered snapshot, consumes erasable secret, emits existing spawn proof once', () => {
  const f = fixture(), h = createInvitedBrowserHandshake(f.input);
  assert.ok(f.input.provision.secret.every(b => b === 0));
  f.input.advertisement.boot_id = 'changed';
  const frame = h.advertise();
  assert.equal(frame.kind, 'invited-advertise');
  assert.equal(frame.advertisement.boot_id, 'boot/browser');
  assert.equal(h.prove(frame.claim).kind, 'spawn-proof');
  assert.equal(f.calls(), 1);
  assert.throws(() => h.prove(frame.claim)); assert.throws(() => h.advertise());
});
test('changed challenge is never signed and cancellation cannot revive proof', () => {
  for (const change of [c => c.body_id = 'foreign', c => c.invitation_id = 'foreign',
    c => c.nonce[0] = 9, c => c.expires_at_millis++]) {
    const f = fixture(), h = createInvitedBrowserHandshake(f.input), frame = h.advertise();
    change(frame.claim); assert.throws(() => h.prove(frame.claim)); assert.equal(f.calls(), 0);
  }
  const f = fixture(), h = createInvitedBrowserHandshake(f.input), frame = h.advertise();
  h.retire(); assert.throws(() => h.prove(frame.claim)); assert.equal(f.calls(), 0);
});
test('requires independently expected Body, current incarnation, finite expiry and exact key', () => {
  for (const change of [i => i.expectedBodyId = null, i => i.expectedBodyId = 'foreign',
    i => i.provision.claim.expires_at_millis = 1000, i => i.provision.secret.fill(0),
    i => i.verifyingKey.pop(), i => i.advertisement.boot_id = 'foreign']) {
    const f = fixture(); change(f.input); assert.throws(() => createInvitedBrowserHandshake(f.input));
    assert.equal(f.calls(), 0);
  }
});

import { openVerifiedBrowserSporeMembership } from './browser-spore-membership.mjs';
// A deliberately stubbed runtime verifier: these tests prove composition and
// custody ordering, not Ed25519 verification (covered by the canonical ABI).
function verifiedFixture() {
  const f = fixture(), events = [];
  const credential = { credential_id: 'credential/test', body_id: 'body/test',
    part_id: 'part/test', host_id: f.input.hostId, boot_id: f.input.bootId, issued_at_millis: 1 };
  let stored = null;
  const options = { ...f.input, retainedCredential: null,
    invitedProvision: f.input.provision,
    sporeRoot: { rootAuthority: { schema: 'conduit.body/root-authority@1',
      body_id: 'body/test', owner_host_id: 'owner/test', public_key: Array(32).fill(11) },
      rootProducer: { host_id: 'owner/test', boot_id: 'owner-boot/test' } },
    membership: { hostId: f.input.hostId, bootId: f.input.bootId,
      prepareMembershipScope(input) { events.push('prepare');
        assert.deepEqual(input.expectedRoot, Array(32).fill(11));
        assert.equal(input.producerBoot, 'owner-boot/test');
        return { ...input.claim, host_id: f.input.hostId, boot_id: f.input.bootId, offer_generation: 1 };
      },
      adoptMembershipScope(receipt) { events.push('verify'); if (receipt?.signature !== 'valid') throw Error('Root signature refused'); return credential; },
      exportPartMembership() { events.push('export'); return new Uint8Array([7]); },
      importPartMembership() { events.push('import'); return { ...credential, boot_id: 'boot/prior' }; },
      retireMembershipScope(scope) { events.push(['retire', scope]); },
    },
    storage: {
      async readBytes() { events.push('read'); return stored?.slice() ?? null; },
      async writeBytes(key, bytes) { events.push('write'); stored = bytes.slice(); },
      async deleteJson() { events.push('delete'); stored = null; },
    },
  };
  return { options, events, credential, seedStored() { stored = new Uint8Array([7]); } };
}
test('Root-aware composition loads before preparation, verifies before retaining, and never erases a verified Part on cancellation', async () => {
  const f = verifiedFixture(), scope = await openVerifiedBrowserSporeMembership(f.options);
  assert.deepEqual(f.events, ['read', 'prepare']);
  await assert.rejects(scope.accept(null), /requires a signed/);
  await assert.rejects(scope.accept({ signature: 'invalid' }), /signature refused/);
  const verified = await scope.accept({ signature: 'valid' }, f.credential);
  assert.deepEqual(verified, f.credential);
  assert.deepEqual(f.events.slice(-3), ['verify', 'export', 'write']);
  scope.cancelPending(); assert.equal(f.events.some(event => Array.isArray(event)), false);
  await assert.rejects(scope.accept({ signature: 'valid' }), /No pending/);
});
test('refuses different Root, producer, prepared Boot or offer generation before receipt callback', async () => {
  for (const change of [o => o.sporeRoot.rootAuthority.body_id = 'foreign',
    o => o.sporeRoot.rootProducer.host_id = 'foreign',
    o => o.membership.bootId = 'foreign', o => o.advertisement.offer_generation = 2]) {
    const f = verifiedFixture(); change(f.options);
    await assert.rejects(openVerifiedBrowserSporeMembership(f.options));
    assert.equal(f.events.includes('verify'), false);
    assert.equal(f.events.includes('write'), false);
  }
});
test('pending cancellation erases only unverified scope; storage pressure preserves the verified signer', async () => {
  const f = verifiedFixture(), scope = await openVerifiedBrowserSporeMembership(f.options);
  scope.cancelPending(); scope.cancelPending();
  assert.equal(f.events.filter(event => Array.isArray(event)).length, 1);
  await assert.rejects(scope.accept({ signature: 'valid' }), /No pending/);
  const g = verifiedFixture(); g.options.storage.writeBytes = async () => { throw Error('quota'); };
  const accepted = await openVerifiedBrowserSporeMembership(g.options);
  await assert.rejects(accepted.accept({ signature: 'valid' }), /quota/);
  accepted.cancelPending(); assert.equal(g.events.some(event => Array.isArray(event)), false);
});
test('fresh Boot Return requires selected verified Part custody and preserves its original admission credential', async () => {
  const f = verifiedFixture(); f.seedStored();
  const scope = await openVerifiedBrowserSporeMembership(f.options);
  assert.equal(scope.returning, true); assert.equal(scope.priorCredential.boot_id, 'boot/prior');
  assert.deepEqual(f.events, ['read', 'import']);
  assert.ok(f.options.invitedProvision.secret.every(byte => byte === 0));
  scope.cancelPending(); assert.deepEqual(f.events, ['read', 'import']);
  const g = verifiedFixture(); g.options.retainedCredential = g.credential;
  await assert.rejects(openVerifiedBrowserSporeMembership(g.options), /no matching verified Part/);
  const h = verifiedFixture(); h.seedStored(); h.options.retainedCredential = { ...h.credential, part_id: 'foreign' };
  await assert.rejects(openVerifiedBrowserSporeMembership(h.options), /no matching verified Part/);
});
