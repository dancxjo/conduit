import test from "node:test";
import assert from "node:assert/strict";
import { openBrowserRemoteFragment, runBrowserRemoteFragment } from "../../targets/browser/host/assets/browser-remote-fragment.mjs";

function fixture() {
  const memory = { buffer: new ArrayBuffer(1024 * 1024) };
  const inputPointer = 512 * 1024;
  let outputLength = 0, cancelled = 0, startRequest = null;
  const output = value => {
    const bytes = new TextEncoder().encode(JSON.stringify(value));
    new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
    outputLength = bytes.length;
  };
  const input = length => new Uint8Array(memory.buffer, inputPointer, length).slice();
  const api = {
    memory,
    conduit_browser_remote_input_ptr: () => inputPointer,
    conduit_browser_remote_input_capacity: () => 256 * 1024,
    conduit_browser_remote_output_ptr: () => 0,
    conduit_browser_remote_output_len: () => outputLength,
    conduit_browser_remote_endpoint_count: () => 1,
    conduit_browser_remote_start(length) {
      startRequest = JSON.parse(new TextDecoder().decode(input(length)));
      output({ schema: "conduit.browser/remote-fragment-started@1", plan_id: "plan/voice",
        active_play_id: "play/browser", endpoints: [0], egress_endpoints: [0], initial_frames: [[1], [2]] });
      return 0;
    },
    conduit_browser_remote_exchange(length) {
      assert.deepEqual([...input(length)], [3]);
      output({ schema: "conduit.browser/remote-session-exchange@1", endpoint: 0, message: "ready", active: true, responses: [[4], [5]] });
      return 0;
    },
    conduit_browser_remote_drive() {
      output({ schema: "conduit.browser/remote-host-effect@1", effect_kind: "audio-capture" });
      return 1;
    },
    conduit_browser_remote_complete_effect(length, present) {
      assert.equal(present, 1); assert.deepEqual([...input(length)], [6]); return 0;
    },
    conduit_browser_remote_offer_frame(endpoint) {
      assert.equal(endpoint, 0); output({ schema: "conduit.browser/remote-session-frame@1", frame: [7] }); return 5;
    },
    conduit_browser_remote_finish() {
      output({ schema: "conduit.browser/remote-session-finished@1", frames: [[8], [9]] }); return 8;
    },
    conduit_browser_remote_cancel_frames() {
      output({ schema: "conduit.browser/remote-session-cancelled@1", frames: [[11], [12]] }); return 8;
    },
    conduit_browser_remote_cancel() { cancelled++; return 0; },
  };
  const options = { api, plan: { plan_id: "plan/voice", fragments: [
      { host_id: "host/browser", boot_id: "boot/browser" },
      { host_id: "host/voice", boot_id: "boot/voice" },
    ] },
    host: { host_id: "host/browser", boot_id: "boot/browser" },
    preparation: { identity: { host_id: "host/voice", boot_id: "boot/voice", plan_id: "plan/voice", active_play_id: "play/voice" }, hello_frames: [[10]] },
    observations: [{ class_id: "audio/input" }] };
  return { api, options, request: () => startRequest, cancelled: () => cancelled };
}

test("browser remote adapter preserves exact identity and relays only opaque session frames", () => {
  const f = fixture();
  const remote = openBrowserRemoteFragment(f.options);
  assert.equal(f.request().plan.plan_id, "plan/voice");
  assert.equal(f.request().host.host_id, "host/browser");
  assert.equal(f.request().active_play_id, undefined);
  assert.deepEqual(f.request().session_hellos, [[10]]);
  assert.deepEqual(remote.initialFrames.map(frame => [...frame]), [[1], [2]]);
  assert.deepEqual(remote.exchange(Uint8Array.of(3)).responses.map(frame => [...frame]), [[4], [5]]);
  assert.equal(remote.drive().output.effect_kind, "audio-capture");
  remote.completeEffect(Uint8Array.of(6));
  assert.deepEqual([...remote.offer(0)], [7]);
  assert.deepEqual(remote.finish().map(frame => [...frame]), [[8], [9]]);
  remote.close();
  assert.equal(f.cancelled(), 0);
});

test("browser remote cancellation is an explicit bounded Wire outcome", () => {
  const f = fixture();
  const remote = openBrowserRemoteFragment(f.options);
  assert.deepEqual(remote.cancel(7).map(frame => [...frame]), [[11], [12]]);
  remote.close();
  assert.equal(f.cancelled(), 0);
});

test("remote driver performs local effects and relays opaque frames through bilateral completion", async () => {
  const sent = [], incoming = [Uint8Array.of(20), Uint8Array.of(21), Uint8Array.of(22), Uint8Array.of(23)];
  const line = { schema: "conduit.creche/joined-host-line@1",
    async sendSessionFrame(frame) { sent.push([...frame]); },
    async receiveSessionFrame() { return incoming.shift(); } };
  const exchanges = [
    { endpoint: 0, message: "ready", active: true, responses: [] },
    { endpoint: 0, message: "accepted", active: true, responses: [] },
    { endpoint: 0, message: "delivered", active: true, responses: [] },
    { endpoint: 0, message: "terminal", active: false, responses: [] },
  ];
  let drives = 0, completed = null, offered = false;
  const remote = {
    identity: { active_play_id: "play/browser" }, endpoints: [0], egressEndpoints: [0], initialFrames: [Uint8Array.of(1), Uint8Array.of(2)],
    exchange(frame) { assert.ok(frame); return exchanges.shift(); },
    drive() { drives++; return drives === 1
      ? { status: 1, output: { effect_kind: "audio-capture" } }
      : drives < 4 ? { status: 2, output: {} } : { status: 4, output: null }; },
    completeEffect(value) { completed = [...value]; },
    offer() { if (offered) return null; offered = true; return Uint8Array.of(3); },
    finish() { return [Uint8Array.of(4), Uint8Array.of(5)]; },
  };
  const receipt = await runBrowserRemoteFragment({ remote, line,
    perform: async effect => { assert.equal(effect.effect_kind, "audio-capture"); return Uint8Array.of(9); },
    signal: new AbortController().signal });
  assert.deepEqual(completed, [9]);
  assert.deepEqual(sent, [[1], [2], [3], [4], [5]]);
  assert.deepEqual(receipt, { disposition: "completed", active_play_id: "play/browser" });
});

test("browser remote adapter refuses stale identity and cancels unfinished ownership", () => {
  const f = fixture();
  f.options.preparation.identity.boot_id = "stale";
  assert.throws(() => openBrowserRemoteFragment(f.options), /differs from the exact planned/);
  f.options.preparation.identity.boot_id = "boot/voice";
  const remote = openBrowserRemoteFragment(f.options);
  assert.throws(() => openBrowserRemoteFragment(f.options), /already owns/);
  remote.close();
  assert.equal(f.cancelled(), 1);
  assert.throws(() => remote.drive(), /closed/);
  openBrowserRemoteFragment(f.options).close();
  assert.equal(f.cancelled(), 2);
});
