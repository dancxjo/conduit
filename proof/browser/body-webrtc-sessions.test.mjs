import assert from "node:assert/strict";
import test from "node:test";

import { BodyWebRtcSessions } from "../../products/patchbay/html/assets/body-webrtc-sessions.mjs";

function grant(generation) {
  return {
    generation,
    index: 0,
    total: 1,
    grant: {
      negotiation_id: `binding/${generation}`,
      role: "source",
      peer_host_id: "host/peer",
      peer_boot_id: "boot/peer",
      session_hello: [1],
    },
  };
}

test("terminal sessions replan 100,000 times with bounded generation evidence", async () => {
  let requested = null;
  let creations = 0;
  const sessions = new BodyWebRtcSessions({
    wasmBytes: new Uint8Array([0]),
    sendSignal() {},
    requestGrant(generation, index) {
      requested = { generation, index };
    },
    async createSession({ grant: exactGrant }) {
      creations += 1;
      assert.equal(exactGrant.generation, creations - 1);
      let closed = false;
      return {
        close() { closed = true; },
        state() { return { terminalReason: closed ? "closed" : "completed" }; },
        async acceptSignal() {},
      };
    },
  });

  sessions.begin();
  assert.deepEqual(requested, { generation: 0, index: 0 });
  await sessions.acceptGrantFrame(grant(0));
  for (let generation = 1; generation <= 100_000; generation += 1) {
    assert.equal(sessions.replan(), generation);
    assert.deepEqual(requested, { generation, index: 0 });
    await sessions.acceptGrantFrame(grant(generation));
    assert.equal(sessions.state().retiredNegotiations, 1);
  }
  assert.equal(creations, 100_001);
  assert.equal(sessions.state().generation, 100_000);
  assert.equal(sessions.state().activeSessions, 1);

  await assert.rejects(
    sessions.acceptSignal({ signal: { generation: 99_999, negotiation_id: "binding/99999" } }),
    /stale WebRTC signal generation/,
  );
});
