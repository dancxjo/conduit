// Existing conduit_wire CBD1 documents over one already authenticated carrier.
// This adapter creates no identity, membership or execution authority.
export const MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES = 2 * 1024 * 1024;
export const BOUNDED_DOCUMENT_HEADER_BYTES = 44;
const magic = [67, 66, 68, 49];
const isChunk = bytes => bytes.length >= 4 && magic.every((byte, index) => bytes[index] === byte);

export function createBoundedDocumentChannel(carrier) {
  const maximumFrameBytes = carrier.maximumFrameBytes ?? 65_519;
  const timeoutMillis = carrier.documentTimeoutMillis ?? 30_000;
  if (!Number.isSafeInteger(maximumFrameBytes) || maximumFrameBytes < 1 || maximumFrameBytes > 65_519 ||
      !Number.isSafeInteger(timeoutMillis) || timeoutMillis < 1 || timeoutMillis > 300_000) {
    throw new TypeError('Invalid admitted document carrier bounds');
  }
  const payloadBytes = maximumFrameBytes - BOUNDED_DOCUMENT_HEADER_BYTES;
  const maximumChunks = payloadBytes > 0 ? Math.ceil(MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES / payloadBytes) : 0;
  let closed = false, sending = false, receiving = false, assembly = null, scratch = null;
  const cancellations = new Set();
  const current = () => { if (closed) throw new Error('Document carrier is closed'); };
  const close = () => {
    if (closed) return;
    closed = true;
    assembly?.fill(0); assembly = null;
    scratch?.fill(0); scratch = null;
    for (const reject of cancellations) reject(new Error('Document carrier is closed'));
    cancellations.clear();
    carrier.close();
  };
  const frame = bytes => {
    if (!(bytes instanceof Uint8Array) || bytes.length < 1 || bytes.length > maximumFrameBytes) {
      throw new RangeError('Document frame exceeds its admitted bound');
    }
    return bytes;
  };
  const until = async (operation, deadline) => {
    current();
    const remaining = deadline - performance.now();
    if (remaining <= 0) throw new Error('Document deadline elapsed');
    let timer, cancel;
    try {
      const result = await Promise.race([operation(), new Promise((_, reject) => {
        cancel = reject; cancellations.add(cancel);
        timer = setTimeout(() => reject(new Error('Document deadline elapsed')), remaining);
      })]);
      current();
      if (performance.now() >= deadline) throw new Error('Document deadline elapsed');
      return result;
    } finally { clearTimeout(timer); cancellations.delete(cancel); }
  };
  return Object.freeze({
    validate(bytes) {
      current();
      if (!(bytes instanceof Uint8Array) || bytes.length < 1 || bytes.length > MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES ||
          (bytes.length > maximumFrameBytes && payloadBytes <= 0)) {
        throw new RangeError('Membership document exceeds its finite bound');
      }
    },
    async send(bytes) {
      current();
      if (sending) throw new Error('Document send pressure');
      sending = true;
      const deadline = performance.now() + timeoutMillis;
      try {
        this.validate(bytes);
        if (bytes.length <= maximumFrameBytes) {
          await until(() => carrier.sendSessionFrame(bytes), deadline);
          return;
        }
        const digest = new Uint8Array(await until(() => crypto.subtle.digest('SHA-256', bytes), deadline));
        scratch = new Uint8Array(maximumFrameBytes);
        try {
          for (let offset = 0; offset < bytes.length;) {
            current();
            const length = Math.min(payloadBytes, bytes.length - offset);
            scratch.set(magic);
            const header = new DataView(scratch.buffer);
            header.setUint32(4, bytes.length, true);
            header.setUint32(8, offset, true);
            scratch.set(digest, 12);
            scratch.set(bytes.subarray(offset, offset + length), BOUNDED_DOCUMENT_HEADER_BYTES);
            await until(() => carrier.sendSessionFrame(scratch.subarray(0, BOUNDED_DOCUMENT_HEADER_BYTES + length)), deadline);
            offset += length;
          }
        } finally { digest.fill(0); scratch?.fill(0); scratch = null; }
      } catch (error) { close(); throw error; }
      finally { sending = false; }
    },
    async receive() {
      current();
      if (receiving) throw new Error('Document receive pressure');
      receiving = true;
      try {
        let bytes = frame(await carrier.receiveSessionFrame());
        current();
        if (!isChunk(bytes)) return bytes;
        const deadline = performance.now() + timeoutMillis;
        let total, expected, offset = 0, chunks = 0;
        for (;;) {
          if (!isChunk(bytes) || bytes.length <= BOUNDED_DOCUMENT_HEADER_BYTES || ++chunks > maximumChunks) {
            throw new Error('Invalid bounded document chunk');
          }
          const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
          const declared = header.getUint32(4, true), position = header.getUint32(8, true);
          const digest = bytes.subarray(12, BOUNDED_DOCUMENT_HEADER_BYTES);
          if (chunks === 1) {
            if (declared < 1 || declared > MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES) throw new RangeError('Document extent exceeds its finite bound');
            total = declared; expected = digest.slice(); assembly = new Uint8Array(total);
          }
          if (declared !== total || position !== offset || !digest.every((byte, index) => byte === expected[index]) ||
              bytes.length - BOUNDED_DOCUMENT_HEADER_BYTES > total - offset) throw new Error('Bounded document identity or offset mismatch');
          assembly.set(bytes.subarray(BOUNDED_DOCUMENT_HEADER_BYTES), offset);
          offset += bytes.length - BOUNDED_DOCUMENT_HEADER_BYTES;
          if (offset === total) {
            const actual = new Uint8Array(await until(() => crypto.subtle.digest('SHA-256', assembly), deadline));
            if (!actual.every((byte, index) => byte === expected[index])) throw new Error('Bounded document digest mismatch');
            current();
            const completed = assembly; assembly = null;
            return completed;
          }
          bytes = frame(await until(() => carrier.receiveSessionFrame(), deadline));
        }
      } catch (error) { close(); throw error; }
      finally { receiving = false; }
    },
    close,
  });
}
