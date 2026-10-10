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
  const reports = [33, 10, 8].map((count, corpus) => ({
    wasm_sha256, browser: 'HeadlessChrome/151.0.0.0',
    rows: Array.from({ length: count }, (_, index) => {
      const comparator = corpus === 2;
      const kind = comparator ? 'units/converted-equals' : kinds[corpus === 0 ? 0 : index % 4];
      const [left, right, expected] = [
        ['1kHz', 'Hz', '1000Hz'], ['1kHz', 'Hz', '999Hz'],
        ['1Hz', 'm', '1m'], ['1kHz', 'Hz', '1m'],
      ][index % 4];
      const invocation = index < 4 ? 'units/converted-equals' : '=?';
      return { kind, ...(comparator ? {left, right, expected, invocation,
        source: `exact: ${invocation}(expected = ${expected})`} : {}),
        effect: { effect_kind: 'manifestation', presentation_kind: comparator ? 'presentation/text' : 'presentation/bool-value',
          text: comparator ? (index % 4 === 0 ? 'Exactly 1000 Hz' : 'Conversion did not yield exactly 1000 Hz') : 'true', active_play_id: `play/${index}`,
          expanded_gears: [{ kind_id: kind, implementation_id: comparator ? 'browser/converted-equals@1' : 'browser/exact-fixture@1' }] },
        receipt: { disposition: 'completed', active_play_id: `play/${index}` } };
    }),
  }));
  const save = () => reports.forEach((report, index) => {
    const folder = path.join(results, `corpus-${index}`); mkdirSync(folder, { recursive: true });
    writeFileSync(path.join(folder, 'exact-quantity-browser-evidence.json'), JSON.stringify(report));
  });
  save();
  return { results, runtime, destination, reports, save };
}

test('quantity captures retain original bytes and bind all five roles to sealed runtime/source', t => {
  const f = fixture(t);
  retainExactQuantityEvidence(f.results, f.destination, f.runtime, head);
  const manifest = JSON.parse(readFileSync(path.join(f.destination, 'manifest.json')));
  assert.equal(manifest.source_head, head); assert.equal(manifest.kernel_cases, 51);
  assert.equal(manifest.workers, 1); assert.equal(manifest.retries, 0);
  for (const report of manifest.evidence) {
    const bytes = readFileSync(path.join(f.destination, report.file));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), report.sha256);
    assert.equal(JSON.parse(bytes).rows.length, report.cases);
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
  assert.throws(() => retainExactQuantityEvidence(f.results, f.destination, f.runtime, head), /three current/);
  assert.equal(existsSync(f.destination), false);
});
