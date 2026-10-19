// Adapter-envelope tests only: these synthetic records are not guest evidence.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { validateCapture } from './native-face-reconstruction.mjs';

const digest = value => createHash('sha256').update(value).digest('hex');
function fixture() {
  const contract = { value_kind: 'value/text', maximum_bytes: 64 };
  const face = { identity: 'face/fixture', revision: 7,
    subjects: [{ identity: 'collection', name: 'Choices', role: 'Collection' },
      { identity: 'second', name: 'Duplicate', role: 'Choice' }, { identity: 'first', name: 'Duplicate', role: 'Choice' },
      { identity: 'name', name: 'Name', role: 'TextEntry' }],
    properties: [{ subject: 'second', name: 'order', value: { Count: 1 } },
      { subject: 'first', name: 'order', value: { Count: 0 } },
      { subject: 'name', name: 'current-value', value: { TypedValue: { contract, bytes: [...Buffer.from('É雪')] } } }],
    relationships: [{ source: 'collection', kind: 'Contains', target: 'second' }, { source: 'collection', kind: 'Contains', target: 'first' }] };
  const face_json = JSON.stringify(face);
  return { schema: 'conduit.proof/native-face-browser-reconstruction@1', source: 'synthetic-only', image: 'not-a-guest-image', records: [{
    face_json, face_sha256: digest(face_json), face_id: face.identity, face_revision: '7',
    native_show: { face_id: face.identity, face_revision: 7, show_id: 'synthetic-show' },
    linear: { face_id: face.identity, face_revision: 7, proof_class: 'deterministic-semantic-reconstruction',
      input_enactment: false, acknowledged_show: null, lines: ['synthetic'] },
    control_bindings: { face_identity: face.identity, face_revision: '7', text_entries: ['name'], collections: [{ subject: 'collection', mode: 'exclusive' }] },
  }] };
}

test('retains ordered identities despite duplicate labels and original Unicode typed bytes', () => {
  const capture = fixture(), before = JSON.stringify(capture);
  const [prepared] = validateCapture(capture);
  assert.deepEqual(prepared.collections[0].items, ['first', 'second']);
  assert.deepEqual(prepared.fields[0].value.bytes, [...Buffer.from('É雪')]);
  assert.deepEqual(prepared.unsupportedExclusive, ['collection']);
  assert.equal(JSON.stringify(capture), before);
});
test('rejects changed captured bytes and identity mismatches', () => {
  for (const mutate of [record => { record.face_json += ' '; }, record => { record.face_id = 'other'; },
    record => { record.native_show.face_id = 'other'; }, record => { record.linear.face_revision = 8; },
    record => { record.control_bindings.face_revision = '8'; }]) {
    const capture = fixture(); mutate(capture.records[0]); assert.throws(() => validateCapture(capture));
  }
});
test('refuses reconstruction records claiming linear input authority', () => {
  for (const mutate of [linear => { linear.input_enactment = true; }, linear => { linear.acknowledged_show = 'invented'; }]) {
    const capture = fixture(); mutate(capture.records[0].linear); assert.throws(() => validateCapture(capture));
  }
});
test('rejects unsafe revision rather than rounding its identity', () => {
  const capture = fixture(), record = capture.records[0];
  record.face_json = record.face_json.replace('"revision":7', '"revision":9007199254740992');
  record.face_sha256 = digest(record.face_json);
  assert.throws(() => validateCapture(capture), /Unsafe revision/);
});
test('bounds records and prepared references', () => {
  const capture = fixture(); capture.records[0].control_bindings.collections = Array(65).fill({ subject: 'collection', mode: 'exclusive' });
  assert.throws(() => validateCapture(capture));
  capture.records = Array(257).fill(capture.records[0]);
  assert.throws(() => validateCapture(capture));
});

test('cannot silently omit a portable control from both preparation and refusal', () => {
  const capture = fixture(); capture.records[0].control_bindings.text_entries = [];
  assert.throws(() => validateCapture(capture), /Every portable control/);
});
