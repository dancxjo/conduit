import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { connectRendezvousHost, decodeRendezvousCode } from "../../products/creche/browser/creche-rendezvous.mjs";
import { decodeRendezvousCoseSign1, decodeRendezvousManifestation } from "../../products/creche/browser/rendezvous-cbor.mjs";

test("browser decodes the exact canonical Rust and ConduitOS rendezvous vector", () => {
  const hex = readFileSync(new URL("../../architecture/body/schemas/running-host-rendezvous-v1.hex", import.meta.url), "utf8").trim();
  const envelope = `conduit-rendezvous-v1:${Buffer.from(hex, "hex").toString("base64url")}`;
  const decoded = decodeRendezvousManifestation(envelope, 1_700_000_000_000);
  assert.equal(decoded.schema, "conduit.host/rendezvous-cbor@1");
  assert.deepEqual(decoded.candidates.map(({ candidate_id }) => candidate_id),
    ["candidate/direct", "candidate/relay"]);
  assert.deepEqual([...decoded.session_secret], new Array(32).fill(0x55));
});

test("browser decodes the shared bounded WebRTC candidate without retaining signaling", () => {
  const candidate = new Map([
    [0, "candidate/webrtc"],
    [1, 4],
    [2, "webrtc-bootstrap:operator/negotiation-7"],
    [3, new Map([[0, "host/peer/key-7"], [1, new Uint8Array(32).fill(0xef)]])],
    [4, Date.now() + 30_000],
    [5, 1],
    [6, 10_000],
  ]);
  const decoded = decodeRendezvousManifestation(
    `conduit-rendezvous-v1:${Buffer.from(encodeCanonicalCbor(new Map([
      [0, 1], [1, [candidate]], [2, new Uint8Array(32).fill(0xcd)],
    ]))).toString("base64url")}`,
  );
  assert.equal(decoded.candidates[0].line_family, "web-rtc-data-channel");
  assert.equal(decoded.candidates[0].reachability, "webrtc-bootstrap:operator/negotiation-7");
  assert.equal("sdp" in decoded.candidates[0], false);
  assert.equal("ice_credentials" in decoded.candidates[0], false);
});
test("browser verifies the exact RFC 9052 Sign1 vector under caller-owned policy", async () => {
  const hex = readFileSync(new URL("../../architecture/body/schemas/running-host-rendezvous-sign1-v1.hex", import.meta.url), "utf8").trim();
  const publicKey = await crypto.subtle.importKey("raw",
    Buffer.from("ee45ecb9aca01a0abd83ef56dd985c8c874e6e7f4aebcedf20bd8d88c2a0add7", "hex"),
    "Ed25519", false, ["verify"]);
  const verified = await decodeRendezvousCoseSign1(Buffer.from(hex, "hex"), async ({
    keyId, sigStructure, signature,
  }) => {
    assert.equal(new TextDecoder().decode(keyId), "operator-signing-key-1");
    return await crypto.subtle.verify("Ed25519", publicKey, signature, sigStructure)
      ? "configured test operator" : null;
  }, 1_700_000_000_000);
  assert.equal(verified.schema, "conduit.host/rendezvous-cose-sign1@1");
  assert.equal(verified.attribution, "configured test operator");
  assert.equal(verified.descriptor.candidates[0].candidate_id, "candidate/relay");

  const altered = Buffer.from(hex, "hex");
  altered[altered.length - 1] ^= 1;
  await assert.rejects(() => decodeRendezvousCoseSign1(altered, async ({
    sigStructure, signature,
  }) => await crypto.subtle.verify("Ed25519", publicKey, signature, sigStructure),
  1_700_000_000_000), /invalid rendezvous COSE/);
});

test("running host rendezvous code resolves one authenticated loopback WebSocket Line", () => {
  const code = `C1-WS-104D-${"AB".repeat(32)}`;
  const decoded = decodeRendezvousCode(code.toLowerCase());
  assert.equal(decoded.url, "ws://127.0.0.1:4173/conduit");
  assert.equal(decoded.line_id, "conduit-line/loopback-websocket@1");
  assert.deepEqual([...decoded.session_secret], new Array(32).fill(0xab));
});

