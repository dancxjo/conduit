import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeGraphCanvas } from '../../targets/browser/host/assets/application-graph-canvas.mjs';
import { encodeApplicationView, decodeApplicationView, encodeApplicationEvent } from '../../targets/browser/host/assets/application-face.mjs';

const description = () => ({ revision: 1, actions: [{ id: 'fixture.input', event: 'input' }], nodes: [
  { parent: null, component: 'text-input', key: 'source', text: 'Source', value: 'é😀', valueCapacity: 32, action: 0 },
] });

test('view codec preserves leading BOM and scalar Unicode in every text field', () => {
  const source = description();
  source.actions[0].id = '\ufefffixture.input';
  for (const field of ['key', 'text', 'value']) source.nodes[0][field] = '\ufeff' + source.nodes[0][field];
  const decoded = decodeApplicationView(encodeApplicationView(source));
  assert.equal(decoded.actions[0].id, source.actions[0].id);
  for (const field of ['key', 'text', 'value']) assert.equal(decoded.nodes[0][field], source.nodes[0][field]);
});

test('view encoding refuses surrogate replacement and implicit text coercion', () => {
  for (const invalid of ['\ud800', '\udfff', 'a\ud800b', 42, { toString: () => 'coerced' }]) {
    for (const field of ['action', 'key', 'text', 'value']) {
      const source = description();
      if (field === 'action') source.actions[0].id = invalid;
      else source.nodes[0][field] = invalid;
      assert.throws(() => encodeApplicationView(source), { code: 'malformed-encoding' });
    }
    assert.throws(() => encodeApplicationEvent({ revision: 1, action: invalid, kind: 3, value: new Uint8Array() }),
      { code: 'malformed-encoding' });
  }
});

test('view collection admission refuses before iterating overcapacity input', () => {
  for (const [field, count, code] of [['nodes', 129, 'too-many-nodes'], ['actions', 33, 'too-many-actions']]) {
    const source = description();
    source[field] = Array(count);
    source[field][Symbol.iterator] = () => assert.fail('overcapacity collection must not be iterated');
    assert.throws(() => encodeApplicationView(source), { code });
  }
  const empty = description(); empty.nodes = [];
  empty.actions[Symbol.iterator] = () => assert.fail('empty view must refuse before action processing');
  assert.throws(() => encodeApplicationView(empty), { code: 'empty' });
});

test('view codec accepts the exact node and action inventory ceilings', () => {
  const nodes = [{ parent: null, component: 'shell', key: 'root', text: '', action: null },
    ...Array.from({ length: 127 }, (_, index) => ({ parent: 0, component: 'paragraph', key: `node-${index}`, text: 'bounded', action: null }))];
  const actions = Array.from({ length: 32 }, (_, index) => ({ id: `fixture.action-${index}`, event: 'activate' }));
  const decoded = decodeApplicationView(encodeApplicationView({ revision: 1, nodes, actions }));
  assert.equal(decoded.nodes.length, 128);
  assert.equal(decoded.actions.length, 32);
});

const event = value => ({ revision: 1, action: 'fixture.event', kind: 3, value });
test('event codec refuses metadata wrapping and implicit byte coercion', () => {
  for (const revision of [-1, 0x1_0000_0000, 1.5, undefined]) {
    assert.throws(() => encodeApplicationEvent({ ...event(new Uint8Array()), revision }), { code: 'malformed-encoding' });
  }
  for (const kind of ['3', 'toString', 1.5]) {
    assert.throws(() => encodeApplicationEvent({ ...event(new Uint8Array()), kind }), { code: 'unknown-event' });
  }
  for (const value of [1, 'bytes', { length: 1, 0: 42 }, [256], [-1], [1.5], [NaN], Array(1)]) {
    assert.throws(() => encodeApplicationEvent(event(value)), { code: 'malformed-encoding' });
  }
});

