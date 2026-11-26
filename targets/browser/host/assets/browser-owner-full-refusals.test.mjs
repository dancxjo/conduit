import test from "node:test";
import assert from "node:assert/strict";
import { bindBrowserRuntimeBridge } from "./browser-runtime-bridge.mjs";
import { runtime } from "./browser-runtime-bridge-fixture.mjs";

// ABI wrapper conformance only: real selected-frame refusal tests live in Rust.
test("Full preparation refusals preserve stable status and category while legacy errors stay unchanged", () => {
  const api = runtime({ inputCapacity: 4096 });
  api.conduit_browser_owner_face_input_ptr = () => 0;
  api.conduit_browser_owner_face_input_capacity = () => 4096;
  let status = 0;
  api.conduit_browser_owner_face_prepare = () => status;
  const bridge = bindBrowserRuntimeBridge(api, { context: "Full refusal conformance" });
  const names = ["OwnerFaceMalformed", "OwnerFaceUnsupported", "OwnerFaceIdentityMismatch", "OwnerFaceInvalidSeal", "OwnerFacePressure", "OwnerFaceLifecycle", "OwnerFacePreparationFailed"];
  const frame = new TextEncoder().encode('{}');
  for (const [offset, name] of names.entries()) {
    status = -270 - offset;
    assert.throws(() => bridge.ownerFacePrepare({}, frame), error =>
      error.status === status && error.code === name);
    assert.deepEqual([...new Uint8Array(api.memory.buffer, 0, 4)], [0, 0, 0, 0]);
  }
  status = -10;
  assert.throws(() => bridge.ownerFacePrepare({}, frame), error =>
    error.message === "owner Face Mask refused (-10)" && error.code === undefined && error.status === undefined);
});
