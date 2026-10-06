// Correlate one real finite Presenter/Mask speech action with a withdrawn
// producer-owned model route. The underlying Ollama daemon remains running.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const json = async file => JSON.parse(await readFile(file));

export function assertExactFaceRevision(browserRevision, receiptRevision) {
  assert.equal(typeof browserRevision, 'string');
  assert.match(browserRevision, /^(0|[1-9][0-9]*)$/);
  assert.ok(Number.isSafeInteger(receiptRevision) && receiptRevision >= 0,
    'numeric Face revision must be an exact safe integer');
  assert.equal(String(receiptRevision), browserRevision);
}

export async function awaitRouteReady(child, deadlineMillis = 5_000) {
  try {
    return await new Promise((resolve, reject) => {
      let output = '';
      const cleanup = () => {
        clearTimeout(timer);
        child.off('error', failed);
        child.off('exit', exited);
        child.stdout.off('data', received);
      };
      const fail = error => { cleanup(); reject(error); };
      const failed = error => fail(error);
      const exited = code => fail(new Error(`local model route exited: ${code}`));
      const received = chunk => {
        output += chunk.toString();
        if (output.includes('\n')) {
          try {
            const endpoint = JSON.parse(output.split('\n')[0]).endpoint;
            assert.match(endpoint, /^http:\/\/127\.0\.0\.1:\d+$/);
            cleanup();
            resolve(endpoint);
          } catch (error) { fail(error); }
        }
      };
      const timer = setTimeout(() => fail(new Error('local model route readiness deadline')),
        deadlineMillis);
      child.once('error', failed);
      child.once('exit', exited);
      child.stdout.on('data', received);
    });
  } catch (error) {
    await stopRoute({ child });
    throw error;
  }
}

async function startRoute(endpoint) {
  const child = spawn(process.execPath,
    [new URL('./local-model-route.mjs', import.meta.url).pathname, endpoint],
    { stdio: ['ignore', 'pipe', 'pipe'] });
  return { child, endpoint: await awaitRouteReady(child) };
}

async function stopRoute(route) {
  if (route.child.exitCode !== null || route.child.signalCode !== null) return;
  const stopped = new Promise(resolve => route.child.once('close', resolve));
  route.child.kill('SIGTERM');
  const force = setTimeout(() => route.child.kill('SIGKILL'), 1_000);
  await stopped;
  clearTimeout(force);
}

