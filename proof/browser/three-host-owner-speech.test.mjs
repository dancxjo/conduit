import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assertOwnerSelectedSpeech } from './three-host-owner-speech.mjs';

const hash = 'a'.repeat(64);
const face = {
  body_id: 'body/one', face_id: 'face/clock', face_revision: '9', show_id: 'show/browser',
  mask_plan_id: 'plan/browser-mask', mask_play_id: 'play/browser-mask',
  route: { plan_id: 'plan/browser-route', owner_host_id: 'host/linux', owner_boot_id: 'boot/linux' },
};
const expected = { face, bodyId: 'body/one', ownerHostId: 'host/linux',
  ownerBootId: 'boot/linux', operationId: 'selected-speech/one', providerSha256: hash };
const receipt = {
  schema: 'conduit.body/selected-speech-terminal@1',
  operation_id: 'selected-speech/one', outcome: 'completed',
  face_id: 'face/clock', face_revision: 9, source_show_id: 'show/browser',
  source_show_still_current: true, host_id: 'host/linux', boot_id: 'boot/linux',
  offer_generation: 1, provider_sha256: hash, selected_resource_pool_id: 'pool/speaker',
  authority_grant_id: 'grant/speaker',
  batches: [{ stream_identity: 'stream/one', source_segments_sha256: hash,
    plan_id: 'plan/speech', play_id: 'play/speech', provider_sha256: hash,
    speaker_blocks_committed: 2, outcome: 'completed' }],
};

test('owner-selected speaker receipt binds current Show, Body, Host, Plan, and Play', () => {
  const checked = assertOwnerSelectedSpeech(receipt, expected);
  assert.equal(checked.browser_route_plan_id, 'plan/browser-route');
  assert.equal(checked.source_show_id, 'show/browser');
  assert.equal(checked.batches[0].play_id, 'play/speech');
  assert.equal(checked.wav_artifact_from_this_play, false);
  assert.equal(checked.human_hearing_observed, false);
});

test('stale Show, wrong owner, incomplete playback, and absent blocks cannot pass', () => {
  for (const changed of [
    { source_show_id: 'show/stale' }, { host_id: 'host/other' },
    { source_show_still_current: false }, { outcome: 'failed' },
    { batches: [{ ...receipt.batches[0], speaker_blocks_committed: 0 }] },
    { batches: [{ ...receipt.batches[0], provider_sha256: 'b'.repeat(64) }] },
  ]) assert.throws(() => assertOwnerSelectedSpeech({ ...receipt, ...changed }, expected));
  assert.throws(() => assertOwnerSelectedSpeech(receipt, { ...expected,
    face: { ...face, face_revision: '10' } }));
});
