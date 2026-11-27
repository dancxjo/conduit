import test from "node:test";
import assert from "node:assert/strict";
import { bindBrowserRuntimeBridge } from "./browser-runtime-bridge.mjs";
import { runtime } from "./browser-runtime-bridge-fixture.mjs";

// ABI wrapper conformance only. Actual preparation, authority and retained
// storage are qualified by the native/WASM runtime and owner proof entrances.
test("large reading requires explicitly prepared Full interface and keeps exact canonical u64 bytes", () => {
  const pointer = 1024 * 1024;
  const api = runtime({ inputPointer: 2048, inputCapacity: 600 * 1024,
    outputPointer: pointer, outputCapacity: 1024 * 1024 });
  let ready = 0, length = 0;
  api.conduit_browser_owner_face_full_profile_ready = () => ready;
  api.conduit_browser_owner_face_input_ptr = () => 2048;
  api.conduit_browser_owner_face_input_capacity = () => 600 * 1024;
  api.conduit_browser_owner_face_output_ptr = () => pointer;
  api.conduit_browser_owner_face_output_capacity = () => 1024 * 1024;
  api.conduit_browser_owner_face_output_len = () => length;
  const write = bytes => { new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes); length = bytes.length; };
  const view = { schema: "conduit.browser/owner-face-mask@2", face_revision: "9007199254740993",
    complete_reading: "x".repeat(70 * 1024) };
  api.conduit_browser_owner_face_prepare = () => { write(new TextEncoder().encode(JSON.stringify(view))); return 0; };
  const bridge = bindBrowserRuntimeBridge(api, { context: "Full ABI conformance" });
  const frame = new TextEncoder().encode('{"kind":"face-snapshot-response","response":{"revision":9007199254740993}}');
  const basis = { body_id: "body/source", host_id: "host/current", boot_id: "boot/current" };
  assert.throws(() => bridge.ownerFacePrepare(basis, frame), /exceeds its bound/);
  ready = 1;
  assert.deepEqual(bridge.ownerFacePrepare(basis, frame), view);
  const canonical = new TextEncoder().encode(`{"face":{"revision":9007199254740993,"reading":"${"y".repeat(70 * 1024)}"}}`);
  api.conduit_browser_owner_face_canonical_snapshot = () => { write(canonical); return 0; };
  assert.deepEqual(bridge.ownerFaceCanonicalSnapshot(), canonical);
  api.conduit_browser_owner_face_canonical_snapshot = () => { length = 512 * 1024 + 1; return 0; };
  assert.throws(() => bridge.ownerFaceCanonicalSnapshot(), /exceeds its bound/);
});
