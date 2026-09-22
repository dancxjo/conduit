const CAPACITY = 256 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const owners = new WeakSet();

const required = [
  "conduit_browser_remote_input_ptr", "conduit_browser_remote_input_capacity",
  "conduit_browser_remote_output_ptr", "conduit_browser_remote_output_len",
  "conduit_browser_remote_start", "conduit_browser_remote_exchange",
  "conduit_browser_remote_drive", "conduit_browser_remote_complete_effect",
  "conduit_browser_remote_offer_frame", "conduit_browser_remote_finish",
  "conduit_browser_remote_cancel", "conduit_browser_remote_cancel_frames",
  "conduit_browser_remote_endpoint_count",
];

function boundedBytes(value, label) {
  const bytes = value instanceof Uint8Array ? value : Uint8Array.from(value ?? []);
  if (bytes.byteLength < 1 || bytes.byteLength > CAPACITY) {
    throw new Error(`${label} exceeds the browser remote-fragment bound`);
  }
  return bytes;
}

function readOutput(api) {
  const length = api.conduit_browser_remote_output_len();
  if (!Number.isSafeInteger(length) || length < 1 || length > CAPACITY) {
    throw new Error("invalid browser remote-fragment output bound");
  }
  return JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_remote_output_ptr(),
    length,
  )));
}

function write(api, bytes) {
  if (bytes.byteLength > api.conduit_browser_remote_input_capacity()) {
    throw new Error("browser remote-fragment input exceeds its admitted arena");
  }
  const pointer = api.conduit_browser_remote_input_ptr();
  new Uint8Array(api.memory.buffer, pointer, bytes.byteLength).set(bytes);
}

function refusal(api, action, status) {
  const detail = api.conduit_browser_remote_output_len() > 0 ? readOutput(api) : null;
  const error = new Error(`${action} refused (${status}): ${detail?.message ?? "no refusal detail"}`);
  error.status = status;
  error.refusal = detail;
  return error;
}

/**
 * Own one already-planned browser fragment. This adapter owns neither Body
 * membership nor transport: its caller must relay every returned opaque frame
 * over the exact joined Line named by the immutable Plan.
 */
