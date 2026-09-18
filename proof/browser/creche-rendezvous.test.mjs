import test from "node:test";
import assert from "node:assert/strict";
import { connectRendezvousHost, decodeRendezvousCode } from "../../products/creche/browser/creche-rendezvous.mjs";

test("running Host rendezvous code resolves one authenticated loopback WebSocket Line", () => {
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
  const descriptor = JSON.stringify({
    schema: "conduit.host/rendezvous-descriptor@1",
    candidates: [{
      candidate_id: "candidate/secure-lan",
      line_family: "authenticated-tls-stream",
      reachability: "wss://conduit-host.test:7443/conduit",
      authentication: {
        server_identity: "conduit-host.test",
        transport_binding_sha256: new Array(32).fill(0xab),
      },
      expires_at_millis: Date.now() + 30_000,
      maximum_attempts: 1,
      attempt_timeout_millis: 10_000,
    }],
    session_secret: new Array(32).fill(0xcd),
  });
  const decoded = decodeRendezvousCode(descriptor);
  assert.equal(decoded.url, "wss://conduit-host.test:7443/conduit");
  assert.equal(decoded.line_id, "conduit-line/authenticated-tls-stream@1");
  assert.equal(decoded.maximum_attempts, 1);
  assert.equal(decoded.transport_binding_sha256, "ab".repeat(32));

  const stale = JSON.parse(descriptor);
  stale.candidates[0].expires_at_millis = Date.now() - 1;
  assert.throws(() => decodeRendezvousCode(JSON.stringify(stale)), { code: "InvalidDescriptor" });
  const relabelled = JSON.parse(descriptor);
  relabelled.candidates[0].reachability = "ws://conduit-host.test:7443/conduit";
  assert.throws(() => decodeRendezvousCode(JSON.stringify(relabelled)), { code: "InvalidDescriptor" });
});

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
    } : request.kind === "prepare-remote" ? {
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
