const CAPACITY = 256 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const owners = new WeakSet();

const required = [
  "conduit_browser_pool_member_input_ptr", "conduit_browser_pool_member_input_capacity",
  "conduit_browser_pool_member_output_ptr", "conduit_browser_pool_member_output_len",
  "conduit_browser_pool_member_start", "conduit_browser_pool_member_offer",
  "conduit_browser_pool_member_exchange", "conduit_browser_pool_member_close",
];

function bytes(value, label) {
  const result = value instanceof Uint8Array ? value : Uint8Array.from(value ?? []);
  if (result.byteLength < 1 || result.byteLength > CAPACITY) {
    throw new Error(`${label} exceeds the finite pool-member arena`);
  }
  return result;
}

function write(api, value) {
  const input = bytes(value, "pool-member input");
  if (input.byteLength > api.conduit_browser_pool_member_input_capacity()) {
    throw new Error("pool-member input exceeds the runtime arena");
  }
  new Uint8Array(
    api.memory.buffer, api.conduit_browser_pool_member_input_ptr(), input.byteLength,
  ).set(input);
  return input.byteLength;
}

function read(api) {
  const length = api.conduit_browser_pool_member_output_len();
  if (!Number.isSafeInteger(length) || length < 1 || length > CAPACITY) {
    throw new Error("invalid pool-member output bound");
  }
  return JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer, api.conduit_browser_pool_member_output_ptr(), length,
  )));
}

function invoke(api, name, value, json = false) {
  const input = json
    ? encoder.encode(JSON.stringify(value))
    : bytes(value, "pool-member frame").slice();
  const status = api[name](write(api, input));
  input.fill(0);
  const output = read(api);
  if (status !== 0) {
    throw new Error(`${name} refused: ${output?.message ?? "no refusal detail"}`);
  }
  return output;
}

export function openBrowserPoolMemberClient({
  api, plan, selection, consumerPlacementId, sessionHellos,
}) {
  if (!api?.memory || required.some((name) => typeof api[name] !== "function")) {
    throw new Error("browser pool-member runtime is incomplete");
  }
  if (owners.has(api)) throw new Error("browser pool-member runtime is already owned");
  if (!Array.isArray(sessionHellos) || sessionHellos.length < 1 || sessionHellos.length > 16) {
    throw new Error("pool-member session grants exceed their finite bound");
  }
  const started = invoke(api, "conduit_browser_pool_member_start", {
    plan, selection, consumer_placement_id: consumerPlacementId,
    session_hellos: sessionHellos.map((frame) => Array.from(bytes(frame, "pool-member grant"))),
  }, true);
  if (started?.schema !== "conduit.browser/pool-member-client-started@1"
    || started.plan_id !== plan?.plan_id
    || started.operation_id !== selection?.operation_id
    || !Array.isArray(started.initial_frames)) {
    api.conduit_browser_pool_member_close();
    throw new Error("browser pool-member start returned malformed truth");
  }
  owners.add(api);
  let closed = false;
  const current = () => {
    if (closed) throw new Error("browser pool-member client is closed");
  };
  return Object.freeze({
    initialFrames: Object.freeze(started.initial_frames.map((frame) =>
      bytes(frame, "pool-member initial frame").slice())),
    offer(prompt) {
      current();
      const offered = invoke(
        api, "conduit_browser_pool_member_offer", bytes(prompt, "pool-member prompt"),
      );
      if (offered?.schema !== "conduit.browser/pool-member-client-offer@1") {
        throw new Error("browser pool-member offer returned malformed truth");
      }
      return bytes(offered.frame, "pool-member offered frame").slice();
    },
    exchange(frame) {
      current();
      const exchanged = invoke(api, "conduit_browser_pool_member_exchange", frame);
      if (exchanged?.schema !== "conduit.browser/pool-member-client-exchange@1"
        || typeof exchanged.message !== "string" || !Array.isArray(exchanged.responses)) {
        throw new Error("browser pool-member exchange returned malformed truth");
      }
      return Object.freeze({
        message: exchanged.message,
        active: exchanged.active === true,
        result: exchanged.result == null
          ? null : bytes(exchanged.result, "pool-member result").slice(),
        responses: Object.freeze(exchanged.responses.map((response) =>
          bytes(response, "pool-member response").slice())),
      });
    },
    close() {
      if (closed) return;
      closed = true;
      api.conduit_browser_pool_member_close();
      owners.delete(api);
    },
  });
}
