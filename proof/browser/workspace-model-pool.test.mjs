import test from "node:test";
import assert from "node:assert/strict";

import { prepareWorkspaceModelPool } from "../../products/workspace/browser/workspace-model-pool.mjs";

function joined(index, calls) {
  const hostId = `host/model-${index}`;
  const bootId = `boot/model-${index}/1`;
  return {
    host_id: hostId,
    boot_id: bootId,
    advertisement: { host_id: hostId, boot_id: bootId, offer_generation: 1 },
    line: {
      schema: "conduit.creche/joined-host-line@1",
      async observeLocalModelPool(realization) {
        calls.push(realization.host_id);
        return {
          host_id: hostId, boot_id: bootId, offer_generation: 1,
          capability_id: realization.capability_id,
          implementation_id: realization.implementation_id,
          artifact_id: realization.artifact_id,
          health: "Ready", sign_id: `sign/${hostId}/current`, resources: [],
        };
      },
    },
  };
}

function realization(index) {
  return {
    host_id: `host/model-${index}`, boot_id: `boot/model-${index}/1`, offer_generation: 1,
    capability_id: `capability/model-${index}/generate`, implementation_id: "std/local-model@1",
    artifact_id: "model/shared-digest", member_capacity: 1, resources: [],
  };
}

test("Workspace maps kernel selection to one exact observed joined Host", async () => {
  const observed = [];
  const transitions = [];
  const joinedHosts = [joined(0, observed), joined(1, observed)];
  const plan = {
    plan_id: "plan/model-pool",
    fragments: [{
      host_id: "host/browser", boot_id: "boot/browser/1", offer_generation: 3,
      shared_pools: [{ pool_id: "model/workers", realization_envelope: [realization(0), realization(1)] }],
    }],
  };
  const runtime = {
    admit({ operationId, observations }) {
      assert.equal(observations.length, 2);
      return {
        disposition: "selected",
        member: { realization: operationId.endsWith("2") ? 1 : 0 },
        evidence: { operation_id: operationId },
      };
    },
    trigger(member) { transitions.push(["trigger", member.realization]); },
    failPreparation(member) { transitions.push(["preparation-failed", member.realization]); },
    release(member) { transitions.push(["release", member.realization]); },
    providerLost({ member }) {
      transitions.push(["provider-lost", member.realization]);
      return { evidence: { disposition: "ProviderLost" } };
    },
    close() { transitions.push(["close"]); },
  };
  const pool = prepareWorkspaceModelPool({
    api: {}, plan,
    localAdvertisement: { host_id: "host/browser", boot_id: "boot/browser/1", offer_generation: 3 },
    joinedHosts, poolId: "model/workers", openPool: () => runtime,
  });
  const first = await pool.admit({
    operationId: "request/1", key: new Uint8Array(32), selectionSignId: "sign/select/1",
  });
  const second = await pool.admit({
    operationId: "request/2", key: new Uint8Array(32).fill(2), selectionSignId: "sign/select/2",
  });
  const third = await pool.admit({
    operationId: "request/3", key: new Uint8Array(32).fill(3), selectionSignId: "sign/select/3",
  });
  assert.equal(first.joined, joinedHosts[0]);
  assert.equal(second.joined, joinedHosts[1]);
  assert.deepEqual(observed, [
    "host/model-0", "host/model-1", "host/model-0", "host/model-1",
    "host/model-0", "host/model-1",
  ]);
  pool.activate(first);
  await pool.providerLost({ admission: first, operationId: "request/1", lossSignId: "sign/lost/1" });
  pool.activate(second);
  pool.release(second);
  pool.failPreparation(third);
  pool.close();
  assert.deepEqual(transitions, [
    ["trigger", 0], ["provider-lost", 0], ["trigger", 1], ["release", 1],
    ["preparation-failed", 0], ["close"],
  ]);
});

test("Workspace refuses a stale or ambiguous joined Host mapping before observing", () => {
  const plan = {
    plan_id: "plan/model-pool",
    fragments: [{
      host_id: "host/browser", boot_id: "boot/browser/1", offer_generation: 1,
      shared_pools: [{ pool_id: "model/workers", realization_envelope: [realization(0)] }],
    }],
  };
  const duplicate = joined(0, []);
  assert.throws(() => prepareWorkspaceModelPool({
    api: {}, plan,
    localAdvertisement: { host_id: "host/browser", boot_id: "boot/browser/1", offer_generation: 1 },
    joinedHosts: [duplicate, duplicate], poolId: "model/workers", openPool: () => ({}),
  }), /no unique exact joined Host Line/);
});
