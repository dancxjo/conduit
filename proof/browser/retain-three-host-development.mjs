// Curate a completed local producer run into source-carried, public-safe
// development evidence. CI verifies and copies this bundle; it never reruns
// QEMU, the installed speaker, or the local model.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { THREE_HOST_DEVELOPMENT_PROOF, THREE_HOST_DEVELOPMENT_SUITE } from
  '../../tools/ci/pipeline/three-host-development-evidence.mjs';

const [runArg, outputArg] = process.argv.slice(2);
if (!runArg || !outputArg || process.argv.length !== 4) {
  throw new Error('usage: retain-three-host-development.mjs PRIVATE_COMPLETED_RUN NEW_SOURCE_DIRECTORY');
}
const run = path.resolve(runArg);
const output = path.resolve(outputArg);
const report = JSON.parse(await readFile(path.join(run, 'three-host/report.json'), 'utf8'));
assert.match(report.native_source_commit, /^[a-f0-9]{40}$/);
assert.match(report.run_id, /^[A-Za-z0-9._-]+$/);
assert.ok(report.body_id && report.owner_selected_speech && report.owner_llm_speech,
  'the development page requires one Body and both selected listener speech modes');
assert.ok(report.birth && report.screen_free_clock,
  'the development page requires real nonvisual Birth and clock actions');
assert.equal(report.walkthrough?.path, 'walkthrough.html');

const html = await readFile(path.join(run, 'three-host/walkthrough.html'), 'utf8');
assert.match(html, /This is a live local proof, not the complete eight-chapter public journey/);
const refs = [...html.matchAll(/\b(?:href|src)="([^"]+)"/g)].map(match => match[1]);
const files = new Map();
for (const ref of refs) {
  if (ref.startsWith('https://') || ref.startsWith('#')) continue;
  assert.ok(!ref.startsWith('/') && !ref.includes('?') && !ref.includes('#'),
    `unsupported local page asset ${ref}`);
  const parent = ref.startsWith('../');
  const relative = parent ? ref.slice(3) : ref;
  assert.match(relative, /^[a-zA-Z0-9._/-]+$/);
  assert.ok(relative.split('/').every(segment => segment && segment !== '.' && segment !== '..'));
  if (relative.endsWith('.wav')) {
    assert.ok(relative.startsWith('owner-selected-speech/') ||
      relative.startsWith('owner-llm-selected/'),
    'only the selected listener Play may be offered as playable audio');
  }
  files.set(relative, path.join(run, parent ? relative : `three-host/${relative}`));
}
for (const relative of [
  'report.json', 'native/guest.serial.log', 'native/qmp.jsonl',
  'native/owner-action-proof.json',
]) files.set(relative, path.join(run, `three-host/${relative}`));
for (const relative of [
  'birth-input.txt', 'birth-transcript.txt', 'zero-body-before.json',
  'clock-start-input.txt', 'clock-start-transcript.txt',
  'clock-lull-input.txt', 'clock-lull-transcript.txt',
]) files.set(relative, path.join(run, relative));
for (const observation of report.observations ?? []) {
  const relative = observation.path;
  assert.match(relative, /^observations\/[a-z-]+\.json$/);
  files.set(relative, path.join(run, `three-host/${relative}`));
}
// The original terminal transcript is retained byte-for-byte. Its inline
// documentary copy should be readable without ANSI cursor/style instructions.
const index = html.replaceAll('href="../', 'href="').replaceAll('src="../', 'src="')
  .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replaceAll('\r', '');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const kindAndType = relative => {
  if (relative.endsWith('.png')) return ['screenshot', 'image/png'];
  if (relative.endsWith('.wav')) return ['audio', 'audio/wav'];
  if (relative.endsWith('.json')) return ['machine-readable-manifest', 'application/json'];
  if (relative.endsWith('.css')) return ['document', 'text/css; charset=utf-8'];
  if (relative.endsWith('.html')) return ['document', 'text/html; charset=utf-8'];
  return ['console-transcript', 'text/plain; charset=utf-8'];
};
const selectedWav = report.owner_llm_speech.wav;
assert.ok(files.has(selectedWav.path), 'page must link the model listener WAV');
assert.equal(digest(await readFile(path.join(run, 'three-host', selectedWav.path))),
  selectedWav.sha256, 'model listener WAV differs from its completed Play receipt');

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
  for (const [relative, source] of [...files].sort(([a], [b]) => a.localeCompare(b))) {
    const bytes = source === null ? Buffer.from(index) : await readRegularFile(source);
    assert.ok(bytes.length > 0 && bytes.length <= 16 * 1024 * 1024,
      `asset is empty or over the evidence limit: ${relative}`);
    if (relative.endsWith('.wav')) {
      assert.ok(bytes.length > 44 && bytes.toString('ascii', 0, 4) === 'RIFF' &&
        bytes.toString('ascii', 8, 12) === 'WAVE', `invalid listener WAV: ${relative}`);
    }
    if (relative.endsWith('.json') || relative.endsWith('.txt') || relative.endsWith('.log') ||
        relative.endsWith('.jsonl') || relative.endsWith('.html')) {
      assert.ok(!bytes.toString('utf8').includes('-----BEGIN PRIVATE KEY-----') &&
        !/"secret"\s*:/.test(bytes.toString('utf8')),
      `private material appears in ${relative}`);
    }
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

async function readRegularFile(file) {
  const metadata = await lstat(file);
  assert.ok(metadata.isFile() && !metadata.isSymbolicLink(), `asset is not a regular file: ${file}`);
  return readFile(file);
}