test("malformed, zero-secret, and non-loopback codes refuse", () => {
  assert.throws(() => decodeRendezvousCode("ws://example.test/host"), { code: "InvalidCode" });
  assert.throws(() => decodeRendezvousCode(`C1-WS-104D-${"00".repeat(32)}`), { code: "InvalidCode" });
  assert.throws(() => decodeRendezvousCode(`C1-TCP-104D-${"AB".repeat(32)}`), { code: "InvalidCode" });
});

test("serial rendezvous code selects the browser-attended serial Line", () => {
  const decoded = decodeRendezvousCode(`C1-SERIAL-${"CD".repeat(32)}`);
  assert.equal(decoded.carrier, "serial");
  assert.equal(decoded.url, null);
  assert.equal(decoded.line_id, "conduit-line/serial-text@1");
});

test("finite secure LAN descriptor selects the authenticated TLS Line", () => {
  const descriptor = secureDescriptor();
  const decoded = decodeRendezvousCode(descriptor);
  assert.equal(decoded.url, "wss://conduit-host.test:7443/conduit");
  assert.equal(decoded.line_id, "conduit-line/authenticated-tls-stream@1");
  assert.equal(decoded.maximum_attempts, 1);
  assert.equal(decoded.transport_binding_sha256, "ab".repeat(32));

  assert.throws(() => decodeRendezvousCode(secureDescriptor({ expiresAt: Date.now() - 1 })),
    { code: "InvalidDescriptor" });
  assert.throws(() => decodeRendezvousCode(secureDescriptor({ scheme: "ws" })),
    { code: "InvalidDescriptor" });
  assert.throws(() => decodeRendezvousCode(`conduit-rendezvous-v1:${descriptor.split(":")[1]}A`),
    { code: "InvalidDescriptor" });
});

function secureDescriptor({ expiresAt = Date.now() + 30_000, scheme = "wss" } = {}) {
  const candidate = new Map([
    [0, "candidate/secure-lan"],
    [1, 0],
    [2, `${scheme}://conduit-host.test:7443/conduit`],
    [3, new Map([[0, "conduit-host.test"], [1, new Uint8Array(32).fill(0xab)]])],
    [4, expiresAt],
    [5, 1],
    [6, 10_000],
  ]);
  const bytes = encodeCanonicalCbor(new Map([
    [0, 1], [1, [candidate]], [2, new Uint8Array(32).fill(0xcd)],
  ]));
  return `conduit-rendezvous-v1:${Buffer.from(bytes).toString("base64url")}`;
}

function encodeCanonicalCbor(value) {
  if (Number.isSafeInteger(value)) return encodeCborHead(0, value);
  if (typeof value === "string") {
    const bytes = new TextEncoder().encode(value);
    return concat(encodeCborHead(3, bytes.length), bytes);
  }
  if (value instanceof Uint8Array) return concat(encodeCborHead(2, value.length), value);
  if (Array.isArray(value)) return concat(encodeCborHead(4, value.length), ...value.map(encodeCanonicalCbor));
  if (value instanceof Map) return concat(encodeCborHead(5, value.size),
    ...[...value].flatMap(([key, item]) => [encodeCanonicalCbor(key), encodeCanonicalCbor(item)]));
  throw new TypeError("unsupported test CBOR value");
}

function encodeCborHead(major, value) {
  if (value < 24) return Uint8Array.of(major << 5 | value);
  const size = value <= 0xff ? 1 : value <= 0xffff ? 2 : value <= 0xffff_ffff ? 4 : 8;
  const result = new Uint8Array(1 + size);
  result[0] = major << 5 | ({ 1: 24, 2: 25, 4: 26, 8: 27 })[size];
  let remaining = value;
  for (let index = size; index > 0; index -= 1) {
    result[index] = remaining % 256;
    remaining = Math.floor(remaining / 256);
  }
  return result;
}

