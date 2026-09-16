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
  const remote = await join.line.prepareRemote({ plan_id: "plan/retained" });
  assert.equal(remote.identity.host_id, "host/test");
  assert.equal(remote.identity.boot_id, "boot/test");
  assert.equal(remote.identity.plan_id, "plan/retained");
  assert.deepEqual(remote.hello_frames, [[1, 2, 3, 4]]);
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
    const request = JSON.parse(new TextDecoder().decode(bytes));
    this.sent.push(request);
    if (request.kind === "close") return;
    const advertisement = { host_id: "host/test", boot_id: "boot/test", offer_generation: 1 };
    const response = request.kind === "hello" ? {
      kind: "host", protocol: 1, friendly_label: "This running computer",
      target_id: "std/x86_64/computer", image_content_digest: `sha256:${"1".repeat(64)}`,
      advertisement, lines: ["conduit-line/loopback-websocket@1"],
    } : request.kind === "prepare-remote" ? {
      kind: "remote-prepared", protocol: 1,
      identity: {
        host_id: advertisement.host_id, boot_id: advertisement.boot_id,
        plan_id: request.plan.plan_id, active_play_id: "play/retained", play_sequence: 1,
      },
      hello_frames: [[1, 2, 3, 4]],
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
