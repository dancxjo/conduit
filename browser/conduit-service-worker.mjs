globalThis.addEventListener("install", (event) => event.waitUntil(globalThis.skipWaiting()));
globalThis.addEventListener("activate", (event) => event.waitUntil(globalThis.clients.claim()));
globalThis.addEventListener("message", (event) => {
  const reply = event.ports[0];
  if (!reply) return;
  const { operation, value } = event.data ?? {};
  reply.postMessage(operation === "echo"
    ? { ok: true, value }
    : { ok: false, code: "unsupported-operation" });
});
