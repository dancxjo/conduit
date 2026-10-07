// Retain one producer-owned, complete #4807 run for the strict Rust renderer.
// This adapter only copies captured bytes and transcribes explicit producer
// correlation. It cannot assign an action, Face revision, or audio Play later.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const CHAPTERS = ['birth', 'join', 'start', 'see', 'hear', 'loss', 'return', 'lull'];
const MAX_FILE = 16 * 1024 * 1024;
const MAX_TOTAL = 128 * 1024 * 1024;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const token = value => typeof value === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(value);
const identity = value => typeof value === 'string' && value.trim() && value.length <= 256 &&
  !/[\x00-\x1f\x7f]/.test(value);
const prose = value => identity(value) && value.length <= 2048;
const relative = value => typeof value === 'string' &&
  /^[A-Za-z0-9._/-]+$/.test(value) && !value.startsWith('.') &&
  value.split('/').every(part => part && part !== '.' && part !== '..');
const json = bytes => JSON.parse(bytes.toString('utf8'));
const rendererSource = value => ({
  'pinned-chromium': 'chromium',
  'screen-free-transcript': 'terminal',
  'selected-speaker-same-play': 'speaker-play',
})[value] ?? value;

async function file(source, name) {
  assert.ok(relative(name), `unsafe source path: ${name}`);
  const full = path.join(source, name);
  const metadata = await lstat(full);
  assert.ok(metadata.isFile() && !metadata.isSymbolicLink(), `not a regular capture: ${name}`);
  const resolved = await realpath(full);
  assert.ok(resolved.startsWith(`${source}${path.sep}`), `capture escapes source: ${name}`);
  const bytes = await readFile(full);
  assert.ok(bytes.length > 0 && bytes.length <= MAX_FILE, `capture size is invalid: ${name}`);
  return bytes;
}

function publicSafe(name, bytes) {
  if (!/\.(json|jsonl|txt|log|html|css)$/.test(name)) return;
  const contents = bytes.toString('utf8');
  assert.ok(!/-----BEGIN (?:OPENSSH |RSA |EC )?PRIVATE KEY-----/.test(contents) &&
    !/"(?:secret|client[_-]?secret|password|token|api[_-]?key|private[_-]?key|authorization|cookie)"\s*:/i.test(contents) &&
    !/\bBearer\s+[A-Za-z0-9._~+/-]{12,}/i.test(contents),
  `private material appears in ${name}`);
}

function outputKind(name) {
  if (name.endsWith('.png')) return ['screenshot', 'image/png'];
  if (name.endsWith('.wav')) return ['audio', 'audio/wav'];
  if (name.endsWith('.json')) return ['machine-readable-manifest', 'application/json'];
  return ['console-transcript', 'text/plain; charset=utf-8'];
}

function sameSource(report, value, label) {
  assert.equal(value?.source_commit, report.native_source_commit, `${label} source changed`);
  assert.equal(value?.run_id, report.run_id, `${label} run changed`);
  assert.equal(value?.body_id, report.body_id, `${label} Body changed`);
}

