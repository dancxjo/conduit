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
