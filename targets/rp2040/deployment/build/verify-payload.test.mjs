import assert from 'node:assert/strict';
import test from 'node:test';
import {verifyPayload} from './verify-payload.mjs';
import {RP2040_UF2} from '../browser/uf2.mjs';
function image(address=RP2040_UF2.flashStart) {
  const bytes=new Uint8Array(512),view=new DataView(bytes.buffer);
  for(const [offset,value] of [[0,0x0a324655],[4,0x9e5d5157],[8,0x2000],[12,address],[16,256],[20,0],[24,1],[28,RP2040_UF2.familyId],[508,0x0ab16f30]])view.setUint32(offset,value,true);
  return bytes;
}
test('a standalone payload preserves the shared credentials and bootstrap regions',()=>{
  assert.equal(verifyPayload(image()).reserved_regions_preserved,true);
  assert.throws(()=>verifyPayload(image(RP2040_UF2.credentialsAddress)),/credential/);
  assert.throws(()=>verifyPayload(image(RP2040_UF2.sporeSectorAddress)),/bootstrap/);
});
test('foreign family and malformed sequence cannot be sealed as an RP2040 payload',()=>{
  const foreign=image();new DataView(foreign.buffer).setUint32(28,0,true);
  assert.throws(()=>verifyPayload(foreign),/RP2040 family/);
  const sequence=image();new DataView(sequence.buffer).setUint32(24,2,true);
  assert.throws(()=>verifyPayload(sequence),/sequence/);
});
