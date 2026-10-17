// Carry only public, producer-declared bytes from one local development run.
// This is a diagnostic carrier; the complete public Journey has a separate gate.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { THREE_HOST_DEVELOPMENT_PROOF, THREE_HOST_DEVELOPMENT_SUITE } from
  '../../tools/ci/pipeline/three-host-development-evidence.mjs';

const [runArg, outputArg] = process.argv.slice(2);
if (!runArg || !outputArg || process.argv.length !== 4) {
  throw new Error('usage: retain-three-host-development.mjs PRIVATE_COMPLETED_RUN NEW_SOURCE_DIRECTORY');
}
const run = path.resolve(runArg);
const output = path.resolve(outputArg);
const source = path.join(run, 'three-host');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const escape = value => String(value).replace(/[&<>"']/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
})[character]);
const safePath = value => typeof value === 'string' && value.length > 0 &&
  /^[a-zA-Z0-9._/-]+$/.test(value) && !value.startsWith('/') &&
  value.split('/').every(segment => segment && segment !== '.' && segment !== '..');

async function readRegularFile(file) {
  const resolved = await realpath(file);
  const relative = path.relative(source, resolved);
  assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative),
    `asset escapes the producer bundle: ${file}`);
  const metadata = await lstat(file);
  assert.ok(metadata.isFile() && !metadata.isSymbolicLink(), `asset is not a regular file: ${file}`);
  return readFile(file);
}
for (const directory of [run, source]) {
  const metadata = await lstat(directory);
  assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink(), 'run roots must be real directories');
}
const reportBytes = await readRegularFile(path.join(source, 'report.json'));
const report = JSON.parse(reportBytes);
assert.equal(report.schema, 'conduit.body/three-host-owner-journey@1');
assert.match(report.proof_class, /^live-local-installed-owner-qmp-pinned-chromium/);
assert.match(report.native_source_commit, /^[a-f0-9]{40}$/);
assert.match(report.run_id, /^[A-Za-z0-9._-]+$/);
assert.ok(report.body_id && report.owner_host_id && report.guest_host_id && report.browser_host_id);
assert.equal(report.walkthrough?.path, 'walkthrough.html');
const rawHtml = await readRegularFile(path.join(source, 'walkthrough.html'));
assert.equal(rawHtml.length, report.walkthrough.bytes);
assert.equal(digest(rawHtml), report.walkthrough.sha256);
const html = rawHtml.toString('utf8');
assert.match(html, /Development capture · \d of 8 chapters complete/);
assert.equal((html.match(/<article id="(?:birth|join|start|see|hear|loss|return|lull)"/g) ?? []).length, 8);
assert.match(html, /The public Journey and accepted release require separate gates/);