test('event codec checks payload ceilings before array iteration and preserves exact supported bytes', () => {
  const oversized = Array(65_537);
  oversized[Symbol.iterator] = () => assert.fail('oversized bytes must refuse before iteration');
  assert.throws(() => encodeApplicationEvent(event(oversized)), { code: 'event-too-large' });
  const bytes = new Uint8Array(65_536); bytes[0] = 1; bytes[bytes.length - 1] = 255;
  for (const value of [bytes, bytes.buffer, Array.from(bytes)]) {
    const encoded = encodeApplicationEvent(event(value));
    const view = new DataView(encoded.buffer);
    assert.equal(view.getUint32(7, true), 65_536);
    assert.equal(encoded.at(-65_536), 1);
    assert.equal(encoded.at(-1), 255);
  }
});

test('event array conversion keeps the admitted length if a byte getter grows its source', () => {
  const values = [0];
  Object.defineProperty(values, 0, { get() { values.push(256); return 42; } });
  const encoded = encodeApplicationEvent(event(values));
  assert.equal(new DataView(encoded.buffer).getUint32(7, true), 1);
  assert.equal(encoded.at(-1), 42);
});

test('view decode admits byte forms and limits before conversion', () => {
  const oversized = Array(131_073);
  oversized[Symbol.iterator] = () => assert.fail('oversized view must refuse before iteration');
  assert.throws(() => decodeApplicationView(oversized), { code: 'oversized-encoding' });
  for (const invalid of [1, 'bytes', { length: 1, 0: 10 }, [-1], [256], Array(1)]) {
    assert.throws(() => decodeApplicationView(invalid), { code: 'malformed-encoding' });
  }
  const encoded = encodeApplicationView(description());
  const expected = decodeApplicationView(encoded);
  for (const input of [encoded.buffer, Array.from(encoded)]) assert.deepEqual(decodeApplicationView(input), expected);
});

test('view decoding respects padded typed-array and Node Buffer offsets', () => {
  const source = description(); source.revision = 0x01020304;
  const encoded = encodeApplicationView(source);
  const expected = decodeApplicationView(encoded);
  const padded = new Uint8Array(encoded.length + 17).fill(0xa5);
  padded.set(encoded, 7);
  const nodeBuffer = Buffer.alloc(encoded.length + 17, 0xa5);
  nodeBuffer.set(encoded, 7);
  for (const input of [padded.subarray(7, 7 + encoded.length), nodeBuffer.subarray(7, 7 + encoded.length), Buffer.from(encoded)]) {
    assert.deepEqual(decodeApplicationView(input), expected);
  }
});

test('oversized text fields refuse before UTF-8 allocation', () => {
  const NativeEncoder = globalThis.TextEncoder;
  try {
    for (const [field, maximum, code] of [['action', 48, 'action-id-too-long'], ['key', 32, 'text-too-long'],
      ['text', 256, 'text-too-long'], ['value', 65_536, 'invalid-control-value']]) {
      const oversized = 'x'.repeat(maximum + 1);
      globalThis.TextEncoder = class extends NativeEncoder {
        encode(value) { assert.notEqual(value, oversized, 'oversized field must refuse before UTF-8 encoding'); return super.encode(value); }
      };
      const source = description();
      if (field === 'action') source.actions[0].id = oversized;
      else source.nodes[0][field] = oversized;
      assert.throws(() => encodeApplicationView(source), { code });
    }
  } finally { globalThis.TextEncoder = NativeEncoder; }
});

function largeView() {
  return { revision: 1, actions: [], nodes: [
    { parent: null, component: 'shell', key: 'root', text: '', action: null },
    ...['a', 'b'].map(key => ({ parent: 0, component: 'textarea', key, text: key,
      value: '', valueCapacity: 65_536, action: null })),
  ] };
}
test('view envelope refuses before processing later fields and accepts its exact ceiling', () => {
  const source = largeView();
  const overhead = encodeApplicationView(source).length;
  source.nodes[1].value = 'x'.repeat(65_536);
  source.nodes[2].value = 'y'.repeat(131_072 - overhead - 65_536);
  assert.equal(encodeApplicationView(source).length, 131_072);
  source.nodes[2].value += 'y';
  source.nodes.push({ parent: 0, component: 'paragraph', key: 'later', action: null,
    get text() { assert.fail('oversized envelope must refuse before later fields'); } });
  assert.throws(() => encodeApplicationView(source), { code: 'oversized-encoding' });
});

