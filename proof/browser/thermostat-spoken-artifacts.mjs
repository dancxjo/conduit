// Retain actual peer artifacts and verify their existing receipts before publication.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstat, readFile, realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

export async function retainSpokenArtifacts(evidence, peerState, output) {
  const reading = evidence.execution;
  const words = Buffer.from(reading.template.wording);
  assert.ok(words.length > 0 && words.length <= 8192);
  assert.equal(reading.completed_wording_bytes, words.length);
  assert.deepEqual(reading.completed_wording_sha256, [...createHash('sha256').update(words).digest()]);
  assert.deepEqual(reading.template.wording_digest, reading.completed_wording_sha256);
  assert.ok(reading.frames.length > 0 && reading.frames.length <= 64);
  const root = await realpath(peerState);
  const plays = new Set();
  const files = [];
  let offset = 0;
  const pcm = [];
  let header;
  for (const [index, frame] of reading.frames.entries()) {
    assert.equal(frame.frame_index, index);
    assert.equal(frame.source_face_id, reading.template.source_face_identity);
    assert.equal(frame.source_show_id, reading.source_show.show_id);
    assert.equal(frame.host_id, reading.source_show.show.host_id);
    assert.equal(frame.boot_id, reading.source_show.show.boot_id);
    assert.ok(!plays.has(frame.speech_play_id)); plays.add(frame.speech_play_id);
    assert.equal(frame.provider_sha256, reading.frames[0].provider_sha256);
    assert.match(frame.provider_sha256, /^[a-f0-9]{64}$/);
    assert.ok(frame.text_bytes > 0 && frame.text_bytes <= 96);
    const text = words.subarray(offset, offset + frame.text_bytes);
    new TextDecoder('utf-8', { fatal: true }).decode(text);
    assert.deepEqual(frame.text_sha256, [...createHash('sha256').update(text).digest()]);
    offset += frame.text_bytes;
    const actual = await realpath(frame.artifact_locator);
    const relative = path.relative(root, actual);
    assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative), 'artifact must belong to the explicitly installed peer');
    const stat = await lstat(frame.artifact_locator);
    assert.ok(stat.isFile() && !stat.isSymbolicLink() && stat.size > 44 && stat.size <= 44 + 30 * 48000 * 4);
    const wav = await readFile(actual);
    assert.equal(wav.length, stat.size);
    assert.equal(wav.subarray(0, 4).toString(), 'RIFF');
    assert.equal(wav.subarray(8, 12).toString(), 'WAVE');
    assert.equal(wav.readUInt16LE(20), 1);
    assert.equal(wav.readUInt16LE(22), 2);
    assert.equal(wav.readUInt32LE(24), 48000);
    assert.equal(wav.readUInt16LE(34), 16);
    assert.equal(wav.subarray(36, 40).toString(), 'data');
    assert.equal(wav.readUInt32LE(40), wav.length - 44);
    assert.equal(frame.pcm_bytes, wav.length - 44);
    assert.ok(['pcm', 'wav'].includes(frame.artifact_digest_scope));
    assert.equal(hash(frame.artifact_digest_scope === 'pcm' ? wav.subarray(44) : wav), frame.artifact_sha256);
    header ??= Buffer.from(wav.subarray(0, 44));
    pcm.push(wav.subarray(44));
    assert.ok(pcm.reduce((size, bytes) => size + bytes.length, 44) <= 16 * 1024 * 1024, 'recorded reading exceeds its finite export bound');
    const name = `spoken-${String(index).padStart(2, '0')}.wav`;
    await writeFile(path.join(output, name), wav, { flag: 'wx' });
    files.push({ frame_index: index, path: `capture/${name}`, bytes: wav.length, sha256: hash(wav) });
  }
  assert.equal(offset, words.length);
  const payload = Buffer.concat(pcm);
  header.writeUInt32LE(payload.length + 36, 4);
  header.writeUInt32LE(payload.length, 40);
  const recording = Buffer.concat([header, payload]);
  await writeFile(path.join(output, 'spoken-reading.wav'), recording, { flag: 'wx' });
  return { frames: files, recording: {
    path: 'capture/spoken-reading.wav', bytes: recording.length, sha256: hash(recording),
    proof_class: 'derived concatenation of the retained ordered PCM frames; no new runtime Play',
    source_frames: files.map(file => file.path),
  } };
}
