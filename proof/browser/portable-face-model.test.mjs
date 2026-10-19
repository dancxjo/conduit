// Renderer reconstruction over a real source-derived specimen; no DOM/owner proof.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { portableFaceModel } from '../../targets/browser/host/assets/portable-face-model.mjs';
const source = JSON.parse(await readFile(new URL('../conformance/face-waist/domain-derived/prebirth-face.json', import.meta.url)));
function fixture() {
  const face = structuredClone(source);
  return { face,
    view: { face_id: face.identity, face_revision: String(face.revision), show_id: 'model-only-show', mask_plan_id: 'model-only-plan' },
    control_bindings: { face_identity: face.identity, face_revision: String(face.revision),
      text_entries: face.subjects.filter(subject => subject.role === 'TextEntry').map(subject => subject.identity),
      collections: face.subjects.filter(subject => subject.role === 'Collection').map(subject => ({ subject: subject.identity,
        mode: face.properties.find(property => property.subject === subject.identity && property.name === 'choice-multiplicity')?.value.Text ?? 'information' })),
    },
  };
}
test('actual Crèche reconstructs distinct field and collection meanings without dispatch', () => {
  const encounter = fixture(), before = JSON.stringify(encounter);
  const model = portableFaceModel(encounter);
  assert.equal(model.fields.length, 3);
  assert.deepEqual(model.collections.map(value => value.mode).sort(), ['exclusive', 'independent', 'information', 'information']);
  const fixed = model.fields.find(field => field.policy === 'fixed');
  assert.equal(fixed.action.identity, 'creche.naming');
  assert.equal(fixed.options[0].name, 'Surprise me');
  assert.equal(fixed.options[0].text, 'surprise');
  const free = model.fields.find(field => field.policy === 'free');
  assert.equal(free.action.identity, 'creche.name');
  assert.equal(free.options.length, 1);
  assert.equal(JSON.stringify(encounter), before);
});
test('foreign Face or prepared references refuse before browser control preparation', () => {
  const foreign = fixture(); foreign.view.face_id = 'foreign';
  assert.throws(() => portableFaceModel(foreign), /source binding/);
  const bindings = fixture(); bindings.control_bindings.face_identity = 'foreign';
  assert.throws(() => portableFaceModel(bindings), /prepared control binding/);
});
test('ambiguous source properties and repeated option references refuse', () => {
  const ambiguous = fixture();
  ambiguous.face.properties.push(structuredClone(ambiguous.face.properties[0]));
  assert.throws(() => portableFaceModel(ambiguous), /ambiguous/);
  const repeated = fixture();
  const collection = repeated.face.subjects.find(subject => subject.role === 'Collection' && subject.identity.endsWith('/input-details'));
  const relation = repeated.face.relationships.find(relation => relation.source === collection.identity);
  repeated.face.relationships.push(structuredClone(relation));
  repeated.face.properties.find(property => property.subject === collection.identity && property.name === 'item-count').value.Count += 1;
  assert.throws(() => portableFaceModel(repeated), /identity or order/);
});
test('native text draft encoding preserves Unicode and enforces source bytes', () => {
  const model = portableFaceModel(fixture());
  assert.equal(new TextDecoder().decode(model.textBytes('é'.repeat(32), 64)), 'é'.repeat(32));
  assert.throws(() => model.textBytes('é'.repeat(33), 64), /byte bound/);
  assert.throws(() => model.textBytes('\ud800', 64), /Unicode/);
});
