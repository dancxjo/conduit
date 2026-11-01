import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash, webcrypto } from 'node:crypto';
import { createBoundedDocumentChannel, MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES } from './browser-bounded-document.mjs';
if (!globalThis.crypto) globalThis.crypto = webcrypto;
const hash = bytes => new Uint8Array(createHash('sha256').update(bytes).digest());
function carrier(frames = [], maximumFrameBytes = 65_519, documentTimeoutMillis = 1000) {
  return { frames: [...frames], sent: [], closed: 0, maximumFrameBytes, documentTimeoutMillis,
    async sendSessionFrame(bytes) { this.sent.push(bytes.slice()); },
    async receiveSessionFrame() { if (!this.frames.length) throw Error('fixture exhausted'); return this.frames.shift(); },
    close() { this.closed++; } };
}
async function chunks(bytes, maximumFrameBytes = 65_519) {
  const c = carrier([], maximumFrameBytes);
  await createBoundedDocumentChannel(c).send(bytes);
  return c.sent;
}
test('701632 byte complete document survives eleven unchanged protected frame bounds', async () => {
  const bytes = Uint8Array.from({ length: 701632 }, (_, i) => i % 251);
  const frames = await chunks(bytes);
  assert.equal(frames.length, 11);
  assert(frames.every(frame => frame.length <= 65_519));
  const c = carrier(frames), channel = createBoundedDocumentChannel(c);
  assert.deepEqual(await channel.receive(), bytes);
  assert.equal(c.closed, 0);
});
test('CBD1 layout matches Rust little endian total offset digest and payload', async () => {
  const bytes = new Uint8Array(61).fill(0x5a), frames = await chunks(bytes, 60);
  assert.equal(frames.length, 4);
  assert.deepEqual([...frames[0].slice(0, 12)], [67,66,68,49,61,0,0,0,0,0,0,0]);
  assert.deepEqual(frames[0].slice(12, 44), hash(bytes));
  assert.deepEqual(frames[0].slice(44), bytes.slice(0,16));
  assert.equal(new DataView(frames[1].buffer).getUint32(8,true), 16);
  assert.deepEqual(await createBoundedDocumentChannel(carrier(frames,60)).receive(),bytes);
});
for (const [name, mutate] of [
  ['reordered', frames => [frames[1],frames[0],...frames.slice(2)]],
  ['mixed raw control', frames => [frames[0],new TextEncoder().encode('{"kind":"control"}')]],
  ['changed digest identity', frames => { frames[1][12] ^= 1; return frames; }],
  ['changed payload with unchanged digest', frames => { frames[1][44] ^= 1; return frames; }],
  ['duplicate', frames => [frames[0],frames[0],...frames.slice(1)]],
  ['declared overlimit', frames => { new DataView(frames[0].buffer).setUint32(4,MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES+1,true);return frames;}],
  ['truncated', frames => frames.slice(0,-1)],
]) {
  test(`document refuses ${name} without publishing`, async () => {
    const frames = mutate(await chunks(new Uint8Array(120000).fill(9)));
    const c = carrier(frames), channel = createBoundedDocumentChannel(c);
    await assert.rejects(channel.receive()); assert.equal(c.closed,1);
    await assert.rejects(channel.receive(),/closed/);
  });
}
test('exact existing 2MiB document limit and small admitted carrier remain finite', async () => {
  const bytes = new Uint8Array(MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES);
  assert.equal((await chunks(bytes)).length,33);
  const c = carrier(), channel = createBoundedDocumentChannel(c);
  assert.throws(()=>channel.validate(new Uint8Array(MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES+1)),/bound/);
  const tiny = carrier([],44);assert.throws(()=>createBoundedDocumentChannel(tiny).validate(new Uint8Array(45)),/bound/);
  const smaller = await chunks(new Uint8Array(2048),512);
  assert.equal(smaller.length,5);assert(smaller.every(frame=>frame.length<=512));
});
test('raw JSON and CNDS remain byte identical on the same authenticated carrier', async () => {
  for (const bytes of [new TextEncoder().encode('{"kind":"advertise"}'), new TextEncoder().encode('CNDS-data')]) {
    const c = carrier([bytes]), channel=createBoundedDocumentChannel(c);
    await channel.send(bytes);assert.deepEqual(c.sent,[bytes]);assert.deepEqual(await channel.receive(),bytes);
  }
});
test('one absolute deadline spans multiple chunks and rejects a late completion', async () => {
  const frames=await chunks(new Uint8Array(150000).fill(7));
  const c=carrier(frames,65_519,25);let count=0;
  c.receiveSessionFrame=async()=>{const frame=c.frames.shift();if(count++>0)await new Promise(resolve=>setTimeout(resolve,18));return frame;};
  await assert.rejects(createBoundedDocumentChannel(c).receive(),/deadline/);assert.equal(c.closed,1);
});
test('cancelled held final chunk cannot publish and retained send scratch is cleared', async () => {
  const frames=await chunks(new Uint8Array(120000).fill(8));
  const c=carrier(frames);let release;
  c.receiveSessionFrame=async()=>{const frame=c.frames.shift();if(!c.frames.length)await new Promise(resolve=>release=resolve);return frame;};
  const channel=createBoundedDocumentChannel(c), pending=channel.receive();
  while(!release) await new Promise(resolve=>setImmediate(resolve));
  channel.close();release();await assert.rejects(pending,/closed/);assert.equal(c.closed,1);
  const s=carrier();let unblock,held;
  s.sendSessionFrame=bytes=>{held=bytes;return new Promise(resolve=>unblock=resolve);};
  const sending=createBoundedDocumentChannel(s), sent=sending.send(new Uint8Array(120000).fill(4));
  while(!unblock)await new Promise(resolve=>setImmediate(resolve));
  sending.close();assert(held.every(byte=>byte===0));unblock();await assert.rejects(sent,/closed/);assert.equal(s.closed,1);
});
