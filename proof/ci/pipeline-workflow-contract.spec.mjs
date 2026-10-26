import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, rmSync, truncateSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { runWorkflowContract } from '../../tools/ci/pipeline/workflow-contract.mjs';

// These fixture Rust sources never invoke Node. Calling the real repository
// wrapper from this corpus would recursively launch this same test suite.
function fixture(t, source) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'workflow-contract-fixture-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const directory = path.join(root, 'repository');
  const temporaryParent = path.join(root, 'temporary');
  mkdirSync(path.join(directory, 'tools/xtask/tests'), { recursive: true }); mkdirSync(temporaryParent);
  writeFileSync(path.join(directory, 'tools/xtask/tests/ci_workflow_contract.rs'), source);
  const output = [];
  const run = overrides => runWorkflowContract({ root: directory, temporaryParent,
    emit: (text, stream) => output.push({ text, stream }), ...overrides });
  return { directory, temporaryParent, output, run };
}

test('standalone Rust wrapper actually compiles and executes with explicit manifest directory and uncaptured output', t => {
  const f = fixture(t, `#[test] fn pipeline_preserves_target_and_artifact_identity() {
    assert!(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ci_workflow_contract.rs").is_file());
    println!("actual fixture proof executed");
  }`);
  const calls = [];
  const result = f.run({ command(program, args, options) {
    calls.push({ program, args, options }); return spawnSync(program, args, options);
  } });
  assert.equal(result.compiled, true); assert.equal(result.executed, true);
  assert.ok(result.executableBytes > 0 && result.executableBytes <= 32 * 1024 ** 2);
  assert.equal(calls.length, 3); assert.equal(calls[0].program, 'rustc');
  assert.ok(calls[0].args.includes('--test')); assert.ok(calls[0].args.includes('warnings'));
  assert.equal(calls[0].options.env.CARGO_MANIFEST_DIR, path.join(f.directory, 'tools/xtask'));
  assert.ok(calls[2].args.includes('--nocapture'));
  assert.match(f.output.map(x => x.text).join(''), /actual fixture proof executed/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('real Rust compilation failure refuses and cleans the private output', t => {
  const f = fixture(t, '#[test] fn broken() { let _: u8 = "invalid type"; }');
  assert.throws(f.run, /compilation failed/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('real Rust test failure refuses and retains its uncaptured diagnostic in output', t => {
  const f = fixture(t, '#[test] fn pipeline_preserves_target_and_artifact_identity() { panic!("deliberate fixture failure"); }');
  assert.throws(f.run, /execution failed/);
  assert.match(f.output.map(x => x.text).join(''), /deliberate fixture failure/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('compile timeout has a finite budget and cannot leave a reusable executable', t => {
  const f = fixture(t, '#[test] fn pass() {}');
  assert.throws(() => f.run({ command(_program, _args, options) {
    assert.equal(options.timeout, 120000); assert.equal(options.maxBuffer, 8 * 1024 ** 2);
    return { error: new Error('timeout fixture'), signal: 'SIGTERM', status: null };
  } }), /compilation failed/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('oversized compiler output is never executed and private files are removed', t => {
  const f = fixture(t, '#[test] fn pass() {}'); let calls = 0;
  assert.throws(() => f.run({ command(_program, args) {
    calls++; const binary = args[args.indexOf('-o') + 1];
    writeFileSync(binary, 'x'); truncateSync(binary, 33 * 1024 ** 2);
    return { status: 0, stdout: '', stderr: '' };
  } }), /output bound/);
  assert.equal(calls, 1); assert.deepEqual(readdirSync(f.temporaryParent), []);
});


test('empty Rust test inventory cannot silently remove the Node corpus', t => {
  const f = fixture(t, '');
  assert.throws(f.run, /single expected Rust test/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});