test('view encoding uses admitted entries rather than custom array iteration', () => {
  const source = description();
  const expected = decodeApplicationView(encodeApplicationView(source));
  for (const field of ['nodes', 'actions']) {
    source[field][Symbol.iterator] = () => assert.fail('encoding must use the admitted indexed entries');
  }
  assert.deepEqual(decodeApplicationView(encodeApplicationView(source)), expected);
});

test('view encoding keeps admitted counts when source getters append entries', () => {
  const source = description();
  Object.defineProperty(source.actions[0], 'event', { get() {
    source.actions.push({ id: 'not-admitted', event: 'activate' }); return 'input';
  } });
  Object.defineProperty(source.nodes[0], 'text', { get() {
    source.nodes.push({ parent: 0, component: 'paragraph', key: 'not-admitted', text: 'late', action: null }); return 'Source';
  } });
  const decoded = decodeApplicationView(encodeApplicationView(source));
  assert.equal(decoded.actions.length, 1);
  assert.equal(decoded.nodes.length, 1);
  assert.equal(decoded.nodes[0].text, 'Source');
});

const graphFields = () => ['conduit.face/graph-canvas@1', 'checked', 'expanded', 'plan', 'body-plan', 'play', 'gear/source',
  '1', 'gear/source', 'Source', 'source-kind', '0', '0'];
const encodeGraphFields = fields => fields.map(value => `${new TextEncoder().encode(value).length}:${value}`).join('');
test('graph codec preserves BOM-bearing provenance and subject identities', () => {
  const fields = graphFields();
  for (const index of [1, 2, 3, 4, 5, 6, 8, 9, 10]) fields[index] = '\ufeff' + fields[index];
  const graph = decodeGraphCanvas(encodeGraphFields(fields));
  for (const [index, key] of ['checked_plot', 'expanded_plot', 'plan', 'body_plan', 'play', 'selected'].entries()) {
    assert.equal(graph[key], fields[index + 1]);
  }
  assert.deepEqual(graph.nodes, [{ identity: fields[8], label: fields[9], kind: fields[10] }]);
});

test('graph codec refuses schema normalization and surrogate replacement', () => {
  const schema = graphFields(); schema[0] = '\ufeff' + schema[0];
  assert.throws(() => decodeGraphCanvas(encodeGraphFields(schema)), /Unknown graph canvas schema/);
  const malformed = graphFields(); malformed[9] = '\ud800';
  assert.throws(() => decodeGraphCanvas(encodeGraphFields(malformed)), /scalar Unicode/);
});

test('graph record counts refuse signed decimal forms for every collection', () => {
  for (const index of [7, 11, 12]) {
    const fields = graphFields(); fields[index] = '+' + fields[index];
    assert.throws(() => decodeGraphCanvas(encodeGraphFields(fields)), /Graph record count exceeds its bound/);
  }
});

test('select initial values require an owned option while empty scaffolding remains valid', () => {
  const view = { revision: 1, actions: [], nodes: [
    { parent: null, component: 'select', key: 'select', text: 'Choice', value: 'absent', valueCapacity: 16, action: null },
    { parent: 0, component: 'option', key: 'one', text: 'Unicode', value: 'é😀', valueCapacity: 16, action: null },
  ] };
  assert.throws(() => encodeApplicationView(view), { code: 'invalid-control-value' });
  view.nodes[0].value = 'é😀';
  assert.equal(decodeApplicationView(encodeApplicationView(view)).nodes[0].value, 'é😀');
  view.nodes[0].value = '';
  assert.equal(decodeApplicationView(encodeApplicationView(view)).nodes[0].value, '');
  view.nodes.length = 1;
  assert.equal(decodeApplicationView(encodeApplicationView(view)).nodes.length, 1);
});