export async function captureLlmChapter({ xtask, owner, state, output, sourceCommit, runId,
  bodyId, ownerHostId, ownerBootId, faceId, faceRevision,
  speechExecutable, speechData, speechEngine, speechLanguageCoverage, model, ollamaEndpoint, admittedMemoryMib }) {
  let route = await startRoute(ollamaEndpoint);
  const actionId = 'explain-current-face-llm';
  const args = (directory, action) => [
    'prove', 'one-body-spoken-chapter', '--mode', 'llm-assisted',
    '--conduit-bin', owner, '--state-dir', state, '--output', directory,
    '--run-id', runId, '--action-id', action,
    '--model', model, '--ollama-endpoint', route.endpoint,
    '--admitted-memory-mib', String(admittedMemoryMib),
    '--speech-executable', speechExecutable, '--speech-data', speechData,
    '--speech-engine', speechEngine,
    '--speech-language-coverage', speechLanguageCoverage,
  ];
  try {
    const directory = path.join(output, 'speech-llm');
    const produced = spawnSync(xtask, args(directory, actionId),
      { encoding: 'utf8', timeout: 180_000, maxBuffer: 4 * 1024 * 1024 });
    assert.equal(produced.status, 0, produced.error ?? produced.stderr ?? produced.stdout);
    const [manifestBytes, receiptBytes, originalBytes, validationBytes,
      wordsBytes, modelValidationBytes, modelProofBytes, wav] = await Promise.all([
      'manifest.json', 'speech-receipt.json', 'original-model-output.json',
      'validation.json', 'speech-transcript.json', 'model-validation.json',
      'model-proof.json', 'speech.wav',
    ].map(name => readFile(path.join(directory, name))));
    const [manifest, receipt, original, validation, words, modelValidation, modelProof] =
      [manifestBytes, receiptBytes, originalBytes, validationBytes, wordsBytes,
        modelValidationBytes, modelProofBytes].map(bytes => JSON.parse(bytes));
    assert.equal(manifest.result, 'complete');
    assert.equal(receipt.proof_class, 'live-local-model');
    for (const item of [receipt, validation]) {
      assert.equal(item.source_commit, sourceCommit);
      assert.equal(item.run_id, runId);
      assert.equal(item.body_id, bodyId);
      assert.equal(item.owner_host_id, ownerHostId);
      assert.equal(item.owner_boot_id, ownerBootId);
      assert.equal(item.face_id, faceId);
    }
    assert.equal(receipt.action_id, actionId);
    assertExactFaceRevision(faceRevision, receipt.face_revision);
    assert.equal(receipt.owner_snapshot_before_after_equal, true);
    assert.equal(receipt.local_spoken_mask_show_observed, true);
    assert.equal(receipt.owner_sealed_spoken_mask_route_observed, false);
    assert.equal(receipt.playback_observed, false);
    assert.equal(receipt.human_hearing_observed, false);
    assert.equal(validation.accepted, true);
    assert.equal(validation.presenter_play_completed, true);
    assertExactFaceRevision(faceRevision, validation.face_revision);
    assert.ok(typeof original.output === 'string' && original.output.length > 0);
    assert.equal(original.sha256, digest(Buffer.from(original.output)));
    assert.equal(validation.original_model_output_sha256, original.sha256);
    assert.equal(receipt.original_model_output_sha256, original.sha256);
    assert.equal(words.original_model_output, original.output);
    assert.equal(words.text, validation.accepted_wording);
    assert.equal(words.source_commit, sourceCommit);
    assert.equal(words.run_id, runId);
    assert.equal(words.body_id, bodyId);
    assert.equal(words.face_revision, faceRevision);
    assert.equal(receipt.accepted_wording_sha256, digest(Buffer.from(words.text)));
    assert.equal(modelValidation.accepted, true);
    assert.equal(modelValidation.source_commit, sourceCommit);
    assert.equal(modelValidation.run_id, runId);
    assert.equal(modelValidation.body_id, bodyId);
    assert.equal(modelValidation.face_revision, faceRevision);
    assert.equal(modelValidation.original_output_sha256, original.sha256);
    assert.equal(modelValidation.validated_text_sha256, digest(Buffer.from(words.text)));
    const showId = receipt.acknowledged_show.show.show_id;
    assert.equal(words.show_id, showId);
    assert.equal(modelValidation.show_id, showId);
    for (const item of [validation, receipt, modelValidation]) {
      assert.equal(item.provider_id, validation.provider_id);
      assert.equal(item.model_id, validation.model_id);
    }
    assert.equal(receipt.model_content_identity, modelProof.model_content_identity);
    assert.equal(receipt.wav_artifact.wav_sha256, digest(wav));
    assert.equal(receipt.wav_artifact.wav_bytes, wav.length);
    assert.ok(wav.length > 44 && wav.subarray(0, 4).toString() === 'RIFF');
    const speech = {
      proof_class: receipt.proof_class,
      action_id: actionId,
      provider_id: receipt.provider_id,
      model_id: receipt.model_id,
      model_content_identity: receipt.model_content_identity,
      owner_host_id: ownerHostId,
      owner_boot_id: ownerBootId,
      model_host_id: receipt.model_host_id,
      model_boot_id: receipt.model_boot_id,
      mask_host_id: receipt.mask_host_id,
      mask_boot_id: receipt.mask_boot_id,
      face_id: receipt.face_id,
      show_id: showId,
      validated_text: words.text,
      original_model_output_sha256: original.sha256,
      speech_receipt_sha256: digest(receiptBytes),
      speech_manifest_sha256: digest(manifestBytes),
      wav: { path: 'speech-llm/speech.wav', bytes: wav.length, sha256: digest(wav) },
      records: await Promise.all([
        'original-model-output.json', 'validation.json', 'speech-transcript.json',
        'model-validation.json', 'model-proof.json',
      ].map(async name => ({ path: `speech-llm/${name}`,
        sha256: digest(await readFile(path.join(directory, name))) }))),
    };

    // Withdraw only the route used by this action. The Ollama daemon and
    // model remain available through their original endpoint.
    await stopRoute(route);
    const refusedDir = path.join(output, 'speech-llm-route-loss');
    const refused = spawnSync(xtask, args(refusedDir, 'explain-after-model-route-loss'),
      { encoding: 'utf8', timeout: 20_000, maxBuffer: 2 * 1024 * 1024 });
    assert.notEqual(refused.status, 0, 'withdrawn model route unexpectedly produced speech');
    assert.match(refused.stderr, /bounded command exited with exit status: 7/);
    assert.equal(existsSync(path.join(refusedDir, 'speech.wav')), false);
    const current = spawnSync(owner, ['body', 'face', '--state-dir', state, '--json'],
      { encoding: 'utf8', timeout: 10_000 });
    assert.equal(current.status, 0, current.stderr);
    const face = JSON.parse(current.stdout);
    assert.equal(face.presentation.identity, faceId);
    assertExactFaceRevision(faceRevision, face.presentation.revision);
    assert.equal(face.presentation.basis.body_id, bodyId);
    assert.equal(face.advertisement.host_id, ownerHostId);
    assert.equal(face.advertisement.boot_id, ownerBootId);
    const refusalText = `${refused.stdout}${refused.stderr}`;
    await writeFile(path.join(output, 'model-route-loss.txt'), refusalText);
    const routeLoss = {
      proof_class: 'configured-local-model-route-withdrawal-refusal',
      action_id: 'explain-after-model-route-loss',
      configured_endpoint: route.endpoint,
      underlying_ollama_stopped: false,
      owner_face_unchanged: true,
      speech_wav_produced: false,
      exit_status: refused.status,
      transcript: { path: 'model-route-loss.txt', sha256: digest(Buffer.from(refusalText)) },
    };
    route = await startRoute(ollamaEndpoint);
    const restoredDir = path.join(output, 'speech-llm-restored');
    const restoredActionId = 'explain-after-model-route-restoration';
    const restored = spawnSync(xtask, args(restoredDir, restoredActionId),
      { encoding: 'utf8', timeout: 180_000, maxBuffer: 4 * 1024 * 1024 });
    assert.equal(restored.status, 0, restored.error ?? restored.stderr ?? restored.stdout);
    const [restoredManifestBytes, restoredReceiptBytes, restoredOriginalBytes,
      restoredValidationBytes, restoredWordsBytes, restoredModelValidationBytes,
      restoredModelProofBytes, restoredWav] = await Promise.all([
      'manifest.json', 'speech-receipt.json', 'original-model-output.json',
      'validation.json', 'speech-transcript.json', 'model-validation.json',
      'model-proof.json', 'speech.wav',
    ].map(name => readFile(path.join(restoredDir, name))));
    const [restoredManifest, restoredReceipt, restoredOriginal, restoredValidation,
      restoredWords, restoredModelValidation, restoredModelProof] = [
      restoredManifestBytes, restoredReceiptBytes, restoredOriginalBytes,
      restoredValidationBytes, restoredWordsBytes, restoredModelValidationBytes,
      restoredModelProofBytes,
    ].map(bytes => JSON.parse(bytes));
    const restoration = assertRestoredSpeech({
      sourceCommit, runId, bodyId, ownerHostId, ownerBootId, faceId, faceRevision,
      actionId: restoredActionId, initial: receipt, routeLoss,
      manifest: restoredManifest, receipt: restoredReceipt, original: restoredOriginal,
      validation: restoredValidation, words: restoredWords,
      modelValidation: restoredModelValidation, modelProof: restoredModelProof,
      wav: restoredWav,
    });
    return {
      speech, routeLoss,
      restoration: {
        ...restoration,
        configured_endpoint: route.endpoint,
        speech_receipt_sha256: digest(restoredReceiptBytes),
        speech_manifest_sha256: digest(restoredManifestBytes),
        wav: { path: 'speech-llm-restored/speech.wav', bytes: restoredWav.length,
          sha256: digest(restoredWav) },
      },
    };
  } finally {
    await stopRoute(route);
  }
}

