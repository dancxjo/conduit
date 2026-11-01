import { createBoundedDocumentChannel, MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES } from "./browser-bounded-document.mjs";

// Frame adaptation only. The supplied carrier must already be admitted and
// authenticated for its exact endpoints; this module creates no authority.
export function createBrowserMembershipSession({ url, carrier = null, documentWorkspace = null, WebSocketType = globalThis.WebSocket }) {
  // Preserve the canonical existing WS admission envelope. A supplied
  // protected carrier retains its own smaller admitted payload limit.
  const maximumWebSocketFrameBytes = 2 * 1024 * 1024 + 1024;
  const maximumFrameBytes = carrier === null ? maximumWebSocketFrameBytes
    : carrier.maximumFrameBytes ?? 65_519;
  if (!Number.isSafeInteger(maximumFrameBytes) || maximumFrameBytes < 1
    || maximumFrameBytes > maximumWebSocketFrameBytes) {
    throw new TypeError('Invalid admitted membership frame bound');
  }
  const maximumPendingSends = 4;
  if (carrier !== null && (!carrier || ['sendSessionFrame','receiveSessionFrame','close'].some(key => typeof carrier[key] !== 'function'))) {
    throw new TypeError('Invalid admitted browser membership session carrier');
  }
  const documents = carrier ? createBoundedDocumentChannel(carrier, documentWorkspace) : null;
  let socket, handlers, open = false, closed = false, pending = 0, largePending = false;
  let sends = Promise.resolve();
  const frame = (value, maximum = maximumFrameBytes, copy = true) => {
    const bytes = typeof value === 'string' ? new TextEncoder().encode(value)
      : value instanceof Uint8Array ? value : value instanceof ArrayBuffer ? new Uint8Array(value) : null;
    if (!bytes || bytes.length < 1 || bytes.length > maximum) throw new RangeError('Membership session frame exceeds its finite bound');
    return copy ? bytes.slice() : bytes;
  };
  const retainedSends = new Set();
  const finish = (code = 1000, reason = "") => {
    if (closed) return;
    closed = true; open = false;
    for (const record of retainedSends) record.release();
    retainedSends.clear();
    try { carrier ? documents.close() : socket?.close(typeof code === 'number' ? code : 1000, typeof reason === 'string' ? reason : ''); }
    catch { /* Closing cannot restore a failed carrier. */ }
    handlers?.close();
  };
  return Object.freeze({
    isOpen: () => open && !closed,
    start(callbacks) {
      if (handlers || closed) throw new Error('Membership session already started or closed');
      handlers = callbacks;
      if (carrier) {
        queueMicrotask(async () => {
          if (closed) return;
          open = true;
          try {
            await handlers.open();
            while (!closed) {
              const received = await documents.receive();
              if (!(received instanceof Uint8Array)) throw new TypeError('Admitted carrier must deliver bounded byte frames');
              const bytes = received;
              if (closed) break;
              try { await handlers.message({ data: bytes }); }
              finally { documents.releaseReceived(bytes); }
            }
          } catch { finish(); }
        });
      } else {
        socket = new WebSocketType(url);
        socket.binaryType = 'arraybuffer';
        socket.addEventListener('open', () => {
          if (closed) return;
          open = true;
          Promise.resolve().then(() => handlers.open()).catch(finish);
        });
        // Preserve native WS ordering while preventing asynchronous frame
        // handlers from racing credential persistence or challenge handling.
        let receiving = Promise.resolve(), pendingReceives = 0;
        socket.addEventListener('message', event => {
          if (closed) return;
          let bytes;
          try { bytes = frame(event.data); if (++pendingReceives > 4) throw new Error('Membership receive pressure'); }
          catch { finish(); return; }
          receiving = receiving.then(async () => { if (!closed) await handlers.message({data:bytes}); })
            .catch(finish).finally(() => { pendingReceives--; });
        });
        socket.addEventListener('close', finish);
        socket.addEventListener('error', finish);
      }
    },
    send(value) {
      if (!open || closed) throw new Error('Membership session is closed');
      const input = frame(value, carrier ? MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES : maximumFrameBytes, false);
      if (!carrier) { socket.send(input.slice()); return; }
      documents.validate(input);
      const large = input.length > maximumFrameBytes;
      if (pending >= maximumPendingSends || (large && largePending)) throw new Error('Membership send pressure');
      const retained = documents.retain(input);
      const bytes = retained.bytes;
      if (large) largePending = true;
      pending++;
      retainedSends.add(retained);
      sends = sends.then(() => { if (!closed) return documents.send(bytes); })
        .catch(finish).finally(() => { pending--; if (large) largePending = false; retainedSends.delete(retained); retained.release(); });
    },
    close: finish,
  });
}
