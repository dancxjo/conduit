const INPUT_CAPACITY = 4096;
const hostId = `browser/${crypto.randomUUID()}`;
const bootId = `browser-boot/${crypto.randomUUID()}`;
const runtime = await WebAssembly.instantiateStreaming(fetch("/runtime.wasm"), {});
const api = runtime.instance.exports;
const required = [
  "memory",
  "conduit_browser_membership_input_ptr",
  "conduit_browser_membership_input_capacity",
  "conduit_browser_membership_initialize",
];
if (required.some((name) => !(name in api)) ||
    api.conduit_browser_membership_input_capacity() !== INPUT_CAPACITY) {
  throw new Error("browser Host runtime ABI is incomplete");
}
const encoder = new TextEncoder();
const host = encoder.encode(hostId);
const boot = encoder.encode(bootId);
const seed = crypto.getRandomValues(new Uint8Array(32));
const initialization = new Uint8Array(host.length + boot.length + seed.length);
initialization.set(host);
initialization.set(boot, host.length);
initialization.set(seed, host.length + boot.length);
new Uint8Array(api.memory.buffer, api.conduit_browser_membership_input_ptr(), initialization.length)
  .set(initialization);
const status = api.conduit_browser_membership_initialize(host.length, boot.length);
seed.fill(0);
initialization.fill(0);
if (status < 0) throw new Error(`browser Host initialization failed ${status}`);

document.querySelector("#host-id").textContent = hostId;
document.querySelector("#boot-id").textContent = bootId;
document.querySelector("#identity").hidden = false;
document.querySelector("#status").textContent = "Current and independently initialized";
globalThis.__conduitBrowserHost = Object.freeze({ hostId, bootId, runtime: api });
const { createBrowserMediaHost } = await import("/media-host.mjs");
const media = createBrowserMediaHost({ api, hostId, bootId });
document.querySelector("#media").hidden = false;
document.querySelector("#camera").addEventListener("click", () => media.acquire("camera"));
document.querySelector("#microphone").addEventListener("click", () => media.acquire("microphone"));
globalThis.__conduitBrowserHost = Object.freeze({ hostId, bootId, runtime: api, media });
