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
  face_id: 'face/clock', face_revision: 9, face_revision_decimal: '9',
  source_show_id: 'show/browser',
  source_show_still_current: true, host_id: 'host/linux', boot_id: 'boot/linux',
  offer_generation: 1, provider_sha256: hash, selected_resource_pool_id: 'pool/speaker',
  authority_grant_id: 'grant/speaker',
  batches: [{ stream_identity: 'stream/one', source_segments_sha256: hash,
    spoken_segments: ['The current clock interval is 500 milliseconds.'],
    plan_id: 'plan/speech', play_id: 'play/speech', provider_sha256: hash,
    speaker_blocks_committed: 2, speaker_frames_committed: 100,
    wav_artifact_id: `play-${hash}.wav`, wav_sha256: hash, wav_bytes: 444,
    pcm_sha256: hash, pcm_bytes: 400, pcm_blocks: 2, outcome: 'completed' }],
};

test('owner-selected speaker receipt binds current Show, Body, Host, Plan, and Play', () => {
  const checked = assertOwnerSelectedSpeech(receipt, expected);
  assert.equal(checked.browser_route_plan_id, 'plan/browser-route');
  assert.equal(checked.source_show_id, 'show/browser');
  assert.equal(checked.batches[0].play_id, 'play/speech');
  assert.equal(checked.wav_artifact_from_this_play, true);
  assert.equal(checked.human_hearing_observed, false);
});

test('large Face revisions use the exact decimal wire value', () => {
  const decimal = '9007199254740993';
  const roundedByJavaScript = JSON.parse('{"face_revision":9007199254740993}').face_revision;
  assert.notEqual(String(roundedByJavaScript), decimal);
  assertOwnerSelectedSpeech({ ...receipt, face_revision: roundedByJavaScript,
    face_revision_decimal: decimal }, { ...expected,
    face: { ...face, face_revision: decimal } });
});

test('stale Show, wrong owner, incomplete playback, and absent blocks cannot pass', () => {
  for (const changed of [
    { source_show_id: 'show/stale' }, { host_id: 'host/other' },
    { source_show_still_current: false }, { outcome: 'failed' },
    { batches: [{ ...receipt.batches[0], speaker_blocks_committed: 0 }] },
    { batches: [{ ...receipt.batches[0], speaker_frames_committed: 99 }] },
    { batches: [{ ...receipt.batches[0], wav_artifact_id: '../escape.wav' }] },
    { batches: [{ ...receipt.batches[0], spoken_segments: [] }] },
    { batches: [{ ...receipt.batches[0], provider_sha256: 'b'.repeat(64) }] },
  ]) assert.throws(() => assertOwnerSelectedSpeech({ ...receipt, ...changed }, expected));
  assert.throws(() => assertOwnerSelectedSpeech(receipt, { ...expected,
    face: { ...face, face_revision: '10' } }));
  assert.throws(() => assertOwnerSelectedSpeech({ ...receipt,
    face_revision_decimal: '9007199254740993' }, expected));
});
