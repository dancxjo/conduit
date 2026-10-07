// Capture the installed owner's selected model Mask and its listener audio.
// The Mask's first artifact Play is distinct from the selected speaker Play.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFile, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { ownerModelRouteControl } from './owner-model-route-control.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

export function assertModelLossFaceBoundary({ beforeRefusal, afterRefusal,
  afterCheckpoint, checkpoint, bodyId, routeId }) {
  assert.equal(beforeRefusal.presentation.basis.body_id, bodyId);
  assert.equal(afterRefusal.presentation.identity, beforeRefusal.presentation.identity,
    'refused Start cannot change the owner Face');
  assert.equal(afterRefusal.presentation_revision_decimal,
    beforeRefusal.presentation_revision_decimal);
  assert.equal(afterCheckpoint.presentation.basis.body_id, bodyId);
  assert.ok(BigInt(afterCheckpoint.presentation_revision_decimal) >=
    BigInt(afterRefusal.presentation_revision_decimal),
  'the checkpoint cannot move the owner Face revision backward');
  if (checkpoint) {
    assert.equal(checkpoint.body_id, bodyId);
    assert.equal(checkpoint.route_id, routeId);
    assert.equal(checkpoint.route_available, false);
    assert.ok(BigInt(afterCheckpoint.presentation_revision_decimal) >=
      BigInt(checkpoint.face_revision),
    'owner Face cannot precede the completed screen-free checkpoint');
    if (afterCheckpoint.presentation_revision_decimal === checkpoint.face_revision) {
      assert.equal(afterCheckpoint.presentation.identity, checkpoint.face_id,
        'owner Face must match the completed checkpoint at the same revision');
    }
  }
}

