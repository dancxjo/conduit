import { initializeBrowserHost as initializeFromBytes } from "./browser-host-membership.mjs";

// Bound the reviewed superset independently of incidental optimizer output.
const MAXIMUM_RUNTIME_BYTES = 8 * 1024 * 1024;

export async function initializeBrowserHost(options = {}) {
  if (options.runtimeBytes) return initializeFromBytes(options.runtimeBytes, options);
  const response = await fetch(options.runtimeUrl ?? new URL("./runtime.wasm", import.meta.url));
  if (!response.ok) throw new Error("browser Host runtime is unavailable");
  const runtimeBytes = new Uint8Array(await response.arrayBuffer());
  if (runtimeBytes.length === 0 || runtimeBytes.length > MAXIMUM_RUNTIME_BYTES) {
    throw new Error("browser Host runtime exceeds its admitted bound");
  }
  return initializeFromBytes(runtimeBytes, options);
}