function concat(...parts) {
  const result = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) { result.set(part, offset); offset += part.length; }
  return result;
}

test("one code carries a current advertisement and exactly one invitation proof", async () => {
  const code = `C1-WS-104D-${"AB".repeat(32)}`;
  const session = await connectRendezvousHost(code, { WebSocketClass: FakeWebSocket });
  assert.equal(session.descriptor.target_id, "std/x86_64/computer");
  const prepared = {
    spore_id: "spore/test",
    image_id: "image/test",
    invitation_id: "invitation/test",
    body_id: "body/test",
    invitation_nonce: new Array(32).fill(17),
    invitation_secret: new Array(32).fill(13),
    invitation_expires_at_millis: Date.now() + 60_000,
  };
  const join = await session.invite(prepared);
  assert.equal(join.host_id, "host/test");
  assert.equal(join.signature.length, 64);
  assert.ok(prepared.invitation_secret.every((byte) => byte === 0));
  await assert.rejects(() => session.invite(prepared), { code: "Replay" });
});

test("Workspace may retain the authenticated joined Line until explicit close", async () => {
  const session = await connectRendezvousHost(`C1-WS-104D-${"AB".repeat(32)}`, {
    WebSocketClass: FakeWebSocket,
    retainLine: true,
  });
  const prepared = {
    spore_id: "spore/retained", image_id: "image/retained", invitation_id: "invitation/retained",
    body_id: "body/retained", invitation_nonce: new Array(32).fill(23),
    invitation_secret: new Array(32).fill(31), invitation_expires_at_millis: Date.now() + 60_000,
  };
  const join = await session.invite(prepared);
  assert.equal(join.line.schema, "conduit.creche/joined-host-line@1");
  assert.equal(FakeWebSocket.last.readyState, 1);
  await assert.rejects(() => join.line.prepareRemote({ plan_id: "plan/too-early" }), { code: "MembershipNotRetained" });
  const credential = {
    credential_id: "credential/retained", body_id: "body/retained", part_id: "part/retained",
    host_id: "host/test", boot_id: "boot/test", issued_at_millis: Date.now(),
  };
  const retained = await join.line.retainMembership(credential);
  assert.equal(retained.body_id, credential.body_id);
  assert.equal(retained.part_id, credential.part_id);
  await assert.rejects(() => join.line.retainMembership(credential), { code: "Replay" });
  const realization = {
    host_id: "host/test", boot_id: "boot/test", offer_generation: 1,
    capability_id: "capability/model/generate", implementation_id: "std/local-model@1",
    artifact_id: "model/sha256-fixture", member_capacity: 1,
    resources: [{
      pool_id: "std/local-model-inference-slots", class_id: "ai/local-model-inference-slot",
      units: 1,
    }],
  };
  const observation = await join.line.observeLocalModelPool(realization);
  assert.equal(observation.health, "Ready");
  assert.equal(observation.host_id, realization.host_id);
  assert.equal(observation.resources[0].unreserved_units, 1);
  await assert.rejects(() => join.line.observeLocalModelPool({
    ...realization, boot_id: "boot/stale",
  }), { code: "PoolRealization" });
  await assert.rejects(() => join.line.prepareRemote({ plan_id: "plan/too-early" }), { code: "BodyContextAbsent" });
  const context = {
    schema: "conduit.body/conversation-context-value@2",
    display_name: "Retained Test Body",
    body_id: "body/retained",
    wake_id: "wake/retained/1",
    wake_sequence: 1,
    basis: { body_id: "body/retained", wake_id: "wake/retained/1", wake_sequence: 1, revision: 4 },
    hosts: [{ host_id: "host/test", present: true }],
    active_forms: ["source/live-conversation"],
    current_plan_id: "plan/retained",
    active_play_id: null,
    lines: [],
    recent_sign_ids: [],
  };
  const installed = await join.line.installBodyContext(context);
  assert.equal(installed.body_id, context.body_id);
  assert.equal(installed.basis_revision, context.basis.revision);
  await assert.rejects(() => join.line.preparePoolMember({
    plan: { plan_id: "plan/retained" },
    selection: { plan_id: "plan/retained", disposition: "CapacityRefused" },
    consumerPlacementId: "placement/client",
  }), { code: "PoolMemberSelection" });
  const poolPrepared = await join.line.preparePoolMember({
    plan: { plan_id: "plan/pool" },
    selection: {
      plan_id: "plan/pool", pool_id: "pool/workers", operation_id: "request/1",
      selected_realization: 0, observation_sign_ids: ["sign/provider/1"],
      disposition: "Selected", sign_id: "sign/selection/1",
    },
    consumerPlacementId: "placement/client",
  });
  assert.equal(poolPrepared.identity.plan_id, "plan/pool");
  assert.equal(FakeWebSocket.last.sent.at(-1).kind, "prepare-pool-member");
  await join.line.releaseRemote();
  const remote = await join.line.prepareRemote({ plan_id: "plan/retained" });
  assert.equal(remote.identity.host_id, "host/test");
  assert.equal(remote.identity.boot_id, "boot/test");
  assert.equal(remote.identity.plan_id, "plan/retained");
  assert.deepEqual(remote.hello_frames, [[0x43, 0x4e, 0x44, 0x53, 4]]);
  await assert.rejects(() => join.line.prepareRemote({ plan_id: "plan/overlap" }), { code: "RemotePlayActive" });
  await join.line.sendSessionFrame(new Uint8Array([0x43, 0x4e, 0x44, 0x53, 4]));
  assert.deepEqual(
    [...await join.line.receiveSessionFrame()],
    [0x43, 0x4e, 0x44, 0x53, 4, 2],
  );
  await join.line.releaseRemote();
  assert.equal(FakeWebSocket.last.sent.at(-1).kind, "release-remote");
  const next = await join.line.prepareRemote({ plan_id: "plan/retained-next" });
  assert.equal(next.identity.plan_id, "plan/retained-next");
  await join.line.releaseRemote();
  await assert.rejects(() => join.line.releaseRemote(), { code: "RemotePlayAbsent" });
  await join.line.close();
  assert.equal(FakeWebSocket.last.sent.at(-1).kind, "close");
  assert.equal(FakeWebSocket.last.readyState, 3);
});

