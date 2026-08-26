const MAXIMUM_MESSAGE_BYTES = 256;
const MAXIMUM_HISTORY_ITEMS = 16;

export async function createPoolWebchat({ url, list, input, button, status }) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const socket = new WebSocket(url);
  socket.binaryType = "arraybuffer";
  let disconnected = false;
  let sent = 0;
  let received = 0;
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", () => reject(new Error("CND-POOL-CHAT-001 line open failed")), { once: true });
  });
  status.textContent = "joined";
  button.disabled = false;
  socket.addEventListener("message", (event) => {
    const bytes = new Uint8Array(event.data);
    if (bytes.length === 0 || bytes.length > MAXIMUM_MESSAGE_BYTES) {
      throw new Error("CND-POOL-CHAT-002 invalid addressed delivery");
    }
    const item = document.createElement("li");
    item.textContent = decoder.decode(bytes);
    list.append(item);
    while (list.children.length > MAXIMUM_HISTORY_ITEMS) list.firstElementChild.remove();
    received += 1;
  });
  socket.addEventListener("close", () => {
    disconnected = true;
    button.disabled = true;
    status.textContent = "left";
  });

  function submit() {
    const bytes = encoder.encode(input.value);
    if (bytes.length === 0 || bytes.length > MAXIMUM_MESSAGE_BYTES) {
      throw new Error("CND-POOL-CHAT-003 message must contain 1..256 bytes");
    }
    socket.send(bytes);
    input.value = "";
    sent += 1;
  }
  function leave() {
    if (!disconnected) socket.close(1000, "leave shared pool");
  }
  button.addEventListener("click", submit);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") submit();
  });
  return Object.freeze({
    submit: (text) => { input.value = text; submit(); },
    leave,
    proof: () => Object.freeze({
      sent,
      received,
      disconnected,
      history: Object.freeze([...list.children].map((item) => item.textContent)),
    }),
  });
}