export function assertRestoredSpeech({ sourceCommit, runId, bodyId, ownerHostId,
  ownerBootId, faceId, faceRevision, actionId, initial, routeLoss, manifest,
  receipt, original, validation, words, modelValidation, modelProof, wav }) {
  assert.equal(routeLoss.speech_wav_produced, false);
  assert.equal(manifest.result, 'complete');
  assert.equal(receipt.proof_class, 'live-local-model');
  for (const item of [receipt, validation]) {
    assert.equal(item.source_commit, sourceCommit);
    assert.equal(item.run_id, runId);
    assert.equal(item.body_id, bodyId);
    assert.equal(item.owner_host_id, ownerHostId);
    assert.equal(item.owner_boot_id, ownerBootId);
    assert.equal(item.face_id, faceId);
    assertExactFaceRevision(faceRevision, item.face_revision);
  }
  assert.equal(receipt.action_id, actionId);
  assert.notEqual(receipt.action_id, initial.action_id);
  assert.equal(receipt.owner_snapshot_before_after_equal, true);
  assert.equal(receipt.local_spoken_mask_show_observed, true);
  assert.equal(receipt.owner_sealed_spoken_mask_route_observed, false);
  assert.equal(receipt.playback_observed, false);
  assert.equal(receipt.human_hearing_observed, false);
  assert.equal(receipt.provider_id, initial.provider_id);
  assert.equal(receipt.model_id, initial.model_id);
  assert.equal(receipt.model_content_identity, initial.model_content_identity);
  assert.equal(receipt.model_content_identity, modelProof.model_content_identity);
  assert.equal(validation.accepted, true);
  assert.equal(validation.presenter_play_completed, true);
  assert.ok(typeof original.output === 'string' && original.output.length > 0);
  assert.equal(original.sha256, digest(Buffer.from(original.output)));
  assert.equal(validation.original_model_output_sha256, original.sha256);
  assert.equal(receipt.original_model_output_sha256, original.sha256);
  assert.equal(words.original_model_output, original.output);
  assert.equal(words.text, validation.accepted_wording);
  assert.equal(words.source_commit, sourceCommit);
  assert.equal(words.run_id, runId);
  assert.equal(words.body_id, bodyId);
  assert.equal(words.face_revision, faceRevision);
  assert.equal(receipt.accepted_wording_sha256, digest(Buffer.from(words.text)));
  const showId = receipt.acknowledged_show.show.show_id;
  assert.equal(words.show_id, showId);
  assert.equal(modelValidation.show_id, showId);
  assert.equal(modelValidation.accepted, true);
  assert.equal(modelValidation.source_commit, sourceCommit);
  assert.equal(modelValidation.run_id, runId);
  assert.equal(modelValidation.body_id, bodyId);
  assert.equal(modelValidation.face_revision, faceRevision);
  assert.equal(modelValidation.original_output_sha256, original.sha256);
  assert.equal(modelValidation.validated_text_sha256, digest(Buffer.from(words.text)));
  for (const item of [validation, receipt, modelValidation]) {
    assert.equal(item.provider_id, initial.provider_id);
    assert.equal(item.model_id, initial.model_id);
  }
  assert.equal(receipt.wav_artifact.wav_sha256, digest(wav));
  assert.equal(receipt.wav_artifact.wav_bytes, wav.length);
  assert.ok(wav.length > 44 && wav.subarray(0, 4).toString() === 'RIFF');
  return { action_id: actionId, proof_class: receipt.proof_class,
    show_id: showId, validated_text: words.text,
    model_content_identity: receipt.model_content_identity,
    original_model_output_sha256: original.sha256,
    owner_face_unchanged: true, model_route_restored: true,
    playback_observed: false, human_hearing_observed: false };
}
