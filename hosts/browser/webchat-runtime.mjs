const EFFECT_OPEN = 1;
const EFFECT_RECEIVE = 2;
const EFFECT_SEND = 3;
const EFFECT_CLOSE = 4;
const EFFECT_APPEND = 5;
const STATUS_COMPLETE = 1;
const MAXIMUM_MESSAGE_BYTES = 256;
const MAXIMUM_HISTORY_ITEMS = 16;
const INPUT_CAPACITY = 4096;

function requireApi(api) {
  const names = [
    "memory",
    "conduit_browser_webchat_input_ptr",
    "conduit_browser_webchat_input_capacity",
    "conduit_browser_webchat_start",
    "conduit_browser_webchat_status",
    "conduit_browser_webchat_effect_kind",
    "conduit_browser_webchat_effect_ptr",
    "conduit_browser_webchat_effect_len",
    "conduit_browser_webchat_complete_effect",
    "conduit_browser_webchat_receive",
    "conduit_browser_webchat_submit",
    "conduit_browser_webchat_disconnect",
    "conduit_browser_webchat_identity_ptr",
    "conduit_browser_webchat_identity_len",
    "conduit_browser_webchat_disconnected",
    "conduit_browser_webchat_capacity_stable",
    "conduit_browser_webchat_request_count",
  ];
  if (names.some((name) => !(name in api)) ||
      api.conduit_browser_webchat_input_capacity() !== INPUT_CAPACITY) {
    throw new Error("CND-CHAT-001 incomplete browser webchat ABI");
  }
}

function readBytes(api, pointer, length) {
  if (length < 0 || length > INPUT_CAPACITY) {
    throw new Error("CND-CHAT-002 invalid WASM frame length");
  }
  return new Uint8Array(api.memory.buffer, pointer, length).slice();
}

function writeInput(api, bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length > INPUT_CAPACITY) {
    throw new Error("CND-CHAT-003 invalid host input");
  }
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_webchat_input_ptr(),
    bytes.length,
  ).set(bytes);
}

function requireStatus(status, action) {
  if (status < 0) throw new Error(`CND-CHAT-004 ${action} failed ${status}`);
}

export async function createWebchatRuntime({ wasmBytes, url, list, input, button, status }) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  requireApi(api);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const hostId = `browser/${crypto.randomUUID()}`;
  const bootId = `browser-boot/${crypto.randomUUID()}`;
  const startFrame = encoder.encode(`${url}\n${hostId}\n${bootId}`);
  writeInput(api, startFrame);
  requireStatus(api.conduit_browser_webchat_start(startFrame.length), "start");

  let socket = null;
  let closed = false;
  let chain = Promise.resolve();
  const enqueue = (action) => {
    chain = chain.then(action).catch((error) => {
      status.textContent = `error:${error.stack ?? error}`;
      button.disabled = true;
      throw error;
    });
    return chain;
  };
  const effectBytes = () => readBytes(
    api,
    api.conduit_browser_webchat_effect_ptr(),
    api.conduit_browser_webchat_effect_len(),
  );

  async function pump() {
    for (;;) {
      const effect = api.conduit_browser_webchat_effect_kind();
      if (effect === EFFECT_RECEIVE || api.conduit_browser_webchat_status() === STATUS_COMPLETE) {
        button.disabled = effect !== EFFECT_RECEIVE;
        status.textContent = effect === EFFECT_RECEIVE ? "connected" : "disconnected";
        return;
      }
      if (effect === EFFECT_OPEN) {
        const requestedUrl = decoder.decode(effectBytes());
        socket = new WebSocket(requestedUrl);
        socket.binaryType = "arraybuffer";
        await new Promise((resolve, reject) => {
          socket.addEventListener("open", resolve, { once: true });
          socket.addEventListener("error", () => reject(new Error("CND-CHAT-005 WebSocket open failed")), { once: true });
        });
        socket.addEventListener("message", (event) => enqueue(async () => {
          const bytes = new Uint8Array(event.data);
          if (bytes.length > MAXIMUM_MESSAGE_BYTES) {
            throw new Error("CND-CHAT-006 oversized inbound message");
          }
          writeInput(api, bytes);
          requireStatus(api.conduit_browser_webchat_receive(bytes.length), "receive");
          await pump();
        }));
        socket.addEventListener("close", () => enqueue(async () => {
          if (!closed && api.conduit_browser_webchat_effect_kind() === EFFECT_RECEIVE) {
            closed = true;
            requireStatus(api.conduit_browser_webchat_disconnect(), "disconnect");
            await pump();
          }
        }));
        requireStatus(api.conduit_browser_webchat_complete_effect(), "open completion");
        continue;
      }
      if (effect === EFFECT_SEND) {
        const bytes = effectBytes();
        if (socket?.readyState !== WebSocket.OPEN) {
          throw new Error("CND-CHAT-007 send without an open socket");
        }
        socket.send(bytes);
        requireStatus(api.conduit_browser_webchat_complete_effect(), "send completion");
        continue;
      }
      if (effect === EFFECT_APPEND) {
        const text = decoder.decode(effectBytes());
        const item = document.createElement("li");
        item.textContent = text;
        list.append(item);
        while (list.children.length > MAXIMUM_HISTORY_ITEMS) list.firstElementChild.remove();
        requireStatus(api.conduit_browser_webchat_complete_effect(), "append completion");
        continue;
      }
      if (effect === EFFECT_CLOSE) {
        socket?.close(1000, "Conduit close");
        requireStatus(api.conduit_browser_webchat_complete_effect(), "close completion");
        continue;
      }
      throw new Error(`CND-CHAT-008 unsupported effect ${effect}`);
    }
  }

  async function submit() {
    const bytes = encoder.encode(input.value);
    if (bytes.length === 0 || bytes.length > MAXIMUM_MESSAGE_BYTES) {
      throw new Error("CND-CHAT-009 message must contain 1..256 bytes");
    }
    writeInput(api, bytes);
    requireStatus(api.conduit_browser_webchat_submit(bytes.length), "submit");
    input.value = "";
    await pump();
  }

  async function disconnect() {
    if (!closed && api.conduit_browser_webchat_effect_kind() === EFFECT_RECEIVE) {
      closed = true;
      requireStatus(api.conduit_browser_webchat_disconnect(), "disconnect");
      socket?.close(1000, "Conduit disconnect");
      await pump();
    }
  }

  button.addEventListener("click", () => enqueue(submit));
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") enqueue(submit);
  });
  await pump();
  const identity = decoder.decode(readBytes(
    api,
    api.conduit_browser_webchat_identity_ptr(),
    api.conduit_browser_webchat_identity_len(),
  ));
  return Object.freeze({
    submit: (text) => enqueue(async () => { input.value = text; await submit(); }),
    disconnect: () => enqueue(disconnect),
    proof: () => Object.freeze({
      identity,
      history: Object.freeze([...list.children].map((item) => item.textContent)),
      requestCount: api.conduit_browser_webchat_request_count(),
      capacityStable: api.conduit_browser_webchat_capacity_stable() === 1,
      disconnected: api.conduit_browser_webchat_disconnected() === 1,
      status: api.conduit_browser_webchat_status(),
    }),
  });
}
