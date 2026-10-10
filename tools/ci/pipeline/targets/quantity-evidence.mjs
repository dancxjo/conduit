import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

// Retain the current browser execution receipts inside the existing product
// artifact, bound to the sealed runtime rather than an unrelated cached WASM.
export function retainExactQuantityEvidence(results, destination, runtime, sourceHead) {
  if (!/^[a-f0-9]{40}$/.test(sourceHead ?? '')) throw new Error('Quantity proof requires exact source head');
  const reports = readdirSync(results, { recursive: true })
    .filter(name => path.basename(name) === 'exact-quantity-browser-evidence.json');
  if (reports.length !== 3) throw new Error('Expected three current exact quantity browser proofs');
  const wasmDigest = sha256(readFileSync(runtime));
  const kinds = new Set();
  const comparisons = new Set();
  const comparatorCases = new Map([
    ['1kHz|Hz|1000Hz', 'Exactly 1000 Hz'],
    ['1kHz|Hz|999Hz', 'Conversion did not yield exactly 1000 Hz'],
    ['1Hz|m|1m', 'Conversion did not yield exactly 1000 Hz'],
    ['1kHz|Hz|1m', 'Conversion did not yield exactly 1000 Hz'],
  ]);
  const captures = reports.map(name => {
    const file = path.join(results, name);
    const bytes = readFileSync(file);
    if (bytes.length > 4 * 1024 * 1024) throw new Error('Quantity proof exceeds capture bound');
    const proof = JSON.parse(bytes);
    if (proof.wasm_sha256 !== wasmDigest || !proof.browser?.includes('Chrome/')) {
      throw new Error('Quantity proof differs from sealed Chromium runtime');
    }
    if (!Array.isArray(proof.rows)) throw new Error('Quantity proof has no execution rows');
    for (const row of proof.rows) {
      const planned = row.effect?.expanded_gears?.find(gear => gear.kind_id === row.kind);
      const comparator = row.kind === 'units/converted-equals';
      const caseKey = `${row.left}|${row.right}|${row.expected}`;
      const comparisonKey = `${row.invocation}|${caseKey}`;
      const expectedText = comparator ? comparatorCases.get(caseKey) : 'true';
      if (comparator) {
        if (!expectedText || !['units/converted-equals', '=?'].includes(row.invocation)
            || comparisons.has(comparisonKey)
            || !row.source?.includes(`exact: ${row.invocation}(expected = ${row.expected})`)) {
          throw new Error('Quantity proof lacks complete full-name and alias comparator cases');
        }
        comparisons.add(comparisonKey);
      }
      if (row.effect?.effect_kind !== 'manifestation'
          || row.effect.presentation_kind !== (comparator ? 'presentation/text' : 'presentation/bool-value')
          || row.effect.text !== expectedText
          || (comparator ? planned?.implementation_id !== 'browser/converted-equals@1'
                         : !planned?.implementation_id?.startsWith('browser/exact-'))
          || !row.effect.active_play_id || row.receipt?.disposition !== 'completed'
          || row.receipt.active_play_id !== row.effect.active_play_id) {
        throw new Error('Quantity proof lacks exact completed kernel execution');
      }
      kinds.add(row.kind);
    }
    return { file, bytes, cases: proof.rows.length, browser: proof.browser };
  });
  const expectedKinds = ['units/convert', 'units/convert-temperature-difference',
    'units/compare', 'units/compare-temperature-differences', 'units/converted-equals'];
  if (captures.map(capture => capture.cases).sort((a, b) => a - b).join(',') !== '8,10,33'
      || kinds.size !== 5 || comparisons.size !== 8 || expectedKinds.some(kind => !kinds.has(kind))) {
    throw new Error('Quantity proof does not cover all three complete corpora and all five roles');
  }
  mkdirSync(destination);
  const evidence = captures.map(capture => {
    const name = `corpus-${capture.cases}.json`;
    copyFileSync(capture.file, path.join(destination, name));
    return { file: name, sha256: sha256(capture.bytes), cases: capture.cases, browser: capture.browser };
  });
  writeFileSync(path.join(destination, 'manifest.json'), JSON.stringify({
    schema: 'conduit.browser/exact-quantity-proof@1', source_head: sourceHead,
    wasm_sha256: wasmDigest, project: 'chromium', workers: 1, retries: 0,
    kernel_cases: 51, evidence,
    proof_class: 'Actual WebAssembly browser kernel execution; no physical or firmware claim',
  }, null, 2));
}
