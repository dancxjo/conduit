// Direct speech capture for an installed Owner; actions are issued once.
// This module retains raw results and never declares publication complete.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const MAX_WAV = 16 * 1024 * 1024;

async function selectedWav(state, locator) {
  const name = path.basename(locator);
  assert.match(name, /^play-[a-f0-9]{64}\.wav$/);
  const file = path.join(state, 'spoken-artifacts', name);
  if (path.isAbsolute(locator)) assert.equal(path.resolve(locator), file);
  else assert.equal(locator, name);
  const stat = await lstat(file);
  assert.ok(stat.isFile() && !stat.isSymbolicLink() && stat.size <= MAX_WAV);
  const bytes = await readFile(file);
  assert.equal(bytes.toString('ascii', 0, 4), 'RIFF');
  assert.equal(bytes.toString('ascii', 8, 12), 'WAVE');
  assert.equal(bytes.readUInt32LE(4) + 8, bytes.length);
  // The selected Host's supported artifact profile is fixed stereo i16/48kHz.
  assert.equal(bytes.toString('ascii', 12, 16), 'fmt ');
  assert.equal(bytes.readUInt32LE(16), 16);
  assert.equal(bytes.readUInt16LE(20), 1);
  assert.equal(bytes.readUInt16LE(22), 2);
  assert.equal(bytes.readUInt32LE(24), 48000);
  assert.equal(bytes.readUInt32LE(28), 192000);
  assert.equal(bytes.readUInt16LE(32), 4);
  assert.equal(bytes.readUInt16LE(34), 16);
  assert.equal(bytes.toString('ascii', 36, 40), 'data');
  assert.equal(bytes.readUInt32LE(40), bytes.length - 44);
  const pcm = bytes.subarray(44);
  assert.ok(pcm.length > 0 && pcm.length % 4 === 0);
  return { bytes, pcm, name };
}

export async function captureDirectTodoSpeech({ owner, state, output, bodyId, readRemaining = true }) {
  await mkdir(output, { mode: 0o700 }); // Refuse existing output, preserve failures.
  const invoke = async (label, args) => {
    const value = owner(['body', 'spoken-mask', '--state-dir', state, ...args]);
    await writeFile(path.join(output, `${label}.json`), `${JSON.stringify(value, null, 2)}\n`);
    return value;
  };
  const terminal = async (label, started) => {
    assert.equal(typeof started.operation_id, 'string');
    const deadline = Date.now() + 90000;
    const observations = [];
    while (Date.now() < deadline) {
      const current = await invoke(`${label}-status`, ['status', started.operation_id]);
      assert.equal(current.operation_id, started.operation_id);
      observations.push(current);
      if (current.state !== 'running') {
        await writeFile(path.join(output, `${label}-observations.json`), `${JSON.stringify(observations)}\n`);
        return current;
      }
      await new Promise(resolve => setTimeout(resolve, 100)); // Only read-only observation.
    }
    await writeFile(path.join(output, `${label}-observations.json`), `${JSON.stringify(observations)}\n`);
    throw new Error('Speech observation deadline; inspect the live operation before further actions');
  };
  await invoke('admit', ['admit']);
  await invoke('select', ['select']);
  const selected = owner(['body', 'face', '--state-dir', state, '--json']);
  const installation = JSON.parse(await readFile(path.join(state, 'installation.json')));
  assert.equal(installation.selected_speech.artifact_only, true);
  assert.equal(selected.presentation.basis.body_id, bodyId);
  await writeFile(path.join(output, 'selected-face.json'), `${JSON.stringify(selected)}\n`);
  const opening = await terminal('opening', await invoke('start', ['start']));
  const openingObservedAt = Date.now();
  assert.equal(opening.outcome, 'available');
  assert.equal(opening.mask_artifact_scope, 'opening');
  assert.equal(opening.source_face_id, selected.presentation.identity);
  assert.equal(opening.speaker_played, false);
  assert.equal(opening.artifact.active_play_id, opening.active_play_id);
  assert.equal(typeof opening.artifact.completion_sign_id, 'string');
  assert.ok(opening.artifact.completion_sign_id.length > 0);
  const openingWav = await selectedWav(state, opening.artifact.artifact_locator);
  assert.equal(digest(openingWav.pcm), opening.artifact.content_sha256);
  assert.equal(openingWav.pcm.length, opening.artifact.pcm_bytes);
  await writeFile(path.join(output, 'opening.wav'), openingWav.bytes);
  let remaining = null, remainingObservedAt = null;
  if (readRemaining) {
    remaining = await terminal('remaining', await invoke('read-remaining', ['read-remaining']));
    remainingObservedAt = Date.now();
    assert.equal(remaining.outcome, 'completed');
    assert.equal(remaining.reader_scope, 'remaining-items');
    assert.equal(remaining.output_mode, 'wav-artifact');
    assert.equal(remaining.source_show_id, opening.show_id);
    assert.equal(remaining.source_show_still_current, true);
    assert.equal(remaining.face_id, selected.presentation.identity);
    assert.equal(remaining.face_revision_decimal, selected.presentation_revision_decimal);
    assert.equal(remaining.host_id, selected.advertisement.host_id);
    assert.equal(remaining.boot_id, selected.advertisement.boot_id);
    const disclosures = new Map(selected.presentation.disclosures.map(item => [item.subject, item.level]));
    const expected = selected.presentation.subjects.filter(item => item.role === 'Item'
      && (disclosures.get(item.identity) ?? 'Primary') === 'Primary').map(item => `${item.name}, item.`);
    assert.ok(remaining.batches.length > 0 && remaining.batches.length <= 64);
    assert.deepEqual(remaining.batches.flatMap(batch => batch.spoken_segments), expected);
    assert.equal(remaining.completed_segments, expected.length);
    for (const [index, batch] of remaining.batches.entries()) {
      assert.equal(batch.provider_sha256, installation.selected_speech.provider_sha256);
      // The typed segment digest also binds sequence, Face, Show and commit reason;
      // it is not a hash of this terminal receipt's simplified string array.
      assert.match(batch.source_segments_sha256, /^[a-f0-9]{64}$/);
      assert.equal(batch.outcome, 'completed');
      assert.equal(batch.output_mode, 'wav-artifact');
      assert.equal(batch.speaker_frames_committed, 0);
      assert.equal(batch.speaker_blocks_committed, 0);
      const wav = await selectedWav(state, batch.wav_artifact_id);
      assert.equal(digest(wav.bytes), batch.wav_sha256);
      assert.equal(digest(wav.pcm), batch.pcm_sha256);
      assert.equal(wav.bytes.length, batch.wav_bytes);
      assert.equal(wav.pcm.length, batch.pcm_bytes);
      await writeFile(path.join(output, `remaining-${index}.wav`), wav.bytes);
    }
  }
  const after = owner(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(after.presentation.basis.body_id, bodyId);
  assert.deepEqual(after.presentation.properties.filter(item => item.subject.startsWith('todo/')),
    selected.presentation.properties.filter(item => item.subject.startsWith('todo/')));
  return { opening, remaining, selected_face_id: selected.presentation.identity,
    selected_face_revision: selected.presentation_revision_decimal,
    selected_host_id: selected.advertisement.host_id, selected_boot_id: selected.advertisement.boot_id,
    selected_provider_sha256: installation.selected_speech.provider_sha256,
    selected_voice: installation.selected_speech.voice,
    opening_observed_at_unix_ms: openingObservedAt,
    remaining_observed_at_unix_ms: remainingObservedAt,
    observed_at_unix_ms: Date.now(), physical_playback: false, human_listening: false };
}
