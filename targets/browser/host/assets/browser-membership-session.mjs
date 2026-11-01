// Frame adaptation only. The supplied carrier must already be admitted and
// authenticated for its exact endpoints; this module creates no authority.
export function createBrowserMembershipSession({ url, carrier = null, WebSocketType = globalThis.WebSocket }) {
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
  let socket, handlers, open = false, closed = false, pending = 0;
  let sends = Promise.resolve();
  const frame = value => {
    const bytes = typeof value === 'string' ? new TextEncoder().encode(value)
      : value instanceof Uint8Array ? value : value instanceof ArrayBuffer ? new Uint8Array(value) : null;
    if (!bytes || bytes.length < 1 || bytes.length > maximumFrameBytes) throw new RangeError('Membership session frame exceeds its finite bound');
    return bytes.slice();
  };
  const retainedSends = new Set();
  const finish = (code = 1000, reason = "") => {
    if (closed) return;
    closed = true; open = false;
    for (const bytes of retainedSends) bytes.fill(0);
    retainedSends.clear();
    try { carrier ? carrier.close() : socket?.close(typeof code === 'number' ? code : 1000, typeof reason === 'string' ? reason : ''); }
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
              const received = await carrier.receiveSessionFrame();
              if (!(received instanceof Uint8Array)) throw new TypeError('Admitted carrier must deliver bounded byte frames');
              const bytes = frame(received);
              if (closed) break;
              await handlers.message({ data: bytes });
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
      const bytes = frame(value);
      if (!carrier) { socket.send(bytes); return; }
      if (pending >= maximumPendingSends) throw new Error('Membership send pressure');
      pending++;
      retainedSends.add(bytes);
      sends = sends.then(() => { if (!closed) return carrier.sendSessionFrame(bytes); })
        .catch(finish).finally(() => { pending--; retainedSends.delete(bytes); bytes.fill(0); });
    },
    close: finish,
  });
}
