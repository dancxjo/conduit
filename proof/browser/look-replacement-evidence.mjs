// Assertions over observations from an actual owner and acknowledged renderer.
// This helper compares evidence; calling it with fixtures proves no execution.
import assert from 'node:assert/strict';

const requiredIdentity = (value, label) => {
  assert.equal(typeof value, 'string', `${label} must be an observed identity`);
  assert.ok(value.length > 0, `${label} is missing`);
  return value;
};

function assertExactJSONNumbers(value, path = 'canonicalFace') {
  if (typeof value === 'number') {
    assert.ok(Number.isSafeInteger(value) && !Object.is(value, -0),
      `${path} contains a number whose exact canonical integer is not preserved`);
  } else if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      assertExactJSONNumbers(child, `${path}.${key}`);
    }
  }
}

/**
 * Each snapshot contains canonicalFace (the whole owner Presentation), owner
 * {sourceDocumentId, bodyId, hostId, bootId, planId, playId}, component
 * {sourceDocumentId, checkedPlotId, foreId, recipe}, and realization
 * {expandedPlotId, planId, playId, showId, acknowledgedShowId, expression}.
 * retirement is an actual stale-return receipt, including owner observations
 * immediately before and after the rejected return.
 */
export function assertLookReplacement({ before, after, retirement }) {
  assert.ok(before && after && retirement, 'replacement needs both occurrences and retirement evidence');
  for (const snapshot of [before, after]) {
    assert.ok(snapshot.canonicalFace && typeof snapshot.canonicalFace === 'object');
    assertExactJSONNumbers(snapshot.canonicalFace);
    requiredIdentity(snapshot.canonicalFace.identity, 'canonicalFace.identity');
    assert.ok((Number.isSafeInteger(snapshot.canonicalFace.revision) && snapshot.canonicalFace.revision >= 0)
      || (typeof snapshot.canonicalFace.revision === 'string'
        && snapshot.canonicalFace.revision.length <= 20
        && /^(0|[1-9][0-9]*)$/.test(snapshot.canonicalFace.revision)
        && BigInt(snapshot.canonicalFace.revision) <= 18446744073709551615n),
    'canonical Face revision must be exact; unsafe JSON numbers are not evidence');
    assert.ok(snapshot.canonicalFace.basis && typeof snapshot.canonicalFace.basis === 'object',
      'canonical Face needs its original source basis');
    for (const field of ['subjects', 'relationships', 'properties', 'text', 'actions', 'disclosures']) {
      assert.ok(Array.isArray(snapshot.canonicalFace[field]), `canonicalFace.${field} is missing`);
    }
    assert.ok(snapshot.canonicalFace.subjects.length > 0, 'canonical Face must contain its subjects');
    for (const field of ['bodyId', 'hostId', 'bootId']) {
      requiredIdentity(snapshot.owner?.[field], `owner.${field}`);
    }
    // A lulled Body may have no current workload source/Plan/Play. Preserve
    // its explicit absence; missing evidence and fabricated Mask IDs refuse.
    for (const field of ['sourceDocumentId', 'planId', 'playId']) {
      if (snapshot.owner?.[field] !== null) {
        requiredIdentity(snapshot.owner?.[field], `owner.${field}`);
      }
    }
    for (const [ownerField, basisField] of [
      ['sourceDocumentId', 'source_document_id'], ['bodyId', 'body_id'],
      ['planId', 'plan_id'], ['playId', 'active_play_id'],
    ]) {
      assert.equal(snapshot.owner[ownerField], snapshot.canonicalFace.basis[basisField],
        `owner.${ownerField} must come from the original Face basis`);
    }
    for (const field of ['sourceDocumentId', 'checkedPlotId', 'foreId']) {
      requiredIdentity(snapshot.component?.[field], `component.${field}`);
    }
    assert.ok(snapshot.component.recipe && typeof snapshot.component.recipe === 'object');
    for (const field of ['expandedPlotId', 'planId', 'playId', 'showId', 'acknowledgedShowId']) {
      requiredIdentity(snapshot.realization?.[field], `realization.${field}`);
    }
    assert.equal(snapshot.realization.acknowledgedShowId, snapshot.realization.showId,
      'renderer acknowledgement must name the observed occurrence');
    assert.ok(snapshot.realization.expression && typeof snapshot.realization.expression === 'object');
  }
  assert.deepEqual(after.owner, before.owner, 'Look replacement must retain the owner source, Body, Host, Boot, Plan and Play');
  assert.deepEqual(after.canonicalFace, before.canonicalFace,
    'Look replacement must preserve the whole canonical Face, including revision and typed action contracts');
  assert.deepEqual(after.component, before.component,
    'the source-checked component, Fore and structural recipe must stay identical');
  assert.notDeepEqual(after.realization.expression, before.realization.expression,
    'replacement must actually select a different visual expression');
  assert.notEqual(after.realization.showId, before.realization.showId,
    'replacement must acknowledge a new Show');
  assert.equal(retirement.attemptedShowId, before.realization.showId);
  assert.equal(retirement.currentShowId, after.realization.showId);
  assert.equal(retirement.refusedBeforeSourceAdmission, true,
    'old Show return must refuse before any source action is admitted');
  assert.deepEqual(retirement.ownerBefore, after.owner);
  assert.deepEqual(retirement.ownerAfter, retirement.ownerBefore,
    'retired Show return must leave owner execution unchanged');
  assert.deepEqual(retirement.faceBefore, after.canonicalFace);
  assert.deepEqual(retirement.faceAfter, retirement.faceBefore,
    'retired Show return must leave the exact canonical Face unchanged');
}
