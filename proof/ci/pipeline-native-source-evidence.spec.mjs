import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stageCheckedSourceEvidence } from '../../tools/ci/pipeline/targets/native-source-evidence.mjs';

// Retained real collector bytes; this fixture does not assert a fresh product run.
const packet = fileURLToPath(new URL('../../docs/audits/evidence/native-checked-source-20261023/f88-success/patchbay-checked-source/', import.meta.url));
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'native-source-custody-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const evidence = path.join(root, 'input'), destination = path.join(root, 'output');
  const source = path.join(evidence, 'patchbay-checked-source');
  mkdirSync(evidence); mkdirSync(destination); cpSync(packet, source, { recursive: true });
  const section = JSON.parse(readFileSync(path.join(source, 'manifest.json')));
  const journey = { source_commit: 'f88b3c481b20b296195b4ac8920add42078fd6ca',
    image_sha256: '3b75f2452998dd947c3689e0ec510fff12ad7b52dad6b29653de913f57fbeaf7',
    patchbay_actions: { checked_source_inspection: section } };
  return { source, destination, section, journey,
    rewrite: () => writeFileSync(path.join(source, 'manifest.json'), JSON.stringify(section)),
    stage: () => stageCheckedSourceEvidence(journey, evidence, destination) };
}
test('retains all seven real Source PNGs and exact manifest bytes', t => {
  const f = fixture(t); f.stage();
  const retained = path.join(f.destination, 'patchbay-checked-source');
  assert.equal(f.section.captures.length, 7);
  assert.deepEqual(readdirSync(retained).sort(), readdirSync(f.source).sort());
  for (const name of readdirSync(f.source)) assert.deepEqual(readFileSync(path.join(retained, name)), readFileSync(path.join(f.source, name)));
});
test('ordinary journeys without Source inspection need no additional directory', t => {
  const f = fixture(t); stageCheckedSourceEvidence({ patchbay_actions: {} }, '/absent-evidence', f.destination);
  assert.deepEqual(readdirSync(f.destination), []);
});
for (const name of ['manifest.json', 'source-selected.png']) {
  test(`missing ${name} refuses before retention`, t => {
    const f = fixture(t); rmSync(path.join(f.source, name));
    assert.throws(f.stage); assert.deepEqual(readdirSync(f.destination), []);
  });
}
test('substituted PNG refuses despite correct filename', t => {
  const f = fixture(t); writeFileSync(path.join(f.source, 'source-selected.png'), readFileSync(path.join(f.source, 'details-gear.png')));
  assert.throws(f.stage, /capture bytes differ/); assert.deepEqual(readdirSync(f.destination), []);
});
test('manifest cannot substitute a different Source identity', t => {
  const f = fixture(t), manifest = structuredClone(f.section); manifest.checked_plot_id = '0'.repeat(64);
  writeFileSync(path.join(f.source, 'manifest.json'), JSON.stringify(manifest));
  assert.throws(f.stage, /manifest differs/);
});
for (const [name, mutate] of [
  ['escaping file', f => { f.section.captures[0].image.png = '../source-selected.png'; }],
  ['escaping checkpoint', f => { f.section.captures[0].checkpoint = '../source-selected'; }],
  ['duplicate', f => { f.section.captures.push(f.section.captures[0]); }],
  ['length', f => { f.section.captures[0].image.png_bytes += 1; }],
  ['dimensions', f => { f.section.captures[0].image.width += 1; }],
  ['format', f => { f.section.captures[0].image.pixel_format = 'RGB8'; }],
  ['pixel digest', f => { f.section.captures[0].image.pixel_sha256 = null; }],
  ['bound', f => { f.section.captures = Array(261).fill(f.section.captures[0]); }],
  ['empty', f => { f.section.captures = []; }],
]) {
  test(`matching manifest and journey still refuse invalid ${name}`, t => {
    const f = fixture(t); mutate(f); f.rewrite(); assert.throws(f.stage);
    assert.deepEqual(readdirSync(f.destination), []);
  });
}
test('unlisted files refuse', t => {
  const f = fixture(t); writeFileSync(path.join(f.source, 'unlisted.png'), 'unlisted');
  assert.throws(f.stage, /inventory/);
});
for (const name of ['manifest.json', 'source-selected.png']) {
  test(`symlinked ${name} refuses`, t => {
    const f = fixture(t), file = path.join(f.source, name); rmSync(file); symlinkSync(path.join(packet, name), file);
    assert.throws(f.stage, /regular file/);
  });
}
test('symlinked capture directory refuses', t => {
  const f = fixture(t); rmSync(f.source, { recursive: true }); symlinkSync(packet, f.source);
  assert.throws(f.stage, /real directory/);
});
for (const field of ['source_commit', 'image_sha256']) {
  test(`missing journey ${field} refuses`, t => {
    const f = fixture(t); delete f.journey[field]; assert.throws(f.stage, /source\/image identity/);
  });
}
