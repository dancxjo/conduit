import { openBrowserHostIdentity, resetBrowserHostIdentity } from "./browser-host-identity.mjs";

const INPUT_CAPACITY = 4096;

export async function initializeBrowserHost(runtimeBytes, options = {}) {
  if (!(runtimeBytes instanceof Uint8Array) || runtimeBytes.length === 0) {
    throw new Error("browser Host runtime bytes were not admitted");
  }
  const identity = await openBrowserHostIdentity(options);
  const hostId = identity.hostId;
  const bootId = `browser-boot/${crypto.randomUUID()}`;
  const runtime = await WebAssembly.instantiate(runtimeBytes, {});
  const api = runtime.instance.exports;
  const required = [
    "memory",
    "conduit_browser_membership_input_ptr",
    "conduit_browser_membership_input_capacity",
    "conduit_browser_membership_initialize",
    "conduit_browser_membership_output_ptr",
    "conduit_browser_membership_output_len",
    "conduit_browser_membership_advertisement",
    "conduit_browser_membership_prove",
    "conduit_browser_membership_prove_return",
    "conduit_browser_membership_prove_spawn",
  ];
  if (required.some((name) => !(name in api)) || api.conduit_browser_membership_input_capacity() !== INPUT_CAPACITY) {
    throw new Error("browser Host runtime ABI is incomplete");
  }
  const encoder = new TextEncoder();
  const host = encoder.encode(hostId);
  const boot = encoder.encode(bootId);
  const seed = identity.seed.slice();
  const initialization = new Uint8Array(host.length + boot.length + seed.length);
  initialization.set(host);
  initialization.set(boot, host.length);
  initialization.set(seed, host.length + boot.length);
  new Uint8Array(api.memory.buffer, api.conduit_browser_membership_input_ptr(), initialization.length).set(initialization);
  const status = api.conduit_browser_membership_initialize(host.length, boot.length);
  seed.fill(0);
  initialization.fill(0);
  if (status < 0) throw new Error(`browser Host initialization failed ${status}`);
  const membership = createMembershipClient(api, hostId, bootId);
  return Object.freeze({
    schema: "conduit.browser/host-incarnation@1",
    profile: identity.profile,
    hostId,
    bootId,
    runtime: api,
    membership,
    resetHostIdentity(confirmation) {
      if (confirmation !== "conduit.browser/reset-host-identity@1") {
        throw new Error("browser Host identity reset requires explicit confirmation");
      }
      return resetBrowserHostIdentity();
    },
  });
}

function createMembershipClient(api, hostId, bootId) {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });

  function outputBytes() {
    return new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_membership_output_ptr(),
      api.conduit_browser_membership_output_len(),
    ).slice();
  }

  function invokeJson(operation, value) {
    const encoded = encoder.encode(JSON.stringify(value));
    if (encoded.length === 0 || encoded.length > INPUT_CAPACITY) {
      throw new Error("browser Host membership request exceeds its admitted bound");
    }
    new Uint8Array(api.memory.buffer, api.conduit_browser_membership_input_ptr(), encoded.length).set(encoded);
    const code = api[operation](encoded.length);
    if (code < 0) throw new Error(`browser Host membership request refused (${code})`);
    return outputBytes();
  }

  return Object.freeze({
    schema: "conduit.browser/body-membership-client@1",
    hostId,
    bootId,
    advertisement() {
      const code = api.conduit_browser_membership_advertisement();
      if (code < 0) throw new Error(`browser Host advertisement refused (${code})`);
      return JSON.parse(decoder.decode(outputBytes()));
    },
    proveAdmission(challenge) {
      return Object.freeze({ hostId, bootId, signature: Array.from(invokeJson("conduit_browser_membership_prove", challenge)) });
    },
    proveReturn(challenge) {
      return Object.freeze({ hostId, bootId, signature: Array.from(invokeJson("conduit_browser_membership_prove_return", challenge)) });
    },
    proveSpawn(claim, secret) {
      return Object.freeze({ hostId, bootId, signature: Array.from(invokeJson("conduit_browser_membership_prove_spawn", { claim, secret })) });
    },
  });
}
