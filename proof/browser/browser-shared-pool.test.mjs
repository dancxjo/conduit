import test from "node:test";
import assert from "node:assert/strict";

import { openBrowserSharedPool } from "../../targets/browser/host/assets/browser-shared-pool.mjs";

class FakePoolApi {
  constructor() {
    this.memory = { buffer: new ArrayBuffer(64 * 1024) };
    this.output = new Uint8Array();
    this.member = null;
    this.closed = false;
  }
  conduit_browser_shared_pool_input_ptr() { return 0; }
  conduit_browser_shared_pool_input_capacity() { return 32 * 1024; }
  conduit_browser_shared_pool_output_ptr() { return 32 * 1024; }
  conduit_browser_shared_pool_output_len() { return this.output.byteLength; }
  request(length) {
    return JSON.parse(new TextDecoder().decode(new Uint8Array(this.memory.buffer, 0, length)));
  }
  respond(value) {
    this.output = new TextEncoder().encode(JSON.stringify(value));
    new Uint8Array(this.memory.buffer, 32 * 1024, this.output.byteLength).set(this.output);
    return 0;
  }
  conduit_browser_shared_pool_start(length) {
    const request = this.request(length);
    return this.respond({
      schema: "conduit.browser/shared-pool-started@1",
      plan_id: request.plan.plan_id, pool_id: request.pool_id, population: 0,
    });
  }
  conduit_browser_shared_pool_admit(length) {
    const request = this.request(length);
    const refused = request.operation_id === "request/refused";
    this.member = refused ? null : {
      pool: 0, key: request.member_key, slot: 0, epoch: 1,
      node: 10, realization: 1, play: 4,
    };
    return this.respond({
      schema: "conduit.browser/shared-pool-selection@1",
      disposition: refused ? "refused" : "selected",
      ...(this.member ? { member: this.member } : {}),
      evidence: {
        plan_id: "plan/pool", pool_id: "model/workers",
        operation_id: request.operation_id,
        selected_realization: refused ? null : 1,
        observation_sign_ids: ["sign/host-b/current"],
        disposition: refused ? "CapacityRefused" : "Selected",
        sign_id: request.selection_sign_id,
      },
      population: refused ? 1 : 1,
    });
  }
  memberTransition(length) {
    const request = this.request(length);
    return this.respond({
      schema: "conduit.browser/shared-pool-member@1",
      member: request.member, population: 1,
    });
  }
  conduit_browser_shared_pool_trigger(length) { return this.memberTransition(length); }
  conduit_browser_shared_pool_release(length) { return this.memberTransition(length); }
  conduit_browser_shared_pool_provider_lost(length) {
    const request = this.request(length);
    return this.respond({
      schema: "conduit.browser/shared-pool-provider-loss@1",
      member: request.member, population: 1,
      evidence: {
        plan_id: "plan/pool", pool_id: "model/workers",
        operation_id: request.operation_id, selected_realization: 1,
        observation_sign_ids: ["sign/host-b/lost"], disposition: "ProviderLost",
        sign_id: request.loss_sign_id,
      },
    });
  }
  conduit_browser_shared_pool_close() { this.closed = true; }
}

test("browser delegates pool selection and lifecycle to the common finite kernel", () => {
  const api = new FakePoolApi();
  const pool = openBrowserSharedPool({
    api, plan: { plan_id: "plan/pool" }, fragmentIndex: 0,
    poolId: "model/workers", firstMemberNode: 10, play: 4,
  });
  const observations = [{ sign_id: "sign/host-b/current" }];
  const selected = pool.admit({
    operationId: "request/1", key: new Uint8Array(32).fill(1), observations,
    selectionSignId: "sign/selection/1",
  });
  assert.equal(selected.member.realization, 1);
  assert.equal(selected.evidence.disposition, "Selected");
  assert.equal(pool.trigger(selected.member).member.epoch, 1);

  const loss = pool.providerLost({
    operationId: "request/1", member: selected.member,
    observations: [{ sign_id: "sign/host-b/lost" }],
    lossSignId: "sign/provider-lost/1",
  });
  assert.equal(loss.evidence.disposition, "ProviderLost");

  const refused = pool.admit({
    operationId: "request/refused", key: new Uint8Array(32).fill(2), observations,
    selectionSignId: "sign/selection/refused",
  });
  assert.equal(refused.disposition, "refused");
  assert.equal(refused.evidence.disposition, "CapacityRefused");
  pool.close();
  assert.equal(api.closed, true);
  assert.throws(() => pool.release(selected.member), /closed/);
});
