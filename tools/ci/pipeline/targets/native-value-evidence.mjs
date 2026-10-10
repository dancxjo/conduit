import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

// Bind executable Source and refusals to the exact sealed browser runtime.
export function retainNativeValueEvidence(results, destination, runtime, sourceHead) {
  if (!/^[a-f0-9]{40}$/.test(sourceHead ?? '')) throw new Error('Native Type proof requires exact source head');
  const reports = readdirSync(results, { recursive: true })
    .filter(name => path.basename(name) === 'native-value-browser-evidence.json');
  if (reports.length !== 4) throw new Error('Expected four current native Type browser proofs');
  const wasmDigest = sha256(readFileSync(runtime));
  const names = new Set();
  const captures = reports.map(file => {
    const bytes = readFileSync(path.join(results, file));
    if (bytes.length > 1024 * 1024) throw new Error('Native Type proof exceeds capture bound');
    const proof = JSON.parse(bytes);
    if (proof.wasm_sha256 !== wasmDigest || !proof.browser?.includes('Chrome/')
        || !Array.isArray(proof.rows) || proof.rows.length !== 1) {
      throw new Error('Native Type proof differs from sealed Chromium runtime');
    }
    const row = proof.rows[0];
    if (names.has(row.name) || typeof row.source !== 'string' || row.source.length > 8192) {
      throw new Error('Duplicate or unbounded native Type Source');
    }
    names.add(row.name);
    if (row.name === 'computed window' || row.name === 'equivalent checked arguments') {
      if (row.effect?.effect_kind !== 'manifestation'
          || row.effect.presentation_kind !== 'presentation/bool-value' || row.effect.text !== 'true'
          || !row.effect.active_play_id || row.receipt?.disposition !== 'completed'
          || row.receipt.active_play_id !== row.effect.active_play_id) {
        throw new Error('Native Type proof lacks completed kernel execution');
      }
    } else if (!Number.isInteger(row.status) || row.status >= 0
        || row.refusal?.disposition !== 'refused-before-play' || row.effect || row.receipt) {
      throw new Error('Invalid native Type Source reached Play');
    }
    return { file, bytes, name: row.name, browser: proof.browser };
  });
  const expected = ['computed window', 'equivalent checked arguments', 'wrong history count', 'checked argument overflow'];
  if (expected.some(name => !names.has(name))) throw new Error('Native Type proof misses an acceptance case');
  mkdirSync(destination);
  const evidence = captures.map((capture, index) => {
    const file = `case-${index + 1}.json`;
    copyFileSync(path.join(results, capture.file), path.join(destination, file));
    return { file, sha256: sha256(capture.bytes), name: capture.name, browser: capture.browser };
  });
  writeFileSync(path.join(destination, 'manifest.json'), JSON.stringify({
    schema: 'conduit.browser/native-value-type-proof@1', source_head: sourceHead,
    wasm_sha256: wasmDigest, project: 'chromium', workers: 1, retries: 0,
    kernel_cases: 2, source_refusals: 2, evidence,
    proof_class: 'Actual WebAssembly browser kernel execution; no physical or firmware claim',
  }, null, 2));
}
