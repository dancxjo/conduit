const PREFIX = "conduit-rendezvous-v1:";
const MAXIMUM_CBOR_BYTES = 3_656;
const MAXIMUM_TEXT_BYTES = 256;
const textDecoder = new TextDecoder("utf-8", { fatal: true });
const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

export function decodeRendezvousManifestation(value, nowMillis = Date.now()) {
  const encoded = value.startsWith(PREFIX) ? value.slice(PREFIX.length) : invalid();
  const bytes = decodeBase64Url(encoded);
  if (bytes.length > MAXIMUM_CBOR_BYTES) invalid();
  const reader = new CanonicalCborReader(bytes);
  const fieldCount = reader.map();
  if (fieldCount < 3 || fieldCount > 7) invalid();
  reader.exactUnsigned(0);
  reader.exactUnsigned(1);
  reader.exactUnsigned(1);
  const candidateCount = reader.array();
  if (candidateCount < 1 || candidateCount > 4) invalid();
  const candidates = Array.from({ length: candidateCount }, () => decodeCandidate(reader));
  reader.exactUnsigned(2);
  const sessionSecret = reader.bytes();
  if (sessionSecret.length !== 32 || sessionSecret.every((byte) => byte === 0)) invalid();
  let previousExtension = 127;
  for (let index = 3; index < fieldCount; index += 1) {
    const key = reader.unsigned();
    if (key < 128 || key > 131 || key <= previousExtension) invalid();
    previousExtension = key;
    if (reader.bytes().length > 64) invalid();
  }
  if (!reader.finished()) invalid();
  const seen = new Set();
  for (const candidate of candidates) {
    if (seen.has(candidate.candidate_id) || candidate.expires_at_millis <= nowMillis
      || candidate.maximum_attempts < 1 || candidate.maximum_attempts > 3
      || candidate.attempt_timeout_millis < 1 || candidate.attempt_timeout_millis > 30_000
      || candidate.transport_binding_sha256.every((byte) => byte === 0)) invalid();
    seen.add(candidate.candidate_id);
    if (candidate.line_family === "local-loopback-websocket"
      && !candidate.reachability.startsWith("ws://127.0.0.1:")
      && !candidate.reachability.startsWith("ws://[::1]:")) invalid();
  }
  return Object.freeze({
    schema: "conduit.host/rendezvous-cbor@1",
    candidates: Object.freeze(candidates),
    session_secret: new Uint8Array(sessionSecret),
  });
}

function decodeCandidate(reader) {
  if (reader.map() !== 7) invalid();
  reader.exactUnsigned(0);
  const candidateId = reader.text();
  reader.exactUnsigned(1);
  const lineFamily = ["authenticated-tls-stream", "authenticated-conduit-line",
    "local-loopback-websocket", "attended-serial"][reader.unsigned()];
  if (!lineFamily) invalid();
  reader.exactUnsigned(2);
  const reachability = reader.text();
  reader.exactUnsigned(3);
  if (reader.map() !== 2) invalid();
  reader.exactUnsigned(0);
  const serverIdentity = reader.text();
  reader.exactUnsigned(1);
  const transportBinding = reader.bytes();
  if (transportBinding.length !== 32) invalid();
  reader.exactUnsigned(4);
  const expiresAtMillis = reader.unsigned();
  reader.exactUnsigned(5);
  const maximumAttempts = reader.unsigned();
  reader.exactUnsigned(6);
  const attemptTimeoutMillis = reader.unsigned();
  return Object.freeze({
    candidate_id: candidateId,
    line_family: lineFamily,
    reachability,
    server_identity: serverIdentity,
    transport_binding_sha256: new Uint8Array(transportBinding),
    expires_at_millis: expiresAtMillis,
    maximum_attempts: maximumAttempts,
    attempt_timeout_millis: attemptTimeoutMillis,
  });
}

class CanonicalCborReader {
  constructor(bytes) { this.bytesValue = bytes; this.offset = 0; }
  finished() { return this.offset === this.bytesValue.length; }
  exactUnsigned(expected) { if (this.unsigned() !== expected) invalid(); }
  unsigned() { return this.head(0); }
  array() { return this.head(4); }
  map() { return this.head(5); }
  bytes() { return this.sequence(2); }
  text() {
    const bytes = this.sequence(3);
    if (bytes.length < 1 || bytes.length > MAXIMUM_TEXT_BYTES) invalid();
    try { return textDecoder.decode(bytes); } catch { return invalid(); }
  }
  sequence(major) {
    const length = this.head(major);
    if (this.offset + length > this.bytesValue.length) invalid();
    const result = this.bytesValue.subarray(this.offset, this.offset + length);
    this.offset += length;
    return result;
  }
  head(expectedMajor) {
    const first = this.take();
    if ((first >> 5) !== expectedMajor) invalid();
    const additional = first & 31;
    if (additional < 24) return additional;
    const sizes = { 24: 1, 25: 2, 26: 4, 27: 8 };
    const size = sizes[additional];
    if (!size || this.offset + size > this.bytesValue.length) invalid();
    let value = 0;
    for (let index = 0; index < size; index += 1) value = value * 256 + this.take();
    if (!Number.isSafeInteger(value)
      || (size === 1 && value < 24)
      || (size === 2 && value <= 0xff)
      || (size === 4 && value <= 0xffff)
      || (size === 8 && value <= 0xffff_ffff)) invalid();
    return value;
  }
  take() {
    if (this.offset >= this.bytesValue.length) invalid();
    return this.bytesValue[this.offset++];
  }
}

function decodeBase64Url(value) {
  if (!value || value.length % 4 === 1 || !/^[A-Za-z0-9_-]+$/.test(value)) invalid();
  const output = new Uint8Array(Math.floor(value.length * 6 / 8));
  let accumulator = 0, bits = 0, offset = 0;
  for (const character of value) {
    const digit = alphabet.indexOf(character);
    accumulator = accumulator * 64 + digit;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      output[offset++] = Math.floor(accumulator / 2 ** bits) & 0xff;
      accumulator %= 2 ** bits;
    }
  }
  if (accumulator !== 0) invalid();
  return output;
}

function invalid() {
  throw new TypeError("invalid canonical rendezvous manifestation");
}
