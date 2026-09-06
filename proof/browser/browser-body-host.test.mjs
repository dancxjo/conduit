import test from "node:test";
import assert from "node:assert/strict";
import { acquireBrowserBodyHost } from "../../targets/browser/host/assets/browser-body-host.mjs";

function fixture({ timer = false, text = "hello" } = {}) {
  const memory = { buffer: new ArrayBuffer(512 * 1024) };
  let length = 0, starts = 0, cancels = 0, request;
  const output = value => {
    const bytes = new TextEncoder().encode(JSON.stringify(value));
    new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);length = bytes.length;
  };
  const effect = { host_id: "host", boot_id: "boot", active_play_id: "play", placement_id: "placement", plan_id: "partition", request_sequence: 0,
    ...(timer ? { effect_kind: "timer", duration_millis: 10_000 } : { effect_kind: "manifestation", presentation_kind: "presentation/text", text }) };
  const api = {
    memory,
    conduit_browser_form_human_machinery() { output({ schema: "conduit.browser/selected-human-machinery@1", implementations: ["browser/dom-presentation@1", "browser/pointer-events@1"] });return 0; },
    conduit_browser_form_output_ptr: () => 0, conduit_browser_form_output_len: () => length,
    conduit_browser_body_input_ptr: () => 256 * 1024, conduit_browser_body_input_capacity: () => 256 * 1024,
    conduit_browser_body_start(length) {
      starts++;request = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, 256 * 1024, length)));
      output({ schema: "conduit.browser/body-started@1", play: { active_play_id: "play" }, progress: effect });return 0;
    },
    conduit_browser_form_pending_capacity: () => 16,
    conduit_browser_form_poll_effect() { output({ disposition: "waiting" });return 0; },
    conduit_browser_form_input_ptr: () => 128 * 1024, conduit_browser_form_input_capacity: () => 64 * 1024,
    conduit_browser_form_complete_effect() { output({ disposition: "completed", active_play_id: "play" });return 0; },
    conduit_tour_cancel() { cancels++;output({ disposition: "cancelled", active_play_id: "play" });return 0; },
  };
  const window = { setTimeout, clearTimeout, performance, crypto };
  const document = { defaultView: window, createElement() { return { dataset: {}, setAttribute() {}, remove() { this.isConnected = false; } }; } };
  const outputRoot = { isConnected: true, ownerDocument: document, children: [], append(element) { this.children.push(element);element.isConnected = true; } };
  const inputTarget = { isConnected: true };
  const resource = timer ? { pool_id: "browser/timer", class_id: "conduit.resource/timer-slot@1", units: 1 } : { pool_id: "browser/presentation", class_id: "conduit.resource/presentation-slot@1", units: 1 };
  const proposal = { schema: "conduit.patchbay/body-execution-proposal@1", wake: { lifecycle: "AwaitingPlan", plans: [] }, plan: { forms: [{ plan: { fragments: [{ host_id: "host", boot_id: "boot", offer_generation: 1, placements: [{ placement_id: "placement", gear_id: "gear", resources: [resource] }] }] } }] } };
  return { api, proposal, outputRoot, inputTarget, hostId: "host", bootId: "boot", count: () => ({ starts, cancels }), request: () => request, output };
}

test("acquisition reports owned slots without starting a Play or copying offer capacity", async () => {
  const f = fixture();
  const owner = acquireBrowserBodyHost(f);
  assert.equal(f.count().starts, 0);
  assert.equal(f.outputRoot.children.length, 1);
  assert.equal(owner.observations()[0].unreserved_units, 1);
  assert.throws(() => acquireBrowserBodyHost(f), /already acquired/);
  const original = structuredClone(f.proposal);
  f.proposal.plan.forms.length = 0;
  owner.start(1);
  assert.deepEqual(f.request().plan, original.plan);
  assert.throws(() => owner.observations(), /reserved/);
  const receipt = await owner.run();
  assert.equal(receipt.disposition, "completed");
  assert.equal(f.outputRoot.children[0].textContent, "hello");
  assert.equal(f.outputRoot.children[0].dataset.activePlayId, "play");
  assert.throws(() => owner.run(), /exactly once/);
  const closed = owner.close();
  assert.equal(closed.status, null);
  assert.equal(closed.receipt, receipt);
  assert.equal(f.count().cancels, 0);
  assert.equal(f.outputRoot.children[0].isConnected, false);
});

test("wrong Boot, excessive demand, unsupported pools, and lost slots refuse", () => {
  for (const change of [
    f => { f.bootId = "wrong"; },
    f => { f.proposal.plan.forms[0].plan.fragments[0].placements[0].resources[0].units = 17; },
    f => { f.proposal.plan.forms[0].plan.fragments[0].placements[0].resources[0].pool_id = "other"; },
  ]) {
    const f = fixture();change(f);
    assert.throws(() => acquireBrowserBodyHost(f));
    assert.equal(f.count().starts, 0);
  }
  const f = fixture(), owner = acquireBrowserBodyHost(f);
  f.outputRoot.children[0].remove();
  assert.throws(() => owner.start(1), /resources lost/);
  assert.equal(f.count().starts, 0);
  owner.close();
});

test("closing pending timer work settles the dispatcher and releases the owner", async () => {
  const f = fixture({ timer: true }), owner = acquireBrowserBodyHost(f);
  owner.start(1);
  const running = owner.run();
  const closed = owner.close();
  assert.equal(closed.receipt.disposition, "cancelled");
  await assert.rejects(running, /timer cancelled/);
  assert.equal(f.count().cancels, 1);
  owner.close();
  assert.equal(f.count().cancels, 1);
  const replacement = acquireBrowserBodyHost(f);
  replacement.close();
});

test("adapter failure is not converted into successful execution", async () => {
  const f = fixture({ text: null }), owner = acquireBrowserBodyHost(f);
  owner.start(1);
  await assert.rejects(owner.run(), /unsupported browser manifestation/);
  owner.close();
  assert.equal(f.count().cancels, 1);
});

test("malformed successful start output still retires the acquired session", () => {
  const f = fixture();
  const start = f.api.conduit_browser_body_start;
  f.api.conduit_browser_body_start = length => {
    start(length);f.output({ schema: "wrong-start-envelope" });return 0;
  };
  const owner = acquireBrowserBodyHost(f);
  assert.throws(() => owner.start(1), /invalid browser Body start output/);
  assert.throws(() => owner.observations(), /reserved/);
  assert.equal(owner.close().receipt.disposition, "cancelled");
  assert.equal(f.count().cancels, 1);
});
