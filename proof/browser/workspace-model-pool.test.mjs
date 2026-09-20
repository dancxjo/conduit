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
      async preparePoolMember(request) {
        calls.push(`prepare:${request.consumerPlacementId}:${index}`);
        return {
          identity: { plan_id: request.plan.plan_id, host_id: hostId },
          hello_frames: [[10], [11]],
        };
      },
      async sendSessionFrame(frame) { calls.push(`send:${index}:${frame[0]}`); },
      async receiveSessionFrame() { return new Uint8Array([20]); },
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
      shared_pools: [{
        pool_id: "model/workers", consumers: ["placement/client"],
        realization_envelope: [realization(0), realization(1)],
      }],
    }],
  };
  const runtime = {
    admit({ operationId, observations }) {
      assert.equal(observations.length, 2);
      return {
        disposition: "selected",
        member: { realization: operationId.endsWith("2") ? 1 : 0 },
        evidence: {
          plan_id: plan.plan_id, pool_id: "model/workers", operation_id: operationId,
          selected_realization: operationId.endsWith("2") ? 1 : 0,
          observation_sign_ids: ["sign/provider/current"], disposition: "Selected",
          sign_id: `sign/selection/${operationId}`,
        },
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
    openMemberClient() {
      let exchange = 0;
      return {
        initialFrames: [new Uint8Array([12]), new Uint8Array([13]),
          new Uint8Array([14]), new Uint8Array([15])],
        offer(prompt) {
          assert.deepEqual([...prompt], [1, 2, 3]);
          return new Uint8Array([16]);
        },
        exchange() {
          exchange += 1;
          if (exchange <= 2) return { message: "ready", responses: [], result: null };
          if (exchange === 3) return { message: "accepted", responses: [], result: null };
          if (exchange === 4) return { message: "delivered", responses: [], result: null };
          return {
            message: "offered", responses: [new Uint8Array([17]), new Uint8Array([18])],
            result: new Uint8Array([42]),
          };
        },
        close() { observed.push("client:closed"); },
      };
    },
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
  const prepared = await pool.prepare(first, "placement/client");
  assert.equal(prepared.identity.host_id, "host/model-0");
  assert.deepEqual(observed, [
    "host/model-0", "host/model-1", "host/model-0", "host/model-1",
    "host/model-0", "host/model-1", "prepare:placement/client:0",
  ]);
  pool.activate(first);
  const executed = await pool.execute({
    admission: first, preparation: prepared, consumerPlacementId: "placement/client",
    prompt: new Uint8Array([1, 2, 3]),
  });
  assert.deepEqual([...executed.result], [42]);
  assert.equal(executed.hostId, "host/model-0");
  const receipt = pool.receipt({
    bodyId: "body/1", wakeId: "wake/1", sourceDocumentId: "source/1",
    checkedFormId: "checked/1", expandedFormId: "expanded/1", playId: "play/1",
  });
  assert.equal(receipt.schema, "conduit.proof/live-local-model-pool@1");
  assert.equal(receipt.operations.length, 1);
  assert.equal(receipt.operations[0].operation_id, "request/1");
  assert.equal(receipt.operations[0].result_bytes, 1);
  assert.equal(receipt.prompt_content_retained, false);
  assert.equal(JSON.stringify(receipt).includes("1,2,3"), false);
  assert.deepEqual(observed.slice(-8, -1), [
    "send:0:12", "send:0:13", "send:0:14", "send:0:15", "send:0:16",
    "send:0:17", "send:0:18",
  ]);
  assert.equal(observed.at(-1), "client:closed");
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
