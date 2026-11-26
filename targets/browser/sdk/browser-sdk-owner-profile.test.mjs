import assert from "node:assert/strict";
import test from "node:test";
import { prepareOwnerFaceFullProfile } from "./browser-sdk-owner-profile.mjs";

function fixture({ selected = true, status = 0, ready = status === 0 ? 1 : 0 } = {}) {
  const events = [];
  const memory = new WebAssembly.Memory({ initial: 1 });
  const state = { hostId: "actual-host", boot: { boot_id: "actual-boot", implementation_registry:
    selected ? [{ id: "browser/websocket-owner-face@1", revision: 1 }] : [] }, api: {
    memory,
    conduit_browser_membership_input_capacity: () => 4096,
    conduit_browser_membership_input_ptr: () => 0,
    conduit_browser_membership_owner_full_profile_admit(length) {
      events.push(JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, 0, length))));
      return status;
    },
    conduit_browser_owner_face_full_profile_ready: () => ready,
  } };
  return { state, events };
}

test("checked optional setup prepares actual Host and Boot before advertisement or join", () => {
  const { state, events } = fixture();
  assert.deepEqual(prepareOwnerFaceFullProfile(state, { optional: true }), {
    selected: true, profile: "browser/owner-face-full@1",
  });
  assert.deepEqual(events, [{ host_id: "actual-host", boot_id: "actual-boot", profile: "browser/owner-face-full@1" }]);
});

test("unselected and normally refused Full setup preserve the initialized legacy Host", () => {
  for (const options of [{ selected: false }, { status: -266 }]) {
    const { state, events } = fixture(options);
    const originalApi = state.api;
    assert.deepEqual(prepareOwnerFaceFullProfile(state, { optional: true }), { selected: false });
    assert.equal(state.api, originalApi);
    assert.equal(events.length, options.selected === false ? 0 : 1);
    assert.throws(() => prepareOwnerFaceFullProfile(state), /not selected|preparation refused/);
  }
});

test("unexpected ABI failures or contradictory readiness do not masquerade as optional refusal", () => {
  for (const options of [{ status: -1 }, { status: -266, ready: 1 }, { status: 0, ready: 0 }]) {
    assert.throws(() => prepareOwnerFaceFullProfile(fixture(options).state, { optional: true }), /preparation failed/);
  }
});


test("explicit setup reuses the same verified default preparation without reinitializing live buffers", () => {
  const { state, events } = fixture();
  const selected = prepareOwnerFaceFullProfile(state, { optional: true });
  state.api.conduit_browser_membership_owner_full_profile_admit = () => {
    throw new Error("already initialized profile must retain its live buffers");
  };
  assert.deepEqual(prepareOwnerFaceFullProfile(state), selected);
  assert.equal(events.length, 1);
});