export async function captureOwnerLlmSpeaker({ owner, state, output, installation,
  bodyId, runId, sourceCommit, model, directoryName = 'owner-llm-selected' }) {
  assert.equal(installation.selected_model?.model_name, model,
    'installed owner must select the reviewed local model before this Boot');
  assert.equal(installation.release_source_identity, sourceCommit);
  const command = (verb, ...extra) => owner(['body', 'spoken-mask', '--state-dir', state,
    verb, '--llm', ...extra]);
  const admitted = command('admit');
  const selected = command('select');
  assert.equal(admitted.schema, 'conduit.body/llm-spoken-route@1');
  assert.equal(selected.schema, admitted.schema);
  assert.equal(selected.selected, true);
  const before = owner(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(before.presentation.basis.body_id, bodyId);
  const started = command('start');
  assert.equal(started.schema, 'conduit.body/llm-spoken-start@1');
  assert.equal(started.state, 'running');
  assert.ok(started.operation_id);
  let terminal;
  for (let attempt = 0; attempt < 1800; attempt += 1) {
    const status = command('status', started.operation_id);
    if (status.schema === 'conduit.body/owner-spoken-terminal@1') {
      terminal = status;
      break;
    }
    assert.equal(status.schema, 'conduit.body/owner-spoken-status@1');
    assert.equal(status.state, 'running');
    await pause(100);
  }
  assert.ok(terminal, 'owner LLM spoken Mask did not reach a terminal result');
  assert.equal(terminal.outcome, 'available', terminal.detail);
  assert.equal(terminal.mode, 'llm-assisted');
  assert.equal(terminal.speaker_played, true);
  assert.equal(terminal.route_plan_id, selected.route_plan_id);
  assert.equal(terminal.source_face_id, before.presentation.identity);
  const generation = terminal.generation_evidence;
  assert.ok(generation?.provider_identity && generation.model_identity,
    'selected model Show lost its provider and model identities');
  assert.ok(generation.candidate_digest && generation.validation_receipt_identity,
    'selected model Show lost its same-generation validation identity');
  assert.ok(generation.original_model_output &&
    Buffer.byteLength(generation.original_model_output) <= 2048,
  'selected model Show lost its bounded original model output');
  const played = terminal.speaker_playback;
  assert.equal(played.schema, 'conduit.body/owner-spoken-speaker-play@1');
  assert.equal(played.outcome, 'completed');
  assert.equal(played.source_show_id, terminal.show_id);
  assert.equal(played.source_face_id, before.presentation.identity);
  assert.equal(played.source_face_revision_decimal, before.presentation_revision_decimal);
  assert.equal(played.play_id !== terminal.active_play_id, true,
    'model artifact and listener audio must retain distinct Play identities');
  assert.equal(played.spoken_segments.join(''), terminal.accepted_wording);
  assert.equal(played.pcm_blocks, played.speaker_blocks_committed);
  assert.equal(played.pcm_bytes / 4, played.speaker_frames_committed);
  assert.ok(played.pcm_blocks > 0 && played.speaker_frames_committed > 0);
  const root = path.join(state, 'spoken-artifacts');
  const source = path.resolve(played.wav_artifact_locator);
  assert.equal(path.dirname(source), root, 'owner WAV must be retained beneath this installation');
  assert.match(path.basename(source), /^play-[0-9a-f]{64}\.wav$/);
  const wav = await readFile(source);
  assert.equal(wav.subarray(0, 4).toString(), 'RIFF');
  assert.equal(wav.subarray(8, 12).toString(), 'WAVE');
  assert.equal(wav.length, played.wav_bytes);
  assert.equal(digest(wav), played.wav_sha256);
  assert.equal(wav.length - 44, played.pcm_bytes);
  assert.equal(digest(wav.subarray(44)), played.pcm_sha256);
  assert.ok(wav.subarray(44).some(byte => byte !== 0), 'model listener WAV is silent');
  const after = owner(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(after.presentation.basis.body_id, bodyId);
  assert.equal(after.presentation.identity, before.presentation.identity,
    'spoken Mask may not invent a new Face');
  const directory = path.join(output, directoryName);
  await mkdir(directory, { mode: 0o700 });
  const destination = path.join(directory, path.basename(source));
  await copyFile(source, destination);
  const terminalBytes = Buffer.from(`${JSON.stringify(terminal, null, 2)}\n`);
  await writeFile(path.join(directory, 'terminal.json'), terminalBytes);
  const record = {
    proof_class: 'installed-owner-selected-model-and-same-play-speaker',
    source_commit: sourceCommit, run_id: runId, body_id: bodyId,
    face_id: before.presentation.identity,
    face_revision_decimal: before.presentation_revision_decimal,
    model_name: model,
    model_content_identity:
      installation.selected_model.reviewed_offer.identity.model_content_identity,
    route_plan_id: selected.route_plan_id,
    show_id: terminal.show_id,
    model_artifact_play_id: terminal.active_play_id,
    listener_plan_id: played.plan_id, listener_play_id: played.play_id,
    accepted_wording: terminal.accepted_wording,
    provider_identity: generation.provider_identity,
    model_identity: generation.model_identity,
    candidate_digest: generation.candidate_digest,
    validation_receipt_identity: generation.validation_receipt_identity,
    original_model_output_sha256: digest(Buffer.from(generation.original_model_output)),
    terminal: { path: `${directoryName}/terminal.json`, sha256: digest(terminalBytes) },
    wav: { path: `${directoryName}/${path.basename(source)}`,
      bytes: wav.length, sha256: digest(wav) },
    physical_hearing_observed: false,
  };
  return record;
}

export async function captureOwnerModelRouteLoss({ owner, state, output, installation,
  bodyId, runId, sourceCommit, model, controlSocket, successful,
  observeWardrobe }) {
  const endpoint = installation.selected_model?.endpoint;
  assert.ok(endpoint && successful?.route_plan_id,
    'owner route loss requires a completed owner-selected model Show');
  const route = action => ownerModelRouteControl(controlSocket, action, endpoint);
  assert.equal((await route('status')).state, 'available');
  const routeId = `route/${successful.route_plan_id}`;
  const { wardrobe: availableWardrobe } = await observeWardrobe(routeId, true);
  const before = owner(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(before.presentation.basis.body_id, bodyId);
  const artifactDir = path.join(state, 'spoken-artifacts');
  const artifactsBefore = (await readdir(artifactDir)).sort();
  let refused, unavailableWardrobe;
  await route('withdraw');
  try {
    const beforeRefusal = owner(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(beforeRefusal.presentation.basis.body_id, bodyId);
    const attempted = spawnSync(installation.product_executable,
      ['body', 'spoken-mask', '--state-dir', state, 'start', '--llm'],
      { encoding: 'utf8', timeout: 10_000 });
    const afterRefusal = owner(['body', 'face', '--state-dir', state, '--json']);
    assert.notEqual(attempted.status, 0,
      'owner unexpectedly started a model Mask through the withdrawn provider');
    assert.match(attempted.stderr,
      /LLM spoken route has no current model\/voice witness/,
      'owner refusal must come from its current model route witness');
    assert.equal(attempted.stdout.trim(), '', 'a refused Start has no operation receipt');
    refused = { exit_status: attempted.status,
      stderr: attempted.stderr, stdout: attempted.stdout,
      selected_route_plan_id: successful.route_plan_id,
      operation_started: false,
      face_id_before: beforeRefusal.presentation.identity,
      face_id_after: afterRefusal.presentation.identity,
      face_revision_before: beforeRefusal.presentation_revision_decimal,
      face_revision_after: afterRefusal.presentation_revision_decimal };
    assert.equal((await route('status')).state, 'withdrawn');
    const unavailableObservation = await observeWardrobe(routeId, false);
    unavailableWardrobe = unavailableObservation.wardrobe;
    assert.equal(unavailableWardrobe.body_id, bodyId);
    const after = owner(['body', 'face', '--state-dir', state, '--json']);
    const checkpoint = unavailableObservation.checkpoint?.resume;
    assertModelLossFaceBoundary({ beforeRefusal, afterRefusal,
      afterCheckpoint: after, checkpoint, bodyId, routeId });
    refused.face_id_after_checkpoint = after.presentation.identity;
    refused.face_revision_after_checkpoint = after.presentation_revision_decimal;
    refused.face_advanced_during_checkpoint =
      after.presentation.identity !== afterRefusal.presentation.identity;
    assert.deepEqual((await readdir(artifactDir)).sort(), artifactsBefore,
      'failed model Play must not produce a listener WAV');
  } finally {
    assert.equal((await route('restore')).state, 'available');
  }
  const restored = await captureOwnerLlmSpeaker({ owner, state, output, installation,
    bodyId, runId, sourceCommit, model, directoryName: 'owner-llm-restored' });
  const { wardrobe: restoredWardrobe } =
    await observeWardrobe(`route/${restored.route_plan_id}`, true);
  assert.equal(restoredWardrobe.body_id, bodyId);
  assert.ok(BigInt(restored.face_revision_decimal) >= BigInt(successful.face_revision_decimal),
    'restored model speech cannot use an older owner Face');
  assert.notEqual(restored.model_artifact_play_id, successful.model_artifact_play_id);
  assert.notEqual(restored.listener_play_id, successful.listener_play_id);
  const refusalBytes = Buffer.from(`${JSON.stringify(refused, null, 2)}\n`);
  await writeFile(path.join(output, 'owner-llm-route-loss.json'), refusalBytes);
  const wardrobeBytes = Buffer.from(`${JSON.stringify({ available: availableWardrobe,
    unavailable: unavailableWardrobe, restored: restoredWardrobe }, null, 2)}\n`);
  await writeFile(path.join(output, 'owner-llm-route-wardrobe.json'), wardrobeBytes);
  return {
    proof_class: 'installed-owner-selected-model-route-withdrawal-and-restoration',
    source_commit: sourceCommit, run_id: runId, body_id: bodyId,
    selected_endpoint: endpoint, route_plan_id: successful.route_plan_id,
    refusal_exit_status: refused.exit_status,
    refusal_detail: refused.stderr, operation_started_on_loss: false,
    owner_face_unchanged: true,
    face_advanced_during_checkpoint: refused.face_advanced_during_checkpoint,
    face_id_after_checkpoint: refused.face_id_after_checkpoint,
    face_revision_after_checkpoint: refused.face_revision_after_checkpoint,
    new_listener_wav_on_failure: false,
    upstream_service_termination_requested: false,
    refusal: { path: 'owner-llm-route-loss.json', sha256: digest(refusalBytes) },
    wardrobe: { path: 'owner-llm-route-wardrobe.json',
      sha256: digest(wardrobeBytes) },
    restored,
  };
}
