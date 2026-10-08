import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { deflateSync } from 'node:zlib';
import { renderTodoJourney, validateTodoJourney } from '../../tools/ci/pipeline/todo-journey.mjs';

const commit = 'a'.repeat(40);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const chapters = ['birth', 'add', 'join', 'complete', 'inspect', 'hear', 'read', 'recover'];
const base = { source_commit: commit, run_id: 'run/todo-1', body_id: 'body/todo-1' };
function crc32(bytes) {
  let value = 0xffffffff;
  for (const byte of bytes) {
    value ^= byte;
    for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ ((value & 1) ? 0xedb88320 : 0);
  }
  return (value ^ 0xffffffff) >>> 0;
}
function pngChunk(kind, bytes) {
  const name = Buffer.from(kind);
  const size = Buffer.alloc(4); size.writeUInt32BE(bytes.length);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([name, bytes])));
  return Buffer.concat([size, name, bytes, crc]);
}
function fixturePng() {
  const width = 640; const height = 360;
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width); header.writeUInt32BE(height, 4);
  header[8] = 8; header[9] = 2;
  const rows = Buffer.alloc(height * (1 + width * 3));
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const at = y * (1 + width * 3) + 1 + x * 3;
    rows[at] = x < width / 2 ? 38 : 217;
    rows[at + 1] = y < height / 2 ? 68 : 141;
    rows[at + 2] = 105;
  }
  return Buffer.concat([Buffer.from('89504e470d0a1a0a', 'hex'),
    pngChunk('IHDR', header), pngChunk('IDAT', deflateSync(rows)), pngChunk('IEND', Buffer.alloc(0))]);
}
const png = fixturePng();
function wav() {
  const pcm = Buffer.from([0, 0, 1, 0]);
  const bytes = Buffer.alloc(44 + pcm.length);
  bytes.write('RIFF'); bytes.writeUInt32LE(bytes.length - 8, 4); bytes.write('WAVE', 8);
  bytes.write('fmt ', 12); bytes.writeUInt32LE(16, 16); bytes.writeUInt16LE(1, 20);
  bytes.writeUInt16LE(1, 22); bytes.writeUInt32LE(16000, 24); bytes.writeUInt32LE(32000, 28);
  bytes.writeUInt16LE(2, 32); bytes.writeUInt16LE(16, 34);
  bytes.write('data', 36); bytes.writeUInt32LE(pcm.length, 40); pcm.copy(bytes, 44);
  return bytes;
}