export function openBrowserRemoteFragment({ api, plan, host, preparation, observations }) {
  if (!api?.memory || required.some(name => typeof api[name] !== "function")) {
    throw new Error("browser remote-fragment runtime is incomplete");
  }
  if (owners.has(api)) throw new Error("browser remote fragment already owns this runtime");
  const identity = preparation?.identity;
  const fragments = Array.isArray(plan?.fragments) ? plan.fragments : [];
  const localFragment = fragments.find(fragment =>
    fragment.host_id === host?.host_id && fragment.boot_id === host?.boot_id);
  const remoteFragment = fragments.find(fragment =>
    fragment.host_id === identity?.host_id && fragment.boot_id === identity?.boot_id);
  if (typeof plan?.plan_id !== "string" || !plan.plan_id ||
      !localFragment || !remoteFragment || localFragment === remoteFragment ||
      identity.plan_id !== plan.plan_id || typeof identity.active_play_id !== "string" ||
      !identity.active_play_id || !Array.isArray(preparation?.hello_frames) ||
      preparation.hello_frames.length < 1 || preparation.hello_frames.length > 16 ||
      !Array.isArray(observations)) {
    throw new Error("remote preparation differs from the exact planned host, Boot, or Play");
  }
  const helloFrames = preparation.hello_frames.map((frame, index) =>
    Array.from(boundedBytes(frame, `remote Hello ${index}`)));
  const request = encoder.encode(JSON.stringify({
    plan,
    host,
    session_hellos: helloFrames,
    observations,
  }));
  owners.add(api);
  let closed = false, finished = false;
  try {
    write(api, request);
    const status = api.conduit_browser_remote_start(request.byteLength);
    if (status !== 0) throw refusal(api, "browser remote-fragment start", status);
    const started = readOutput(api);
    if (started?.schema !== "conduit.browser/remote-fragment-started@1" ||
        started.plan_id !== plan.plan_id || typeof started.active_play_id !== "string" ||
        !started.active_play_id || started.active_play_id === identity.active_play_id ||
        !Array.isArray(started.endpoints) || started.endpoints.length !== helloFrames.length ||
        started.endpoints.length !== api.conduit_browser_remote_endpoint_count() ||
        !Array.isArray(started.egress_endpoints) ||
        started.egress_endpoints.some(endpoint => !started.endpoints.includes(endpoint)) ||
        !Array.isArray(started.initial_frames) || started.initial_frames.length !== helloFrames.length * 2) {
      throw new Error("browser remote-fragment start returned stale or malformed truth");
    }
    const initialFrames = started.initial_frames.map((frame, index) =>
      boundedBytes(frame, `browser initial frame ${index}`).slice());
    const current = () => {
      if (closed) throw new Error("browser remote fragment is closed");
    };
    return Object.freeze({
      identity: Object.freeze({
        host_id: host.host_id,
        boot_id: host.boot_id,
        plan_id: plan.plan_id,
        active_play_id: started.active_play_id,
      }),
      peerIdentity: Object.freeze({ ...identity }),
      endpoints: Object.freeze([...started.endpoints]),
      egressEndpoints: Object.freeze([...started.egress_endpoints]),
      initialFrames: Object.freeze(initialFrames),
      exchange(frame) {
        current();
        const bytes = boundedBytes(frame, "remote session frame");
        write(api, bytes);
        const status = api.conduit_browser_remote_exchange(bytes.byteLength);
        if (status !== 0) throw refusal(api, "browser remote session exchange", status);
        const exchanged = readOutput(api);
        if (exchanged?.schema !== "conduit.browser/remote-session-exchange@1" ||
            !Number.isSafeInteger(exchanged.endpoint) || !started.endpoints.includes(exchanged.endpoint) ||
            typeof exchanged.message !== "string" || !Array.isArray(exchanged.responses)) {
          throw new Error("browser remote session exchange returned malformed truth");
        }
        return Object.freeze({
          active: exchanged.active === true,
          endpoint: exchanged.endpoint,
          message: exchanged.message,
          responses: Object.freeze(exchanged.responses.map((response, index) =>
            boundedBytes(response, `browser response frame ${index}`).slice())),
        });
      },
      drive() {
        current();
        const status = api.conduit_browser_remote_drive();
        if (status < 0) throw refusal(api, "browser remote-fragment drive", status);
        if (![1, 2, 3, 4].includes(status)) throw new Error("unknown browser remote drive status");
        return Object.freeze({ status, output: status === 1 || status === 2 ? readOutput(api) : null });
      },
      completeEffect(output) {
        current();
        const present = output !== undefined && output !== null;
        const bytes = present ? boundedBytes(output, "browser Host effect output") : new Uint8Array();
        if (present) write(api, bytes);
        const status = api.conduit_browser_remote_complete_effect(bytes.byteLength, present ? 1 : 0);
        if (status !== 0) throw refusal(api, "browser remote host effect completion", status);
      },
      offer(endpoint) {
        current();
        if (!Number.isSafeInteger(endpoint) || endpoint < 0 || endpoint > 0xffff) {
          throw new Error("invalid browser remote endpoint");
        }
        const status = api.conduit_browser_remote_offer_frame(endpoint);
        if (status < 0) throw refusal(api, "browser remote offer", status);
        if (status === 2) return null;
        if (status !== 5) throw new Error("unknown browser remote offer status");
        const offered = readOutput(api);
        if (offered?.schema !== "conduit.browser/remote-session-frame@1") {
          throw new Error("browser remote offer returned malformed truth");
        }
        return boundedBytes(offered.frame, "browser offered frame").slice();
      },
      finish() {
        current();
        const status = api.conduit_browser_remote_finish();
        if (status !== 8) throw refusal(api, "browser remote-fragment finish", status);
        const result = readOutput(api);
        if (result?.schema !== "conduit.browser/remote-session-finished@1" || !Array.isArray(result.frames)) {
          throw new Error("browser remote finish returned malformed truth");
        }
        finished = true;
        return Object.freeze(result.frames.map((frame, index) =>
          boundedBytes(frame, `browser terminal frame ${index}`).slice()));
      },
      cancel(code = 1) {
        current();
        if (!Number.isSafeInteger(code) || code < 1 || code > 0xffff) {
          throw new Error("invalid browser remote cancellation code");
        }
        const status = api.conduit_browser_remote_cancel_frames(code);
        if (status !== 8) throw refusal(api, "browser remote-fragment cancellation", status);
        const result = readOutput(api);
        if (result?.schema !== "conduit.browser/remote-session-cancelled@1" || !Array.isArray(result.frames)) {
          throw new Error("browser remote cancellation returned malformed truth");
        }
        finished = true;
        return Object.freeze(result.frames.map((frame, index) =>
          boundedBytes(frame, `browser cancellation frame ${index}`).slice()));
      },
      close() {
        if (closed) return;
        closed = true;
        try {
          if (!finished && api.conduit_browser_remote_cancel() < 0) {
            throw refusal(api, "browser remote-fragment cancellation", -1);
          }
        } finally {
          owners.delete(api);
        }
      },
    });
  } catch (error) {
    try { api.conduit_browser_remote_cancel(); } catch {}
    owners.delete(api);
    throw error;
  }
}

