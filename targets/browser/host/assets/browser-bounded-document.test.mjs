import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash, webcrypto } from 'node:crypto';
import { createBoundedDocumentChannel, MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES } from './browser-bounded-document.mjs';
if (!globalThis.crypto) globalThis.crypto = webcrypto;
const workspace = () => ({ maximumDocumentBytes: MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES,
  capacityBytes: 2 * MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES + 4 * 65_519 + 64 });
const selectedChannel = (carrier, selected = workspace()) => createBoundedDocumentChannel(carrier, selected);
const hash = bytes => new Uint8Array(createHash('sha256').update(bytes).digest());
function carrier(frames = [], maximumFrameBytes = 65_519, documentTimeoutMillis = 1000) {
  return { frames: [...frames], sent: [], closed: 0, maximumFrameBytes, documentTimeoutMillis,
    async sendSessionFrame(bytes) { this.sent.push(bytes.slice()); },
    async receiveSessionFrame() { if (!this.frames.length) throw Error('fixture exhausted'); return this.frames.shift(); },
    close() { this.closed++; } };
}
async function chunks(bytes, maximumFrameBytes = 65_519) {
  const c = carrier([], maximumFrameBytes);
  await selectedChannel(c).send(bytes);
  return c.sent;
}
test('701632 byte complete document survives eleven unchanged protected frame bounds', async () => {
  const bytes = Uint8Array.from({ length: 701632 }, (_, i) => i % 251);
  const frames = await chunks(bytes);
  assert.equal(frames.length, 11);
  assert(frames.every(frame => frame.length <= 65_519));
  const c = carrier(frames), channel = selectedChannel(c);
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
  assert.deepEqual(await selectedChannel(carrier(frames,60)).receive(),bytes);
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
    const c = carrier(frames), channel = selectedChannel(c);
    await assert.rejects(channel.receive()); assert.equal(c.closed,1);
    await assert.rejects(channel.receive(),/closed/);
  });
}
test('exact existing 2MiB document limit and small admitted carrier remain finite', async () => {
  const bytes = new Uint8Array(MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES);
  assert.equal((await chunks(bytes)).length,33);
  const c = carrier(), channel = selectedChannel(c);
  assert.throws(()=>channel.validate(new Uint8Array(MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES+1)),/bound/);
  const tiny = carrier([],44);assert.throws(()=>selectedChannel(tiny).validate(new Uint8Array(45)),/bound/);
  const smaller = await chunks(new Uint8Array(2048),512);
  assert.equal(smaller.length,5);assert(smaller.every(frame=>frame.length<=512));
});
test('raw JSON and CNDS remain byte identical on the same authenticated carrier', async () => {
  for (const bytes of [new TextEncoder().encode('{"kind":"advertise"}'), new TextEncoder().encode('CNDS-data')]) {
    const c = carrier([bytes]), channel=selectedChannel(c);
    await channel.send(bytes);assert.deepEqual(c.sent,[bytes]);assert.deepEqual(await channel.receive(),bytes);
  }
});
test('one absolute deadline spans multiple chunks and rejects a late completion', async () => {
  const frames=await chunks(new Uint8Array(150000).fill(7));
  const c=carrier(frames,65_519,25);let count=0;
  c.receiveSessionFrame=async()=>{const frame=c.frames.shift();if(count++>0)await new Promise(resolve=>setTimeout(resolve,18));return frame;};
  await assert.rejects(selectedChannel(c).receive(),/deadline/);assert.equal(c.closed,1);
});
test('cancelled held final chunk cannot publish and retained send scratch is cleared', async () => {
  const frames=await chunks(new Uint8Array(120000).fill(8));
  const c=carrier(frames);let release;
  c.receiveSessionFrame=async()=>{const frame=c.frames.shift();if(!c.frames.length)await new Promise(resolve=>release=resolve);return frame;};
  const channel=selectedChannel(c), pending=channel.receive();
  while(!release) await new Promise(resolve=>setImmediate(resolve));
  channel.close();release();await assert.rejects(pending,/closed/);assert.equal(c.closed,1);
  const s=carrier();let unblock,held;
  s.sendSessionFrame=bytes=>{held=bytes;return new Promise(resolve=>unblock=resolve);};
  const sending=selectedChannel(s), sent=sending.send(new Uint8Array(120000).fill(4));
  while(!unblock)await new Promise(resolve=>setImmediate(resolve));
  sending.close();assert(held.every(byte=>byte===0));unblock();await assert.rejects(sent,/closed/);assert.equal(s.closed,1);
});

test('large send and assembly require an independently selected workspace',async()=>{
 const bytes=new Uint8Array(120000).fill(7),frames=await chunks(bytes);
 const plain=carrier(),channel=createBoundedDocumentChannel(plain);
 const input=new Uint8Array(120000);input.slice=()=>{throw Error('allocated before workspace admission');};
 assert.throws(()=>channel.retain(input),/workspace/);
 await assert.rejects(channel.send(bytes),/workspace/);assert.equal(plain.sent.length,0);
 const receiving=carrier(frames);await assert.rejects(createBoundedDocumentChannel(receiving).receive(),/workspace/);
 const small=carrier(),limited=selectedChannel(small,{maximumDocumentBytes:MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES,capacityBytes:100});
 assert.throws(()=>limited.retain(input),/workspace/);assert.equal(small.sent.length,0);
});
test('declared assembly extent is refused before a large backing allocation',async()=>{
 const frames=await chunks(new Uint8Array(120000));
 const c=carrier(frames),channel=selectedChannel(c,{maximumDocumentBytes:MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES,capacityBytes:100});
 const Original=globalThis.Uint8Array;let largeAllocations=0;
 globalThis.Uint8Array=new Proxy(Original,{construct(target,args){if(typeof args[0]==='number'&&args[0]>100)largeAllocations++;return Reflect.construct(target,args);}});
 try{await assert.rejects(channel.receive(),/workspace/);assert.equal(largeAllocations,0);}
 finally{globalThis.Uint8Array=Original;}
});
test('selected workspace sizes are captured and queued snapshots share one finite budget',()=>{
 const descriptor={maximumDocumentBytes:MAXIMUM_MEMBERSHIP_DOCUMENT_BYTES,capacityBytes:100};
 const c=carrier(),channel=selectedChannel(c,descriptor);descriptor.capacityBytes=1000000;
 const retained=channel.retain(new Uint8Array(80).fill(4));
 assert.throws(()=>channel.retain(new Uint8Array(21)),/workspace/);
 retained.release();const next=channel.retain(new Uint8Array(100).fill(5));
 channel.close();assert(next.bytes.every(byte=>byte===0));
});
test('initial receive starts the same absolute deadline before the first frame',async()=>{
 const c=carrier([],65_519,20);let release;c.receiveSessionFrame=()=>new Promise(resolve=>release=resolve);
 const channel=selectedChannel(c),pending=channel.receive();
 await assert.rejects(pending,/deadline/);assert.equal(c.closed,1);
 release(new TextEncoder().encode('{}'));await new Promise(resolve=>setImmediate(resolve));
});
