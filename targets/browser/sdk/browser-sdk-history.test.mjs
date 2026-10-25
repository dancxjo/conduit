import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';

// Exercise the actual production participation methods with explicit CPU
// carrier/bridge fixtures. Only constructor access is added to an in-memory
// module; no fixture factory or bypass is exported by the shipped SDK.
const sdkUrl = new URL('./browser-sdk.mjs',import.meta.url);
const source = (await readFile(sdkUrl,'utf8')).replace(/from (["'])(\.{1,2}\/[^"']+)\1/g,
  (_,quote,path) => `from ${quote}${new URL(path,sdkUrl).href}${quote}`);
const {fixtureParticipation} = await import(`data:text/javascript;base64,${Buffer.from(source +
  '\nexport const fixtureParticipation = state => new BrowserBodyParticipation(BROWSER_PARTICIPATION_KEY,state);').toString('base64')}`);

function fixture() {
  const credential = {body_id:'body/cpu',host_id:'host/cpu',boot_id:'boot/cpu',credential_id:'credential/cpu',part_id:'part/cpu'};
  const canonical = Uint8Array.of(1,2,3,4); // opaque canonical-byte fixture, no Rust decoding claim
  const received = [];
  let failPreparation = true;
  const bridge = {
    ownerHistoryLookIntent(intent,prior) {
      assert.deepEqual(intent,{unit:'kelvin',density:'compact',palette:'editorial'});
      assert.equal(prior,undefined);
      return canonical;
    },
    ownerFaceShowReceipt:() => new TextEncoder().encode('{"fixture":"acknowledged Show"}'),
    ownerFaceClear() {},
    ownerFacePrepare(basis,frame,{historyLookBytes}) {
      received.push(historyLookBytes?.slice());
      // A bridge cannot mutate retained SDK intent through its input argument.
      historyLookBytes?.fill(99);
      if (failPreparation) throw new Error('CPU display preparation failed');
      return {schema:'conduit.browser/owner-face-mask@2',body_id:credential.body_id,
        face_id:'face/cpu',face_revision:1,show_id:'show/retry',show_state:'prepared',interactions_admitted:false};
    },
  };
  const membership = {
    hostId:credential.host_id,bootId:credential.boot_id,
    membershipCredential:() => credential,
    presenceState:() => 'available',
    async selectHistoryMask({lookBytes}) {assert.deepEqual([...lookBytes],[1,2,3,4]);lookBytes.fill(77);},
    async requestFaceSnapshot() {return new Uint8Array([123,125]);},
    async selectComponentMask() {},
    close() {return 'closed';},
  };
  const state = {host:{bridge},membership,lastOwnerShow:{show_id:'show/initial'}};
  return {participation:fixtureParticipation(state),state,canonical,received,credential,retry:()=>{failPreparation=false;}};
}

test('accepted history selection survives failed preparation and Show discard before ordinary retry',async () => {
  const {participation,state,canonical,received,retry} = fixture();
  await assert.rejects(participation.selectOwnerHistoryMask({unit:'kelvin',density:'compact',palette:'editorial'}),/CPU display preparation failed/);
  canonical.fill(88);
  assert.equal(state.lastOwnerShow,null);
  participation.discardOwnerFaceMask();
  retry();
  const prepared = await participation.prepareOwnerFaceMask();
  assert.equal(prepared.show_id,'show/retry');
  assert.deepEqual(received.map(bytes=>[...bytes]),[[1,2,3,4],[1,2,3,4]]);
  assert.deepEqual([...state.ownerHistoryLook.bytes],[1,2,3,4]);
});

for (const field of ['body_id','host_id','boot_id','credential_id','part_id']) {
  test(`actual participation clears independent intent on ${field} replacement`,async () => {
    const {participation,state,credential} = fixture();
    await assert.rejects(participation.selectOwnerHistoryMask({unit:'kelvin',density:'compact',palette:'editorial'}));
    if (['body_id','credential_id','part_id'].includes(field)) credential[field] = 'replacement';
    else state.membership[field === 'host_id' ? 'hostId' : 'bootId'] = 'replacement';
    participation.membershipCredential();
    assert.equal(state.ownerHistoryLook,null);
  });
}

test('actual participation close retires retained history intent',async () => {
  const {participation,state} = fixture();
  await assert.rejects(participation.selectOwnerHistoryMask({unit:'kelvin',density:'compact',palette:'editorial'}));
  assert.ok(state.ownerHistoryLook);
  assert.equal(participation.close(),'closed');
  assert.equal(state.ownerHistoryLook,null);
});

test('successful explicit nonhistory selection clears accepted history intent',async () => {
  const {participation,state,received,retry} = fixture();
  await assert.rejects(participation.selectOwnerHistoryMask({unit:'kelvin',density:'compact',palette:'editorial'}));
  retry(); state.lastOwnerShow = {show_id:'fixture/current-acknowledgement'};
  await participation.selectOwnerComponentMask({look:'compact',recipe:'flat'});
  assert.equal(state.ownerHistoryLook,null);
  assert.equal(received.at(-1),undefined);
});

for (const field of ['credential_id','part_id']) {
  test(`pending accepted history selection cannot retain intent after ${field} replacement`,async () => {
    const {participation,state,credential,received} = fixture();
    let accept;
    state.membership.selectHistoryMask = () => new Promise(resolve => {accept=resolve;});
    const selected = participation.selectOwnerHistoryMask({unit:'kelvin',density:'compact',palette:'editorial'});
    credential[field] = 'replacement';
    const refused = assert.rejects(selected,/no longer current/);
    accept(); await refused;
    assert.equal(state.ownerHistoryLook,undefined);
    assert.deepEqual(received,[]);
  });
}
