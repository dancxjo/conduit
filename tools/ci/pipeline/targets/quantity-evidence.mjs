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
  if (reports.length !== 4) throw new Error('Expected four current exact quantity browser proofs');
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
    if (proof.rows.length === 3) {
      if (proof.rows.map(row => row.snapshot_scale).join(',') !== '1.7018,2,1.7018'
          || proof.rows.some((row, index) => row.kind !== 'units/convert' || row.left !== '1smoot' || row.right !== 'm'
            || !row.source?.includes(`unit smoot : Distance = { reference: m, scale: ${row.snapshot_scale} }`)
            || !row.source.includes(`coefficient == ${index === 1 ? 2 : 17018}`)
            || !row.source.includes(`exponent == ${index === 1 ? 0 : -4}`))) {
        throw new Error('Quantity proof lacks immutable authored unit snapshots');
      }
    }
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
          || row.effect.face_kind !== (comparator ? 'face/text' : 'face/bool-value')
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
  if (captures.map(capture => capture.cases).sort((a, b) => a - b).join(',') !== '3,8,10,33'
      || kinds.size !== 5 || comparisons.size !== 8 || expectedKinds.some(kind => !kinds.has(kind))) {
    throw new Error('Quantity proof does not cover all four complete corpora and all five roles');
  }
  const derivedReports = readdirSync(results, { recursive: true })
    .filter(name => path.basename(name) === 'derived-quantity-browser-evidence.json');
  if (derivedReports.length !== 1) throw new Error('Expected one current derived quantity browser proof');
  const derivedFile = path.join(results, derivedReports[0]);
  const derivedBytes = readFileSync(derivedFile);
  if (derivedBytes.length > 4 * 1024 * 1024) throw new Error('Derived quantity proof exceeds capture bound');
  const derived = JSON.parse(derivedBytes);
  const predicates = new Set([
    '(30cm / 2s) == (15cm / 1s)',
    '(2m * 3s) == (3s * 2m)',
    '(2km * 3m) == (2000m * 300cm)',
    '(30cm / 2s) != (14cm / 1s)',
  ]);
  if (derived.wasm_sha256 !== wasmDigest || !derived.browser?.includes('Chrome/')
      || !Array.isArray(derived.rows) || derived.rows.length !== 4) {
    throw new Error('Derived quantity proof differs from sealed Chromium runtime or complete corpus');
  }
  for (const row of derived.rows) {
    const planned = row.effect?.expanded_gears?.find(gear => gear.kind_id === 'units/dimension');
    if (row.kind !== 'units/dimension' || row.right !== 'true' || !predicates.delete(row.left)
        || !row.source?.includes(`operation.dimension >> (${row.left}) >> show.value`)
        || !row.source.includes('operation: units/dimension(source=m)')
        || row.effect?.effect_kind !== 'manifestation' || row.effect.face_kind !== 'face/bool-value'
        || row.effect.text !== 'true' || planned?.implementation_id !== 'browser/unit-dimension@1'
        || !row.effect.active_play_id || row.receipt?.disposition !== 'completed'
        || row.receipt.active_play_id !== row.effect.active_play_id) {
      throw new Error('Derived quantity proof lacks distinct completed predicates and installed dimension Back');
    }
  }
  captures.push({ file: derivedFile, bytes: derivedBytes, cases: 4, browser: derived.browser });
  mkdirSync(destination);
  const evidence = captures.map(capture => {
    const name = `corpus-${capture.cases}.json`;
    copyFileSync(capture.file, path.join(destination, name));
    return { file: name, sha256: sha256(capture.bytes), cases: capture.cases, browser: capture.browser };
  });
  writeFileSync(path.join(destination, 'manifest.json'), JSON.stringify({
    schema: 'conduit.browser/exact-quantity-proof@1', source_head: sourceHead,
    wasm_sha256: wasmDigest, project: 'chromium', workers: 1, retries: 0,
    kernel_cases: 58, evidence,
    proof_class: 'Actual WebAssembly browser kernel execution; no physical or firmware claim',
  }, null, 2));
}
