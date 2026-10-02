import test from 'node:test';
import assert from 'node:assert/strict';
import { readWorkspaceHandoff } from '../../targets/browser/workspace/workspace-handoff.mjs';
import { selectedReviewedSource } from '../../targets/browser/workspace/reviewed-plot-selection.mjs';

const plot = { name: 'notes', source_document_id: 'source/notes', checked_plot_id: 'checked/notes' };
const inventory = { plots: [plot] };
const search = '?plot=notes&source_document_id=source%2Fnotes&checked_plot_id=checked%2Fnotes';
test('Gallery handoff accepts one complete current identity and refuses substitution or ambiguous fields', () => {
  assert.equal(readWorkspaceHandoff({ search }, inventory), plot);
  assert.equal(readWorkspaceHandoff({ search: '' }, inventory), null);
  for (const query of ['?plot=notes', search + '&plot=notes', search.replace('checked%2Fnotes', 'checked%2Fold')]) assert.throws(() => readWorkspaceHandoff({ search: query }, inventory));
});
test('birth admission preserves selected source bytes without admitting the whole catalog', () => {
  const notes = { slug: 'notes', entry: 'notes', source: 'plot notes { }\n' };
  const other = { slug: 'other', entry: 'other', source: '# unselected\n'.repeat(900) + 'plot other { }' };
  const source = JSON.stringify({ schema: 'conduit.creche/reviewed-plot-bundle@2', plots: [notes, other] });
  assert.ok(source.length > 8192);
  const selected = selectedReviewedSource(source, [plot]);
  assert.deepEqual(JSON.parse(selected).plots, [notes]);
  assert.equal(JSON.parse(selected).plots[0].source, notes.source);
  assert.deepEqual(JSON.parse(selectedReviewedSource(source, [])).plots, [notes]);
  assert.throws(() => selectedReviewedSource(source, [{ name: 'unknown' }]));
});


test('long birth refusals retain their detail without overflowing a status label', async () => {
  const { birthFeedbackNodes } = await import('../../targets/browser/workspace/body-bootstrap.mjs');
  const reason = 'Exact typed-port mismatch: ' + '理由'.repeat(500);
  const nodes = birthFeedbackNodes(reason, 'failure-status');
  assert.equal(nodes.at(-1).value, reason);
  assert.ok(nodes.every(node => new TextEncoder().encode(node.text).length <= 256));
  const oversized = birthFeedbackNodes('理由'.repeat(20000), 'failure-status').at(-1).value;
  assert.ok(new TextEncoder().encode(oversized).length <= 65_536);
  assert.ok(oversized.endsWith('[Details exceed the display bound.]'));
  assert.ok(!oversized.includes('�'));
});
