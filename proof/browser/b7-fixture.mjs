export async function installB7Devices(page, { staleStatus = false } = {}) {
  await page.addInitScript(({ staleStatus }) => {
    const framed = (value) => {
      const payload = new TextEncoder().encode(JSON.stringify(value));
      const bytes = new Uint8Array(payload.length + 2);
      new DataView(bytes.buffer).setUint16(0, payload.length, false);
      bytes.set(payload, 2);
      return bytes;
    };
    const littleU32 = (value) => {
      const bytes = new Uint8Array(4);
      new DataView(bytes.buffer).setUint32(0, value, true);
      return bytes;
    };
    const littleU64 = (value) => {
      const bytes = new Uint8Array(8);
      new DataView(bytes.buffer).setBigUint64(0, BigInt(value), true);
      return bytes;
    };
    const transcript = async (provision, hostId, bootId, generation) => {
      const values = ["spawn-invitation-v1", provision.body_id, provision.invitation_id, hostId, bootId];
      const parts = [];
      for (const value of values) {
        const encoded = new TextEncoder().encode(value);
        parts.push(littleU32(encoded.length), encoded);
      }
      parts.push(littleU64(generation), new Uint8Array(provision.nonce), littleU64(provision.expires_at_millis));
      const length = parts.reduce((sum, part) => sum + part.length, 0);
      const bytes = new Uint8Array(length);
      let offset = 0;
      for (const part of parts) { bytes.set(part, offset); offset += part.length; }
      return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
    };
    const sign = async (seed, bytes) => {
      const prefix = Uint8Array.from([0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20]);
      const encoded = new Uint8Array(prefix.length + seed.length);
      encoded.set(prefix);
      encoded.set(seed, prefix.length);
      const key = await crypto.subtle.importKey("pkcs8", encoded, { name: "Ed25519" }, false, ["sign"]);
      return Array.from(new Uint8Array(await crypto.subtle.sign("Ed25519", key, bytes)));
    };

    class PicobootDevice {
      constructor() { this.vendorId = 0x2e8a; this.productId = 0x0003; this.configuration = null; this.pending = null; }
      async open() {}
      async selectConfiguration(value) { this.configuration = { configurationValue: value }; }
      async claimInterface() {}
      async releaseInterface() {}
      async close() {}
      async transferOut(_endpoint, bytes) {
        const value = new Uint8Array(bytes);
        if (value.byteLength === 32 && new DataView(value.buffer, value.byteOffset).getUint32(0, true) === 0x431fd10b) {
          const view = new DataView(value.buffer, value.byteOffset);
          this.pending = { token: view.getUint32(4, true), command: view.getUint8(8) };
        }
        return { status: "ok", bytesWritten: value.byteLength };
      }
      async transferIn() { return { status: "ok", data: new DataView(new ArrayBuffer(0)) }; }
      async controlTransferOut(_setup, bytes) { return { status: "ok", bytesWritten: bytes.byteLength }; }
      async controlTransferIn() {
        const bytes = new Uint8Array(16);
        const view = new DataView(bytes.buffer);
        view.setUint32(0, this.pending.token + (staleStatus ? 1 : 0), true);
        view.setUint32(4, 0, true);
        view.setUint8(8, this.pending.command);
        return { status: "ok", data: new DataView(bytes.buffer) };
      }
    }
    const usb = new EventTarget();
    usb.requestDevice = async () => new PicobootDevice();
    Object.defineProperty(navigator, "usb", { configurable: true, value: usb });

    const port = new EventTarget();
    port.responses = [];
    port.signals = [];
    port.open = async () => {};
    port.close = async () => {};
    port.setSignals = async (signals) => port.signals.push({ ...signals });
    port.getInfo = () => ({ usbVendorId: 0x2e8a, usbProductId: 0x000a });
    port.writable = { getWriter: () => ({
      write: async (bytes) => {
        const length = new DataView(bytes.buffer, bytes.byteOffset).getUint16(0, false);
        const provision = JSON.parse(new TextDecoder().decode(bytes.subarray(2, length + 2)));
        const hostId = "s4/pico-local";
        const bootId = "pico-boot/b7-browser-proof";
        const advertisement = {
          protocol_version: 1, host_id: hostId, boot_id: bootId, offer_generation: 1,
          profile: "pico-w-signal-kernel",
          resources: [{ pool_id: "s4/pico-cyw43-led", class_id: "conduit.resource/presentation-slot@1", capacity_units: 1, compute: null }],
          capabilities: [{
            startup_parameters: [], shorthand: null, capability_id: "pico-led-show-1",
            kind_id: "presentation/show", kind_contract_revision: "conduit.signal/presentation-show@1",
            inputs: [{ port_id: "signal", value_kind: "value/signal", direction: "Input", temporal: "Value" }], outputs: [],
            execution_profile_id: "conduit.signal/show-hosted@1",
            implementation_id: "pico-w/kernel-cyw43-show-signal-v1",
            artifact_id: "conduit-signal/pico-cyw43-show-artifact-v1",
            host_operations: [{ contract_id: "conduit.host/present@1", target_kind: "presentation/signal", maximum_in_flight: 1, maximum_input_bytes: 9, maximum_output_bytes: 256 }],
            resource_requirements: [{ class_id: "conduit.resource/presentation-slot@1", units: 1, protected_role: null, compute: null }],
            authority_requirements: [], limits: { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 9 },
          }],
          planner_capabilities: [],
        };
        const signature = await sign(new Uint8Array(provision.secret), await transcript(provision, hostId, bootId, 1));
        port.responses.push(
          framed({ protocol: 1, advertisement, friendly_label: "Pico W", verifying_key: Array(32).fill(1), freshness_sequence: 1 }),
          framed({ protocol: 2, spore_id: provision.spore_id, image_id: provision.image_id,
            invitation_id: provision.invitation_id, body_id: provision.body_id, host_id: hostId,
            boot_id: bootId, offer_generation: 1, nonce: provision.nonce, signature }),
        );
      },
      releaseLock() {},
    }) };
    port.readable = { getReader: () => ({ read: async () => ({ value: port.responses.shift(), done: false }), releaseLock() {} }) };
    Object.defineProperty(navigator, "serial", { configurable: true, value: { requestPort: async () => port } });
  }, { staleStatus });
}

export function picoUf2() {
  const bytes = new Uint8Array(512);
  const view = new DataView(bytes.buffer);
  for (const [offset, word] of [[0, 0x0a324655], [4, 0x9e5d5157], [8, 0x2000], [12, 0x10000000],
    [16, 256], [20, 0], [24, 1], [28, 0xe48bff56], [508, 0x0ab16f30]]) {
    view.setUint32(offset, word, true);
  }
  bytes.fill(1, 32, 288);
  return bytes;
}
