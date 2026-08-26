const FRAME_BYTES = 10;
const ACK_BYTES = 9;

export async function createR1InputPeer({ url, peer, target, status }) {
  const peerCode = peer === "browser-a" ? 1 : peer === "browser-b" ? 2 : 0;
  if (peerCode === 0) throw new Error("CND-R1-IN-001 unknown planned browser peer");
  const socket = new WebSocket(url);
  socket.binaryType = "arraybuffer";
  let sequence = 0;
  const acknowledgements = [];
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", () => reject(new Error("CND-R1-IN-002 input Line open failed")), { once: true });
  });
  status.textContent = "ready";

  function send(level) {
    if (sequence >= 2) throw new Error("CND-R1-IN-003 bounded input count exhausted");
    const bytes = new Uint8Array(FRAME_BYTES);
    const view = new DataView(bytes.buffer);
    bytes[0] = peerCode;
    view.setBigUint64(1, BigInt(sequence), true);
    bytes[9] = level ? 1 : 0;
    socket.send(bytes);
    sequence += 1;
  }
  target.addEventListener("keydown", (event) => {
    if (event.code === "Space" && !event.repeat) send(true);
  });
  target.addEventListener("keyup", (event) => {
    if (event.code === "Space") send(false);
  });
  socket.addEventListener("message", (event) => {
    const bytes = new Uint8Array(event.data);
    if (bytes.length !== ACK_BYTES) throw new Error("CND-R1-IN-004 invalid kernel acknowledgement");
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    acknowledgements.push(Object.freeze({
      mergedSequence: Number(view.getBigUint64(0, true)),
      level: bytes[8] === 1,
    }));
    status.textContent = acknowledgements.length === 2 ? "complete" : "delivered";
  });
  return Object.freeze({
    proof: () => Object.freeze({ peer, sent: sequence, acknowledgements: Object.freeze([...acknowledgements]) }),
  });
}
