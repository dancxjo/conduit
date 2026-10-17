// Retain the installed owner's completed speaker Play, not another synthesis.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { constants } from 'node:fs';
import { mkdir, open, writeFile } from 'node:fs/promises';
import path from 'node:path';

const MAX_WAV_BYTES = 30 * 48_000 * 4 + 44;
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

export async function retainOwnerSpeechArtifacts(state, output, batches) {
  assert.ok(Array.isArray(batches) && batches.length > 0 && batches.length <= 64);
  const destination = path.join(output, 'owner-selected-speech');
  await mkdir(destination, { mode: 0o700 });
  const names = new Set();
  const plays = new Set();
  const retained = await Promise.all(batches.map(async batch => {
    const name = batch.wav_artifact_id;
    assert.match(name, /^play-[0-9a-f]{64}\.wav$/);
    assert.equal(names.has(name), false, 'speaker WAV artifact identity repeated');
    assert.equal(plays.has(batch.play_id), false, 'speaker Play identity repeated');
    names.add(name);
    plays.add(batch.play_id);
    assert.match(batch.wav_sha256, /^[0-9a-f]{64}$/);
    assert.match(batch.pcm_sha256, /^[0-9a-f]{64}$/);
    assert.ok(Number.isSafeInteger(batch.wav_bytes) && batch.wav_bytes >= 48
      && batch.wav_bytes <= MAX_WAV_BYTES);
    assert.ok(Number.isSafeInteger(batch.pcm_bytes) && batch.pcm_bytes > 0);
    assert.equal(batch.wav_bytes, batch.pcm_bytes + 44);
    assert.equal(batch.pcm_blocks, batch.speaker_blocks_committed);
    assert.equal(batch.pcm_bytes / 4, batch.speaker_frames_committed);
    const file = await open(path.join(state, 'spoken-artifacts', name),
      constants.O_RDONLY | constants.O_NOFOLLOW);
    let bytes;
    try {
      const stat = await file.stat();
      assert.ok(stat.isFile() && stat.size === batch.wav_bytes);
      bytes = await file.readFile();
    } finally { await file.close(); }
    assert.equal(bytes.length, batch.wav_bytes);
    assert.equal(bytes.toString('ascii', 0, 4), 'RIFF');
    assert.equal(bytes.readUInt32LE(4), bytes.length - 8);
    assert.equal(bytes.toString('ascii', 8, 12), 'WAVE');
    assert.equal(bytes.toString('ascii', 12, 16), 'fmt ');
    assert.equal(bytes.readUInt32LE(16), 16);
    assert.equal(bytes.readUInt16LE(20), 1);
    assert.equal(bytes.readUInt16LE(22), 2);
    assert.equal(bytes.readUInt32LE(24), 48_000);
    assert.equal(bytes.readUInt32LE(28), 192_000);
    assert.equal(bytes.readUInt16LE(32), 4);
    assert.equal(bytes.readUInt16LE(34), 16);
    assert.equal(bytes.toString('ascii', 36, 40), 'data');
    assert.equal(bytes.readUInt32LE(40), batch.pcm_bytes);
    const audible = bytes.subarray(44).some(value => value !== 0);
    assert.equal(sha256(bytes), batch.wav_sha256);
    assert.equal(sha256(bytes.subarray(44)), batch.pcm_sha256);
    const relative = `owner-selected-speech/${name}`;
    await writeFile(path.join(output, relative), bytes, { flag: 'wx', mode: 0o600 });
    return { ...batch, wav: { path: relative, sha256: batch.wav_sha256,
      bytes: batch.wav_bytes, source: 'same-selected-speaker-play', audible } };
  }));
  assert.ok(retained.some(batch => batch.wav.audible),
    'selected speaker completed no audible Play');
  return retained;
}
