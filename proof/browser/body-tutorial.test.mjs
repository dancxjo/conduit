import test from 'node:test';
import assert from 'node:assert/strict';
import { deriveBodyTutorial } from '../../products/workspace/browser/body-tutorial.mjs';

const context = (state, { wakes = [], workloadRevision = 0, playback = 'Playing' } = {}) => ({
  current: state === null ? null : { state, workload_revision: workloadRevision },
  evidence: state === null ? null : { evidence: { wakes } }, playback: { state: playback },
});

test('guidance advances only from real Body lifecycle and workload evidence', () => {
  assert.equal(deriveBodyTutorial(context(null)).phase, 'birth');
  assert.equal(deriveBodyTutorial(context('LULLED')).phase, 'wake');
  assert.equal(deriveBodyTutorial(context('AWAKE', { wakes: [{ lifecycle: 'Playing' }] })).phase, 'living');
  assert.match(deriveBodyTutorial(context('AWAKE', { wakes: [{ lifecycle: 'Playing' }], playback: 'Idle' })).detail, /Finite means bounded/);
  assert.equal(deriveBodyTutorial(context('AWAKE', { wakes: [{}, {}] })).phase, 'continuity');
  assert.equal(deriveBodyTutorial(context('AWAKE', { wakes: [{}], workloadRevision: 1 })).phase, 'revised');
  assert.equal(deriveBodyTutorial(context('LULLED', { wakes: [{ lifecycle: 'Lulled' }] })).phase, 'lull');
  assert.equal(deriveBodyTutorial(context('LULLED', { wakes: [{ lifecycle: 'Failed' }] })).phase, 'repair');
  assert.equal(deriveBodyTutorial(context('FULFILLED')).phase, 'fulfilled');
});

test('presentation state alone cannot claim repair, revision, or fulfillment', () => {
  const projected = deriveBodyTutorial(context('AWAKE', { wakes: [{ lifecycle: 'Playing' }], playback: 'Fulfilled' }));
  assert.equal(projected.phase, 'living');
  assert.doesNotMatch(projected.detail, /complete|repair|changed workset/i);
});
