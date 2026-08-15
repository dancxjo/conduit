const DEFAULT_MAXIMUM_MESSAGE_BYTES = 2048;
const DEFAULT_MAXIMUM_BUFFERED_BYTES = 8192;
const DEFAULT_MAXIMUM_RECEIVED_MESSAGES = 4;

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new TypeError(`${label} must be a positive safe integer`);
  }
  return value;
}

function bytesFromMessage(data) {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  return null;
}

export class BrowserWebRtcDataChannelLine {
  #channel;
  #maximumMessageBytes;
  #maximumBufferedBytes;
  #maximumReceivedMessages;
  #received = [];
  #receivedBytes = 0;
  #receiver = null;
  #terminal = null;
  #closed;
  #resolveClosed;

  constructor({
    channel,
    maximumMessageBytes = DEFAULT_MAXIMUM_MESSAGE_BYTES,
    maximumBufferedBytes = DEFAULT_MAXIMUM_BUFFERED_BYTES,
    maximumReceivedMessages = DEFAULT_MAXIMUM_RECEIVED_MESSAGES,
  }) {
    if (!(channel instanceof RTCDataChannel)) {
      throw new TypeError("channel must be an RTCDataChannel");
    }
    this.#maximumMessageBytes = positiveInteger(
      maximumMessageBytes,
      "maximumMessageBytes",
    );
    this.#maximumBufferedBytes = positiveInteger(
      maximumBufferedBytes,
      "maximumBufferedBytes",
    );
    if (this.#maximumBufferedBytes < this.#maximumMessageBytes) {
      throw new RangeError("maximumBufferedBytes must admit one maximum message");
    }
    this.#maximumReceivedMessages = positiveInteger(
      maximumReceivedMessages,
      "maximumReceivedMessages",
    );
    this.#channel = channel;
    channel.binaryType = "arraybuffer";
    channel.bufferedAmountLowThreshold = Math.floor(this.#maximumBufferedBytes / 2);
    this.#closed = new Promise((resolve) => {
      this.#resolveClosed = resolve;
    });
    channel.addEventListener("message", (event) => this.#ingest(event.data));
    channel.addEventListener("close", () => this.#finish("closed"));
    channel.addEventListener("error", () => this.#finish("transport-error"));
  }

  async open() {
    if (this.#terminal !== null) {
      throw new Error(`datachannel-terminal:${this.#terminal.reason}`);
    }
    if (this.#channel.readyState === "open") return this;
    if (this.#channel.readyState !== "connecting") {
      throw new Error(`datachannel-open-refused:${this.#channel.readyState}`);
    }
    await new Promise((resolve, reject) => {
      const opened = () => {
        cleanup();
        resolve();
      };
      const refused = () => {
        cleanup();
        reject(new Error(`datachannel-open-refused:${this.#channel.readyState}`));
      };
      const cleanup = () => {
        this.#channel.removeEventListener("open", opened);
        this.#channel.removeEventListener("close", refused);
        this.#channel.removeEventListener("error", refused);
      };
      this.#channel.addEventListener("open", opened, { once: true });
      this.#channel.addEventListener("close", refused, { once: true });
      this.#channel.addEventListener("error", refused, { once: true });
    });
    return this;
  }

  send(value) {
    if (this.#terminal !== null) {
      return {
        accepted: false,
        reason: "line-terminal",
        terminalReason: this.#terminal.reason,
      };
    }
    const bytes = bytesFromMessage(value);
    if (bytes === null) return { accepted: false, reason: "non-binary-message" };
    if (bytes.byteLength > this.#maximumMessageBytes) {
      return { accepted: false, reason: "message-too-large" };
    }
    if (this.#channel.readyState !== "open") {
      return { accepted: false, reason: "line-not-open" };
    }
    if (this.#channel.bufferedAmount + bytes.byteLength > this.#maximumBufferedBytes) {
      return { accepted: false, reason: "buffer-pressure" };
    }
    try {
      this.#channel.send(bytes);
      return { accepted: true };
    } catch {
      this.#finish("transport-error");
      return { accepted: false, reason: "transport-error" };
    }
  }

  receive() {
    if (this.#terminal !== null) return Promise.resolve(this.#terminal);
    if (this.#received.length > 0) {
      const bytes = this.#received.shift();
      this.#receivedBytes -= bytes.byteLength;
      return Promise.resolve({ ok: true, bytes });
    }
    if (this.#receiver !== null) {
      return Promise.reject(new Error("receive-already-pending"));
    }
    return new Promise((resolve) => {
      this.#receiver = resolve;
    });
  }

  close() {
    if (this.#channel.readyState !== "closed") this.#channel.close();
  }

  closed() {
    return this.#closed;
  }

  state() {
    return {
      readyState: this.#channel.readyState,
      bufferedBytes: this.#channel.bufferedAmount,
      retainedMessages: this.#received.length,
      retainedBytes: this.#receivedBytes,
      terminalReason: this.#terminal?.reason ?? null,
    };
  }

  #ingest(data) {
    if (this.#terminal !== null) return;
    const bytes = bytesFromMessage(data);
    if (bytes === null) {
      this.#refuseIngress("non-binary-message");
      return;
    }
    if (bytes.byteLength > this.#maximumMessageBytes) {
      this.#refuseIngress("message-too-large");
      return;
    }
    if (this.#receiver !== null) {
      const receiver = this.#receiver;
      this.#receiver = null;
      receiver({ ok: true, bytes: bytes.slice() });
      return;
    }
    if (
      this.#received.length === this.#maximumReceivedMessages ||
      this.#receivedBytes + bytes.byteLength > this.#maximumBufferedBytes
    ) {
      this.#refuseIngress("receive-pressure");
      return;
    }
    const retained = bytes.slice();
    this.#received.push(retained);
    this.#receivedBytes += retained.byteLength;
  }

  #refuseIngress(reason) {
    this.#finish(reason);
    this.#channel.close();
  }

  #finish(reason) {
    if (this.#terminal !== null) return;
    this.#terminal = { ok: false, reason };
    this.#received = [];
    this.#receivedBytes = 0;
    if (this.#receiver !== null) {
      const receiver = this.#receiver;
      this.#receiver = null;
      receiver(this.#terminal);
    }
    this.#resolveClosed(this.#terminal);
  }
}
