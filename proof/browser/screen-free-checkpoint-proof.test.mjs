import assert from 'node:assert/strict';
import test from 'node:test';
import { verifyCheckpointReady, verifyCheckpointWardrobe } from './screen-free-checkpoint-proof.mjs';

const part = { host_id: 'host/owner', boot_id: 'boot/owner' };
const face = { presentation: { identity: 'face/current', basis: { body_id: 'body/test' } },
  presentation_revision_decimal: '7', advertisement: part };
const ready = { schema: 'conduit.proof/screen-free-checkpoint@1',
  phase: 'model-provider-unavailable', body_id: 'body/test',
  owner_host_id: part.host_id, owner_boot_id: part.boot_id,
  face_id: face.presentation.identity, face_revision: '7',
  wardrobe_revision: '12', route_id: 'route/model', route_available: false };

test('checkpoint binds the held phase to the current owner Face and route', () => {
  verifyCheckpointReady(ready, ready.phase, 'body/test', part, face);
  const output = `Owner wardrobe revision 12. Worn: browser. Preference: browser. Selected: route/browser. Current acknowledged Show: show/browser. Planning: NotRequired.\n` +
    `  model on Host host/owner: unavailable. Route route/model.\n` +
    `Current Mask wardrobe\nText Face revision=12 Show=show/reading\n`;
  const result = verifyCheckpointWardrobe(output, ready, part, false);
  assert.equal(result.route_available, false);
  assert.equal(result.wardrobe_revision, '12');
  assert.throws(() => verifyCheckpointWardrobe(output,
    { ...ready, route_available: true }, part, false),
  /did not announce the exact available route/);
  assert.throws(() => verifyCheckpointReady(
    { ...ready, face_revision: '8' }, ready.phase, 'body/test', part, face),
  /Expected values to be strictly equal/);
});

test('browser leave reports a stale Plan without inventing a current route', () => {
  const lost = { ...ready, phase: 'browser-presentation-unavailable',
    route_id: 'route/browser' };
  const output = `The Body or Face changed. Its previous Mask Plan is stale, so no route from that Plan can be used now. Read the current Face; inspect the wardrobe after a Host offers a fresh Mask.\n` +
    `{"schema":"conduit.body/screen-free-wardrobe-refusal@1","code":"stale-body-or-face"}\n`;
  const result = verifyCheckpointWardrobe(output, lost, part, true);
  assert.equal(result.wardrobe_revision, null);
  assert.equal(result.refusal_code, 'stale-body-or-face');
  assert.throws(() => verifyCheckpointWardrobe(output,
    { ...lost, phase: 'model-provider-unavailable' }, part, true),
  /lacks current owner wardrobe revision/);
});
