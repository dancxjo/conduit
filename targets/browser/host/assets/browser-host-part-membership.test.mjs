import test from 'node:test';
import assert from 'node:assert/strict';
import { initializeBrowserHostInstance } from './browser-host-membership.mjs';
const encoder = new TextEncoder(), decoder = new TextDecoder();
async function fixture() {
  const memory = new WebAssembly.Memory({ initial: 1 });
  let length = 0, imported = null;
  const output = value => { const bytes = encoder.encode(JSON.stringify(value)); new Uint8Array(memory.buffer, 8192, bytes.length).set(bytes); length = bytes.length; };
  const read = size => JSON.parse(decoder.decode(new Uint8Array(memory.buffer, 0, size)));
  const api = {
    memory, conduit_browser_membership_input_ptr: () => 0, conduit_browser_membership_input_capacity: () => 4096,
    conduit_browser_membership_output_ptr: () => 8192, conduit_browser_membership_output_len: () => length,
    conduit_browser_membership_initialize() { new Uint8Array(memory.buffer, 8192, 32).fill(7); length = 32; return 0; },
    conduit_browser_membership_advertisement() { return 0; }, conduit_browser_membership_prove() { return 0; },
    conduit_browser_membership_prove_return() { return 0; }, conduit_browser_membership_prove_spawn() { return 0; },
    conduit_browser_membership_export_part(size) {
      assert.deepEqual(read(size), { expected_body_id: 'body/test', expected_root: Array(32).fill(11) });
      output({ secret: Array(32).fill(9) }); return 0;
    },
    conduit_browser_membership_import_part(size) {
      imported = read(size);
      output({ body_id: 'body/test', part_id: 'part/test' }); return 0;
    },
    conduit_browser_membership_retire_scope(size) {
      assert.deepEqual(read(size), { body_id: 'body/test', part_id: 'part/test' }); output({}); return 0;
    },
  };
  const host = await initializeBrowserHostInstance({ exports: api }, { durable: false, bootId: 'boot/test' });
  new Uint8Array(memory.buffer, 0, 4096).fill(0); // fixture initialization does not implement runtime scrubbing
  return { host, api, memory, imported: () => imported };
}
test('membership adapter passes independent pins and wipes exported private Wasm output/input copies', async () => {
  const f = await fixture(), pins = { expectedBodyId: 'body/test', expectedRoot: Array(32).fill(11) };
  const record = f.host.membership.exportPartMembership(pins);
  assert.deepEqual(JSON.parse(decoder.decode(record)).secret, Array(32).fill(9));
  assert.ok(new Uint8Array(f.memory.buffer, 8192, record.length).every(byte => byte === 0));
  const credential = f.host.membership.importPartMembership(record, pins);
  assert.equal(credential.part_id, 'part/test');
  assert.deepEqual(f.imported().expected_root, pins.expectedRoot);
  assert.ok(new Uint8Array(f.memory.buffer, 0, 4096).every(byte => byte === 0));
  f.host.membership.retireMembershipScope({ expectedBodyId: 'body/test', partId: 'part/test' });
  record.fill(0);
});
test('old runtimes or rejected imports cannot silently substitute a Host seed', async () => {
  const f = await fixture(); delete f.api.conduit_browser_membership_export_part;
  assert.throws(() => f.host.membership.exportPartMembership({ expectedBodyId: 'body/test', expectedRoot: Array(32).fill(11) }), /lacks/);
  f.api.conduit_browser_membership_import_part = () => -265;
  const record = encoder.encode(JSON.stringify({ secret: Array(32).fill(9) }));
  assert.throws(() => f.host.membership.importPartMembership(record,
    { expectedBodyId: 'body/test', expectedRoot: Array(32).fill(11) }), /refused/);
  assert.ok(new Uint8Array(f.memory.buffer, 0, 4096).every(byte => byte === 0));
  record.fill(0);
});
