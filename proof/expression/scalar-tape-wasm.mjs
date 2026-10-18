// Actual WASM execution of the existing portable evaluator, not browser installation.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const target = resolve(process.env.CARGO_TARGET_DIR ?? 'target');
const wasm = await readFile(resolve(target,
  'wasm32-unknown-unknown/debug/examples/scalar_tape_wasm.wasm'));
const module = new WebAssembly.Module(wasm);
assert.deepEqual(WebAssembly.Module.imports(module), [], 'proof has no ambient imports');
const instance = new WebAssembly.Instance(module, {});
const api = instance.exports;
assert.equal(api.scalar_tape_proof_run(), 0);
const pointer = api.scalar_tape_proof_pointer();
const length = api.scalar_tape_proof_length();
assert.ok(length > 0 && length <= 1_000_000);
assert.ok(pointer > 0 && pointer + length <= api.memory.buffer.byteLength);
const receipt = JSON.parse(Buffer.from(api.memory.buffer, pointer, length).toString('utf8'));
assert.equal(receipt.schema, 'conduit.scalar-tape/wasm-parity@1');
assert.equal(receipt.proof_class, 'portable-wasm-evaluator');
assert.equal(receipt.profiles.length, 8);
for (const domain of ['integer', 'boolean']) {
  assert.deepEqual(receipt.profiles.filter(p => p.domain === domain).map(p => p.operations),
    [1, 3, 8, 32]);
}
for (const profile of receipt.profiles) {
  assert.equal(profile.cases.length, profile.domain === 'integer' ? 7 : 5);
  assert.ok(profile.maximum_instructions <= 128);
  assert.ok(profile.scratch_payload_bytes > 0 && profile.output_capacity_bytes > 0);
  for (const name of ['source_document_id', 'checked_plot_id', 'expanded_plot_id', 'program']) {
    assert.ok(profile.identity[name]);
  }
  for (const item of profile.cases) {
    assert.equal(item.warm_iterations, 16);
    assert.equal(item.allocations, 0);
    assert.equal(item.reallocations, 0);
  }
}
assert.equal(api.scalar_tape_proof_run(), 2, 'bounded proof instance refuses repeated execution');
assert.equal(api.scalar_tape_proof_pointer(), pointer);
assert.equal(api.scalar_tape_proof_length(), length);
const fixtures = {};
for (const name of ['expression_baseline', 'expression_boolean_baseline']) {
  const path = `architecture/plot/tests/fixtures/${name}.conduit`;
  fixtures[path] = digest(await readFile(path));
}
const nativePath = 'docs/audits/evidence/scalar-tape-mixed-cpu-samples.json';
const nativeBytes = await readFile(nativePath);
const native = JSON.parse(nativeBytes);
for (const [path, hash] of Object.entries(fixtures)) {
  assert.equal(native.file_sha256[path], hash, 'same exact fixtures as installed std evidence');
}
let matchedPlays = 0;
for (const profile of receipt.profiles) {
  const plays = native.records.filter(row => row.class === 'SCALAR_PRODUCT_SAMPLE'
    && row.domain === profile.domain && row.operations === profile.operations);
  assert.ok(plays.some(row => row.tape === 'true'));
  assert.ok(plays.some(row => row.tape === 'false'));
  for (const row of plays) {
    assert.equal(profile.identity.source_document_id, row.source_document);
    assert.equal(profile.identity.checked_plot_id, row.checked);
    assert.equal(profile.identity.expanded_plot_id, row.expanded);
    assert.ok(row.plan && row.play && row.provider && row.artifact);
    matchedPlays++;
  }
}
const git = (...args) => execFileSync('git', args, {encoding: 'utf8'}).trim();
console.log(JSON.stringify({...receipt, provenance: {
  source_commit: git('rev-parse', 'HEAD'),
  source_clean: git('status', '--porcelain').length === 0,
  wasm_sha256: digest(wasm), fixtures, node: process.version,
  target: "wasm32-unknown-unknown", profile: "dev",
  compiler: execFileSync(process.env.RUSTC ?? "rustc", ["--version", "--verbose"], {encoding: "utf8"}).trim(),
  native_identity_evidence: {path: nativePath, sha256: digest(nativeBytes),
    source_commit: native.measurement_code_source, matched_plays: matchedPlays},
  scope: 'WASM parity and allocation proof only; no installed browser Back or target speed claim',
}}, null, 2));
