import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { retainExactQuantityEvidence } from '../../tools/ci/pipeline/targets/quantity-evidence.mjs';

const head = 'a'.repeat(40);
const kinds = ['units/convert', 'units/convert-temperature-difference',
  'units/compare', 'units/compare-temperature-differences'];
function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-quantity-evidence-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const results = path.join(root, 'results');
  const runtime = path.join(root, 'runtime.wasm');
  const destination = path.join(root, 'retained');
  writeFileSync(runtime, 'exact sealed runtime');
  const wasm_sha256 = createHash('sha256').update(readFileSync(runtime)).digest('hex');
  const reports = [33, 10, 8, 3].map((count, corpus) => ({
    wasm_sha256, browser: 'HeadlessChrome/151.0.0.0',
    rows: Array.from({ length: count }, (_, index) => {
      const comparator = corpus === 2;
      const kind = comparator ? 'units/converted-equals' : kinds[corpus === 0 || corpus === 3 ? 0 : index % 4];
      const [left, right, expected] = [
        ['1kHz', 'Hz', '1000Hz'], ['1kHz', 'Hz', '999Hz'],
        ['1Hz', 'm', '1m'], ['1kHz', 'Hz', '1m'],
      ][index % 4];
      const invocation = index < 4 ? 'units/converted-equals' : '=?';
      const snapshot_scale = ['1.7018', '2', '1.7018'][index];
      return { kind, ...(corpus === 3 ? { left: '1smoot', right: 'm', snapshot_scale,
        source: `unit smoot : Distance = { reference: m, scale: ${snapshot_scale} } coefficient == ${index === 1 ? 2 : 17018} exponent == ${index === 1 ? 0 : -4}` } : {}), ...(comparator ? {left, right, expected, invocation,
        source: `exact: ${invocation}(expected = ${expected})`} : {}),
        effect: { effect_kind: 'manifestation', face_kind: comparator ? 'face/text' : 'face/bool-value',
          text: comparator ? (index % 4 === 0 ? 'Exactly 1000 Hz' : 'Conversion did not yield exactly 1000 Hz') : 'true', active_play_id: `play/${index}`,
          expanded_gears: [{ kind_id: kind, implementation_id: comparator ? 'browser/converted-equals@1' : 'browser/exact-fixture@1' }] },
        receipt: { disposition: 'completed', active_play_id: `play/${index}` } };
    }),
  }));
  const derived = {
    wasm_sha256, browser: 'HeadlessChrome/151.0.0.0',
    rows: ['(30cm / 2s) == (15cm / 1s)', '(2m * 3s) == (3s * 2m)',
      '(2km * 3m) == (2000m * 300cm)', '(30cm / 2s) != (14cm / 1s)'].map((predicate, index) => ({
      kind: 'units/dimension', left: predicate, right: 'true',
      source: `operation: units/dimension(source=m) operation.dimension >> (${predicate}) >> show.value`,
      effect: { effect_kind: 'manifestation', face_kind: 'face/bool-value', text: 'true',
        active_play_id: `derived/${index}`, expanded_gears: [{ kind_id: 'units/dimension', implementation_id: 'browser/unit-dimension@1' }] },
      receipt: { disposition: 'completed', active_play_id: `derived/${index}` },
    })),
  };
  const save = () => {
    mkdirSync(results, { recursive: true });
    writeFileSync(path.join(results, 'derived-quantity-browser-evidence.json'), JSON.stringify(derived));
    reports.forEach((report, index) => {
    const folder = path.join(results, `corpus-${index}`); mkdirSync(folder, { recursive: true });
    writeFileSync(path.join(folder, 'exact-quantity-browser-evidence.json'), JSON.stringify(report));
  });
  };
  save();
  return { results, runtime, destination, reports, derived, save };
}