function fixture({ guestAudio = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'todo-journey-test-'));
  const outputs = [];
  const add = (id, kind, bytes, media_type, name = `${id}.json`) => {
    const data = Buffer.isBuffer(bytes) ? bytes : Buffer.from(JSON.stringify(bytes));
    const file = path.join(root, name);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, data);
    outputs.push({ id, kind, path: name, media_type, required: true, bytes: data.length,
      sha256: hash(data), scenario_id: base.run_id, proof_class: 'producer-correlated-live-local' });
    return outputs.at(-1);
  };
  const story = [];
  const kinds = { birth: ['terminal'], add: ['terminal'], join: ['chromium', 'qmp', 'native'],
    complete: ['qmp'], inspect: ['chromium'], hear: [guestAudio ? 'qemu-audio' : 'speaker-play'],
    read: ['speaker-play'], recover: ['terminal'] };
  for (const [index, id] of chapters.entries()) {
    const event = { ...base, schema: 'conduit.todo-journey/producer-event@1', chapter_id: id,
      event_id: `event/${id}`, observed_at_unix_ms: index + 1, face_id: `face/${id}`,
      face_revision: index, show_id: `show/${id}` };
    if (['add', 'complete', 'read'].includes(id)) {
      event.interaction_id = `interaction/${id}`; event.action_id = `todo.${id}`;
      event.mask_kind = id === 'add' ? 'terminal' : id === 'complete' ? 'conduitos-graphical' : 'direct-spoken';
    }
    if (['add', 'complete'].includes(id)) {
      event.queue_sequence = index; event.child_sign_id = `sign/${id}`; event.outcome = 'produced';
    }
    if (['hear', 'read'].includes(id)) {
      event.play_id = `play/${id}`; event.plan_id = `plan/${id}`;
      event.delivered_pcm_sha256 = hash(wav().subarray(44));
      event.speaker_frames_committed = 2; event.channels = 1;
      event.sample_rate_hz = 16000; event.bits_per_sample = 16;
      event.spoken_text_sha256 = hash(Buffer.from(`Groceries. Test ${id}.`));
      if (id === 'hear' && guestAudio) {
        event.qemu_boot_id = 'boot/qemu-todo'; event.qemu_output_id = 'hear-qemu-output';
        event.qemu_output_sha256 = hash(wav().subarray(44));
        event.qemu_audio_frames_captured = 2;
      }
    }
    if (id === 'recover') {
      event.previous_boot_id = 'boot/one'; event.new_boot_id = 'boot/two';
      event.pre_lull_state_sha256 = hash(Buffer.from('same Todo state'));
      event.recovered_state_sha256 = event.pre_lull_state_sha256;
    }
    event.media = [];
    const media = kinds[id].map((source, n) => {
      const outputId = `${id}-media-${n}`;
      const isAudio = source === 'speaker-play' || source === 'qemu-audio';
      const isTerminal = source === 'terminal';
      const bytes = isAudio ? wav() : isTerminal ? Buffer.from(`${id} visible Todo state\n`) : png;
      const output = add(outputId, isAudio ? 'audio' : isTerminal ? 'console-transcript' : 'screenshot',
        bytes, isAudio ? 'audio/wav' : isTerminal ? 'text/plain; charset=utf-8' : 'image/png',
        `${outputId}.${isAudio ? 'wav' : isTerminal ? 'txt' : 'png'}`);
      event.media.push({ media_output_id: outputId, capture_source: source, media_sha256: output.sha256 });
      const capture = { ...base, schema: 'conduit.todo-journey/capture-receipt@1', chapter_id: id,
        event_id: event.event_id, face_id: event.face_id, face_revision: event.face_revision,
        show_id: event.show_id, source_receipt_id: `${id}-source`, media_output_id: outputId,
        media_sha256: output.sha256, capture_source: source };
      if (isAudio) Object.assign(capture, { speech_mode: 'direct', play_id: event.play_id,
        plan_id: event.plan_id, voice_id: 'test-voice', delivered_pcm_sha256: event.delivered_pcm_sha256,
        channels: 1, sample_rate_hz: 16000, bits_per_sample: 16,
        transcript_output_id: `${id}-transcript`, transcript_sha256: event.spoken_text_sha256 });
      if (source === 'qemu-audio') Object.assign(capture, {
        qemu_boot_id: event.qemu_boot_id, qemu_output_id: event.qemu_output_id,
      });
      if (isAudio) add(`${id}-transcript`, 'document', Buffer.from(`Groceries. Test ${id}.`),
        'text/plain; charset=utf-8', `${id}-transcript.txt`);
      if (source === 'qemu-audio') add(event.qemu_output_id, 'audio', wav().subarray(44),
        'application/octet-stream', 'hear-qemu-output.pcm');
      add(`${outputId}-receipt`, 'machine-readable-manifest', capture, 'application/json');
      return { output_id: outputId, receipt_id: `${outputId}-receipt`, alt: `${id} ${source} capture` };
    });
    const receipt = { ...event, schema: 'conduit.todo-journey/chapter-receipt@1',
      source_receipt_id: `${id}-source` };
    add(`${id}-source`, 'machine-readable-manifest', event, 'application/json');
    add(`${id}-receipt`, 'machine-readable-manifest', receipt, 'application/json');
    story.push({ id, title: `Test ${id}`, intention: `Understand ${id}`, action: `Perform ${id}`,
      result: `Observe ${id}`, why: `${id} matters`, next: `Continue after ${id}`,
      limitations: ['Fixture correlation is not live proof.'], receipt_id: `${id}-receipt`, media });
  }
  add('producer-terminal', 'machine-readable-manifest', {
    ...base, schema: 'conduit.todo-journey/producer-terminal@1',
    capture_entrance: 'cargo xtask prove todo-journey', outcome: 'completed',
    event_ids: chapters.map(id => `event/${id}`),
    media_output_ids: story.flatMap(chapter => chapter.media.map(item => item.output_id)),
  }, 'application/json');
  add('journey', 'machine-readable-manifest', { ...base, schema: 'conduit.journey/todo@1',
    producer_terminal_receipt_id: 'producer-terminal', chapters: story }, 'application/json', 'journey.json');
  const manifest = { schema: 'conduit.evidence-manifest/v1', result: 'complete', git_commit: commit,
    proof_id: 'journey-todo-one-body', suite_id: 'journey-gallery', outputs };
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  return { root, manifest, story };
}

test('complete fixture renders task sequence in shared shell with real media links', t => {
  const { root } = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const outputRoot = mkdtempSync(path.join(tmpdir(), 'todo-render-'));
  t.after(() => rmSync(outputRoot, { recursive: true, force: true }));
  const destination = path.join(outputRoot, 'todo');
  renderTodoJourney(root, destination, commit,
    readFileSync('targets/browser/host/assets/conduit.css', 'utf8') + readFileSync('site/chrome.css', 'utf8'),
    readFileSync('site/navigation.html', 'utf8'), { checkAncestry: false });
  const html = readFileSync(path.join(destination, 'index.html'), 'utf8');
  assert.match(html, /Main navigation/);
  assert.match(html, /Keep one Todo list with you/);
  assert.match(html, /audio controls/);
  assert.match(html, /Complete terminal capture/);
  assert.equal((html.match(/<article id=/g) || []).length, 8);
});

