import { spawn } from "node:child_process";
import { createHash, X509Certificate } from "node:crypto";
import { once } from "node:events";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { promisify } from "node:util";
import { chromium, expect, test } from "@playwright/test";

const execute = promisify(execFile);

const vector = JSON.parse(await readFile(
  new URL("../../architecture/protected-line/vectors/noise-nnpsk0-v1.json", import.meta.url),
  "utf8",
));

test("browser WASM matches the native protected-Line vector", async ({ page }) => {
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
    const candidate = new TextEncoder().encode(JSON.stringify({
      schema: "conduit.relay/endpoint-candidate@1",
      relay_implementation_id: "conduit.relay/opaque-two-endpoint@1",
      relay_locator: "wss://relay.example/conduit",
      relay_server_identity: "relay.example",
      certificate_binding_sha256: Array(32).fill(1),
      negotiation_id: vector.binding.negotiation_id,
      route_id: vector.binding.candidate_binding,
      role: "initiator",
      endpoint_binding: `${vector.binding.initiator_host_id}/${vector.binding.initiator_boot_id}`,
      session_binding: {
        initiator: { host_id: vector.binding.initiator_host_id, boot_id: vector.binding.initiator_boot_id },
        responder: { host_id: vector.binding.responder_host_id, boot_id: vector.binding.responder_boot_id },
        negotiation_id: vector.binding.negotiation_id,
        line_session_id: vector.binding.line_session_id,
        candidate_binding: vector.binding.candidate_binding,
        transport_binding: vector.binding.transport_binding,
      },
      expires_at_millis: 10_000,
      relay_capability: Array(32).fill(7),
      protected_session_psk: Array(32).fill(8),
      bounds: {
        maximum_protected_frame_bytes: vector.limits.maximum_payload_bytes + 34,
        maximum_attempts: 1,
        attempt_timeout_millis: 2_000,
        maximum_payload_bytes: vector.limits.maximum_payload_bytes,
        maximum_frames_per_direction: vector.limits.maximum_frames_per_direction,
        maximum_bytes_per_direction: vector.limits.maximum_bytes_per_direction,
        handshake_timeout_millis: 2_000,
        idle_timeout_millis: 5_000,
      },
    }));
    input(candidate);
    const relayCandidate = api.conduit_browser_relay_candidate_validate(candidate.length, 1_000, 0);
    const candidateInputCleared = memory()
      .slice(api.conduit_browser_protected_line_input_ptr(), api.conduit_browser_protected_line_input_ptr() + candidate.length)
      .every((byte) => byte === 0);

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
      relayCandidate,
      candidateInputCleared,
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
  expect(result.relayCandidate).toBe(0);
  expect(result.candidateInputCleared).toBe(true);
});

