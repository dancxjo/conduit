import assert from 'node:assert/strict';
import { test } from 'node:test';
import { faceActionReady, projectFaceCollections } from './owner-participation.mjs';

test('one Face collection keeps open items prominent and completed items behind disclosure', () => {
  const items = Array.from({ length: 20 }, (_, index) => ({
    identity: `todo/item/${index}`, name: `Task ${index}`,
    role: 'Item', disclosure: index < 3 ? 'Primary' : 'SelectedDetail',
    flags: [{ name: 'complete', value: index >= 3 }],
  }));
  const view = {
    subjects: [{ identity: 'todo/list', name: 'Groceries', role: 'Collection', disclosure: 'Context' },
      ...items, { identity: 'todo/status', name: 'Progress', role: 'Status', disclosure: 'Primary',
        text: ['3 things left · 17 completed'] }],
    relationships: [...items.map(item => ({ source: 'todo/list', target: item.identity, kind: 'Contains' })),
      { source: 'todo/list', target: 'todo/status', kind: 'Contains' }],
  };
  const result = projectFaceCollections(view);
  assert.equal(result.collections.length, 1);
  assert.deepEqual(result.collections[0].open.map(item => item.name), ['Task 0', 'Task 1', 'Task 2']);
  assert.equal(result.collections[0].completed.length, 17);
  assert.equal(result.placed.has('todo/status'), true);
  assert.equal(result.collections[0].count, '3 things left · 17 completed');
});

test('a different Status fact remains visible beside the checklist', () => {
  const view = {
    subjects: [
      { identity: 'todo/list', role: 'Collection', disclosure: 'Primary' },
      { identity: 'todo/status', role: 'Status', disclosure: 'Primary', text: ['Sync paused'] },
    ],
    relationships: [{ source: 'todo/list', target: 'todo/status', kind: 'Contains' }],
  };
  const result = projectFaceCollections(view);
  assert.equal(result.collections[0].count, '0 things left · 0 completed');
  assert.equal(result.placed.has('todo/status'), false);
});

test('an action leaves the primary checklist until this Show admits interaction', () => {
  const action = { identity: 'todo.add', availability: 'available' };
  const view = { show_state: 'available', interactions_admitted: true };
  assert.equal(faceActionReady(view, action), true);
  assert.equal(faceActionReady({ ...view, interactions_admitted: false }, action), false);
  assert.equal(faceActionReady({ ...view, show_state: 'prepared' }, action), false);
  assert.equal(faceActionReady(view, { ...action, availability: 'unavailable' }), false);
});

test('an incomplete or contradictory Face falls back to generic subject rendering', () => {
  const item = { identity: 'todo/item/one', role: 'Item', disclosure: 'Primary',
    flags: [{ name: 'complete', value: true }] };
  const view = {
    subjects: [{ identity: 'todo/list', role: 'Collection', disclosure: 'Context' }, item],
    relationships: [{ source: 'todo/list', target: item.identity, kind: 'Contains' }],
  };
  const result = projectFaceCollections(view);
  assert.equal(result.collections.length, 0);
  assert.equal(result.placed.size, 0);
});