test("the same invitation exchange runs over a Web Serial stream", async () => {
  const port = new FakeSerialPort();
  const session = await connectRendezvousHost(`C1-SERIAL-${"AB".repeat(32)}`, {
    serial: { requestPort: async () => port },
  });
  assert.equal(session.line_id, "conduit-line/serial-text@1");
  const prepared = {
    spore_id: "spore/serial", image_id: "image/serial", invitation_id: "invitation/serial",
    body_id: "body/serial", invitation_nonce: new Array(32).fill(19),
    invitation_secret: new Array(32).fill(29), invitation_expires_at_millis: Date.now() + 60_000,
  };
  const join = await session.invite(prepared);
  assert.equal(join.spore_id, "spore/serial");
  assert.equal(join.signature.length, 64);
  assert.equal(port.closed, true);
});

class FakeWebSocket extends EventTarget {
  constructor(url) {
    super();
    this.url = url;
    this.readyState = 0;
    this.sent = [];
    FakeWebSocket.last = this;
    queueMicrotask(() => {
      this.readyState = 1;
      this.dispatchEvent(new Event("open"));
    });
  }

  send(bytes) {
    if (new Uint8Array(bytes)[0] === 0x43) {
      this.sent.push(new Uint8Array(bytes));
      const data = new Uint8Array([0x43, 0x4e, 0x44, 0x53, 4, 2]).buffer;
      queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data })));
      return;
    }
    const request = JSON.parse(new TextDecoder().decode(bytes));
    this.sent.push(request);
    if (request.kind === "close") return;
    const advertisement = { host_id: "host/test", boot_id: "boot/test", offer_generation: 1 };
    const response = request.kind === "hello" ? {
      kind: "host", protocol: 1, friendly_label: "This running computer",
      target_id: "std/x86_64/computer", image_content_digest: `sha256:${"1".repeat(64)}`,
      advertisement, lines: ["conduit-line/loopback-websocket@1"],
    } : request.kind === "admitted" ? {
      kind: "admission-retained", protocol: 1,
      body_id: request.credential.body_id, part_id: request.credential.part_id,
    } : request.kind === "body-context" ? {
      kind: "body-context-installed", protocol: 1,
      body_id: request.context.body_id, basis_revision: request.context.basis.revision,
    } : request.kind === "observe-local-model-pool" ? {
      kind: "local-model-pool-observed", protocol: 1,
      observation: {
        host_id: request.realization.host_id, boot_id: request.realization.boot_id,
        offer_generation: request.realization.offer_generation,
        capability_id: request.realization.capability_id,
        implementation_id: request.realization.implementation_id,
        artifact_id: request.realization.artifact_id,
        health: "Ready", sign_id: "sign/provider/current",
        resources: request.realization.resources.map((binding, index) => ({
          host_id: request.realization.host_id, boot_id: request.realization.boot_id,
          offer_generation: request.realization.offer_generation,
          pool_id: binding.pool_id, class_id: binding.class_id, health: "Ready",
          unreserved_units: 1, utilized_units: 0, sign_id: `sign/resource/${index}`,
        })),
      },
    } : request.kind === "prepare-remote" || request.kind === "prepare-pool-member" ? {
      kind: "remote-prepared", protocol: 1,
      identity: {
        host_id: advertisement.host_id, boot_id: advertisement.boot_id,
        plan_id: request.plan.plan_id, active_play_id: "play/retained", play_sequence: 1,
      },
      hello_frames: [[0x43, 0x4e, 0x44, 0x53, 4]],
    } : request.kind === "release-remote" ? {
      kind: "remote-released", protocol: 1,
    } : {
      kind: "join", protocol: 1, spore_id: request.spore_id, image_id: request.image_id,
      advertisement, invitation_id: request.claim.invitation_id, body_id: request.claim.body_id,
      host_id: advertisement.host_id, boot_id: advertisement.boot_id, nonce: request.claim.nonce,
      signature: new Array(64).fill(5), observed_at_millis: Date.now(),
    };
    const data = new TextEncoder().encode(JSON.stringify(response)).buffer;
    queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data })));
  }

  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event("close"));
  }
}