test("two outbound browser clients carry one end-to-end protected session through relay semantics", async ({ page }) => {
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const result = await page.evaluate(async () => {
    const { openBrowserRelayLine } = await import("/targets/browser/host/assets/browser-relay-line.mjs");
    const wasm = await (await fetch("/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")).arrayBuffer();
    const api = async () => (await WebAssembly.instantiate(wasm.slice(0), {})).instance.exports;
    const enc = new TextEncoder();
    const dec = new TextDecoder();
    const sockets = [];
    const forwarded = [];

    class RelaySocket {
      static OPEN = 1;
      constructor(url) {
        this.url = url;
        this.readyState = 0;
        this.listeners = new Map();
        sockets.push(this);
        queueMicrotask(() => {
          this.readyState = RelaySocket.OPEN;
          this.emit("open", {});
        });
      }
      addEventListener(name, listener) {
        const list = this.listeners.get(name) ?? [];
        list.push(listener);
        this.listeners.set(name, list);
      }
      emit(name, event) {
        for (const listener of this.listeners.get(name) ?? []) listener(event);
      }
      send(value) {
        const bytes = value instanceof Uint8Array ? value.slice() : new Uint8Array(value);
        if (bytes[0] === 0x43 && bytes[1] === 0x4e && bytes[2] === 0x44 && bytes[3] === 0x52) {
          forwarded.push(bytes);
          const peer = sockets.find((socket) => socket !== this && socket.attachment?.route_id === this.attachment.route_id);
          if (!peer) throw new Error("fake relay peer missing");
          queueMicrotask(() => peer.emit("message", { data: bytes.buffer }));
          return;
        }
        const control = JSON.parse(dec.decode(bytes));
        if (control.schema === "conduit.relay/control@1") {
          this.closedByProtocol = true;
          return;
        }
        if (control.schema !== "conduit.relay/attach@1") throw new Error("fake relay control mismatch");
        this.attachment = control;
        const outcome = (status) => enc.encode(JSON.stringify({
          schema: "conduit.relay/outcome@1",
          implementation_id: "conduit.relay/opaque-two-endpoint@1",
          route_id: control.route_id,
          status,
        })).buffer;
        const peer = sockets.find((socket) => socket !== this && socket.attachment?.route_id === control.route_id);
        if (!peer) {
          queueMicrotask(() => this.emit("message", { data: outcome("waiting-for-peer") }));
        } else {
          queueMicrotask(() => {
            this.emit("message", { data: outcome("paired") });
            peer.emit("message", { data: outcome("paired") });
          });
        }
      }
      close() { this.readyState = 3; }
    }

    const binding = {
      initiator: { host_id: "host/browser/one", boot_id: "boot/browser/one" },
      responder: { host_id: "host/browser/two", boot_id: "boot/browser/two" },
      negotiation_id: "negotiation/browser-relay",
      line_session_id: "line/browser-relay",
      candidate_binding: "route/browser-relay",
      transport_binding: "relay/wss/certificate/browser-proof",
    };
    const candidate = (role, capability, endpoint) => ({
      schema: "conduit.relay/endpoint-candidate@1",
      relay_implementation_id: "conduit.relay/opaque-two-endpoint@1",
      relay_locator: "wss://relay.example/conduit",
      relay_server_identity: "relay.example",
      certificate_binding_sha256: new Uint8Array(32).fill(1),
      negotiation_id: binding.negotiation_id,
      route_id: binding.candidate_binding,
      role,
      endpoint_binding: endpoint,
      session_binding: binding,
      expires_at_millis: 10_000,
      relay_capability: new Uint8Array(32).fill(capability),
      protected_session_psk: new Uint8Array(32).fill(8),
      bounds: {
        maximum_protected_frame_bytes: 290,
        maximum_attempts: 1,
        attempt_timeout_millis: 2_000,
        maximum_payload_bytes: 256,
        maximum_frames_per_direction: 8,
        maximum_bytes_per_direction: 2_048,
        handshake_timeout_millis: 2_000,
        idle_timeout_millis: 2_000,
      },
    });
    const firstCandidate = candidate("initiator", 7, "host/browser/one/boot/browser/one");
    const secondCandidate = candidate("responder", 8, "host/browser/two/boot/browser/two");
    const [first, second] = await Promise.all([
      openBrowserRelayLine({
        api: await api(),
        candidate: firstCandidate,
        ephemeralPrivateKey: new Uint8Array(32).fill(4),
        WebSocketType: RelaySocket,
        nowMillis: 1_000,
      }),
      openBrowserRelayLine({
        api: await api(),
        candidate: secondCandidate,
        ephemeralPrivateKey: new Uint8Array(32).fill(5),
        WebSocketType: RelaySocket,
        nowMillis: 1_000,
      }),
    ]);
    await first.sendSessionFrame(enc.encode("browser secret one"));
    const atSecond = dec.decode(await second.receiveSessionFrame());
    await second.sendSessionFrame(enc.encode("browser secret two"));
    const atFirst = dec.decode(await first.receiveSessionFrame());
    first.close();
    second.close();
    return {
      atFirst,
      atSecond,
      forwarded: forwarded.length,
      leaked: forwarded.some((frame) => dec.decode(frame).includes("browser secret")),
      erased: [firstCandidate, secondCandidate].every((item) =>
        item.relay_capability.every((byte) => byte === 0) &&
        item.protected_session_psk.every((byte) => byte === 0)),
      explicitlyClosed: sockets.some((socket) => socket.closedByProtocol),
    };
  });
  expect(result).toEqual({
    atFirst: "browser secret two",
    atSecond: "browser secret one",
    forwarded: 4,
    leaked: false,
    erased: true,
    explicitlyClosed: true,
  });
});

