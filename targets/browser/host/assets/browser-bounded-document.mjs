// Existing conduit_wire CBD1 documents over one already authenticated carrier.
// This adapter creates no identity, membership or execution authority.
export const MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES = 2 * 1024 * 1024;
export const BOUNDED_DOCUMENT_HEADER_BYTES = 44;
const magic = [67, 66, 68, 49];
const isChunk = bytes => bytes.length >= 4 && magic.every((byte, index) => bytes[index] === byte);

export function createBoundedDocumentChannel(carrier, documentWorkspace = null) {
  const maximumFrameBytes = carrier.maximumFrameBytes ?? 65_519;
  const timeoutMillis = carrier.documentTimeoutMillis ?? 30_000;
  if (!Number.isSafeInteger(maximumFrameBytes) || maximumFrameBytes < 1 || maximumFrameBytes > MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES + 1024 ||
      !Number.isSafeInteger(timeoutMillis) || timeoutMillis < 1 || timeoutMillis > 300_000) {
    throw new TypeError('Invalid admitted document carrier bounds');
  }
  // Local storage selection only. The caller must derive this independently
  // from actual admission; frame traffic capacity never implies workspace.
  const selectedMaximum = documentWorkspace?.maximumDocumentBytes;
  const workspaceCapacity = documentWorkspace?.capacityBytes;
  if (documentWorkspace !== null && (!documentWorkspace ||
      !Number.isSafeInteger(selectedMaximum) || selectedMaximum < 1 || selectedMaximum > MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES ||
      !Number.isSafeInteger(workspaceCapacity) || workspaceCapacity < 1 || workspaceCapacity > 0xffff_ffff)) {
    throw new TypeError('Invalid selected document workspace');
  }
  const maximumDocumentBytes = selectedMaximum ?? Math.min(maximumFrameBytes, MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES);
  let retainedBytes = 0;
  const reservations = new Set(), buffers = new Set();
  const reserve = length => {
    if (closed) throw new Error('Document carrier is closed');
    if (documentWorkspace !== null && length > workspaceCapacity - retainedBytes) {
      throw new RangeError('Selected document workspace capacity exceeded');
    }
    if (documentWorkspace === null) return () => {};
    retainedBytes += length;
    let released = false;
    const release = () => { if (!released) { released = true; retainedBytes -= length; reservations.delete(release); } };
    reservations.add(release);
    return release;
  };
  const buffer = (bytes, release) => {
    if (closed) { bytes.fill(0); release(); throw new Error('Document carrier is closed'); }
    let released = false;
    const record = { bytes, release() { if (!released) { released = true; bytes.fill(0); release(); buffers.delete(record); } } };
    buffers.add(record); return record;
  };
  const allocate = length => {
    const release = reserve(length);
    try { return buffer(new Uint8Array(length), release); }
    catch (error) { release(); throw error; }
  };
  const payloadBytes = maximumFrameBytes - BOUNDED_DOCUMENT_HEADER_BYTES;
  const maximumChunks = payloadBytes > 0 ? Math.ceil(MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES / payloadBytes) : 0;
  let closed = false, sending = false, receiving = false, assembly = null, scratch = null, completed = null;
  const cancellations = new Set();
  const current = () => { if (closed) throw new Error('Document carrier is closed'); };
  const close = () => {
    if (closed) return;
    closed = true;
    for (const record of buffers) record.release();
    for (const release of reservations) release();
    assembly = null; scratch = null; completed = null;
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
  const validate = bytes => {
    current();
    if (!(bytes instanceof Uint8Array) || bytes.length < 1 || bytes.length > MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES ||
        (documentWorkspace !== null && bytes.length > maximumDocumentBytes) ||
        (bytes.length > maximumFrameBytes && (documentWorkspace === null || payloadBytes <= 0 || bytes.length > maximumDocumentBytes))) {
      throw new RangeError('Membership document requires selected workspace within its finite bound');
    }
  };
  const hash = async (bytes, deadline) => {
    const releaseDigest = reserve(32);
    let releaseCopy = () => {};
    try {
      // WebCrypto may copy its input: reserve that finite transient backing too.
      releaseCopy = reserve(bytes.length);
      const digest = await until(() => crypto.subtle.digest('SHA-256', bytes), deadline);
      current();
      return buffer(new Uint8Array(digest), releaseDigest);
    } catch (error) { releaseDigest(); throw error; }
    finally { releaseCopy(); }
  };
  return Object.freeze({
    validate,
    retain(bytes) {
      validate(bytes);
      const release = reserve(bytes.length);
      try { return buffer(bytes.slice(), release); }
      catch (error) { release(); throw error; }
    },
    releaseReceived(bytes) {
      if (completed?.bytes === bytes) { completed.release(); completed = null; }
    },
    async send(bytes) {
      current();
      if (sending) throw new Error('Document send pressure');
      sending = true;
      const deadline = performance.now() + timeoutMillis;
      try {
        validate(bytes);
        if (bytes.length <= maximumFrameBytes) {
          await until(() => carrier.sendSessionFrame(bytes), deadline);
          return;
        }
        const digest = await hash(bytes, deadline);
        scratch = allocate(maximumFrameBytes);
        try {
          for (let offset = 0; offset < bytes.length;) {
            current();
            const length = Math.min(payloadBytes, bytes.length - offset);
            scratch.bytes.set(magic);
            const header = new DataView(scratch.bytes.buffer);
            header.setUint32(4, bytes.length, true);
            header.setUint32(8, offset, true);
            scratch.bytes.set(digest.bytes, 12);
            scratch.bytes.set(bytes.subarray(offset, offset + length), BOUNDED_DOCUMENT_HEADER_BYTES);
            await until(() => carrier.sendSessionFrame(scratch.bytes.subarray(0, BOUNDED_DOCUMENT_HEADER_BYTES + length)), deadline);
            offset += length;
          }
        } finally { digest.release(); scratch?.release(); scratch = null; }
      } catch (error) { close(); throw error; }
      finally { sending = false; }
    },
    async receive() {
      current();
      if (receiving || completed) throw new Error('Document receive pressure');
      receiving = true;
      const deadline = performance.now() + timeoutMillis;
      try {
        let bytes = frame(await until(() => carrier.receiveSessionFrame(), deadline));
        current();
        if (!isChunk(bytes)) {
          if (documentWorkspace !== null && bytes.length > maximumDocumentBytes) throw new RangeError('Document exceeds selected logical bound');
          return bytes;
        }
        if (documentWorkspace === null) throw new RangeError('Bounded document requires selected workspace');
        let total, expected, offset = 0, chunks = 0;
        for (;;) {
          if (!isChunk(bytes) || bytes.length <= BOUNDED_DOCUMENT_HEADER_BYTES || ++chunks > maximumChunks) {
            throw new Error('Invalid bounded document chunk');
          }
          const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
          const declared = header.getUint32(4, true), position = header.getUint32(8, true);
          const digest = bytes.subarray(12, BOUNDED_DOCUMENT_HEADER_BYTES);
          if (chunks === 1) {
            if (declared < 1 || declared > maximumDocumentBytes) throw new RangeError('Document extent exceeds its finite bound');
            total = declared;
            assembly = allocate(total + 32);
            expected = assembly.bytes.subarray(total); expected.set(digest);
          }
          if (declared !== total || position !== offset || !digest.every((byte, index) => byte === expected[index]) ||
              bytes.length - BOUNDED_DOCUMENT_HEADER_BYTES > total - offset) throw new Error('Bounded document identity or offset mismatch');
          assembly.bytes.set(bytes.subarray(BOUNDED_DOCUMENT_HEADER_BYTES), offset);
          offset += bytes.length - BOUNDED_DOCUMENT_HEADER_BYTES;
          if (offset === total) {
            const actual = await hash(assembly.bytes.subarray(0, total), deadline);
            try {
              if (!actual.bytes.every((byte, index) => byte === expected[index])) throw new Error('Bounded document digest mismatch');
              current();
              completed = assembly; assembly = null;
              // Retain the same selected workspace until the consumer releases it.
              const value = completed.bytes.subarray(0, total);
              completed = { bytes: value, release: completed.release };
              return value;
            } finally { actual.release(); }
          }
          bytes = frame(await until(() => carrier.receiveSessionFrame(), deadline));
        }
      } catch (error) { close(); throw error; }
      finally { receiving = false; }
    },
    close,
  });
}