// Every linked local asset must be named by the producer report, and every
// selected screen-free WAV must carry the exact words bound by that producer.
const declared = new Map();
function collect(value) {
  if (!value || typeof value !== 'object') return;
  if (Array.isArray(value)) { value.forEach(collect); return; }
  if (safePath(value.path) && /^[a-f0-9]{64}$/.test(value.sha256 ?? value.wav_sha256 ?? '')) {
    const checksum = value.sha256 ?? value.wav_sha256;
    const previous = declared.get(value.path);
    assert.ok(!previous || previous.sha256 === checksum, `conflicting receipt for ${value.path}`);
    declared.set(value.path, { sha256: checksum, bytes: value.bytes });
  }
  Object.values(value).forEach(collect);
}
collect(report);
declared.set('native/owner-action-proof.json', { sha256: report.native_receipt_sha256 });
const wordedClips = [];
for (const clip of report.screen_free_audio?.selected ?? []) {
  assert.equal(report.screen_free_audio.proof_class,
    'selected-screen-free-same-play-listener-audio-subset');
  assert.equal(report.screen_free_audio.source_commit, report.native_source_commit);
  assert.equal(report.screen_free_audio.human_hearing_observed, false);
  assert.ok(safePath(clip.path) && clip.path.startsWith('screen-free-audio/') &&
    clip.path.endsWith('.wav'));
  if (clip.spoken_words === undefined && clip.spoken_words_html === undefined &&
      clip.spoken_segment_sha256 === undefined) {
    assert.match(html, /Not yet captured in this run/,
      'unlabeled selected audio needs an explicit incomplete chapter');
    continue;
  }
  assert.ok(typeof clip.spoken_words === 'string' && clip.spoken_words.trim());
  assert.equal(clip.spoken_words_html, escape(clip.spoken_words));
  assert.deepEqual(clip.spoken_segment_sha256, [digest(Buffer.from(clip.spoken_words))]);
  assert.ok(clip.plan_id && clip.play_id && clip.source_show_id &&
    clip.speaker_frames_committed > 0);
  wordedClips.push(clip);
}
const approvedAudio = new Set([
  ...wordedClips.map(item => item.path),
  ...(report.owner_selected_speech?.batches ?? []).map(item => item.wav?.path),
  ...(report.owner_direct_speech?.batches ?? []).map(item => item.wav?.path),
  report.owner_llm_speech?.wav?.path, report.owner_model_route_loss?.restored?.wav?.path,
].filter(Boolean));
if (report.owner_direct_speech) {
  const direct = report.owner_direct_speech;
  assert.equal(direct.proof_class, 'installed-owner-direct-mask-and-same-play-speaker');
  assert.equal(direct.source_commit, report.native_source_commit);
  assert.equal(direct.run_id, report.run_id);
  assert.equal(direct.body_id, report.body_id);
  assert.equal(direct.human_hearing_observed, false);
  assert.ok(direct.show_id && direct.route_plan_id && direct.batches?.length > 1);
  const terminalBytes = await readRegularFile(path.join(source, direct.terminal.path));
  assert.equal(digest(terminalBytes), direct.terminal.sha256);
  const terminal = JSON.parse(terminalBytes);
  assert.equal(terminal.schema, 'conduit.body/owner-spoken-terminal@1');
  assert.equal(terminal.mode, 'direct');
  assert.equal(terminal.outcome, 'available');
  assert.equal(terminal.direct_reading_complete, true);
  assert.equal(terminal.speaker_played, true);
  assert.equal(terminal.show_id, direct.show_id);
  assert.equal(terminal.route_plan_id, direct.route_plan_id);
  assert.equal(terminal.speaker_playback?.source_show_id, direct.show_id);
  assert.equal(terminal.speaker_playback?.batches?.length, direct.batches.length);
  for (const [index, batch] of direct.batches.entries()) {
    const observed = terminal.speaker_playback.batches[index];
    assert.equal(batch.play_id, observed.play_id);
    assert.equal(batch.plan_id, observed.plan_id);
    assert.equal(batch.wav_sha256, observed.wav_sha256);
    assert.equal(batch.pcm_sha256, observed.pcm_sha256);
    const wav = await readRegularFile(path.join(source, batch.wav.path));
    assert.equal(wav.length, batch.wav.bytes);
    assert.equal(digest(wav), batch.wav.sha256);
    assert.equal(digest(wav.subarray(44)), batch.pcm_sha256);
  }
}
if (report.owner_llm_speech) {
  const model = report.owner_llm_speech;
  assert.equal(model.proof_class, 'installed-owner-selected-model-and-same-play-speaker');
  assert.equal(model.source_commit, report.native_source_commit);
  assert.equal(model.run_id, report.run_id);
  assert.equal(model.body_id, report.body_id);
  const terminalBytes = await readRegularFile(path.join(source, model.terminal.path));
  assert.equal(digest(terminalBytes), model.terminal.sha256);
  const terminal = JSON.parse(terminalBytes);
  assert.equal(terminal.schema, 'conduit.body/owner-spoken-terminal@1');
  assert.equal(terminal.speaker_played, true);
  assert.equal(terminal.speaker_playback?.play_id, model.listener_play_id);
  assert.equal(terminal.speaker_playback?.source_show_id, model.show_id);
  assert.equal(digest(Buffer.from(terminal.generation_evidence.original_model_output)),
    model.original_model_output_sha256);
  const wav = await readRegularFile(path.join(source, model.wav.path));
  assert.equal(wav.length, model.wav.bytes);
  assert.equal(digest(wav), model.wav.sha256);
}
const refs = [...html.matchAll(/\b(?:href|src)="([^"]+)"/g)].map(match => match[1]);
const files = new Map([['report.json', path.join(source, 'report.json')]]);
for (const ref of refs) {
  if (ref.startsWith('#') || ref.startsWith('https://')) continue;
  assert.ok(!ref.startsWith('/') && !ref.includes('?') && !ref.includes('#') && safePath(ref),
    `unsupported local page asset ${ref}`);
  if (ref === 'report.json') continue;
  assert.ok(declared.has(ref), `page links an undeclared producer asset: ${ref}`);
  if (ref.endsWith('.wav')) assert.ok(approvedAudio.has(ref),
    `page links audio outside a selected listener Play: ${ref}`);
  files.set(ref, path.join(source, ref));
}
for (const clip of wordedClips) {
  files.set(clip.path, path.join(source, clip.path));
}
for (const extra of ['native/guest.serial.log', 'native/qmp.jsonl']) {
  files.set(extra, path.join(source, extra));
}
for (const observation of report.observations ?? []) {
  assert.match(observation.path, /^observations\/[a-z-]+\.json$/);
  files.set(observation.path, path.join(source, observation.path));
}

