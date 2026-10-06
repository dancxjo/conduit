import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { retainOwnerSpeechArtifacts } from './three-host-owner-speech-artifacts.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const name = `play-${'a'.repeat(64)}.wav`;
const pcm = Buffer.alloc(400);
pcm.writeInt16LE(400, 0);
const wav = Buffer.alloc(44 + pcm.length);
wav.write('RIFF', 0); wav.writeUInt32LE(wav.length - 8, 4);
wav.write('WAVEfmt ', 8); wav.writeUInt32LE(16, 16);
wav.writeUInt16LE(1, 20); wav.writeUInt16LE(2, 22);
wav.writeUInt32LE(48_000, 24); wav.writeUInt32LE(192_000, 28);
wav.writeUInt16LE(4, 32); wav.writeUInt16LE(16, 34);
wav.write('data', 36); wav.writeUInt32LE(pcm.length, 40);
pcm.copy(wav, 44);
const batch = { play_id: 'play/one', wav_artifact_id: name, wav_sha256: hash(wav),
  wav_bytes: wav.length, pcm_sha256: hash(pcm), pcm_bytes: pcm.length,
  pcm_blocks: 2, speaker_blocks_committed: 2, speaker_frames_committed: 100 };

test('retains the selected speaker Play bytes and rejects substitution or symlink', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-owner-speech-'));
  try {
    const state = path.join(root, 'state');
    const source = path.join(state, 'spoken-artifacts');
    await mkdir(source, { recursive: true });
    await writeFile(path.join(source, name), wav);
    const output = path.join(root, 'good');
    await mkdir(output);
    const [retained] = await retainOwnerSpeechArtifacts(state, output, [batch]);
    assert.equal(retained.wav.source, 'same-selected-speaker-play');
    assert.deepEqual(await readFile(path.join(output, retained.wav.path)), wav);
    const tampered = path.join(root, 'tampered');
    await mkdir(tampered);
    await assert.rejects(retainOwnerSpeechArtifacts(state, tampered,
      [{ ...batch, wav_sha256: 'b'.repeat(64) }]));
    await rm(path.join(source, name));
    await symlink(path.join(output, retained.wav.path), path.join(source, name));
    const linked = path.join(root, 'symlink');
    await mkdir(linked);
    await assert.rejects(retainOwnerSpeechArtifacts(state, linked, [batch]));
  } finally { await rm(root, { recursive: true, force: true }); }
});
