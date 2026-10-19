import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, cpSync, readFileSync, writeFileSync, rmSync, symlinkSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { verifySharedSpeechEvidence, stageSharedSpeechEvidence, SHARED_SPEECH_EVIDENCE_ROOT } from '../../tools/ci/pipeline/shared-speech-evidence.mjs';
const repository = fileURLToPath(new URL('../../', import.meta.url));
const commit = 'a'.repeat(40);
function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'speech-publish-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  cpSync(path.join(repository, 'site'), path.join(root, 'site'), { recursive: true, filter: source => source === path.join(repository, 'site') || source === path.join(repository, 'site/evidence') || source.startsWith(path.join(repository, SHARED_SPEECH_EVIDENCE_ROOT)) });
  for (const relative of ['site/navigation.html', 'site/chrome.css', 'targets/browser/host/assets/conduit.css']) {
    const destination = path.join(root, relative);
    mkdirSync(path.dirname(destination), { recursive: true });
    cpSync(path.join(repository, relative), destination);
  }
  return root;
}
function change(root, name, fn, rehash = false) {
  const file = path.join(root, SHARED_SPEECH_EVIDENCE_ROOT, name);
  writeFileSync(file, fn(readFileSync(file)));
  if (rehash) {
    const manifestFile = path.join(root, SHARED_SPEECH_EVIDENCE_ROOT, 'manifest.json');
    const manifest = JSON.parse(readFileSync(manifestFile));
    const data = readFileSync(file);
    manifest.files[name] = { bytes: data.length, sha256: createHash('sha256').update(data).digest('hex') };
    writeFileSync(manifestFile, JSON.stringify(manifest));
  }
}
test('stages every retained byte and distinguishes producer from staged source', t => {
  const root = fixture(t);
  const result = stageSharedSpeechEvidence({ sourceTree: root, output: path.join(root, 'published'), sourceCommit: commit });
  for (const name of ['manifest.json', 'you-would.native', 'you-would.wav', 'you-would.json']) assert.deepEqual(readFileSync(path.join(result.destination, name)), readFileSync(path.join(root, SHARED_SPEECH_EVIDENCE_ROOT, name)));
  const receipt = JSON.parse(readFileSync(path.join(result.destination, 'publication.json')));
  assert.equal(receipt.staged_source_commit, commit);
  assert.notEqual(receipt.producer_code_commit, commit);
  const html = readFileSync(path.join(result.destination, 'index.html'), 'utf8');
  assert.ok(html.includes(readFileSync(path.join(repository, 'site/navigation.html'), 'utf8')));
  assert.ok(html.includes(readFileSync(path.join(repository, 'site/chrome.css'), 'utf8')));
  assert.ok(html.includes(readFileSync(path.join(repository, 'targets/browser/host/assets/conduit.css'), 'utf8')));
  assert.ok(html.includes('class="site-content speech-proof"'));
  assert.ok(html.includes('.site-content.speech-proof { max-width: 75ch; }'));
  assert.ok(html.includes('.speech-proof code { overflow-wrap: anywhere; }'));
  assert.ok(html.includes('.speech-proof audio { display: block; width: 100%; }'));
  for (const text of ['audio controls', 'NoAdmittedAcousticBack', 'publication.json', 'No physical-device', 'human listening', commit]) assert.ok(html.includes(text));
});
test('absent optional evidence is allowed; partial evidence is rejected', t => {
  const root = mkdtempSync(path.join(tmpdir(), 'speech-absent-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  assert.equal(verifySharedSpeechEvidence(root), null);
  const retained = fixture(t);
  rmSync(path.join(retained, SHARED_SPEECH_EVIDENCE_ROOT, 'manifest.json'));
  assert.throws(() => verifySharedSpeechEvidence(retained));
});
function foreignPlanProjection(data) {
  const report = JSON.parse(data);
  let changed = false;
  function visit(value) {
    if (!value || typeof value !== 'object') return;
    if (Array.isArray(value.configuration)) for (const entry of value.configuration) {
      if (entry.key === 'projection' && entry.value?.Text) {
        entry.value.Text = 'f'.repeat(64); changed = true;
      }
    }
    for (const child of Object.values(value)) visit(child);
  }
  visit(report.plan);
  assert.ok(changed, 'fixture contains an actual bound plan projection');
  return JSON.stringify(report);
}
for (const [label, name, mutate, rehash] of [
  ['raw byte corruption', 'you-would.native', data => Buffer.concat([data, Buffer.from([1])]), false],
  ['WAV geometry despite valid digest', 'you-would.wav', data => { data.writeUInt32LE(16000, 24); return data; }, true],
  ['false model conditioning despite valid digest', 'you-would.json', data => { const r = JSON.parse(data); r.model_terminal.retained_without_conditioning = []; return JSON.stringify(r); }, true],
  ['foreign model source despite valid digest', 'you-would.json', data => { const r = JSON.parse(data); r.model_terminal.source_identity = 'f'.repeat(64); return JSON.stringify(r); }, true],
  ['foreign plan projection despite valid digest', 'you-would.json', foreignPlanProjection, true],
  ['manifest path inventory', 'manifest.json', data => { const r = JSON.parse(data); r.files['../escape'] = r.files['you-would.wav']; return JSON.stringify(r); }, false],
  ['false listening claim', 'manifest.json', data => { const r = JSON.parse(data); r.human_listening = true; return JSON.stringify(r); }, false],
]) test(`rejects ${label}`, t => { const root = fixture(t); change(root, name, mutate, rehash); assert.throws(() => verifySharedSpeechEvidence(root)); });
test('rejects symbolic links even when target bytes match', t => {
  const root = fixture(t);
  const file = path.join(root, SHARED_SPEECH_EVIDENCE_ROOT, 'you-would.wav');
  const target = path.join(root, 'same.wav');
  writeFileSync(target, readFileSync(file)); rmSync(file); symlinkSync(target, file);
  assert.throws(() => verifySharedSpeechEvidence(root));
});
