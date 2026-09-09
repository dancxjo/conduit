import test from 'node:test';
import assert from 'node:assert/strict';
import { readWorkspaceHandoff } from '../../products/workspace/browser/workspace-handoff.mjs';
import { selectedReviewedSource } from '../../products/creche/browser/creche-form-selection.mjs';

const form = { name: 'notes', source_document_id: 'source/notes', checked_form_id: 'checked/notes' };
const inventory = { forms: [form] };
const search = '?form=notes&source_document_id=source%2Fnotes&checked_form_id=checked%2Fnotes';
test('Gallery handoff accepts one complete current identity and refuses substitution or ambiguous fields', () => {
  assert.equal(readWorkspaceHandoff({ search }, inventory), form);
  assert.equal(readWorkspaceHandoff({ search: '' }, inventory), null);
  for (const query of ['?form=notes', search + '&form=notes', search.replace('checked%2Fnotes', 'checked%2Fold')]) assert.throws(() => readWorkspaceHandoff({ search: query }, inventory));
});
test('birth admission preserves selected source bytes without admitting the whole catalog', () => {
  const notes = { slug: 'notes', entry: 'notes', source: 'form notes { }\n' };
  const other = { slug: 'other', entry: 'other', source: '# unselected\n'.repeat(900) + 'form other { }' };
  const source = JSON.stringify({ schema: 'conduit.creche/reviewed-form-bundle@1', forms: [notes, other] });
  assert.ok(source.length > 8192);
  const selected = selectedReviewedSource(source, [form]);
  assert.deepEqual(JSON.parse(selected).forms, [notes]);
  assert.equal(JSON.parse(selected).forms[0].source, notes.source);
  assert.deepEqual(JSON.parse(selectedReviewedSource(source, [])).forms, [notes]);
  assert.throws(() => selectedReviewedSource(source, [{ name: 'unknown' }]));
});


test('long birth refusals retain their detail without overflowing a status label', async () => {
  const { birthFeedbackNodes } = await import('../../products/creche/browser/creche-lifecycle.mjs');
  const reason = 'Exact typed-port mismatch: ' + '理由'.repeat(500);
  const nodes = birthFeedbackNodes(reason, 'failure-status');
  assert.equal(nodes.at(-1).value, reason);
  assert.ok(nodes.every(node => new TextEncoder().encode(node.text).length <= 256));
  const oversized = birthFeedbackNodes('理由'.repeat(20000), 'failure-status').at(-1).value;
  assert.ok(new TextEncoder().encode(oversized).length <= 65_536);
  assert.ok(oversized.endsWith('[Details exceed the display bound.]'));
  assert.ok(!oversized.includes('�'));
});
