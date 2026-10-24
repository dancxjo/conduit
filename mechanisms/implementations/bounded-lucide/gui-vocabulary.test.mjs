import assert from 'node:assert/strict';
import { test } from 'node:test';
import { iconForRole, iconForIntent, iconForKey } from '../../../targets/browser/handbook/owner-face-icons.mjs';

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
  }
  for (const value of ['Birth Body', 'creche.birth', 'conduit.intent/creche.birth@2', 'custom/conduit.intent/creche.birth@1']) {
    assert.equal(iconForIntent(value), undefined);
  }
});