class FakeSerialPort {
  constructor() {
    this.closed = false;
    this.readable = new ReadableStream({ start: (controller) => { this.controller = controller; } });
    this.writable = new WritableStream({ write: (bytes) => this.respond(bytes) });
  }
  async open(options) { assert.equal(options.baudRate, 115200); }
  async close() { this.closed = true; }
  respond(bytes) {
    const request = JSON.parse(new TextDecoder().decode(bytes).trim());
    if (request.kind === "close") return;
    const advertisement = { host_id: "host/serial", boot_id: "boot/serial", offer_generation: 1 };
    const response = request.kind === "hello" ? {
      kind: "host", protocol: 1, friendly_label: "This running computer",
      target_id: "std/x86_64/computer", image_content_digest: `sha256:${"2".repeat(64)}`,
      advertisement, lines: ["conduit-line/serial-text@1"],
    } : {
      kind: "join", protocol: 1, spore_id: request.spore_id, image_id: request.image_id,
      advertisement, invitation_id: request.claim.invitation_id, body_id: request.claim.body_id,
      host_id: advertisement.host_id, boot_id: advertisement.boot_id, nonce: request.claim.nonce,
      signature: new Array(64).fill(7), observed_at_millis: Date.now(),
    };
    this.controller.enqueue(new TextEncoder().encode(`${JSON.stringify(response)}\n`));
  }
}
