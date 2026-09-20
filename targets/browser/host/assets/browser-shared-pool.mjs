const CAPACITY = 256 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const owners = new WeakSet();

const required = [
  "conduit_browser_shared_pool_input_ptr", "conduit_browser_shared_pool_input_capacity",
  "conduit_browser_shared_pool_output_ptr", "conduit_browser_shared_pool_output_len",
  "conduit_browser_shared_pool_start", "conduit_browser_shared_pool_admit",
  "conduit_browser_shared_pool_trigger", "conduit_browser_shared_pool_release",
  "conduit_browser_shared_pool_provider_lost", "conduit_browser_shared_pool_close",
];

function read(api) {
  const length = api.conduit_browser_shared_pool_output_len();
  if (!Number.isSafeInteger(length) || length < 1 || length > CAPACITY) {
    throw new Error("invalid browser shared-pool output bound");
  }
  return JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer, api.conduit_browser_shared_pool_output_ptr(), length,
  )));
}

function invoke(api, name, value) {
  const bytes = encoder.encode(JSON.stringify(value));
  if (bytes.byteLength < 1 || bytes.byteLength > api.conduit_browser_shared_pool_input_capacity()) {
    throw new Error("browser shared-pool input exceeds its admitted arena");
  }
  new Uint8Array(api.memory.buffer, api.conduit_browser_shared_pool_input_ptr(), bytes.byteLength)
    .set(bytes);
  const status = api[name](bytes.byteLength);
  bytes.fill(0);
  const output = read(api);
  if (status !== 0) {
    const error = new Error(`${name} refused: ${output?.message ?? "no refusal detail"}`);
    error.status = status;
    error.refusal = output;
    throw error;
  }
  return output;
}

function identity(value, label) {
  if (typeof value !== "string" || value.length < 1 || value.length > 192 || /\s/.test(value)) {
    throw new Error(`${label} is not a bounded identity`);
  }
  return value;
}

function memberKey(value) {
  const key = value instanceof Uint8Array ? value : Uint8Array.from(value ?? []);
  if (key.byteLength !== 32) throw new Error("pool member key must contain exactly 32 bytes");
  return Array.from(key);
}

/** Own finite in-Play selection for one exact pool sealed by an immutable Plan. */
export function openBrowserSharedPool({
  api, plan, fragmentIndex, poolId, firstMemberNode, play,
}) {
  if (!api?.memory || required.some((name) => typeof api[name] !== "function")) {
    throw new Error("browser shared-pool runtime is incomplete");
  }
  if (owners.has(api)) throw new Error("browser shared-pool runtime is already owned");
  if (!Number.isSafeInteger(fragmentIndex) || fragmentIndex < 0
    || !Number.isSafeInteger(firstMemberNode) || firstMemberNode < 0 || firstMemberNode > 0xffff
    || !Number.isSafeInteger(play) || play < 0 || play > 0xffff) {
    throw new Error("browser shared-pool numeric identity is outside its finite bound");
  }
  identity(poolId, "pool identity");
  const started = invoke(api, "conduit_browser_shared_pool_start", {
    plan, fragment_index: fragmentIndex, pool_id: poolId,
    first_member_node: firstMemberNode, play,
  });
  if (started?.schema !== "conduit.browser/shared-pool-started@1"
    || started.plan_id !== plan?.plan_id || started.pool_id !== poolId || started.population !== 0) {
    api.conduit_browser_shared_pool_close();
    throw new Error("browser shared-pool start returned stale or malformed truth");
  }
  owners.add(api);
  let closed = false;
  const current = () => {
    if (closed) throw new Error("browser shared-pool runtime is closed");
  };
  const memberAction = (name, member) => {
    current();
    const result = invoke(api, name, { member });
    if (result?.schema !== "conduit.browser/shared-pool-member@1") {
      throw new Error("browser shared-pool member transition returned malformed truth");
    }
    return Object.freeze(result);
  };
  return Object.freeze({
    planId: started.plan_id,
    poolId: started.pool_id,
    admit({ operationId, key, observations, selectionSignId }) {
      current();
      identity(operationId, "pool operation identity");
      identity(selectionSignId, "pool selection Sign identity");
      if (!Array.isArray(observations)) throw new Error("pool observations are not bounded truth");
      const result = invoke(api, "conduit_browser_shared_pool_admit", {
        operation_id: operationId, member_key: memberKey(key), observations,
        selection_sign_id: selectionSignId,
      });
      if (result?.schema !== "conduit.browser/shared-pool-selection@1"
        || !["selected", "refused"].includes(result.disposition)
        || result.evidence?.plan_id !== plan.plan_id || result.evidence?.pool_id !== poolId
        || result.evidence?.operation_id !== operationId
        || result.disposition === "selected" && !result.member) {
        throw new Error("browser shared-pool admission returned malformed evidence");
      }
      return Object.freeze(result);
    },
    trigger(member) {
      return memberAction("conduit_browser_shared_pool_trigger", member);
    },
    release(member) {
      return memberAction("conduit_browser_shared_pool_release", member);
    },
    providerLost({ operationId, member, observations, lossSignId }) {
      current();
      identity(operationId, "pool operation identity");
      identity(lossSignId, "pool provider-loss Sign identity");
      if (!Array.isArray(observations)) throw new Error("pool observations are not bounded truth");
      const result = invoke(api, "conduit_browser_shared_pool_provider_lost", {
        operation_id: operationId, member, observations, loss_sign_id: lossSignId,
      });
      if (result?.schema !== "conduit.browser/shared-pool-provider-loss@1"
        || result.evidence?.operation_id !== operationId
        || result.evidence?.disposition !== "ProviderLost") {
        throw new Error("browser shared-pool provider loss returned malformed evidence");
      }
      return Object.freeze(result);
    },
    close() {
      if (closed) return;
      closed = true;
      api.conduit_browser_shared_pool_close();
      owners.delete(api);
    },
  });
}
