import assert from 'node:assert/strict';
import test from 'node:test';
import { assertModelLossFaceBoundary } from './three-host-owner-llm.mjs';

const face = (identity, revision) => ({
  presentation: { identity, basis: { body_id: 'body/one' } },
  presentation_revision_decimal: revision,
});
const checkpoint = {
  body_id: 'body/one', route_id: 'route/model', route_available: false,
  face_id: 'face/new', face_revision: '14',
};
const observed = {
  beforeRefusal: face('face/old', '13'),
  afterRefusal: face('face/old', '13'),
  afterCheckpoint: face('face/new', '14'),
  checkpoint, bodyId: 'body/one', routeId: 'route/model',
};

test('a completed screen-free read may advance Face after a refused model Start', () => {
  assert.doesNotThrow(() => assertModelLossFaceBoundary(observed));
  assert.throws(() => assertModelLossFaceBoundary({
    ...observed, afterRefusal: face('face/changed-by-refusal', '14'),
  }), /refused Start cannot change/);
  assert.throws(() => assertModelLossFaceBoundary({
    ...observed, afterCheckpoint: face('face/not-the-checkpoint', '14'),
  }), /completed checkpoint at the same revision/);
  assert.throws(() => assertModelLossFaceBoundary({
    ...observed, afterCheckpoint: face('face/stale', '13'),
  }), /cannot precede the completed screen-free checkpoint/);
});
