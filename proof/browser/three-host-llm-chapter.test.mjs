import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { assertExactFaceRevision, assertRestoredSpeech } from './three-host-llm-chapter.mjs';

test('browser decimal Face revision matches exact numeric receipt', () => {
  assert.doesNotThrow(() => assertExactFaceRevision('13', 13));
  assert.throws(() => assertExactFaceRevision('13', 14));
  assert.throws(() => assertExactFaceRevision('013', 13));
  assert.throws(() => assertExactFaceRevision(13, 13));
});

test('unsafe numeric Face revision refuses instead of rounding', () => {
  assert.throws(() => assertExactFaceRevision('9007199254740992', 9007199254740992),
    /exact safe integer/);
});

test('restored model route must retain the exact owner Face, model, validation, and audio', () => {
  const sha = bytes => createHash('sha256').update(bytes).digest('hex');
  const wav = Buffer.concat([Buffer.from('RIFF'), Buffer.alloc(48)]);
  const original = { output: '{"proposal":"face text"}' };
  original.sha256 = sha(Buffer.from(original.output));
  const words = {
    original_model_output: original.output, text: 'The current clock is running.',
    source_commit: 'commit', run_id: 'run', body_id: 'body', face_revision: '14',
    show_id: 'restored-show',
  };
  const receipt = {
    proof_class: 'live-local-model', source_commit: 'commit', run_id: 'run',
    body_id: 'body', owner_host_id: 'host', owner_boot_id: 'boot',
    face_id: 'face', face_revision: 14, action_id: 'explain-after-restoration',
    owner_snapshot_before_after_equal: true, local_spoken_mask_show_observed: true,
    owner_sealed_spoken_mask_route_observed: false,
    playback_observed: false, human_hearing_observed: false,
    provider_id: 'ollama/provider', model_id: 'model/digest',
    model_content_identity: 'digest', original_model_output_sha256: original.sha256,
    accepted_wording_sha256: sha(Buffer.from(words.text)),
    acknowledged_show: { show: { show_id: 'restored-show' } },
    wav_artifact: { wav_sha256: sha(wav), wav_bytes: wav.length },
  };
  const validation = {
    source_commit: 'commit', run_id: 'run', body_id: 'body',
    owner_host_id: 'host', owner_boot_id: 'boot', face_id: 'face',
    face_revision: 14, accepted: true, presenter_play_completed: true,
    original_model_output_sha256: original.sha256, accepted_wording: words.text,
    provider_id: receipt.provider_id, model_id: receipt.model_id,
  };
  const modelValidation = {
    accepted: true, source_commit: 'commit', run_id: 'run', body_id: 'body',
    face_revision: '14', show_id: 'restored-show',
    original_output_sha256: original.sha256, validated_text_sha256: sha(Buffer.from(words.text)),
    provider_id: receipt.provider_id, model_id: receipt.model_id,
  };
  const proof = {
    sourceCommit: 'commit', runId: 'run', bodyId: 'body', ownerHostId: 'host',
    ownerBootId: 'boot', faceId: 'face', faceRevision: '14',
    actionId: 'explain-after-restoration',
    initial: { action_id: 'explain-current-face-llm', provider_id: receipt.provider_id,
      model_id: receipt.model_id, model_content_identity: 'digest' },
    routeLoss: { speech_wav_produced: false }, manifest: { result: 'complete' },
    receipt, original, validation, words, modelValidation,
    modelProof: { model_content_identity: 'digest' }, wav,
  };
  assert.equal(assertRestoredSpeech(proof).model_route_restored, true);
  assert.throws(() => assertRestoredSpeech({ ...proof,
    receipt: { ...receipt, face_revision: 15 } }));
  assert.throws(() => assertRestoredSpeech({ ...proof,
    receipt: { ...receipt, model_content_identity: 'different' } }));
  assert.throws(() => assertRestoredSpeech({ ...proof,
    receipt: { ...receipt, wav_artifact: { ...receipt.wav_artifact, wav_sha256: 'wrong' } } }));
  assert.throws(() => assertRestoredSpeech({ ...proof,
    validation: { ...validation, accepted: false } }));
});
