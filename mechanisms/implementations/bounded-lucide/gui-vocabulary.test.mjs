import assert from 'node:assert/strict';
import { test } from 'node:test';
import { iconForRole, iconForIntent, iconForKey, iconForToken } from '../../../targets/browser/handbook/owner-face-icons.mjs';

test('search, inspection and lifecycle intents keep distinct shared identities', () => {
  assert.equal(iconForIntent('conduit.intent/creche.search@1'), 'Search');
  assert.equal(iconForIntent('conduit.intent/open-inspection@1'), 'Inspect');
  assert.equal(iconForIntent('conduit.intent/lull@1'), 'Lull');
  assert.equal(iconForIntent('conduit.intent/open-plot-library@1'), 'Library');
  assert.equal(iconForRole('Collection'), 'Collection');
});

test('unknown captions, versions and object/prototype names cannot select decoration', () => {
  for (const value of [undefined, null, {}, ['Body'], '__proto__', 'constructor', 'toString']) {
    assert.equal(iconForRole(value), undefined);
    assert.equal(iconForIntent(value), undefined);
    assert.equal(iconForKey(value), undefined);
    assert.equal(iconForToken(value), undefined);
  }
  for (const value of ['Birth Body', 'creche.birth', 'conduit.intent/creche.birth@2', 'custom/conduit.intent/creche.birth@1']) {
    assert.equal(iconForIntent(value), undefined);
  }
});

test('canonical icon wire tokens share the pinned geometry keys without caption guessing', () => {
  assert.equal(iconForToken('clock'), 'Clock');
  assert.equal(iconForToken('conduit-generic-gear'), 'GenericGear');
  assert.equal(iconForToken('conduit-port'), 'Port');
  assert.equal(iconForToken('face'), 'Face');
  for (const value of ['Clock', 'text', 'repeating flow', 'generic Gear; icon metadata missing', 'clock@2', 'settings']) assert.equal(iconForToken(value), undefined);
});


test('Todo add and remove use exact intent identities and preserve unknown fallback', () => {
  assert.equal(iconForIntent('todo/add@1'), 'Add');
  assert.equal(iconForIntent('todo/remove@1'), 'Remove');
  assert.equal(iconForToken('conduit-add'), 'Add');
  assert.equal(iconForToken('conduit-remove'), 'Remove');
  for (const value of ['Add task', 'Remove task', 'todo/add@2', 'custom/todo/remove@1']) {
    assert.equal(iconForIntent(value), undefined);
  }
});