test('quantity captures retain original bytes and bind all five roles to sealed runtime/source', t => {
  const f = fixture(t);
  retainExactQuantityEvidence(f.results, f.destination, f.runtime, head);
  const manifest = JSON.parse(readFileSync(path.join(f.destination, 'manifest.json')));
  assert.equal(manifest.source_head, head); assert.equal(manifest.kernel_cases, 58);
  assert.equal(manifest.workers, 1); assert.equal(manifest.retries, 0);
  assert.equal(manifest.evidence.length, 5);
  assert.deepEqual(manifest.evidence.map(report => report.file), ['corpus-33.json', 'corpus-10.json', 'corpus-8.json', 'corpus-3.json', 'corpus-4.json']);
  for (const report of manifest.evidence) {
    const bytes = readFileSync(path.join(f.destination, report.file));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), report.sha256);
    assert.equal(JSON.parse(bytes).rows.length, report.cases);
    const original = report.cases === 4 ? path.join(f.results, 'derived-quantity-browser-evidence.json')
      : path.join(f.results, `corpus-${[33, 10, 8, 3].indexOf(report.cases)}`, 'exact-quantity-browser-evidence.json');
    assert.deepEqual(bytes, readFileSync(original));
  }
});

test('quantity capture refusal leaves no success artifact for stale runtime or kernel receipts', t => {
  for (const mutation of [
    f => { f.reports[0].wasm_sha256 = 'b'.repeat(64); },
    f => { f.reports[1].rows[0].receipt.active_play_id = 'foreign/play'; },
    f => { f.reports[0].rows[0].effect.text = 'false'; },
    f => { f.reports[0].rows.pop(); },
    f => { f.reports[2].rows[0].effect.text = 'Conversion did not yield exactly 1000 Hz'; },
    f => { f.reports[2].rows[4] = structuredClone(f.reports[2].rows[0]); },
    f => { f.reports[2].rows[0].effect.expanded_gears[0].implementation_id = 'browser/exact-fixture@1'; },
    f => { f.reports[2].rows[0].source = 'exact: units/converted-equals(expected = 999Hz)'; },
    f => { f.reports[3].rows[1].snapshot_scale = '1.7018'; },
    f => { f.reports[3].rows[0].source = 'unit smoot : Distance = { reference: m, scale: 2 }'; },
    f => { f.reports[1].rows.forEach(row => { row.kind = kinds[0]; }); },
  ]) {
    const f = fixture(t); mutation(f); f.save();
    assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, head));
    assert.equal(existsSync(f.destination), false);
  }
});

test('quantity capture refuses duplicate current corpus and invalid source identity', t => {
  const f = fixture(t);
  assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, 'dev'));
  const duplicate = path.join(f.results, 'stale'); mkdirSync(duplicate);
  writeFileSync(path.join(duplicate, 'exact-quantity-browser-evidence.json'), JSON.stringify(f.reports[0]));
  assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, head), /four current/);
  assert.equal(existsSync(f.destination), false);
});


test('derived quantity capture refuses malformed, stale, incomplete, and duplicate predicates without artifacts', t => {
  for (const mutation of [
    f => { f.derived.wasm_sha256 = 'b'.repeat(64); },
    f => { f.derived.browser = 'Firefox/1'; },
    f => { f.derived.rows = null; },
    f => { f.derived.rows.pop(); },
    f => { f.derived.rows[1] = structuredClone(f.derived.rows[0]); },
    f => { f.derived.rows[0].left = '(1m / 1s) == (1m / 1s)'; },
    f => { f.derived.rows[0].source = 'true'; },
    f => { f.derived.rows[0].effect.text = 'false'; },
    f => { f.derived.rows[0].effect.face_kind = 'face/text'; },
    f => { f.derived.rows[0].effect.expanded_gears[0].implementation_id = 'browser/exact-fixture@1'; },
    f => { f.derived.rows[0].receipt.disposition = 'failed'; },
    f => { f.derived.rows[0].receipt.active_play_id = 'stale/play'; },
    f => { f.derived.rows[0].effect.active_play_id = ''; },
  ]) {
    const f = fixture(t); mutation(f); f.save();
    assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, head));
    assert.equal(existsSync(f.destination), false);
  }
});

test('derived quantity capture requires exactly one current report', t => {
  for (const duplicate of [false, true]) {
    const f = fixture(t);
    const report = path.join(f.results, 'derived-quantity-browser-evidence.json');
    if (duplicate) {
      const folder = path.join(f.results, 'duplicate'); mkdirSync(folder);
      writeFileSync(path.join(folder, 'derived-quantity-browser-evidence.json'), readFileSync(report));
    } else rmSync(report);
    assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, head), /one current derived/);
    assert.equal(existsSync(f.destination), false);
  }
});

test('derived quantity capture refuses malformed JSON before artifact creation', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.results, 'derived-quantity-browser-evidence.json'), '{invalid');
  assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, head));
  assert.equal(existsSync(f.destination), false);
});