test('exclusive initial selection agrees with source lowering while independent choices remain independent', () => {
  const view = { revision: 1, actions: [], nodes: [
    { parent: null, component: 'choice-group', key: 'group', text: 'group', action: null },
    { parent: 0, component: 'choice-group-label', key: 'legend', text: 'Choices', action: null },
    { parent: 0, component: 'choice-option-label', key: 'first-label', text: 'First', action: null },
    { parent: 2, component: 'exclusive-choice', key: 'first', text: 'First', value: 'true', valueCapacity: 5, action: null },
    { parent: 0, component: 'choice-option-label', key: 'second-label', text: 'Second', action: null },
    { parent: 4, component: 'exclusive-choice', key: 'second', text: 'Second', value: 'true', valueCapacity: 5, action: null },
  ] };
  assert.throws(() => encodeApplicationView(view), { code: 'invalid-control-value' });
  view.nodes[5].value = 'false';
  assert.equal(decodeApplicationView(encodeApplicationView(view)).nodes[3].value, 'true');
  view.nodes[3].value = 'false';
  assert.equal(decodeApplicationView(encodeApplicationView(view)).nodes[3].value, 'false');
  for (const index of [3, 5]) { view.nodes[index].component = 'independent-choice'; view.nodes[index].value = 'true'; }
  assert.equal(decodeApplicationView(encodeApplicationView(view)).nodes[5].value, 'true');
});

test('numeric control syntax and padded zero steps agree across masks', () => {
  const control = (component, value) => ({ revision: 1, actions: [], nodes: [
    { parent: null, component, key: 'control', text: 'Control', value, valueCapacity: 16, action: null },
    ...(component === 'stepper' ? ['first', 'second'].map(key => ({ parent: 0, component: 'button', key, text: key, action: null })) : []),
  ] });
  for (const value of ['+1/2', '1/+2', '000001/2', '1/000002']) {
    assert.throws(() => encodeApplicationView(control('progress', value)), { code: 'invalid-control-value' });
  }
  for (const value of ['0/2', '00/2', '00000/2']) {
    assert.throws(() => encodeApplicationView(control('stepper', value)), { code: 'invalid-control-value' });
    assert.equal(decodeApplicationView(encodeApplicationView(control('progress', value))).nodes[0].value, value);
  }
  for (const [component, value] of [['progress', '65535/65535'], ['stepper', '01/2']]) {
    assert.equal(decodeApplicationView(encodeApplicationView(control(component, value))).nodes[0].value, value);
  }
});

test('event encoding retains the admitted revision instead of rereading a getter', () => {
  let reads = 0;
  const source = { ...event(Uint8Array.of(42)), get revision() { return ++reads === 1 ? 7 : 0x1_0000_0000; } };
  const encoded = encodeApplicationEvent(source);
  assert.equal(reads, 1);
  assert.equal(new DataView(encoded.buffer).getUint32(1, true), 7);
});

test('event encoding retains the admitted kind instead of rereading a getter', () => {
  let reads = 0;
  const source = { ...event(Uint8Array.of(42)), get kind() { return ++reads === 1 ? 3 : 99; } };
  const encoded = encodeApplicationEvent(source);
  assert.equal(reads, 1);
  assert.equal(encoded[5], 3);
});

test('payload conversion cannot rewrite already-admitted event metadata', () => {
  const source = event([42]);
  Object.defineProperty(source.value, 0, { get() {
    source.revision = 0x1_0000_0000; source.kind = 99;
    return 42;
  } });
  const encoded = encodeApplicationEvent(source);
  assert.equal(new DataView(encoded.buffer).getUint32(1, true), 1);
  assert.equal(encoded[5], 3);
  assert.equal(encoded.at(-1), 42);
});
