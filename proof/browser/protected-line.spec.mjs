import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

const vector = JSON.parse(await readFile(
  new URL("../../architecture/protected-line/vectors/noise-nnpsk0-v1.json", import.meta.url),
  "utf8",
));

test("browser and std protected Lines interoperate through an inspecting relay", async ({ page }) => {
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const result = await page.evaluate(async (vector) => {
    const response = await fetch("/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm");
    const { instance: { exports: api } } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
    const memory = () => new Uint8Array(api.memory.buffer);
    const input = (bytes) => memory().set(bytes, api.conduit_browser_protected_line_input_ptr());
    const output = () => memory().slice(
      api.conduit_browser_protected_line_output_ptr(),
      api.conduit_browser_protected_line_output_ptr() + api.conduit_browser_protected_line_output_len(),
    );
    const bytes = (hex) => Uint8Array.from(hex.match(/../g), (part) => Number.parseInt(part, 16));
    const hex = (value) => [...value].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    const binding = new TextEncoder().encode(JSON.stringify({
      initiator: { host_id: vector.binding.initiator_host_id, boot_id: vector.binding.initiator_boot_id },
      responder: { host_id: vector.binding.responder_host_id, boot_id: vector.binding.responder_boot_id },
      negotiation_id: vector.binding.negotiation_id,
      line_session_id: vector.binding.line_session_id,
      candidate_binding: vector.binding.candidate_binding,
      transport_binding: vector.binding.transport_binding,
    }));
    const initialize = (role, ephemeral) => {
      const material = new Uint8Array(binding.length + 64);
      material.set(binding);
      material.set(bytes(vector.preshared_key_hex), binding.length);
      material.set(bytes(ephemeral), binding.length + 32);
      input(material);
      return api.conduit_browser_protected_line_initialize(
        role,
        binding.length,
        vector.limits.maximum_payload_bytes,
        vector.limits.maximum_frames_per_direction,
        0,
        vector.limits.maximum_bytes_per_direction,
        0,
      );
    };

    const responderInitialized = initialize(1, vector.responder_ephemeral_private_key_hex);
    input(bytes(vector.first_handshake_message_hex));
    const responderRead = api.conduit_browser_protected_line_read_handshake(
      vector.first_handshake_message_hex.length / 2,
    );
    const responderWrite = api.conduit_browser_protected_line_write_handshake();
    const second = hex(output());
    input(bytes(vector.first_protected_frame_hex));
    const opened = api.conduit_browser_protected_line_open(vector.first_protected_frame_hex.length / 2);
    const plaintext = new TextDecoder().decode(output());
    input(bytes(vector.first_protected_frame_hex));
    const replay = api.conduit_browser_protected_line_open(vector.first_protected_frame_hex.length / 2);

    const rejectedRole = initialize(9, vector.initiator_ephemeral_private_key_hex);
    const inputStart = api.conduit_browser_protected_line_input_ptr();
    const rejectedKeysCleared = memory()
      .slice(inputStart + binding.length, inputStart + binding.length + 64)
      .every((byte) => byte === 0);

    const initiatorInitialized = initialize(0, vector.initiator_ephemeral_private_key_hex);
    const initiatorWrite = api.conduit_browser_protected_line_write_handshake();
    const first = hex(output());
    input(bytes(vector.second_handshake_message_hex));
    const initiatorRead = api.conduit_browser_protected_line_read_handshake(
      vector.second_handshake_message_hex.length / 2,
    );
    input(new TextEncoder().encode(vector.first_plaintext_utf8));
    const sealed = api.conduit_browser_protected_line_seal(vector.first_plaintext_utf8.length);
    const protectedFrame = hex(output());

    return {
      statuses: [responderInitialized, responderRead, responderWrite, opened, initiatorInitialized, initiatorWrite, initiatorRead, sealed],
      first,
      second,
      plaintext,
      protectedFrame,
      replay,
      rejectedRole,
      rejectedKeysCleared,
    };
  }, vector);

  expect(result.statuses).toEqual(Array(8).fill(0));
  expect(result.first).toBe(vector.first_handshake_message_hex);
  expect(result.second).toBe(vector.second_handshake_message_hex);
  expect(result.plaintext).toBe(vector.first_plaintext_utf8);
  expect(result.protectedFrame).toBe(vector.first_protected_frame_hex);
  expect(Buffer.from(result.protectedFrame, "hex").includes(Buffer.from(vector.first_plaintext_utf8))).toBe(false);
  expect(result.replay).toBe(-325);
  expect(result.rejectedRole).toBe(-300);
  expect(result.rejectedKeysCleared).toBe(true);
});
