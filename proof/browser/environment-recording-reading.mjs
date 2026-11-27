// Compare the fresh Owner reading directly. The separately qualified native
// pair supplies only typed display text, keyed by exact canonical payload.
import assert from 'node:assert/strict';
import { captureDecimal } from './exact-capture-json.mjs';

const exactKey = value => JSON.stringify(value);
function signedDecimal(value) {
  if (typeof value === 'number' && Number.isSafeInteger(value)) return String(value);
  if (typeof JSON.isRawJSON === 'function' && JSON.isRawJSON(value)
      && /^-?\d+$/.test(value.rawJSON)) return value.rawJSON;
  throw new Error('reading contains an inexact signed integer');
}
function oracle(nativeFace, nativeView) {
  const result = new Map();
  for (const subject of nativeFace.subjects) {
    const reading = nativeView.subjects.find(s => s.identity === subject.identity);
    assert.ok(reading, 'native oracle subject missing');
    const properties = nativeFace.properties.filter(p => p.subject === subject.identity);
    assert.equal(reading.properties.length, properties.length);
    let typedIndex = 0;
    for (const [index, property] of properties.entries()) {
      const value = property.value;
      if (!('TypedValue' in value) && !('ValueContract' in value)) continue;
      const key = exactKey(value);
      const prefix = `${property.name}: `;
      assert.ok(reading.properties[index].startsWith(prefix));
      const display = reading.properties[index].slice(prefix.length);
      let typed;
      if ('TypedValue' in value) {
        typed = reading.values[typedIndex++];
        assert.ok(typed, 'native typed row missing');
        assert.deepEqual(typed, {
          name: property.name, kind: value.TypedValue.contract.value_kind, text: display,
        });
      }
      const entry = { display, kind: typed?.kind };
      if (result.has(key)) assert.deepEqual(result.get(key), entry, 'inconsistent native display');
      result.set(key, entry);
    }
    assert.equal(typedIndex, reading.values.length);
  }
  return result;
}
function display(value, typedOracle) {
  assert.equal(Object.keys(value).length, 1, 'unknown property enum shape');
  const [kind, payload] = Object.entries(value)[0];
  switch (kind) {
    case 'Identity': case 'Text': case 'BaseImplementationId':
      assert.equal(typeof payload, 'string'); return payload;
    case 'Count': return captureDecimal(payload);
    case 'Signed': return signedDecimal(payload);
    case 'Flag': assert.equal(typeof payload, 'boolean'); return String(payload);
    case 'Content': assert.ok(Array.isArray(payload)); return `${payload.length} content bytes`;
    case 'TypedValue': case 'ValueContract': {
      const entry = typedOracle.get(exactKey(value));
      assert.ok(entry, `no exact native rendering oracle for ${kind}`);
      return entry.display;
    }
    default: throw new Error(`unsupported reading property ${kind}`);
  }
}

export function assertCompleteReading(face, view, nativeFace, nativeView) {
  const typedOracle = oracle(nativeFace, nativeView);
  assert.equal(view.face_id, face.identity);
  assert.equal(view.face_revision, captureDecimal(face.revision));
  assert.equal(view.subjects.length, face.subjects.length);
  for (const [index, subject] of face.subjects.entries()) {
    const reading = view.subjects[index];
    assert.equal(reading.identity, subject.identity);
    assert.equal(reading.name, subject.name);
    if (typeof subject.role === 'string') assert.equal(reading.role, subject.role);
    else {
      assert.deepEqual(Object.keys(subject.role), ['Semantic']);
      assert.equal(reading.semantic_role, subject.role.Semantic);
    }
    assert.deepEqual(reading.text, face.text.filter(t => t.subject === subject.identity).map(t => t.text));
    const properties = face.properties.filter(p => p.subject === subject.identity);
    assert.deepEqual(reading.properties,
      properties.map(p => `${p.name}: ${display(p.value, typedOracle)}`));
    assert.deepEqual(reading.flags, properties.filter(p => 'Flag' in p.value)
      .map(p => ({ name: p.name, value: p.value.Flag })));
    assert.deepEqual(reading.values, properties.filter(p => 'TypedValue' in p.value).map(p => ({
      name: p.name, kind: p.value.TypedValue.contract.value_kind,
      text: display(p.value, typedOracle),
    })));
  }
  assert.deepEqual(view.relationships, face.relationships.map(r => ({
    source: r.source, target: r.target, kind: r.kind,
  })));
  assert.deepEqual(view.actions.map(({ identity, name, intent, target }) => ({ identity, name, intent, target })),
    face.actions.map(({ identity, name, intent, target }) => ({ identity, name, intent, target })));
}
