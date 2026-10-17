import assert from 'node:assert/strict';
import test from 'node:test';
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
