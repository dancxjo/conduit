// Publish a small, phase-selected sample of the exact PCM committed to the
// listener's selected speaker. Full playback receipts stay in the private run.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const MAX_PUBLIC_CLIPS = 16;
const MAX_CLIP_BYTES = 4 * 1024 * 1024;
const MAX_PUBLIC_BYTES = 64 * 1024 * 1024;
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const escapeHtml = value => value.replace(/[&<>"']/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
})[character]);

function verifiedWords(played) {
  const segments = played.spoken_segments;
  assert.ok(Array.isArray(segments) && segments.length === 1,
    'selected Birth speaker Play needs its one bounded ordered SpokenBatch segment');
  const hash = createHash('sha256');
  hash.update(Buffer.from('conduit.spoken-face/segments@1\0'));
  const words = [];
  for (const [index, segment] of segments.entries()) {
    assert.equal(segment.sequence, index);
    assert.equal(typeof segment.text, 'string');
    const textBytes = Buffer.from(segment.text, 'utf8');
    assert.ok(textBytes.length > 0 && textBytes.length <= 64,
      'selected segment exceeds the bounded spoken input');
    assert.ok(hex(segment.text_sha256));
    assert.equal(digest(textBytes), segment.text_sha256);
    assert.equal(segment.reason_code, 1);
    assert.equal(segment.face_id, played.face_id);
    assert.equal(segment.face_revision_decimal, played.face_revision_decimal);
    assert.equal(segment.show_id, played.source_show_id);
    assert.ok(segment.clause_index === null ||
      (Number.isSafeInteger(segment.clause_index) && segment.clause_index >= 0));
    assert.equal(typeof segment.clause_provenance_debug, 'string');
    assert.ok(segment.clause_provenance_debug.length > 0 &&
      Buffer.byteLength(segment.clause_provenance_debug) <= 4096);
    const sequence = Buffer.alloc(4);
    sequence.writeUInt32LE(index);
    const length = Buffer.alloc(4);
    length.writeUInt32LE(textBytes.length);
    const faceRevision = Buffer.alloc(8);
    faceRevision.writeBigUInt64LE(BigInt(segment.face_revision_decimal));
    const clauseIndex = Buffer.alloc(8);
    clauseIndex.writeBigUInt64LE(segment.clause_index === null
      ? 0xffff_ffff_ffff_ffffn : BigInt(segment.clause_index));
    hash.update(sequence);
    hash.update(length);
    hash.update(textBytes);
    hash.update(Buffer.from(segment.text_sha256));
    hash.update(Buffer.from([segment.reason_code]));
    hash.update(Buffer.from(segment.face_id));
    hash.update(faceRevision);
    hash.update(Buffer.from(segment.show_id));
    hash.update(clauseIndex);
    hash.update(Buffer.from(segment.clause_provenance_debug));
    words.push(segment.text);
  }
  assert.equal(hash.digest('hex'), played.source_segments_sha256,
    'published words differ from the committed SpokenBatch source digest');
  const text = words.join('');
  return { spoken_words: text, spoken_words_html: escapeHtml(text),
    spoken_segment_sha256: segments.map(segment => segment.text_sha256) };
}

export function playbackReceipts(output) {
  return output.split('\n').flatMap(line => {
    const start = line.indexOf('{"');
    if (start < 0) return [];
    try {
      const receipt = JSON.parse(line.slice(start));
      return receipt.schema === 'conduit.body/spoken-face-playback@1' ? [receipt] : [];
    } catch { return []; }
  });
}

const turns = output => output.split('\n').flatMap(line => {
  const start = line.indexOf('{"');
  if (start < 0) return [];
  try {
    const receipt = JSON.parse(line.slice(start));
    return receipt.schema === 'conduit.body/spoken-face-turn@1' ? [receipt] : [];
  } catch { return []; }
});