export async function retainOneBodyComplete(runPath, outputPath) {
  const run = path.resolve(runPath);
  const source = path.join(run, 'three-host');
  const output = path.resolve(outputPath);
  for (const root of [run, source]) {
    const metadata = await lstat(root);
    assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink(), 'run root is not a real directory');
  }
  const reportBytes = await file(source, 'report.json');
  const report = json(reportBytes);
  assert.equal(report.schema, 'conduit.body/three-host-owner-journey@1');
  assert.match(report.proof_class, /^live-local-installed-owner-qmp-pinned-chromium/);
  assert.match(report.native_source_commit, /^[a-f0-9]{40}$/);
  assert.ok(token(report.run_id) && identity(report.body_id));
  assert.equal(report.concurrent_part_count, 3);
  assert.equal(report.qemu_alive_through_browser_actions, true);
  assert.ok(report.birth?.confirmation_observed && report.screen_free_clock?.finish &&
    report.presentation_host_recovery && report.owner_direct_speech &&
    report.owner_llm_speech && report.owner_model_route_loss,
  'producer has not completed Birth, three-host recovery, direct/model speech, and screen-free finish');
  assert.equal(report.birth.source_commit, report.native_source_commit);
  assert.equal(report.birth.body_id, report.body_id);
  for (const [label, item] of Object.entries({
    clock: report.screen_free_clock, direct: report.owner_direct_speech,
    model: report.owner_llm_speech, loss: report.owner_model_route_loss })) {
    sameSource(report, item, label);
  }
  const chapters = report.publication_chapters;
  assert.ok(Array.isArray(chapters) && chapters.length === 8,
    'producer report lacks eight publication_chapters with action/media capture correlation');
  assert.deepEqual(chapters.map(item => item.id), CHAPTERS,
    'producer publication chapters are not in the required action order');

  const planned = new Map();
  const plannedPaths = new Set();
  async function add(id, name, expectedSha) {
    assert.ok(identity(id) && relative(name) && /^[a-f0-9]{64}$/.test(expectedSha),
      `invalid declaration ${id}`);
    assert.ok(!planned.has(id), `duplicate output identity ${id}`);
    assert.ok(!plannedPaths.has(name), `source file is used for more than one output: ${name}`);
    const bytes = await file(source, name);
    assert.equal(sha(bytes), expectedSha, `producer digest changed: ${name}`);
    publicSafe(name, bytes);
    const [kind, media_type] = outputKind(name);
    planned.set(id, { id, name, bytes, kind, media_type });
    plannedPaths.add(name);
  }
  await add('producer-report', 'report.json', sha(reportBytes));
  // Publication metadata is created by the live producer while actions happen.
  // Require its exact captures and provenance, never a guessed chapter-to-file
  // mapping assembled from filenames after the run.
  const story = { schema: 'conduit.journey/one-body-five-masks@1',
    source_commit: report.native_source_commit, run_id: report.run_id,
    body_id: report.body_id, chapters: [] };
  const generated = new Map();
  const allEvents = new Set();
  const allSources = new Set();
  const hearModes = new Set();
  for (const chapter of chapters) {
    assert.ok(CHAPTERS.includes(chapter.id), 'unknown chapter');
    for (const key of ['title', 'intention', 'action', 'result', 'why', 'next']) {
      assert.ok(prose(chapter[key]), `${chapter.id} lacks ${key}`);
    }
    assert.ok(Array.isArray(chapter.limitations) && chapter.limitations.length > 0 &&
      chapter.limitations.length <= 8 && chapter.limitations.every(prose),
    `${chapter.id} lacks honest limits`);
    assert.ok(Array.isArray(chapter.events) && chapter.events.length > 0 &&
      chapter.events.length <= 24, `${chapter.id} lacks producer events`);
    const eventMap = new Map();
    for (const event of chapter.events) {
      assert.ok(identity(event.kind) && identity(event.id) &&
        identity(event.face_revision) && relative(event.source_receipt?.path) &&
        /^[a-f0-9]{64}$/.test(event.source_receipt.sha256),
      `${chapter.id} event lacks source receipt or Face revision`);
      assert.ok(!allEvents.has(event.id), `event reused across chapters: ${event.id}`);
      allEvents.add(event.id);
      const sourceBytes = await file(source, event.source_receipt.path);
      assert.equal(sha(sourceBytes), event.source_receipt.sha256,
        `${chapter.id} event receipt changed`);
      const observed = json(sourceBytes);
      assert.equal(observed.source_commit ?? observed.native_source_commit,
        report.native_source_commit, `${chapter.id} event source changed`);
      assert.equal(observed.run_id, report.run_id, `${chapter.id} event run changed`);
      assert.equal(observed.body_id, report.body_id, `${chapter.id} event Body changed`);
      assert.equal(observed.event_kind, event.kind);
      assert.equal(observed.event_id, event.id,
        `${chapter.id} source has no exact event identity`);
      assert.equal(String(observed.resulting_face_revision), String(event.face_revision),
        `${chapter.id} source has no exact resulting Face revision`);
      assert.equal(observed.outcome, 'completed',
        `${chapter.id} source event did not complete`);
      const sourceReceiptId = `${chapter.id}-event-${eventMap.size + 1}-source`;
      await add(sourceReceiptId, event.source_receipt.path,
        event.source_receipt.sha256);
      eventMap.set(event.id, { kind: event.kind,
        face_revision: event.face_revision, source_receipt_id: sourceReceiptId });
    }
    const last = chapter.events.at(-1);
    const chapterReceiptId = `${chapter.id}-chapter-receipt`;
    generated.set(chapterReceiptId, { schema: 'conduit.journey/chapter-receipt@2',
      source_commit: report.native_source_commit, run_id: report.run_id,
      body_id: report.body_id, chapter_id: chapter.id,
      events: chapter.events.map(({ kind, id, face_revision }) => ({
        kind, id, face_revision,
        source_receipt_id: eventMap.get(id).source_receipt_id,
      })),
      resulting_face_revision: last.face_revision, outcome: 'completed' });
    assert.ok(Array.isArray(chapter.media) && chapter.media.length > 0 &&
      chapter.media.length <= 12, `${chapter.id} lacks bounded media`);
    const media = [];
    const sources = new Set();
    for (const [index, item] of chapter.media.entries()) {
      const id = `${chapter.id}-media-${index + 1}`;
      const receiptId = `${id}-receipt`;
      assert.ok(prose(item.alt) && eventMap.has(item.event_id),
        `${chapter.id} media lacks an exact chapter event`);
      assert.equal(item.face_revision, eventMap.get(item.event_id).face_revision,
        `${chapter.id} media Face differs from its event`);
      assert.ok(relative(item.path) && /^[a-f0-9]{64}$/.test(item.sha256),
        `${chapter.id} media lacks producer path/digest`);
      assert.ok(relative(item.source_receipt?.path) &&
        /^[a-f0-9]{64}$/.test(item.source_receipt.sha256),
      `${chapter.id} media lacks producer capture receipt`);
      const sourceBytes = await file(source, item.source_receipt.path);
      assert.equal(sha(sourceBytes), item.source_receipt.sha256,
        `${chapter.id} media source receipt changed`);
      const observed = json(sourceBytes);
      assert.equal(observed.source_commit, report.native_source_commit);
      assert.equal(observed.run_id, report.run_id);
      assert.equal(observed.body_id, report.body_id);
      const event = chapter.events.find(value => value.id === item.event_id);
      assert.equal(observed.event_kind, event.kind);
      assert.equal(observed.event_id, item.event_id,
        `${chapter.id} capture source has no exact event`);
      assert.equal(String(observed.face_revision), String(item.face_revision));
      assert.equal(observed.media_path, item.path);
      assert.equal(observed.media_sha256, item.sha256);
      assert.equal(observed.capture_source, item.capture_source);
      const source = rendererSource(item.capture_source);
      const [mediaKind] = outputKind(item.path);
      assert.ok((mediaKind === 'screenshot' && ['chromium', 'qmp'].includes(source)) ||
        (mediaKind === 'console-transcript' && source === 'terminal') ||
        (mediaKind === 'audio' && ['speaker-play', 'qemu-audio'].includes(source)),
      `${chapter.id} capture source does not match its media`);
      sources.add(source);
      allSources.add(source);
      await add(`${id}-source`, item.source_receipt.path,
        item.source_receipt.sha256);
      await add(id, item.path, item.sha256);
      const capture = { schema: 'conduit.journey/capture-receipt@2',
        source_commit: report.native_source_commit, run_id: report.run_id,
        body_id: report.body_id, chapter_id: chapter.id,
        event_id: item.event_id, event_kind: event.kind,
        face_revision: item.face_revision,
        source_receipt_id: `${id}-source`,
        media_output_id: id, media_sha256: item.sha256,
        capture_source: source };
      if (source === 'speaker-play' || source === 'qemu-audio') {
        const speech = item.speech;
        assert.ok(speech && ['direct', 'llm-assisted'].includes(speech.mode) &&
          identity(speech.show_id) && identity(speech.plan_id) &&
          identity(speech.play_id) && identity(speech.voice_id) &&
          prose(speech.text) && relative(speech.provenance?.path) &&
          /^[a-f0-9]{64}$/.test(speech.provenance.sha256),
        `${chapter.id} audio lacks same-Play provenance`);
        const provenanceId = `${id}-audio-provenance`;
        await add(provenanceId, speech.provenance.path, speech.provenance.sha256);
        const listener = json(planned.get(provenanceId).bytes);
        assert.equal(listener.schema, 'conduit.body/owner-spoken-terminal@1',
          'featured audio must name its actual owner spoken Mask terminal');
        assert.equal(listener.mode, speech.mode);
        assert.equal(listener.outcome, 'available');
        assert.equal(listener.show_id, speech.show_id);
        assert.equal(listener.speaker_played, true);
        if (speech.mode === 'direct') {
          const batches = listener.speaker_playback?.batches;
          assert.ok(listener.direct_reading_complete && Array.isArray(batches));
          const match = batches.filter(batch => batch.play_id === speech.play_id);
          assert.equal(match.length, 1);
          assert.equal(match[0].plan_id, speech.plan_id);
          assert.equal(match[0].wav_sha256, item.sha256);
          assert.equal(match[0].spoken_segments.join(' '), speech.text);
        } else {
          assert.equal(listener.speaker_playback?.play_id, speech.play_id);
          assert.equal(listener.speaker_playback?.plan_id, speech.plan_id);
          assert.equal(listener.speaker_playback?.wav_sha256, item.sha256);
          assert.equal(listener.accepted_wording, speech.text);
          assert.equal(listener.generation_evidence?.original_model_output,
            speech.original_model_output);
          assert.equal(listener.generation_evidence?.provider_identity,
            speech.provider_id);
          assert.equal(listener.generation_evidence?.model_identity,
            speech.model_id);
        }
        hearModes.add(chapter.id === 'hear' ? speech.mode : 'other');
        const transcriptId = `${id}-transcript`;
        const transcript = { schema: 'conduit.journey/speech-transcript@1',
          source_commit: report.native_source_commit, run_id: report.run_id,
          body_id: report.body_id, chapter_id: chapter.id,
          show_id: speech.show_id, face_revision: item.face_revision, text: speech.text };
        if (speech.mode === 'llm-assisted') {
          assert.ok(prose(speech.original_model_output) &&
            identity(speech.provider_id) && identity(speech.model_id),
          'model speech lacks original output/provider/model');
          transcript.original_model_output = speech.original_model_output;
          const validationId = `${id}-model-validation`;
          generated.set(validationId, { schema: 'conduit.journey/model-validation@1',
            source_commit: report.native_source_commit, run_id: report.run_id,
            body_id: report.body_id, show_id: speech.show_id,
            face_revision: item.face_revision, provider_id: speech.provider_id,
            model_id: speech.model_id,
            original_output_sha256: sha(Buffer.from(speech.original_model_output)),
            validated_text_sha256: sha(Buffer.from(speech.text)), accepted: true });
          capture.validation_id = validationId;
          capture.provider_id = speech.provider_id;
          capture.model_id = speech.model_id;
        }
        generated.set(transcriptId, transcript);
        capture.show_id = speech.show_id;
        capture.plan_id = speech.plan_id;
        capture.play_id = speech.play_id;
        capture.voice_id = speech.voice_id;
        capture.transcript_id = transcriptId;
        capture.transcript_sha256 = sha(Buffer.from(`${JSON.stringify(transcript, null, 2)}\n`));
        capture.speech_mode = speech.mode;
        capture.audio_provenance_id = provenanceId;
        if (source === 'qemu-audio') {
          assert.ok(identity(speech.qemu_boot_id));
          capture.qemu_boot_id = speech.qemu_boot_id;
        }
      }
      generated.set(receiptId, capture);
      media.push({ output_id: id, receipt_id: receiptId, alt: item.alt });
    }
    const required = { birth: ['terminal'], join: ['chromium', 'qmp'],
      start: ['chromium', 'qmp'], see: ['chromium', 'terminal'],
      lull: ['terminal'] }[chapter.id] ?? [];
    assert.ok(required.every(source => sources.has(source)),
      `${chapter.id} lacks required user-visible capture source`);
    story.chapters.push({ id: chapter.id, title: chapter.title,
      intention: chapter.intention, action: chapter.action, result: chapter.result,
      why: chapter.why, next: chapter.next, receipt_id: chapterReceiptId,
      media, limitations: chapter.limitations });
  }
  assert.ok(['chromium', 'qmp', 'terminal'].every(source => allSources.has(source)) &&
    (allSources.has('speaker-play') || allSources.has('qemu-audio')) &&
    ['direct', 'llm-assisted'].every(mode => hearModes.has(mode)),
  'journey lacks graphical, terminal, and both same-Play speech captures');
  generated.set('journey', story);
  for (const [id, value] of generated) {
    const name = `${id}.json`;
    assert.ok(!planned.has(id) && !plannedPaths.has(name),
      `generated journey identity or path collides with a capture: ${name}`);
    const bytes = Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
    assert.ok(bytes.length <= MAX_FILE);
    planned.set(id, { id, name, bytes, kind: 'machine-readable-manifest',
      media_type: 'application/json' });
    plannedPaths.add(name);
  }
  assert.ok(planned.size <= 128, 'journey exceeds 128 retained outputs');
  const total = [...planned.values()].reduce((sum, value) => sum + value.bytes.length, 0);
  assert.ok(total <= MAX_TOTAL, 'journey exceeds 128 MiB retained output budget');
  assert.ok(!planned.has('manifest'), 'manifest identity is reserved');
  const manifest = { schema: 'conduit.evidence-manifest/v1', result: 'complete',
    git_commit: report.native_source_commit, proof_id: 'journey-one-body-five-masks',
    suite_id: 'journey-gallery', limits: { maximum_outputs: 128,
      maximum_bytes_per_output: MAX_FILE },
    outputs: [...planned.values()].map(({ id, name, bytes, kind, media_type }) => ({
      id, kind, path: name, media_type, required: true,
      bytes: bytes.length, sha256: sha(bytes), scenario_id: report.run_id,
      proof_class: 'producer-correlated-live-local',
    })) };
  assert.ok(!planned.has('manifest.json'));
  await mkdir(path.dirname(output), { recursive: true });
  await mkdir(output, { recursive: false });
  try {
    for (const item of planned.values()) {
      const destination = path.join(output, item.name);
      await mkdir(path.dirname(destination), { recursive: true });
      await writeFile(destination, item.bytes, { flag: 'wx' });
    }
    await writeFile(path.join(output, 'manifest.json'),
      `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx' });
  } catch (error) {
    await rm(output, { recursive: true, force: true });
    throw error;
  }
  return { outputs: planned.size, bytes: total, commit: report.native_source_commit };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const [run, output] = process.argv.slice(2);
  if (!run || !output || process.argv.length !== 4) {
    throw new Error('usage: retain-one-body-complete.mjs PRIVATE_COMPLETED_RUN NEW_DIRECTORY');
  }
  const retained = await retainOneBodyComplete(run, output);
  process.stdout.write(`retained ${retained.outputs} outputs (${retained.bytes} bytes) from ${retained.commit}\n`);
}