test("Chromium and native std interoperate through an inspecting relay", async ({ page }) => {
  const peer = spawn("target/debug/protected-line-browser-peer", [], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  const exit = once(peer, "exit");
  const errors = [];
  peer.stderr.setEncoding("utf8");
  peer.stderr.on("data", (chunk) => errors.push(chunk));
  const lines = createInterface({ input: peer.stdout })[Symbol.asyncIterator]();
  const relayFrames = [];
  const forwardToNative = (bytes) => {
    const frame = Buffer.from(bytes);
    relayFrames.push(frame);
    peer.stdin.write(`${frame.toString("hex")}\n`);
  };
  const forwardToBrowser = async () => {
    const next = await lines.next();
    if (next.done) throw new Error(`native peer ended before its frame: ${errors.join("")}`);
    const frame = Buffer.from(next.value, "hex");
    relayFrames.push(frame);
    return [...frame];
  };

  try {
    await page.goto("/proof/browser/signal-dom-host.test.html");
    const firstHandshake = await page.evaluate(async () => {
      const wasm = await (await fetch("/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")).arrayBuffer();
      const { instance: { exports: api } } = await WebAssembly.instantiate(wasm, {});
      const binding = {
        initiator: { host_id: "host/browser/one", boot_id: "boot/browser/one" },
        responder: { host_id: "host/native/two", boot_id: "boot/native/two" },
        negotiation_id: "negotiation/browser-native",
        line_session_id: "line/browser-native",
        candidate_binding: "route/browser-native",
        transport_binding: "relay/inspecting-proof@1",
      };
      const encodedBinding = new TextEncoder().encode(JSON.stringify(binding));
      const input = new Uint8Array(api.memory.buffer, api.conduit_browser_protected_line_input_ptr());
      input.set(encodedBinding);
      input.fill(8, encodedBinding.length, encodedBinding.length + 32);
      input.fill(4, encodedBinding.length + 32, encodedBinding.length + 64);
      if (api.conduit_browser_protected_line_initialize(0, encodedBinding.length, 256, 8, 0, 2_048, 0) !== 0) {
        throw new Error("browser initiator refused initialization");
      }
      const secretsErased = new Uint8Array(api.memory.buffer, api.conduit_browser_protected_line_input_ptr())
        .slice(encodedBinding.length, encodedBinding.length + 64)
        .every((byte) => byte === 0);
      if (api.conduit_browser_protected_line_write_handshake() !== 0) {
        throw new Error("browser initiator refused its handshake");
      }
      const output = new Uint8Array(
        api.memory.buffer,
        api.conduit_browser_protected_line_output_ptr(),
        api.conduit_browser_protected_line_output_len(),
      );
      globalThis.protectedLineInterop = { api, secretsErased };
      return [...output];
    });
    forwardToNative(firstHandshake);
    const secondHandshake = await forwardToBrowser();
    const browserProtected = await page.evaluate((handshake) => {
      const { api } = globalThis.protectedLineInterop;
      const input = new Uint8Array(api.memory.buffer, api.conduit_browser_protected_line_input_ptr());
      input.set(handshake);
      if (api.conduit_browser_protected_line_read_handshake(handshake.length) !== 0) {
        throw new Error("browser initiator refused native handshake");
      }
      const plaintext = new TextEncoder().encode("browser-to-native secret");
      input.set(plaintext);
      if (api.conduit_browser_protected_line_seal(plaintext.length) !== 0) {
        throw new Error("browser initiator refused protected payload");
      }
      return [...new Uint8Array(
        api.memory.buffer,
        api.conduit_browser_protected_line_output_ptr(),
        api.conduit_browser_protected_line_output_len(),
      )];
    }, secondHandshake);
    forwardToNative(browserProtected);
    const nativeProtected = await forwardToBrowser();
    const result = await page.evaluate((frame) => {
      const { api, secretsErased } = globalThis.protectedLineInterop;
      const input = new Uint8Array(api.memory.buffer, api.conduit_browser_protected_line_input_ptr());
      input.set(frame);
      if (api.conduit_browser_protected_line_open(frame.length) !== 0) {
        throw new Error("browser initiator refused native protected response");
      }
      const plaintext = new TextDecoder().decode(new Uint8Array(
        api.memory.buffer,
        api.conduit_browser_protected_line_output_ptr(),
        api.conduit_browser_protected_line_output_len(),
      ));
      const closed = api.conduit_browser_protected_line_close();
      delete globalThis.protectedLineInterop;
      return { plaintext, closed, secretsErased };
    }, nativeProtected);
    peer.stdin.end();
    const [code, signal] = await exit;
    expect({ code, signal, errors: errors.join("") }).toEqual({ code: 0, signal: null, errors: "" });
    expect(result).toEqual({
      plaintext: "native-to-browser secret",
      closed: 0,
      secretsErased: true,
    });
    expect(relayFrames).toHaveLength(4);
    expect(relayFrames.some((frame) => frame.includes(Buffer.from("browser-to-native secret")))).toBe(false);
    expect(relayFrames.some((frame) => frame.includes(Buffer.from("native-to-browser secret")))).toBe(false);
  } finally {
    if (peer.exitCode === null) peer.kill("SIGTERM");
  }
});

test("two Chromium clients use the actual user-operated WSS relay service", async () => {
  test.setTimeout(30_000);
  const directory = await mkdtemp(join(tmpdir(), "conduit-browser-relay-"));
  const certificate = join(directory, "certificate.pem");
  const privateKey = join(directory, "private-key.pem");
  const provisioned = join(directory, "private-relay");
  let relay;
  let localCertificateBrowser;
  let context;
  try {
    await execute("openssl", [
      "req", "-x509", "-newkey", "rsa:2048", "-nodes",
      "-keyout", privateKey,
      "-out", certificate,
      "-days", "1",
      "-subj", "/CN=localhost",
      "-addext", "subjectAltName=DNS:localhost",
    ]);
    const certificatePem = await readFile(certificate, "utf8");
    const certificateSha256 = createHash("sha256")
      .update(new X509Certificate(certificatePem).raw)
      .digest("hex");
    const port = await unusedPort();
    const relayUrl = `wss://localhost:${port}/conduit`;
    await execute("target/debug/conduit", [
      "rendezvous-relay", "provision",
      "--relay-address", `127.0.0.1:${port}`,
      "--relay-url", relayUrl,
      "--server-identity", "localhost",
      "--certificate-sha256", certificateSha256,
      "--first-host-id", "host/browser/real-one",
      "--first-boot-id", "boot/browser/real-one",
      "--second-host-id", "host/browser/real-two",
      "--second-boot-id", "boot/browser/real-two",
      "--output", provisioned,
      "--expires-in-seconds", "60",
      "--authorize-provision",
    ]);
    const first = JSON.parse(await readFile(join(provisioned, "endpoint-first.json"), "utf8"));
    const second = JSON.parse(await readFile(join(provisioned, "endpoint-second.json"), "utf8"));
    relay = spawn("target/debug/conduit", [
      "rendezvous-relay", "serve",
      "--bind", `0.0.0.0:${port}`,
      "--public-url", relayUrl,
      "--tls-cert", certificate,
      "--tls-key", privateKey,
      "--slot", join(provisioned, "relay-slot.json"),
      "--accept-timeout-seconds", "10",
      "--authorize-network",
    ], { stdio: ["ignore", "pipe", "pipe"] });
    const relayExit = once(relay, "exit");
    const relayErrors = [];
    relay.stderr.setEncoding("utf8");
    relay.stderr.on("data", (chunk) => relayErrors.push(chunk));
    await waitForOutput(relay.stdout, "Relay ready:", 5_000);

    // This proof owns a one-run self-signed localhost certificate. Production
    // browser relay use still requires ordinary WebPKI validation.
    localCertificateBrowser = await chromium.launch({ args: ["--ignore-certificate-errors"] });
    context = await localCertificateBrowser.newContext({ ignoreHTTPSErrors: true });
    const page = await context.newPage();
    await page.goto("http://127.0.0.1:4173/proof/browser/signal-dom-host.test.html");
    const result = await page.evaluate(async ({ first, second }) => {
      const { openBrowserRelayLine } = await import("/targets/browser/host/assets/browser-relay-line.mjs");
      const wasm = await (await fetch("/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")).arrayBuffer();
      const runtime = async () => (await WebAssembly.instantiate(wasm.slice(0), {})).instance.exports;
      const [firstLine, secondLine] = await Promise.all([
        openBrowserRelayLine({
          api: await runtime(),
          candidate: first.candidate,
          ephemeralPrivateKey: new Uint8Array(32).fill(4),
        }),
        openBrowserRelayLine({
          api: await runtime(),
          candidate: second.candidate,
          ephemeralPrivateKey: new Uint8Array(32).fill(5),
        }),
      ]);
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      await firstLine.sendSessionFrame(encoder.encode("browser one through real relay"));
      const atSecond = decoder.decode(await secondLine.receiveSessionFrame());
      await secondLine.sendSessionFrame(encoder.encode("browser two through real relay"));
      const atFirst = decoder.decode(await firstLine.receiveSessionFrame());
      firstLine.close();
      secondLine.close();
      return { atFirst, atSecond };
    }, { first, second });
    expect(result).toEqual({
      atFirst: "browser two through real relay",
      atSecond: "browser one through real relay",
    });
    const [code, signal] = await relayExit;
    const relayEvidence = relayErrors.join("");
    expect({ code, signal }).toEqual({ code: 0, signal: null });
    expect(relayEvidence).toContain("forwarded_frames: 4");
    expect(relayEvidence).toContain("disposition: Closed");
    expect(relayEvidence).not.toContain("browser one through real relay");
    expect(relayEvidence).not.toContain("browser two through real relay");
  } finally {
    if (relay?.exitCode === null) relay.kill("SIGTERM");
    await context?.close();
    await localCertificateBrowser?.close();
    await rm(directory, { recursive: true, force: true });
  }
});

async function unusedPort() {
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("cannot reserve relay proof port");
  await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  return address.port;
}

function waitForOutput(stream, expected, timeoutMillis) {
  stream.setEncoding("utf8");
  return new Promise((resolve, reject) => {
    let output = "";
    const timeout = setTimeout(() => reject(new Error(`relay did not emit ${expected}: ${output}`)), timeoutMillis);
    stream.on("data", (chunk) => {
      output += chunk;
      if (output.includes(expected)) {
        clearTimeout(timeout);
        resolve();
      }
    });
    stream.on("end", () => {
      clearTimeout(timeout);
      reject(new Error(`relay ended before ${expected}: ${output}`));
    });
  });
}