function selectedReceipt(played, chapter, ownerPart, installation, turn) {
  const selected = installation.selected_speech;
  assert.ok(selected, 'selected WAV needs installed selected speaker equipment');
  assert.equal(played.outcome, 'Completed');
  assert.equal(played.speaker_lifecycle, 'StoppedClosed');
  assert.equal(played.speaker_underruns, 0);
  assert.ok(played.speaker_frames_committed > 0 && played.speaker_blocks_committed > 0);
  assert.equal(played.host_id, ownerPart.host_id);
  assert.equal(played.boot_id, ownerPart.boot_id);
  assert.equal(played.provider_sha256, selected.provider_sha256);
  assert.equal(played.selected_resource_pool_id,
    `std/audio/alsa/${selected.speaker_base_identity}/card-${selected.card_id}/device-${selected.device}`);
  assert.equal(played.authority_grant_id,
    'grant/conduit/installed-screen-free-selected-speaker');
  assert.ok(played.plan_id && played.play_id && played.stream_identity);
  assert.ok(hex(played.source_segments_sha256));
  assert.equal(played.face_id, turn.face_id);
  assert.equal(played.face_revision_decimal, turn.face_revision_decimal);
  assert.equal(played.source_show_id, turn.source_show_id);
  if (chapter.face_id) assert.equal(played.face_id, chapter.face_id);
  if (chapter.face_revision) assert.equal(played.face_revision_decimal, chapter.face_revision);
  const capture = played.same_play_capture;
  assert.ok(capture && hex(capture.wav_sha256) && hex(capture.pcm_sha256),
    `${chapter.name} selected Play lacks a completed same-Play capture`);
  assert.ok(Number.isSafeInteger(capture.wav_bytes) && capture.wav_bytes > 44 &&
    capture.wav_bytes <= MAX_CLIP_BYTES);
  assert.ok(Number.isSafeInteger(capture.pcm_bytes) && capture.pcm_bytes > 0);
  assert.equal(capture.wav_bytes, capture.pcm_bytes + 44);
  assert.equal(capture.pcm_blocks, played.speaker_blocks_committed);
  assert.equal(capture.pcm_bytes / 4, played.speaker_frames_committed);
  return { capture, words: verifiedWords(played) };
}

