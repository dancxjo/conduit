export const BrowserWebSocketFailure = Object.freeze({
  InvalidBinding: "CND-WS-S4-001",
  ConnectFailed: "CND-WS-S4-002",
  BinaryRequired: "CND-WS-S4-003",
  OversizedMessage: "CND-WS-S4-004",
  InboxFull: "CND-WS-S4-005",
  SendBufferFull: "CND-WS-S4-006",
  Disconnected: "CND-WS-S4-007",
  ReceivePending: "CND-WS-S4-008",
});

const encoder = new TextEncoder();
export const MAXIMUM_WEBSOCKET_MESSAGE_BYTES = 64 * 1024;
export const MAXIMUM_WEBSOCKET_BUFFERED_BYTES = 256 * 1024;

function failure(code, detail) {
  return Object.freeze({ ok: false, code, detail });
}

function transportError(code, detail) {
  const error = new Error(`${code} ${detail}`);
  error.code = code;
  error.detail = detail;
  return error;
}

export class BrowserWebSocketLine {
  #socket;
  #pending = null;
  #receiver = null;
  #failure = null;
  #closedPromise;
  #resolveClosed;

  constructor({ url, maximumMessageBytes, maximumBufferedBytes }) {
    const parsed = typeof url === "string" ? new URL(url) : null;
    if (parsed?.protocol !== "ws:" ||
        parsed.hostname !== "127.0.0.1" ||
        parsed.username !== "" ||
        parsed.password !== "" ||
        !Number.isSafeInteger(maximumMessageBytes) ||
        maximumMessageBytes <= 0 ||
        maximumMessageBytes > MAXIMUM_WEBSOCKET_MESSAGE_BYTES ||
        !Number.isSafeInteger(maximumBufferedBytes) ||
        maximumBufferedBytes < maximumMessageBytes ||
        maximumBufferedBytes > MAXIMUM_WEBSOCKET_BUFFERED_BYTES) {
      throw new TypeError(BrowserWebSocketFailure.InvalidBinding);
    }
    this.url = parsed.href;
    this.maximumMessageBytes = maximumMessageBytes;
    this.maximumBufferedBytes = maximumBufferedBytes;
    this.#closedPromise = new Promise((resolve) => {
      this.#resolveClosed = resolve;
    });
  }

  async open() {
    if (this.#socket) throw new TypeError(BrowserWebSocketFailure.InvalidBinding);
    const socket = new WebSocket(this.url);
    socket.binaryType = "arraybuffer";
    this.#socket = socket;
    socket.addEventListener("message", (event) => this.#onMessage(event));
    socket.addEventListener("close", (event) => this.#onClose(event));
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", () => {
        reject(transportError(BrowserWebSocketFailure.ConnectFailed, "websocket-error"));
      }, { once: true });
    });
    return this;
  }

  sendBinary(bytes) {
    if (!(bytes instanceof Uint8Array)) {
      return failure(BrowserWebSocketFailure.BinaryRequired, "send-requires-uint8array");
    }
    if (bytes.length > this.maximumMessageBytes) {
      return failure(BrowserWebSocketFailure.OversizedMessage, String(bytes.length));
    }
    if (!this.#socket || this.#socket.readyState !== WebSocket.OPEN || this.#failure) {
      return failure(BrowserWebSocketFailure.Disconnected, "socket-not-open");
    }
    if (this.#socket.bufferedAmount + bytes.length > this.maximumBufferedBytes) {
      return failure(BrowserWebSocketFailure.SendBufferFull, "browser-send-buffer-full");
    }
    this.#socket.send(bytes);
    return Object.freeze({ ok: true, byteLength: bytes.length });
  }

  receiveBinary() {
    if (this.#failure) return Promise.reject(this.#failure);
    if (this.#pending) {
      const pending = this.#pending;
      this.#pending = null;
      return Promise.resolve(pending);
    }
    if (this.#receiver) {
      return Promise.reject(
        transportError(BrowserWebSocketFailure.ReceivePending, "one-receiver-already-pending"),
      );
    }
    if (!this.#socket || this.#socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(
        transportError(BrowserWebSocketFailure.Disconnected, "socket-not-open"),
      );
    }
    return new Promise((resolve, reject) => {
      this.#receiver = { resolve, reject };
    });
  }

  closed() {
    return this.#closedPromise;
  }

  close(code = 1000, reason = "conduit-terminal") {
    if (!Number.isSafeInteger(code) ||
        (code !== 1000 && (code < 3000 || code > 4999)) ||
        typeof reason !== "string" ||
        encoder.encode(reason).length > 123) {
      throw new TypeError(BrowserWebSocketFailure.InvalidBinding);
    }
    if (this.#socket &&
        (this.#socket.readyState === WebSocket.OPEN ||
         this.#socket.readyState === WebSocket.CONNECTING)) {
      this.#socket.close(code, reason);
    }
    return this.#closedPromise;
  }

  #onMessage(event) {
    if (!(event.data instanceof ArrayBuffer)) {
      this.#fail(BrowserWebSocketFailure.BinaryRequired, "text-or-blob-message");
      return;
    }
    const bytes = new Uint8Array(event.data);
    if (bytes.length > this.maximumMessageBytes) {
      this.#fail(BrowserWebSocketFailure.OversizedMessage, String(bytes.length));
      return;
    }
    const exact = bytes.slice();
    if (this.#receiver) {
      const receiver = this.#receiver;
      this.#receiver = null;
      receiver.resolve(exact);
    } else if (this.#pending) {
      this.#fail(BrowserWebSocketFailure.InboxFull, "one-message-inbox-full");
    } else {
      this.#pending = exact;
    }
  }

  #onClose(event) {
    if (!this.#failure && event.code !== 1000) {
      this.#failure = transportError(
        BrowserWebSocketFailure.Disconnected,
        `code=${event.code}`,
      );
    }
    if (this.#receiver) {
      const receiver = this.#receiver;
      this.#receiver = null;
      receiver.reject(
        this.#failure ?? transportError(BrowserWebSocketFailure.Disconnected, "closed"),
      );
    }
    this.#resolveClosed(Object.freeze({
      ok: !this.#failure,
      code: event.code,
      reason: event.reason,
    }));
  }

  #fail(code, detail) {
    if (this.#failure) return;
    this.#failure = transportError(code, detail);
    if (this.#receiver) {
      const receiver = this.#receiver;
      this.#receiver = null;
      receiver.reject(this.#failure);
    }
    this.#socket?.close(1002, code);
  }
}