// A complete eight-chapter *development* capture is still not the accepted
// public Journey. Retain the page's exact chapter status and proof boundary.
const index = html.replaceAll('href="https://dancxjo.github.io/conduit/', 'href="/conduit/')
  .replace('</style>', '.proof{overflow-wrap:anywhere}</style>')
  .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replaceAll('\r', '');
assert.ok(index.includes('.proof{overflow-wrap:anywhere}</style>'));
const kindAndType = relative => {
  if (relative.endsWith('.png')) return ['screenshot', 'image/png'];
  if (relative.endsWith('.wav')) return ['audio', 'audio/wav'];
  if (relative.endsWith('.json')) return ['machine-readable-manifest', 'application/json'];
  if (relative.endsWith('.css')) return ['document', 'text/css; charset=utf-8'];
  if (relative.endsWith('.html')) return ['document', 'text/html; charset=utf-8'];
  return ['console-transcript', 'text/plain; charset=utf-8'];
};
function publicSafe(relative, bytes) {
  if (!/\.(json|jsonl|txt|log|html|css)$/.test(relative)) return;
  const text = bytes.toString('utf8');
  assert.ok(!/-----BEGIN (?:OPENSSH |RSA |EC )?PRIVATE KEY-----/.test(text) &&
    !/"(?:secret|client[_-]?secret|password|token|api[_-]?key|private[_-]?key|authorization|cookie)"\s*:/i.test(text) &&
    !/\bBearer\s+[A-Za-z0-9._~+/-]{12,}/i.test(text),
  `private material appears in ${relative}`);
}
try {
  await mkdir(path.dirname(output), { recursive: true });
  await mkdir(output, { recursive: false });
} catch (error) {
  if (error.code === 'EEXIST') throw new Error('retained output already exists');
  throw error;
}
try {
  files.set('index.html', null);
  const declarations = [];
  let total = 0;
  for (const [relative, file] of [...files].sort(([a], [b]) => a.localeCompare(b))) {
    assert.ok(safePath(relative));
    const bytes = file === null ? Buffer.from(index) : await readRegularFile(file);
    assert.ok(bytes.length > 0 && bytes.length <= 16 * 1024 * 1024,
      `asset is empty or over the evidence limit: ${relative}`);
    const receipt = declared.get(relative);
    if (receipt) {
      assert.equal(digest(bytes), receipt.sha256, `producer digest changed: ${relative}`);
      if (receipt.bytes !== undefined) assert.equal(bytes.length, receipt.bytes);
    }
    if (relative.endsWith('.wav')) {
      assert.ok(bytes.length > 44 && bytes.toString('ascii', 0, 4) === 'RIFF' &&
        bytes.toString('ascii', 8, 12) === 'WAVE', `invalid listener WAV: ${relative}`);
    }
    publicSafe(relative, bytes);
    const destination = path.join(output, relative);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, bytes, { flag: 'wx' });
    const [kind, media_type] = kindAndType(relative);
    declarations.push({ id: `development-${declarations.length + 1}`, kind, path: relative,
      media_type, required: true, bytes: bytes.length, sha256: digest(bytes),
      scenario_id: report.run_id, proof_class: 'retained-local-development' });
    total += bytes.length;
  }
  assert.ok(declarations.length <= 128 && total <= 128 * 1024 * 1024,
    'retained development evidence exceeds the bounded site carrier');
  const manifest = { schema: 'conduit.evidence-manifest/v1', result: 'diagnostic-incomplete',
    git_commit: report.native_source_commit, proof_id: THREE_HOST_DEVELOPMENT_PROOF,
    suite_id: THREE_HOST_DEVELOPMENT_SUITE,
    limits: { maximum_outputs: 128, maximum_bytes_per_output: 16 * 1024 * 1024 },
    outputs: declarations };
  await writeFile(path.join(output, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`,
    { flag: 'wx' });
  process.stdout.write(`retained ${declarations.length} declared assets (${total} bytes) from ${report.run_id}\n`);
} catch (error) {
  await rm(output, { recursive: true, force: true });
  throw error;
}