export async function retainSelectedScreenFreePlays({ state, privateRoot, walkthroughRoot,
  ownerPart, installation, chapters, sessions }) {
  assert.ok(chapters.length > 0 && chapters.length <= MAX_PUBLIC_CLIPS);
  const complete = sessions.flatMap(session => playbackReceipts(session.transcript)
    .map(receipt => ({ session: session.name, receipt })));
  const fullManifest = { schema: 'conduit.proof/screen-free-full-playback-private@1',
    source_commit: installation.release_source_identity,
    note: 'Private complete receipt manifest; public audio is a deterministic selected subset.',
    playback_receipts: complete };
  await writeFile(path.join(privateRoot, 'screen-free-audio-full.private.json'),
    `${JSON.stringify(fullManifest, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  const audioRoot = path.join(walkthroughRoot, 'screen-free-audio');
  await mkdir(audioRoot, { mode: 0o700 });
  const published = [];
  const publishedPlays = new Set();
  let publicBytes = 0;
  for (const chapter of chapters) {
    assert.match(chapter.name, /^[a-z][a-z0-9-]+$/);
    const ended = turns(chapter.output).filter(turn => turn.outcome === 'Completed' &&
      (!chapter.face_id || turn.face_id === chapter.face_id) &&
      (!chapter.face_revision || turn.face_revision_decimal === chapter.face_revision));
    const candidates = playbackReceipts(chapter.output).filter(play => ended.some(turn =>
      play.face_id === turn.face_id &&
      play.face_revision_decimal === turn.face_revision_decimal &&
      play.source_show_id === turn.source_show_id));
    assert.ok(candidates.length > 0, `${chapter.name} has no completed correlated speaker Play`);
    for (const played of candidates) {
      assert.ok(complete.some(item => item.receipt.play_id === played.play_id &&
        item.receipt.plan_id === played.plan_id &&
        item.receipt.same_play_capture?.wav_sha256 === played.same_play_capture?.wav_sha256),
      `${chapter.name} selected Play is absent from the full private receipt manifest`);
    }
    const eligible = candidates.filter(play => play.same_play_capture &&
      Number.isSafeInteger(play.same_play_capture.wav_bytes) &&
      play.same_play_capture.wav_bytes <= MAX_CLIP_BYTES);
    assert.ok(eligible.length > 0,
      `${chapter.name} has no completed same-Play WAV within the 4 MiB public clip bound`);
    // One original clip per action, plus a closing clip for the long Birth
    // review/result. This is a representative excerpt, never the whole turn.
    const wanted = chapter.includeLast && eligible.length > 1
      ? [eligible[0], eligible.at(-1)] : [eligible[0]];
    for (const [index, played] of wanted.entries()) {
      assert.ok(!publishedPlays.has(played.play_id),
        `${chapter.name} repeated a selected listener Play`);
      publishedPlays.add(played.play_id);
      const turn = ended.find(item => played.face_id === item.face_id &&
        played.face_revision_decimal === item.face_revision_decimal &&
        played.source_show_id === item.source_show_id);
      const { capture, words } = selectedReceipt(played, chapter, ownerPart, installation, turn);
      const sourceRoot = path.join(state, 'screen-free-spoken-artifacts');
      const sourceName = path.basename(capture.wav_artifact_locator);
      assert.match(sourceName, /^play-[0-9a-f]{64}\.wav$/);
      assert.equal(path.resolve(capture.wav_artifact_locator), path.join(sourceRoot, sourceName),
        'selected WAV escaped the installed speaker artifact pool');
      const wav = await readFile(capture.wav_artifact_locator);
      assert.equal(wav.length, capture.wav_bytes);
      assert.equal(digest(wav), capture.wav_sha256);
      assert.equal(wav.subarray(0, 4).toString(), 'RIFF');
      assert.equal(wav.readUInt32LE(4), wav.length - 8);
      assert.equal(wav.subarray(8, 12).toString(), 'WAVE');
      assert.equal(wav.subarray(36, 40).toString(), 'data');
      assert.equal(wav.readUInt32LE(40), capture.pcm_bytes);
      assert.equal(digest(wav.subarray(44)), capture.pcm_sha256);
      publicBytes += wav.length;
      assert.ok(publicBytes <= MAX_PUBLIC_BYTES, 'selected public speaker clips exceed 64 MiB');
      const name = `${chapter.name}${index ? '-closing' : ''}.wav`;
      await writeFile(path.join(audioRoot, name), wav, { flag: 'wx', mode: 0o600 });
      published.push({ moment: chapter.name, excerpt: index ? 'closing' : 'opening',
        path: `screen-free-audio/${name}`, bytes: wav.length,
        wav_sha256: capture.wav_sha256, pcm_sha256: capture.pcm_sha256,
        pcm_bytes: capture.pcm_bytes, pcm_blocks: capture.pcm_blocks,
        face_id: played.face_id, face_revision: played.face_revision_decimal,
        source_show_id: played.source_show_id, stream_identity: played.stream_identity,
        source_segments_sha256: played.source_segments_sha256,
        ...words,
        provider_sha256: played.provider_sha256, selected_resource_pool_id: played.selected_resource_pool_id,
        authority_grant_id: played.authority_grant_id,
        plan_id: played.plan_id, play_id: played.play_id,
        host_id: played.host_id, boot_id: played.boot_id,
        speaker_frames_committed: played.speaker_frames_committed });
    }
  }
  assert.ok(published.length <= MAX_PUBLIC_CLIPS);
  return { proof_class: 'selected-screen-free-same-play-listener-audio-subset',
    source_commit: installation.release_source_identity,
    full_private_playback_receipt_count: complete.length,
    public_selected_clip_count: published.length,
    public_selected_clip_bytes: publicBytes,
    selection: 'Opening completed Play for each named human action and held loss/recovery observation; closing Play also for Birth review and explicit Birth result. Complete receipt manifest remains private.',
    human_hearing_observed: false, selected: published };
}
