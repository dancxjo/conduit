import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { retainSelectedScreenFreePlays } from './screen-free-same-play.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');

async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-screen-free-same-play-'));
  const state = path.join(root, 'owner');
  const pool = path.join(state, 'screen-free-spoken-artifacts');
  const walkthroughRoot = path.join(root, 'public');
  await mkdir(pool, { recursive: true });
  await mkdir(walkthroughRoot);
  const pcm = Buffer.from([1, 0, 0, 0, 2, 0, 0, 0]);
  const wav = Buffer.alloc(44 + pcm.length);
  wav.write('RIFF', 0); wav.writeUInt32LE(wav.length - 8, 4);
  wav.write('WAVE', 8); wav.write('fmt ', 12); wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(48_000, 24); wav.writeUInt32LE(192_000, 28);
  wav.writeUInt16LE(4, 32); wav.writeUInt16LE(32, 34);
  wav.write('data', 36); wav.writeUInt32LE(pcm.length, 40); pcm.copy(wav, 44);
  const locator = path.join(pool, `play-${'a'.repeat(64)}.wav`);
  await writeFile(locator, wav);
  const installation = { release_source_identity: 'source/test', selected_speech: {
    speaker_base_identity: 'speaker/test', card_id: 'Loopback', device: 0,
    provider_sha256: 'b'.repeat(64),
  } };
  const words = 'Review <choices> & birth.';
  const text = Buffer.from(words);
  const segment = { sequence: 0, text: words, text_sha256: digest(text),
    reason_code: 1, face_id: 'face/test', face_revision_decimal: '4',
    show_id: 'show/test', clause_index: null,
    clause_provenance_debug: 'None' };
  const sequence = Buffer.alloc(4);
  const length = Buffer.alloc(4); length.writeUInt32LE(text.length);
  const revision = Buffer.alloc(8); revision.writeBigUInt64LE(4n);
  const noClause = Buffer.alloc(8); noClause.writeBigUInt64LE(0xffff_ffff_ffff_ffffn);
  const sourceDigest = digest(Buffer.concat([
    Buffer.from('conduit.spoken-face/segments@1\0'), sequence, length, text,
    Buffer.from(segment.text_sha256), Buffer.from([1]),
    Buffer.from(segment.face_id), revision, Buffer.from(segment.show_id),
    noClause, Buffer.from(segment.clause_provenance_debug),
  ]));
  const played = { schema: 'conduit.body/spoken-face-playback@1', outcome: 'Completed',
    speaker_lifecycle: 'StoppedClosed', speaker_underruns: 0,
    speaker_frames_committed: 2, speaker_blocks_committed: 1,
    host_id: 'host/test', boot_id: 'boot/test', face_id: 'face/test',
    face_revision_decimal: '4', source_show_id: 'show/test',
    provider_sha256: installation.selected_speech.provider_sha256,
    selected_resource_pool_id: 'std/audio/alsa/speaker/test/card-Loopback/device-0',
    authority_grant_id: 'grant/conduit/installed-screen-free-selected-speaker',
    source_segments_sha256: sourceDigest, spoken_segments: [segment],
    stream_identity: 'stream/test',
    plan_id: 'plan/test', play_id: 'play/test', same_play_capture: {
      wav_artifact_locator: locator, wav_sha256: digest(wav), wav_bytes: wav.length,
      pcm_sha256: digest(pcm), pcm_bytes: pcm.length, pcm_blocks: 1,
    } };
  const turn = { schema: 'conduit.body/spoken-face-turn@1', outcome: 'Completed',
    face_id: played.face_id, face_revision_decimal: played.face_revision_decimal,
    source_show_id: played.source_show_id };
  const output = `${JSON.stringify(played)}\n${JSON.stringify(turn)}\n`;
  const args = { state, privateRoot: root, walkthroughRoot,
    ownerPart: { host_id: 'host/test', boot_id: 'boot/test' }, installation,
    chapters: [{ name: 'birth-review', output, face_id: 'face/test', face_revision: '4' }],
    sessions: [{ name: 'birth', transcript: output }] };
  return { root, wav, args, played };
}

test('publishes only the selected original listener WAV with exact identities', async () => {
  const { root, wav, args } = await fixture();
  try {
    const report = await retainSelectedScreenFreePlays(args);
    assert.equal(report.public_selected_clip_count, 1);
    assert.equal(report.full_private_playback_receipt_count, 1);
    assert.equal(report.selected[0].spoken_words, 'Review <choices> & birth.');
    assert.equal(report.selected[0].spoken_words_html,
      'Review &lt;choices&gt; &amp; birth.');
    assert.deepEqual(await readFile(path.join(args.walkthroughRoot,
      report.selected[0].path)), wav);
    const privateManifest = JSON.parse(await readFile(path.join(root,
      'screen-free-audio-full.private.json'), 'utf8'));
    assert.equal(privateManifest.playback_receipts[0].receipt.play_id, 'play/test');
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('rejects a mismatched provider before publishing a listener WAV', async () => {
  const { root, args, played } = await fixture();
  try {
    played.provider_sha256 = 'd'.repeat(64);
    args.chapters[0].output = `${JSON.stringify(played)}\n${JSON.stringify({
      schema: 'conduit.body/spoken-face-turn@1', outcome: 'Completed',
      face_id: played.face_id, face_revision_decimal: played.face_revision_decimal,
      source_show_id: played.source_show_id })}\n`;
    await assert.rejects(retainSelectedScreenFreePlays(args),
      /Expected values to be strictly equal/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('rejects altered source PCM even when the receipt is still present', async () => {
  const { root, args, played } = await fixture();
  try {
    const file = played.same_play_capture.wav_artifact_locator;
    const altered = await readFile(file);
    altered[44] ^= 1;
    await writeFile(file, altered);
    await assert.rejects(retainSelectedScreenFreePlays(args),
      /Expected values to be strictly equal/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('rejects words that do not match the committed source digest', async () => {
  const { root, args, played } = await fixture();
  try {
    played.spoken_segments[0].text = 'Invented words';
    played.spoken_segments[0].text_sha256 = digest(Buffer.from('Invented words'));
    args.chapters[0].output = `${JSON.stringify(played)}\n${JSON.stringify({
      schema: 'conduit.body/spoken-face-turn@1', outcome: 'Completed',
      face_id: played.face_id, face_revision_decimal: played.face_revision_decimal,
      source_show_id: played.source_show_id })}\n`;
    await assert.rejects(retainSelectedScreenFreePlays(args),
      /published words differ from the committed SpokenBatch source digest/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
