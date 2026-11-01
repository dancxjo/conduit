import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { createHash } from "node:crypto";
import { installWasmResponseCustody } from "./wasm-response-custody.mjs";

function harness() {
  let before, after;
  installWasmResponseCustody({ beforeEach: hook => { before = hook; }, afterEach: hook => { after = hook; } });
  return { before, after };
}
function response(bytes, length = String(bytes.length)) {
  let reads = 0;
  return { url: () => "http://localhost/assets/runtime.wasm", status: () => 200,
    headers: () => ({ "content-type": "application/wasm", "content-length": length }),
    body: async () => { reads++; return bytes; }, reads: () => reads };
}
async function finish(after, page) {
  let artifact;
  await after({ page }, { attach: async (_, value) => { artifact = JSON.parse(value.body); } });
  return artifact;
}

test("response custody reads the observed body and never carries it into another case", async () => {
  const hooks = harness(), page = new EventEmitter();
  const bytes = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]);
  await hooks.before({ page });
  const served = response(bytes);
  page.emit("response", served);
  const first = await finish(hooks.after, page);
  assert.equal(first.records.length, 1);
  assert.equal(first.records[0].sha256, createHash("sha256").update(bytes).digest("hex"));
  assert.equal(served.reads(), 1);
  assert.equal(page.listenerCount("response"), 0);
  await hooks.before({ page });
  const second = await finish(hooks.after, page);
  assert.deepEqual(second.records, []);
});

test("overbound headers refuse before reading and excess responses remain explicitly omitted", async () => {
  const hooks = harness(), page = new EventEmitter();
  await hooks.before({ page });
  const refused = response(Buffer.from([1]), String(20 * 1024 * 1024 + 1));
  page.emit("response", refused);
  for (let i = 0; i < 5; i++) page.emit("response", response(Buffer.from([i])));
  const artifact = await finish(hooks.after, page);
  assert.equal(refused.reads(), 0);
  assert.match(artifact.records[0].refusal, /overbound/);
  assert.equal(artifact.records.length, 4);
  assert.equal(artifact.omitted, 2);
});

test("a changed body length cannot acquire a custody digest", async () => {
  const hooks = harness(), page = new EventEmitter();
  await hooks.before({ page });
  page.emit("response", response(Buffer.from([1, 2]), "1"));
  const artifact = await finish(hooks.after, page);
  assert.match(artifact.records[0].refusal, /declared length/);
  assert.equal(artifact.records[0].sha256, undefined);
});
