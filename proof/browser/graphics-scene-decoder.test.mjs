import {readFileSync} from "node:fs";
import test from 'node:test';
import assert from 'node:assert/strict';
import {decodeGraphics} from './graphics-scene-decoder.mjs';
function command(kind, payload = [], {style=1,role=1,bounds=[0,0,24,24]} = {}) {
  const bytes = new Uint8Array(21 + payload.length), view = new DataView(bytes.buffer);
  bytes.set([kind,3,style]);
  for (const offset of [3,11]) bounds.forEach((value,index)=>view.setInt16(offset+index*2,value,true));
  bytes[19]=payload.length; bytes[20]=role; bytes.set(payload,21); return bytes;
}
const text = (value,role=1)=>command(2,new TextEncoder().encode(value),{role});
function scene(version,...commands) {return new Uint8Array([version,commands.length,...commands.flatMap(c=>[...c])]);}
function polyline(kind=5) {
  const data = new Uint8Array(12), view = new DataView(data.buffer);
  [1,2,9,13,12,3].forEach((v,i)=>view.setInt16(i*2,v,true));
  return command(kind,data,{style:2,bounds:[1,2,12,12]});
}
test('union reader retains binary geometry and Value text in one canonical v3 packet',()=>{
  const commands=decodeGraphics(scene(3,polyline(),text('19.0 °C',10),command(1,[0,2],{style:4})));
  assert.deepEqual(commands[0].points,[{x:1,y:2},{x:9,y:13},{x:12,y:3}]);
  assert.equal(commands[0].payload,'');
  assert.equal(commands[1].textRole,'value');
  assert.equal(commands[1].payload,'19.0 °C');
  assert.deepEqual(commands[2].rectMetrics,{radius:0,stroke:2});
});
test('packet version depends on explicit metrics, not semantic revision or Value/Polyline',()=>{
  assert.equal(decodeGraphics(scene(2,polyline(),text('42',10))).length,2);
  assert.throws(()=>decodeGraphics(scene(3,text('42',10))));
  assert.throws(()=>decodeGraphics(scene(2,command(1,[2,1],{style:3}))));
  assert.equal(decodeGraphics(scene(2)).length,0);
});
test('text-role tags agree with the authored Conduit form',()=>{
  assert.deepEqual(decodeGraphics(scene(2,...[1,2,3,4,10].map(role=>text('x',role)))).map(c=>c.textRole),['body','title','heading','label','value']);
});
test('malformed geometry, extents, roles, UTF-8 and scene bounds refuse',()=>{
  const invalid = [
    scene(2,polyline(4)), // Diagonal segments are not orthogonal.
    scene(3,command(1,[33,1],{style:4})),
    scene(3,command(1,[1,0],{style:4})),
    scene(3,command(1,[1,1],{style:1})),
    scene(2,command(1,[],{role:10})),
    scene(2,command(2,[255])),
    scene(2,command(3,new TextEncoder().encode('unknown-icon'))),
    scene(2,command(1,[],{bounds:[0,0,0,24]})),
    scene(2,command(1,[],{bounds:[32760,0,24,24]})),
    scene(2,...Array.from({length:49},()=>text('x'))),
    new Uint8Array([4,0]), new Uint8Array([2,0,0]),
  ];
  for (const bytes of invalid) assert.throws(()=>decodeGraphics(bytes));
  const valid=scene(2,text('x'));
  for(let size=0;size<valid.length;size++) assert.throws(()=>decodeGraphics(valid.slice(0,size)));
});

test('browser reads the exact union packet pinned by the shared Rust codec',()=>{
  const hex=readFileSync(new URL('../fixtures/graphics-scene-union.hex',import.meta.url),'utf8').trim();
  const packet=new Uint8Array(Buffer.from(hex,'hex'));
  const commands=decodeGraphics(packet);
  assert.deepEqual(commands[0].points,[{x:2,y:3},{x:5,y:9},{x:9,y:4}]);
  assert.equal(commands[1].textRole,'value');
  assert.equal(commands[1].payload,'19.0 °C');
  assert.deepEqual(commands[2].rectMetrics,{radius:0,stroke:2});
});
