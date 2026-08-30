const END = 0xc0;
const ESC = 0xdb;
const ESC_END = 0xdc;
const ESC_ESC = 0xdd;

export class Esp32SlipRefusal extends Error {
  constructor(code, message) {
    super(message);
    this.name = "Esp32SlipRefusal";
    this.code = code;
  }
}

function refuse(code, message) {
  throw new Esp32SlipRefusal(code, message);
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  refuse("Bytes", "ESP32 ROM-loader data must be bytes");
}

export function encodeSlip(value, maximumBytes = 4096) {
  const bytes = bytesOf(value);
  const encoded = [END];
  for (const byte of bytes) {
    if (byte === END) encoded.push(ESC, ESC_END);
    else if (byte === ESC) encoded.push(ESC, ESC_ESC);
    else encoded.push(byte);
    if (encoded.length + 1 > maximumBytes) {
      refuse("FrameTooLarge", "SLIP frame exceeds the admitted serial transfer bound");
    }
  }
  encoded.push(END);
  return new Uint8Array(encoded);
}

export function createSlipReader({ base, maximumFrameBytes = 4096, maximumFragmentsPerFrame = 8 }) {
  if (!base || typeof base.read !== "function") refuse("BaseContract", "SLIP requires an admitted serial Base");
  if (!Number.isSafeInteger(maximumFrameBytes) || maximumFrameBytes < 8 || maximumFrameBytes > 4096) {
    refuse("FrameBound", "SLIP frame bound is invalid");
  }
  if (!Number.isSafeInteger(maximumFragmentsPerFrame) || maximumFragmentsPerFrame < 1 || maximumFragmentsPerFrame > 32) {
    refuse("FragmentBound", "SLIP fragment bound is invalid");
  }

  let pending = [];
  let started = false;
  let escaped = false;

  function consume(chunk) {
    const frames = [];
    for (const byte of bytesOf(chunk)) {
      if (byte === END) {
        if (started && pending.length > 0) frames.push(new Uint8Array(pending));
        pending = [];
        started = true;
        escaped = false;
        continue;
      }
      if (!started) continue;
      if (escaped) {
        if (byte === ESC_END) pending.push(END);
        else if (byte === ESC_ESC) pending.push(ESC);
        else refuse("Escape", "ROM-loader response contains an invalid SLIP escape");
        escaped = false;
      } else if (byte === ESC) {
        escaped = true;
      } else {
        pending.push(byte);
      }
      if (pending.length > maximumFrameBytes) refuse("FrameTooLarge", "ROM-loader response exceeds its frame bound");
    }
    return frames;
  }

  const queued = [];
  async function readFrame() {
    if (queued.length > 0) return queued.shift();
    for (let fragment = 0; fragment < maximumFragmentsPerFrame; fragment += 1) {
      const receipt = await base.read();
      const frames = consume(receipt.bytes);
      if (frames.length > 0) {
        queued.push(...frames.slice(1));
        return frames[0];
      }
    }
    refuse("ProtocolStall", "ROM-loader response exceeded its admitted fragment bound");
  }

  return Object.freeze({ readFrame });
}

export function littleEndianWords(...words) {
  const bytes = new Uint8Array(words.length * 4);
  const view = new DataView(bytes.buffer);
  words.forEach((word, index) => view.setUint32(index * 4, word >>> 0, true));
  return bytes;
}
