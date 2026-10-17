import assert from 'node:assert/strict';
import test from 'node:test';
import { validateOwnerFaceAcknowledgement } from './browser-sdk.mjs';
const prepared = () => ({body_id:'body/1',show_id:'show/1',face_id:'face/1',face_revision:7,
  show_state:'prepared',interactions_admitted:false});
test('acknowledgement admits the WASM-owned permission transition on the same Show',()=>{
  const view=prepared();
  for(const admitted of [false,true]) {
    const shown={...view,show_state:'available',interactions_admitted:admitted};
    assert.deepEqual(validateOwnerFaceAcknowledgement(view,shown),shown);
  }
  assert.equal(view.interactions_admitted,false);
});
test('another Body, Show, Face or revision cannot replace acknowledged identity',()=>{
  const view=prepared();const shown={...view,show_state:'available',interactions_admitted:true};
  for(const [key,value] of [['body_id','body/other'],['show_id','show/other'],['face_id','face/other'],['face_revision',8]])
    assert.throws(()=>validateOwnerFaceAcknowledgement(view,{...shown,[key]:value}),/another owner Face Show/);
});
test('counterfeit preparation admission and invalid lifecycle never grant interactions',()=>{
  const view=prepared();const shown={...view,show_state:'available',interactions_admitted:true};
  assert.throws(()=>validateOwnerFaceAcknowledgement({...view,interactions_admitted:true},shown));
  assert.throws(()=>validateOwnerFaceAcknowledgement(view,{...shown,show_state:'prepared'}));
  assert.throws(()=>validateOwnerFaceAcknowledgement(view,{...shown,interactions_admitted:'true'}));
  const available={...view,show_state:'available'};
  assert.throws(()=>validateOwnerFaceAcknowledgement(available,{...available,interactions_admitted:true}));
});

test('validated acknowledgement retains identity and nested contracts despite caller mutation', () => {
  const view = prepared();
  const shown = { ...view, show_state: 'available', interactions_admitted: true,
    actions: [{ identity: 'action/one', arguments: [{ name: 'value', choices: ['yes'] }] }] };
  const snapshot = validateOwnerFaceAcknowledgement(view, shown);
  shown.face_id = 'face/forged';
  shown.actions[0].arguments[0].choices.push('forged');
  assert.equal(snapshot.face_id, view.face_id);
  assert.deepEqual(snapshot.actions[0].arguments[0].choices, ['yes']);
});
