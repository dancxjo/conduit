import assert from 'node:assert/strict';
import test from 'node:test';
import { assertLookReplacement } from './look-replacement-evidence.mjs';

// Synthetic records exercise the verifier only; they are not renderer receipts.
function evidence() {
  const before = {
    canonicalFace: { identity: 'face', revision: 1,
      actions: [{ identity: 'complete', arguments: [], availability: 'Available' }],
      basis: { body_id: 'body', source_document_id: 'source', plan_id: 'owner-plan', active_play_id: 'owner-play' }, properties: [], text: [],
      subjects: [{ identity: 'item', name: 'Task' }], relationships: [], disclosures: [] },
    owner: { sourceDocumentId: 'source', bodyId: 'body', hostId: 'host', bootId: 'boot',
      planId: 'owner-plan', playId: 'owner-play' },
    component: { sourceDocumentId: 'component-source', checkedPlotId: 'checked',
      foreId: 'fore', recipe: { slots: ['item'] } },
    realization: { expandedPlotId: 'expanded', planId: 'mask-plan', playId: 'mask-play',
      showId: 'show-old', acknowledgedShowId: 'show-old', expression: { density: 'compact' } },
  };
  const after = structuredClone(before);
  Object.assign(after.realization, { showId: 'show-new', acknowledgedShowId: 'show-new',
    expression: { density: 'spacious' } });
  const retirement = {
    attemptedShowId: 'show-old', currentShowId: 'show-new', refusedBeforeSourceAdmission: true,
    ownerBefore: structuredClone(after.owner), ownerAfter: structuredClone(after.owner),
    faceBefore: structuredClone(after.canonicalFace), faceAfter: structuredClone(after.canonicalFace),
  };
  return { before, after, retirement };
}

test('comparison accepts a different acknowledged Look with unchanged owner and Face', () => {
  assertLookReplacement(evidence());
});

test('comparison preserves the exact maximum u64 revision as decimal text', () => {
  const value = evidence();
  for (const face of [value.before.canonicalFace, value.after.canonicalFace,
    value.retirement.faceBefore, value.retirement.faceAfter]) {
    face.revision = '18446744073709551615';
  }
  assertLookReplacement(value);
});

const mutations = [
  ['Face revision', e => e.after.canonicalFace.revision++],
  ['action argument contract', e => e.after.canonicalFace.actions[0].arguments.push({ kind: 'Text' })],
  ['action availability', e => e.after.canonicalFace.actions[0].availability = 'Unavailable'],
  ['semantic relationships', e => e.after.canonicalFace.relationships.push({ from: 'item', to: 'other' })],
  ['required disclosure', e => e.after.canonicalFace.disclosures.push({ subject: 'item', level: 'Hidden' })],
  ['owner Play', e => e.after.owner.playId = 'new-owner-play'],
  ['component recipe', e => e.after.component.recipe.slots.push('other')],
  ['unacknowledged Show', e => e.after.realization.acknowledgedShowId = 'wrong'],
  ['old Show', e => e.after.realization.showId = 'show-old'],
  ['unchanged Look', e => e.after.realization.expression = e.before.realization.expression],
  ['admitted stale return', e => e.retirement.refusedBeforeSourceAdmission = false],
  ['stale return effect', e => e.retirement.faceAfter.revision++],
  ['missing owner source identity', e => delete e.before.owner.sourceDocumentId],
  ['missing canonical Face basis', e => { delete e.before.canonicalFace.basis; delete e.after.canonicalFace.basis; }],
  ['missing canonical actions', e => { delete e.before.canonicalFace.actions; delete e.after.canonicalFace.actions; }],
  ['unsafe revision numbers', e => { e.before.canonicalFace.revision = 9007199254740992; e.after.canonicalFace.revision = 9007199254740992; }],
  ['out-of-domain decimal revisions', e => { e.before.canonicalFace.revision = '18446744073709551616'; e.after.canonicalFace.revision = '18446744073709551616'; }],
  ['rounded property counts', e => {
    const property = { subject: 'item', name: 'Count', value: { Count: 9007199254740992 } };
    e.before.canonicalFace.properties.push(property);
    e.after.canonicalFace.properties.push(structuredClone(property));
  }],
  ['nonfinite property values', e => {
    const property = { subject: 'item', name: 'Count', value: { Count: Infinity } };
    e.before.canonicalFace.properties.push(property);
    e.after.canonicalFace.properties.push(structuredClone(property));
  }],
];
for (const [label, mutate] of mutations) {
  test(`comparison refuses ${label}`, () => {
    const value = evidence();
    mutate(value);
    assert.throws(() => assertLookReplacement(value));
  });
}

for (const field of ['subjects', 'actions', 'disclosures', 'text']) {
  test(`comparison refuses a Look that reorders original ${field}`, () => {
    const value = evidence();
    const records = {
      subjects: [{ identity: 'first', name: 'First' }, { identity: 'second', name: 'Second' }],
      actions: [{ identity: 'first', arguments: [], availability: 'Available' },
        { identity: 'second', arguments: [], availability: 'Unavailable' }],
      disclosures: [{ subject: 'first', level: 'Primary' }, { subject: 'second', level: 'Detail' }],
      text: [{ subject: 'first', text: 'First meaning' }, { subject: 'second', text: 'Second meaning' }],
    }[field];
    for (const face of [value.before.canonicalFace, value.after.canonicalFace,
      value.retirement.faceBefore, value.retirement.faceAfter]) {
      face[field] = structuredClone(records);
    }
    assertLookReplacement(value);
    value.after.canonicalFace[field].reverse();
    assert.throws(() => assertLookReplacement(value));
  });
}

for (const field of ['sourceDocumentId', 'bodyId', 'planId', 'playId']) {
  test(`comparison refuses stable owner ${field} unrelated to the Face basis`, () => {
    const value = evidence();
    for (const owner of [value.before.owner, value.after.owner,
      value.retirement.ownerBefore, value.retirement.ownerAfter]) {
      owner[field] = 'unrelated-stable-identity';
    }
    assert.throws(() => assertLookReplacement(value));
  });
}
