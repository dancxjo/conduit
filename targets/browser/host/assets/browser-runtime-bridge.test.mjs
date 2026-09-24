import test from "node:test";
import assert from "node:assert/strict";
import { bindBrowserRuntimeBridge } from "./browser-runtime-bridge.mjs";

const ABI = new TextEncoder().encode("conduit.browser/runtime-abi");

function runtime({ revision = 1, identity = ABI, inputCapacity = 64, inputPointer = 0, outputCapacity = 1024, outputPointer = 0 } = {}) {
  const memory = new WebAssembly.Memory({ initial: 1 });
  const state = { workspaceOutputLength: 0, bodyOutputLength: 0 };
  const api = {
    memory,
    conduit_browser_runtime_abi_revision: () => revision,
    conduit_browser_runtime_abi_identity_ptr: () => 1024,
    conduit_browser_runtime_abi_identity_len: () => identity.length,
    conduit_workspace_input_ptr: () => inputPointer,
    conduit_workspace_input_capacity: () => inputCapacity,
    conduit_workspace_output_ptr: () => outputPointer,
    conduit_workspace_output_len: () => state.workspaceOutputLength,
    conduit_workspace_output_capacity: () => outputCapacity,
    conduit_workspace_request: () => 0,
    conduit_creche_input_ptr: () => 0,
    conduit_creche_input_capacity: () => 1024,
    conduit_creche_output_ptr: () => 0,
    conduit_creche_output_len: () => 0,
    conduit_creche_current: () => 1,
    conduit_creche_durable_snapshot: () => 1,
    conduit_creche_attach_here: () => 0,
    conduit_creche_restore_durable: () => 0,
    conduit_browser_body_input_ptr: () => 0,
    conduit_browser_body_input_capacity: () => 1024,
    conduit_browser_body_start: () => 0,
    conduit_browser_form_input_ptr: () => 0,
    conduit_browser_form_input_capacity: () => 1024,
    conduit_browser_form_output_ptr: () => 0,
    conduit_browser_form_output_len: () => state.bodyOutputLength,
    conduit_browser_form_acknowledge_cancellation: () => 0,
    conduit_browser_form_complete_effect: () => 0,
    conduit_browser_form_refuse_effect: () => 0,
    setWorkspaceOutput(bytes) {
      new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
      state.workspaceOutputLength = bytes.length;
    },
    setWorkspaceOutputLength(length) { state.workspaceOutputLength = length; },
    setBodyOutput(bytes) {
      new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
      state.bodyOutputLength = bytes.length;
    },
  };
  new Uint8Array(memory.buffer, 1024, identity.length).set(identity);
  return api;
}

test("refuses unsupported runtime ABI revision", () => {
  assert.throws(() => bindBrowserRuntimeBridge(runtime({ revision: 2 }), { context: "bridge proof" }), (error) =>
    error?.code === "IncompatibleRuntimeAbi" && error.runtime_abi_revision === 2);
});

test("refuses malformed runtime ABI revision export", () => {
  const api = runtime();
  api.conduit_browser_runtime_abi_revision = () => "broken";
  assert.throws(() => bindBrowserRuntimeBridge(api, { context: "bridge proof" }), /malformed runtime ABI revision export/);
});

test("refuses ABI identity mismatch", () => {
  const api = runtime({ identity: new TextEncoder().encode("wrong/runtime-abi") });
  assert.throws(() => bindBrowserRuntimeBridge(api, { context: "bridge proof" }), (error) =>
    error?.code === "IncompatibleRuntimeAbi" && error.runtime_abi_revision === null);
});

test("workspace request retires input even when invocation grows memory", () => {
  const api = runtime({ inputPointer: 256 });
  api.conduit_workspace_request = (length) => {
    api.memory.grow(1);
    api.setWorkspaceOutput(new TextEncoder().encode(JSON.stringify({ ok: true })));
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  bridge.workspaceRequest({ action: "Proof" });
  assert.deepEqual([...new Uint8Array(api.memory.buffer, 256, 16)], new Array(16).fill(0));
});

test("workspace request snapshots a shared input/output arena before retiring input", () => {
  const pointer = 256;
  const api = runtime({ inputPointer: pointer, outputPointer: pointer });
  const expected = { schema: "conduit.workspace/proof@1", state: "admitted" };
  api.conduit_workspace_request = () => {
    const bytes = new TextEncoder().encode(JSON.stringify(expected));
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    api.setWorkspaceOutputLength(bytes.length);
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  const result = bridge.workspaceRequest({ action: "Proof" });
  assert.deepEqual(result.outputJson, expected);
  assert.deepEqual([...new Uint8Array(api.memory.buffer, pointer, 16)], new Array(16).fill(0));
});

test("rejects malformed workspace input capacity", () => {
  const api = runtime();
  api.conduit_workspace_input_capacity = () => Number.NaN;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace input exceeds its bound/);
});

test("rejects malformed workspace input pointer", () => {
  const api = runtime();
  api.conduit_workspace_input_ptr = () => Number.MAX_SAFE_INTEGER;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace input exceeds its bound/);
});

test("rejects malformed workspace output capacity", () => {
  const api = runtime({ outputCapacity: Number.NaN });
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace output exceeds its bound/);
});

test("rejects workspace output pointers outside current memory", () => {
  const api = runtime({ outputPointer: Number.MAX_SAFE_INTEGER });
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace output exceeds its bound/);
});

test("rejects malformed workspace output lengths", () => {
  const api = runtime();
  api.conduit_workspace_output_len = () => -1;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace output exceeds its bound/);
});

test("workspace binary request permits empty output", () => {
  const api = runtime();
  api.conduit_workspace_request = () => 0;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  const result = bridge.workspaceRequest({ action: "BinaryProof" });
  assert.equal(result.status, 0);
  assert.equal(result.outputBytes.length, 0);
  assert.equal(result.outputJson, null);
});

test("workspace binary request preserves opaque non-UTF-8 output", () => {
  const api = runtime();
  api.conduit_workspace_request = () => {
    api.setWorkspaceOutput(Uint8Array.of(0xff, 0x00, 0xfe));
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  const result = bridge.workspaceRequest({ action: "BinaryProof" }, { binary: true });
  assert.deepEqual([...result.outputBytes], [0xff, 0x00, 0xfe]);
  assert.equal(result.outputJson, null);
});
