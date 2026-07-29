function execute(message) {
  switch (message.operation) {
    case "uppercase":
      return { id: message.id, ok: true, value: String(message.value).toUpperCase() };
    case "echo":
      return { id: message.id, ok: true, value: message.value };
    default:
      return { id: message.id, ok: false, code: "unsupported-operation" };
  }
}

function attach(port) {
  port.onmessage = (event) => port.postMessage(execute(event.data));
  port.start?.();
}

globalThis.onconnect = (event) => attach(event.ports[0]);
if (typeof globalThis.postMessage === "function") {
  attach(globalThis);
}
