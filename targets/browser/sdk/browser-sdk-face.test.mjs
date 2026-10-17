import assert from "node:assert/strict";
import test from "node:test";
import { BrowserFaceClient, BrowserFaceError } from "./browser-sdk-face.mjs";

const response = (value, status = 200) => new Response(JSON.stringify(value), { status });

test("Face snapshot uses one bounded same-origin SDK request", async () => {
  const calls = [];
  let receiver;
  const client = new BrowserFaceClient({
    base: "https://conduit.test/workbench/",
    fetch: async function (url, options) { receiver = this; calls.push([url, options]); return response({ schema: "face" }); },
  });
  assert.deepEqual(await client.snapshot(), { schema: "face" });
  assert.equal(receiver, globalThis);
  assert.equal(calls[0][0].href, "https://conduit.test/workbench/api/snapshot");
  assert.deepEqual(calls[0][1], {
    method: "GET", cache: "no-store", credentials: "same-origin", redirect: "error",
  });
});

test("Face interaction carries exact caller-owned semantic basis", async () => {
  let request;
  const client = new BrowserFaceClient({
    base: "https://conduit.test/",
    fetch: async (_url, options) => { request = options; return response({ revision: 2 }); },
  });
  const interaction = { face_id: "face/1", face_revision: 1, kind: "select", subject: "gear/a" };
  assert.deepEqual(await client.interact(interaction), { revision: 2 });
  assert.equal(request.method, "POST");
  assert.deepEqual(JSON.parse(request.body), interaction);
});

test("Face client refuses failed, empty, and oversized responses", async () => {
  const base = "https://conduit.test/";
  await assert.rejects(
    new BrowserFaceClient({ base, fetch: async () => response({}, 409) }).snapshot(),
    error => error instanceof BrowserFaceError && error.code === "FaceHttpRefusal" && error.status === 409,
  );
  await assert.rejects(
    new BrowserFaceClient({ base, fetch: async () => new Response(new Uint8Array()) }).snapshot(),
    error => error instanceof BrowserFaceError && error.code === "FaceResponseBound",
  );
  await assert.rejects(
    new BrowserFaceClient({ base, maximumResponseBytes: 4, fetch: async () => response({ too: "large" }) }).snapshot(),
    error => error instanceof BrowserFaceError && error.code === "FaceResponseBound",
  );
});

test("Face response overflow cancels before consuming the rest of the stream", async () => {
  let cancelled = false;
  let pulls = 0;
  const stream = new ReadableStream({
    pull(controller) { pulls++; controller.enqueue(new Uint8Array(5)); },
    cancel() { cancelled = true; },
  }, { highWaterMark: 0 });
  const client = new BrowserFaceClient({ base: "https://conduit.test/", maximumResponseBytes: 4,
    fetch: async () => new Response(stream, { headers: { "content-length": "1" } }) });
  await assert.rejects(client.snapshot(), error => error.code === "FaceResponseBound");
  assert.equal(cancelled, true);
  assert.equal(pulls, 1);
  assert.equal(stream.locked, false);
});

test("Face response streams preserve exact-boundary UTF-8 across chunks", async () => {
  const bytes = new TextEncoder().encode('"é😀"');
  const stream = new ReadableStream({ start(controller) {
    for (const byte of bytes) controller.enqueue(Uint8Array.of(byte));
    controller.close();
  } });
  const client = new BrowserFaceClient({ base: "https://conduit.test/", maximumResponseBytes: bytes.length,
    fetch: async () => new Response(stream) });
  assert.equal(await client.snapshot(), "é😀");
  assert.equal(stream.locked, false);
});

test("Face interrupted response retains the operation and releases its reader", async () => {
  const cause = new Error("connection lost");
  const stream = new ReadableStream({ pull(controller) { controller.error(cause); } });
  const client = new BrowserFaceClient({ base: "https://conduit.test/", fetch: async () => new Response(stream) });
  await assert.rejects(client.snapshot(), error => error instanceof BrowserFaceError
    && error.code === "FaceUnavailable" && error.operation === "BrowserFace.snapshot" && error.cause === cause);
  assert.equal(stream.locked, false);
});
