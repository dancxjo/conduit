import assert from "node:assert/strict";
import test from "node:test";
import { BrowserFaceClient, BrowserFaceError } from "./browser-sdk-face.mjs";

const response = (value, status = 200) => new Response(JSON.stringify(value), { status });

test("Face snapshot uses one bounded same-origin SDK request", async () => {
  const calls = [];
  const client = new BrowserFaceClient({
    base: "https://conduit.test/workbench/",
    fetch: async (url, options) => { calls.push([url, options]); return response({ schema: "face" }); },
  });
  assert.deepEqual(await client.snapshot(), { schema: "face" });
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
  const interaction = { presentation_id: "face/1", presentation_revision: 1, kind: "select", subject: "gear/a" };
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
