const INPUT_CAPACITY = 4096;

export async function initializeBrowserHost() {
  const hostId = `browser/${crypto.randomUUID()}`;
  const bootId = `browser-boot/${crypto.randomUUID()}`;
  const runtimeUrl = new URL("./runtime.wasm", import.meta.url);
  const runtime = await WebAssembly.instantiateStreaming(fetch(runtimeUrl), {});
  const api = runtime.instance.exports;
  const required = [
    "memory",
    "conduit_browser_membership_input_ptr",
    "conduit_browser_membership_input_capacity",
    "conduit_browser_membership_initialize",
  ];
  if (
    required.some((name) => !(name in api)) ||
    api.conduit_browser_membership_input_capacity() !== INPUT_CAPACITY
  ) {
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
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_membership_input_ptr(),
    initialization.length,
  ).set(initialization);
  const status = api.conduit_browser_membership_initialize(host.length, boot.length);
  seed.fill(0);
  initialization.fill(0);
  if (status < 0) throw new Error(`browser Host initialization failed ${status}`);
  return Object.freeze({ hostId, bootId, runtime: api });
}
