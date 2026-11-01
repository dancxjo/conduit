import test from 'node:test';
import assert from 'node:assert/strict';
import { openBrowserPartMembershipStore } from './browser-part-membership.mjs';
function fixture() {
  let stored = null, writtenKey = null, exported = null, imported = null, retired = null;
  const root = Array(32).fill(11);
  const membership = {
    exportPartMembership(pins) {
      assert.equal(pins.expectedBodyId, 'body/test'); assert.deepEqual(pins.expectedRoot, Array(32).fill(11));
      exported = new Uint8Array([7, 8, 9]); return exported;
    },
    importPartMembership(bytes, pins) {
      assert.equal(pins.expectedBodyId, 'body/test'); assert.deepEqual(pins.expectedRoot, Array(32).fill(11));
      imported = bytes;
      assert.deepEqual(Array.from(bytes), [7, 8, 9]);
      return { body_id: 'body/test', part_id: 'part/test', host_id: 'browser/actual-host' };
    },
    retireMembershipScope(scope) { retired = scope; },
  };
  const storage = {
    async writeBytes(key, bytes) { writtenKey = key; stored = bytes.slice(); },
    async readBytes(key) { assert.equal(key, writtenKey); return stored?.slice() ?? null; },
    async deleteJson(key) { assert.equal(key, writtenKey); stored = null; },
  };
  return { root, membership, storage, snapshot: () => ({ stored, exported, imported, retired, writtenKey }) };
}
test('uses selected storage and runtime-verifiable bytes without generating or exposing a Host seed', async () => {
  const f = fixture();
  const store = await openBrowserPartMembershipStore({ membership: f.membership, storage: f.storage,
    expectedBodyId: 'body/test', expectedRoot: f.root });
  f.root.fill(99); // caller mutation cannot replace the pinned Root
  assert.equal((await store.retain()).state, 'retained-private-part');
  assert.ok(f.snapshot().exported.every(byte => byte === 0));
  assert.match(f.snapshot().writtenKey, /^part-membership\/[0-9a-f]{64}$/);
  assert.equal((await store.load()).admissionCredential.part_id, 'part/test');
  assert.ok(f.snapshot().imported.every(byte => byte === 0));
  assert.deepEqual(Array.from(f.snapshot().stored), [7, 8, 9]);
  await store.forget('part/test');
  assert.deepEqual(f.snapshot().retired, { expectedBodyId: 'body/test', partId: 'part/test' });
  assert.equal(await store.load(), null);
});
test('storage failure reports refusal, wipes transfer bytes and invents no durable state', async () => {
  const f = fixture(); f.storage.writeBytes = async () => { throw new Error('quota'); };
  const store = await openBrowserPartMembershipStore({ ...f, expectedBodyId: 'body/test', expectedRoot: f.root });
  await assert.rejects(store.retain(), /quota/);
  assert.ok(f.snapshot().exported.every(byte => byte === 0));
  assert.equal(f.snapshot().stored, null);
  assert.equal(f.snapshot().retired, null, 'retryable storage pressure does not discard the only verified Part key');
});
test('malformed records and absent runtime/storage implementations fail without seed fallback', async () => {
  const f = fixture(); f.storage.readBytes = async () => new Uint8Array(3073).fill(9);
  const store = await openBrowserPartMembershipStore({ ...f, expectedBodyId: 'body/test', expectedRoot: f.root });
  await assert.rejects(store.load(), /bound/); assert.equal(f.snapshot().imported, null);
  await assert.rejects(openBrowserPartMembershipStore({ membership: f.membership, expectedBodyId: 'body/test', expectedRoot: f.root }));
  await assert.rejects(openBrowserPartMembershipStore({ storage: f.storage, membership: {}, expectedBodyId: 'body/test', expectedRoot: f.root }));
});
test('Root verifier refusal prevents recovered membership and erases the read copy', async () => {
  const f = fixture(); let readCopy;
  f.storage.readBytes = async () => { readCopy = new Uint8Array([7, 8, 9]); return readCopy; };
  f.membership.importPartMembership = () => { throw new Error('Root signature refused'); };
  const store = await openBrowserPartMembershipStore({ ...f, expectedBodyId: 'body/test', expectedRoot: f.root });
  await assert.rejects(store.load(), /Root signature/);
  assert.ok(readCopy.every(byte => byte === 0));
});