test('rejects partial and mixed evidence before publication', t => {
  const { root, manifest, story } = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  story[0].media = [];
  const journey = Buffer.from(JSON.stringify({ ...base, schema: 'conduit.journey/todo@1',
    producer_terminal_receipt_id: 'producer-terminal', chapters: story }));
  writeFileSync(path.join(root, 'journey.json'), journey);
  manifest.outputs.find(output => output.id === 'journey').bytes = journey.length;
  manifest.outputs.find(output => output.id === 'journey').sha256 = hash(journey);
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  assert.throws(() => validateTodoJourney(root, commit, { checkAncestry: false }), /birth has no captured medium/);
  const complete = fixture();
  t.after(() => rmSync(complete.root, { recursive: true, force: true }));
  complete.manifest.outputs.find(output => output.id === 'join-media-0').scenario_id = 'another-run';
  writeFileSync(path.join(complete.root, 'manifest.json'), JSON.stringify(complete.manifest));
  assert.throws(() => validateTodoJourney(complete.root, commit, { checkAncestry: false }), /mixed run/);
});

test('rejects substituted speech PCM even when the WAV file digest is updated', t => {
  const { root, manifest } = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const output = manifest.outputs.find(item => item.id === 'hear-media-0');
  const file = path.join(root, output.path);
  const bytes = readFileSync(file);
  bytes.writeUInt16LE(7, 44);
  writeFileSync(file, bytes);
  output.sha256 = hash(bytes);
  const captureOutput = manifest.outputs.find(item => item.id === 'hear-media-0-receipt');
  const capture = JSON.parse(readFileSync(path.join(root, captureOutput.path)));
  capture.media_sha256 = output.sha256;
  const captureBytes = Buffer.from(JSON.stringify(capture));
  writeFileSync(path.join(root, captureOutput.path), captureBytes);
  captureOutput.bytes = captureBytes.length;
  captureOutput.sha256 = hash(captureBytes);
  const producerOutput = manifest.outputs.find(item => item.id === 'hear-source');
  const producer = JSON.parse(readFileSync(path.join(root, producerOutput.path)));
  producer.media[0].media_sha256 = output.sha256;
  const producerBytes = Buffer.from(JSON.stringify(producer));
  writeFileSync(path.join(root, producerOutput.path), producerBytes);
  producerOutput.bytes = producerBytes.length;
  producerOutput.sha256 = hash(producerBytes);
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  assert.throws(() => validateTodoJourney(root, commit, { checkAncestry: false }), /audio differs from claimed delivered Play PCM/);
});

test('guest WAV may package the exact raw QEMU PCM but refuses another run output', t => {
  const { root, manifest } = fixture({ guestAudio: true });
  t.after(() => rmSync(root, { recursive: true, force: true }));
  validateTodoJourney(root, commit, { checkAncestry: false });
  const output = manifest.outputs.find(item => item.id === 'hear-qemu-output');
  const bytes = Buffer.from(readFileSync(path.join(root, output.path)));
  bytes[0] ^= 0xff;
  writeFileSync(path.join(root, output.path), bytes);
  output.sha256 = hash(bytes);
  const sourceOutput = manifest.outputs.find(item => item.id === 'hear-source');
  const source = JSON.parse(readFileSync(path.join(root, sourceOutput.path)));
  source.qemu_output_sha256 = output.sha256;
  const sourceBytes = Buffer.from(JSON.stringify(source));
  writeFileSync(path.join(root, sourceOutput.path), sourceBytes);
  sourceOutput.bytes = sourceBytes.length;
  sourceOutput.sha256 = hash(sourceBytes);
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  assert.throws(() => validateTodoJourney(root, commit, { checkAncestry: false }), /published WAV differs from same-run QEMU PCM/);
});

test('rejects stale Face and source commit drift', t => {
  const { root, manifest } = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const output = manifest.outputs.find(item => item.id === 'complete-receipt');
  const receipt = JSON.parse(readFileSync(path.join(root, output.path)));
  receipt.face_revision = 999;
  const bytes = Buffer.from(JSON.stringify(receipt));
  writeFileSync(path.join(root, output.path), bytes);
  output.bytes = bytes.length; output.sha256 = hash(bytes);
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  assert.throws(() => validateTodoJourney(root, commit, { checkAncestry: false }), /producer event drift/);
  manifest.git_commit = 'b'.repeat(40);
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  assert.throws(() => validateTodoJourney(root, commit, { checkAncestry: false }), /journey identity mismatch/);
});
