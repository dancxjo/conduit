import assert from "node:assert/strict";
import test from "node:test";
import { createPortableSeedAdmission } from "./browser-sdk-seed-admission.mjs";

// CPU wrapper fixture models the runtime boundary, not its authority checks.
// The Rust tests exercise actual core admission and exact receipt correlation.
function fixture() {
  const memory = new WebAssembly.Memory({ initial: 2 });
  const decoder = new TextDecoder(), encoder = new TextEncoder();
  const calls = [], writes = [];
  let length = 0, refuse = false, storageFailure = false;
  const credential = { body_id: "body/one", host_id: "host/one", boot_id: "boot/one", part_id: "part/one" };
  const invoke = name => (size, low, high, authorized) => {
    calls.push({ name, document: JSON.parse(decoder.decode(new Uint8Array(memory.buffer, 0, size))), low, high, authorized });
    if (refuse) return -270;
    const value = name === "prepare" ? { schema: "conduit.body/spawn-admission-request@1", body_id: "body/one", membership_admitted: false } : credential;
    const bytes = encoder.encode(JSON.stringify(value)); length = bytes.length;
    new Uint8Array(memory.buffer, 0, length).set(bytes); return 0;
  };
  const api = { memory,
    conduit_browser_seed_input_ptr: () => 0, conduit_browser_seed_input_capacity: () => 65536,
    conduit_browser_membership_output_ptr: () => 0, conduit_browser_membership_output_len: () => length,
    conduit_browser_seed_prepare: invoke("prepare"), conduit_browser_seed_complete: invoke("complete"),
    conduit_browser_seed_cancel: () => { calls.push({ name: "cancel" }); return 0; },
    conduit_browser_seed_credential: () => 0,
  };
  const admission = createPortableSeedAdmission({ api, assertCurrent() {}, now: () => 0x100000001,
    storage: { async writeJson(key, value) { if (storageFailure) throw new Error("storage full"); writes.push({ key, value: structuredClone(value) }); } },
  });
  return { admission, api, calls, writes, credential, refuse: value => { refuse = value; }, storageFailure: value => { storageFailure = value; } };
}

test("explicit human grants precede runtime validation and exact receipt retention", async () => {
  const f = fixture();
  const invitation = { schema: "conduit.body/spawn-invitation@1", claim: { body_id: "body/one" }, secret: [1] };
  assert.throws(() => f.admission.prepare({ invitation }), /authorization/);
  assert.equal(f.calls.length, 0);
  const request = f.admission.prepare({ invitation, authorizeJoin: true });
  assert.equal(request.membership_admitted, false);
  assert.equal(f.admission.credential(), null);
  assert.deepEqual(f.calls[0], { name: "prepare", document: invitation, low: 1, high: 1, authorized: 1 });
  const receipt = { schema: "conduit.body/spawn-admission-receipt@1", credential: f.credential, membership_admitted: true };
  await assert.rejects(f.admission.complete({ receipt }), /human authorization/);
  assert.equal(f.writes.length, 0);
  assert.deepEqual(await f.admission.complete({ receipt, authorizeMembership: true }), f.credential);
  assert.deepEqual(f.writes[0].value, receipt);
  assert.match(f.writes[0].key, /^portable-seed-admission-receipt\/[a-f0-9]{64}$/);
  await assert.rejects(f.admission.complete({ receipt, authorizeMembership: true }), /unconsumed/);
  assert.throws(() => f.admission.prepare({ invitation, authorizeJoin: true }), /already selected/);
  assert.throws(() => f.admission.assertMayOwnBody(), /already selected/);
});

test("runtime refusal is never retained and pending cancellation cannot restart the Boot", async () => {
  const f = fixture();
  f.admission.prepare({ invitation: {}, authorizeJoin: true });
  f.refuse(true);
  await assert.rejects(f.admission.complete({ receipt: {}, authorizeMembership: true }), /refused/);
  assert.equal(f.writes.length, 0);
  assert.equal(f.admission.credential(), null);
  f.admission.cancel();
  assert.throws(() => f.admission.prepare({ invitation: {}, authorizeJoin: true }), /already selected/);
  await f.admission.close();
  assert.throws(() => f.admission.cancel(), /closed/);
});

test("failed persistence can retry exact verified receipt without a second import", async () => {
  const f = fixture();
  f.admission.prepare({ invitation: {}, authorizeJoin: true });
  f.storageFailure(true);
  await assert.rejects(f.admission.complete({ receipt: { schema: "conduit.body/spawn-admission-receipt@1", credential: f.credential }, authorizeMembership: true }), /storage full/);
  assert.equal(f.admission.credential(), null);
  f.storageFailure(false);
  assert.deepEqual(await f.admission.retryRetention(), f.credential);
  assert.equal(f.calls.filter(call => call.name === "complete").length, 1);
  assert.equal(f.writes.length, 1);
});

test("old runtime and owned Body paths refuse before any portable signing", () => {
  const f = fixture();
  f.admission.assertMayOwnBody();
  assert.throws(() => f.admission.prepare({ invitation: {}, authorizeJoin: true }), /already selected/);
  delete f.api.conduit_browser_seed_complete;
  assert.throws(() => f.admission.cancel(), /lacks portable/);
  assert.equal(f.calls.length, 0);
});