async function sendAll(line, frames) {
  for (const frame of frames) await line.sendSessionFrame(frame);
}

/** Drive one admitted browser fragment over one retained exact joined Line. */
export async function runBrowserRemoteFragment({ remote, line, perform, signal }) {
  if (!remote || line?.schema !== "conduit.creche/joined-host-line@1" ||
      typeof perform !== "function" || !(signal instanceof AbortSignal)) {
    throw new Error("invalid browser remote-fragment driver inputs");
  }
  const requireCurrent = () => {
    if (signal.aborted) throw new Error("browser remote fragment cancelled");
  };
  const relayOne = async (requireLive = true) => {
    if (requireLive) requireCurrent();
    const exchange = remote.exchange(await line.receiveSessionFrame());
    await sendAll(line, exchange.responses);
    return exchange;
  };
  try {
    await sendAll(line, remote.initialFrames);
    const active = new Set();
    while (active.size < remote.endpoints.length) {
      const exchange = await relayOne();
      if (exchange.active) active.add(exchange.endpoint);
    }

    const offered = new Set();
    for (;;) {
      requireCurrent();
      const progress = remote.drive();
      if (progress.status === 1) {
        const result = await perform(progress.output, signal);
        requireCurrent();
        remote.completeEffect(result);
        continue;
      }
      if (progress.status === 4) {
        await sendAll(line, remote.finish());
        const terminal = new Set();
        while (terminal.size < remote.endpoints.length) {
          const exchange = await relayOne();
          if (exchange.message === "terminal" && !exchange.active) terminal.add(exchange.endpoint);
        }
        return Object.freeze({ disposition: "completed", active_play_id: remote.identity.active_play_id });
      }
      let sent = false;
      for (const endpoint of remote.egressEndpoints) {
        if (offered.has(endpoint)) continue;
        const frame = remote.offer(endpoint);
        if (!frame) continue;
        offered.add(endpoint);
        sent = true;
        await line.sendSessionFrame(frame);
      }
      const exchange = await relayOne();
      if (["pressure", "delivered"].includes(exchange.message)) offered.delete(exchange.endpoint);
      // Accepted deliberately keeps the exact transfer pending until Delivered.
      if (!sent && progress.status !== 2 && progress.status !== 3) {
        throw new Error("browser remote fragment made no admissible progress");
      }
    }
  } catch (error) {
    if (signal.aborted) {
      await sendAll(line, remote.cancel(1));
      const terminal = new Set();
      while (terminal.size < remote.endpoints.length) {
        const exchange = await relayOne(false);
        if (exchange.message === "terminal" && !exchange.active) terminal.add(exchange.endpoint);
      }
    }
    throw error;
  }
}
