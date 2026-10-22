import test from 'node:test';
import assert from 'node:assert/strict';
import { runInNewContext } from 'node:vm';
import { showBodyFace } from './browser-sdk-body-face.mjs';

const limit = 2 * 1024 * 1024;
const prepared = new Error('Reached checked bridge');
async function consume(response, inspect = () => {}) {
  return showBodyFace({ coordinator: 'https://owner.test', root: { isConnected: true },
    play: { state: 'playing' }, current: () => true, transport: async () => response,
    bridge: { bodyFacePrepare(bytes) { inspect(bytes); throw prepared; } } });
}
function overflowing(status) {
  let reads = 0, cancelled = false;
  const chunks = [new Uint8Array(limit - 1), new Uint8Array(2), new Uint8Array(1)];
  const response = new Response(new ReadableStream({
    pull(controller) { if (reads < chunks.length) controller.enqueue(chunks[reads++]); else controller.close(); },
    cancel() { cancelled = true; },
  }, { highWaterMark: 0 }), { status, headers: { 'Content-Length': '1' } });
  return { response, state: () => ({ reads, cancelled }) };
}
for (const status of [200, 503]) {
  test(`overflowing ${status === 200 ? 'success' : 'error'} response is cancelled before reading its remaining bytes`, async () => {
    const stream = overflowing(status);
    await assert.rejects(consume(stream.response), /response exceeds its bound/);
    assert.deepEqual(stream.state(), { reads: 2, cancelled: true });
  });
}
test('an exact-limit response reaches the checked bridge without bulk readers', async () => {
  const response = new Response(new Uint8Array(limit).fill(65));
  response.arrayBuffer = response.text = () => { throw new Error('Unbounded bulk reader used'); };
  await assert.rejects(consume(response, bytes => {
    assert.equal(bytes.length, limit); assert.equal(bytes[0], 65); assert.equal(bytes.at(-1), 65);
  }), error => error === prepared);
});
test('a chunked response preserves exact bytes across UTF-8 boundaries', async () => {
  const bytes = new TextEncoder().encode('café λ');
  const response = new Response(new ReadableStream({ start(controller) {
    controller.enqueue(new Uint8Array(0)); controller.enqueue(bytes.slice(0, 4)); controller.enqueue(bytes.slice(4)); controller.close();
  } }));
  await assert.rejects(consume(response, result => assert.deepEqual(result, bytes)), error => error === prepared);
});
test('an empty success cannot reach the checked bridge', async () => {
  await assert.rejects(consume(new Response(null)), /response exceeds its bound/);
});
test('a bounded HTTP refusal retains its status and readable reason', async () => {
  await assert.rejects(consume(new Response('Not current', { status: 409 })), /Body owner refused \(409\): Not current/);
});

test('valid byte chunks from another realm retain their exact payload', async () => {
  const bytes = runInNewContext('new Uint8Array([195, 169])');
  const response = new Response(new ReadableStream({ start(controller) {
    controller.enqueue(bytes); controller.close();
  } }));
  await assert.rejects(consume(response, result => assert.deepEqual([...result], [195, 169])), error => error === prepared);
});
test('an empty HTTP refusal retains its status', async () => {
  await assert.rejects(consume(new Response(null, { status: 409 })), /Body owner refused \(409\)/);
});
