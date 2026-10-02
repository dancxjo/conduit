import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeGraphCanvas } from '../../targets/browser/host/assets/application-graph-canvas.mjs';
const encode = fields => fields.map(value => `${Buffer.byteLength(value)}:${value}`).join('');
const fields = ['conduit.presentation/graph-canvas@1','checked','expanded','plan','body-plan','play','gear','1','gear','Café 🎵','text/upper','2','in','gear','source','input','text/utf8','stream','out','gear','text','output','text/utf8','stream','1','cord','out','in'];
test('decodes exact native canvas shape and Unicode without inventing graph facts', () => {
  const graph = decodeGraphCanvas(encode(fields));
  assert.equal(graph.nodes[0].label, 'Café 🎵');
  assert.deepEqual(graph.cords, [{ identity: 'cord', source: 'out', sink: 'in' }]);
  assert.equal(graph.body_plan, 'body-plan');
  assert.equal(graph.play, 'play');
});
test('rejects stale unknown selection and dangling endpoint', () => {
  const wrongSelection = [...fields]; wrongSelection[6] = 'absent';
  assert.throws(() => decodeGraphCanvas(encode(wrongSelection)), /selection/);
  const dangling = [...fields]; dangling[dangling.length - 1] = 'absent';
  assert.throws(() => decodeGraphCanvas(encode(dangling)), /endpoint/);
});
test('rejects extra bytes and finite bounds overflow', () => {
  assert.throws(() => decodeGraphCanvas(encode(fields) + 'junk'));
  const count = [...fields]; count[7] = '129';
  assert.throws(() => decodeGraphCanvas(encode(count)), /bound/);
  assert.throws(() => decodeGraphCanvas('x'.repeat(65537)), /bound/);
});
