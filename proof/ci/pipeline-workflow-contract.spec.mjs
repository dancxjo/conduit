import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, truncateSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { runWorkflowContract } from '../../tools/ci/pipeline/workflow-contract.mjs';

const actualSource = readFileSync(new URL('../../tools/xtask/tests/ci_workflow_contract.rs', import.meta.url), 'utf8');
function fixture(t, source = actualSource) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'workflow-contract-fixture-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const directory = path.join(root, 'repository'), temporaryParent = path.join(root, 'temporary');
  mkdirSync(path.join(directory, 'tools/xtask/tests'), { recursive: true }); mkdirSync(temporaryParent);
  mkdirSync(path.join(directory, 'proof/ci'), { recursive: true });
  const sourceFile = path.join(directory, 'tools/xtask/tests/ci_workflow_contract.rs');
  writeFileSync(sourceFile, source);
  const files = ['pipeline-first.spec.mjs', 'compiler-cache-second.spec.mjs'].map(name => path.join(directory, 'proof/ci', name));
  // The independent full corpus belongs to CLI. Launcher probes must not replay it.
  for (const file of files) writeFileSync(file, "throw new Error('full corpus must not be replayed by the launcher probe');\n");
  const output = [];
  const run = overrides => runWorkflowContract({ root: directory, temporaryParent,
    emit: (text, stream) => output.push({ text, stream }), ...overrides });
  return { directory, temporaryParent, sourceFile, files, output, run };
}

test('actual unchanged Rust wrapper compiles and runs real passing/failing Node launch probes without replaying the corpus', t => {
  const f = fixture(t), calls = [];
  const result = f.run({ command(program, args, options) {
    calls.push({ program, args, options }); return spawnSync(program, args, options);
  } });
  assert.equal(result.compiled, true); assert.equal(result.executed, true);
  assert.ok(result.executableBytes > 0 && result.executableBytes <= 32 * 1024 ** 2);
  assert.equal(result.canonicalNodeFiles, 2);
  assert.equal(result.passingNodeProbe, true); assert.equal(result.failingNodeProbe, true);
  assert.equal(calls.filter(call => call.program === 'rustc').length, 2);
  assert.equal(calls.length, 5);
  assert.ok(calls[0].args.includes('--test') && calls[0].args.includes('warnings'));
  assert.equal(calls[0].options.env.CARGO_MANIFEST_DIR, path.join(f.directory, 'tools/xtask'));
  assert.equal(calls.filter(call => call.args.includes('--nocapture')).length, 2);
  assert.match(f.output.map(x => x.text).join(''), /bounded launcher passing probe/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('ordinary direct Rust-wrapper execution still runs the complete actual Node corpus and propagates its failure', t => {
  const f = fixture(t), marker = path.join(f.directory, 'visited'); let directVerified = false;
  for (const file of f.files) writeFileSync(file, `import test from 'node:test'; import {appendFileSync} from 'node:fs';\ntest('actual corpus',()=>appendFileSync(${JSON.stringify(marker)},${JSON.stringify(path.basename(file))}+'\\n'));\n`);
  const result = f.run({ command(program, args, options) {
    const response = spawnSync(program, args, options);
    if (program === 'rustc' && args.includes(f.sourceFile) && response.status === 0) {
      const binary = args[args.indexOf('-o') + 1];
      const directEnvironment = { ...options.env }; delete directEnvironment.NODE_TEST_CONTEXT;
      const passing = spawnSync(binary, ['--nocapture'], { cwd: f.directory, env: directEnvironment, encoding: 'utf8' });
      assert.equal(passing.status, 0);
      assert.deepEqual(readFileSync(marker, 'utf8').trim().split('\n').sort(), f.files.map(file => path.basename(file)).sort());
      writeFileSync(f.files[0], "import test from 'node:test';import assert from 'node:assert/strict';test('actual corpus failure',()=>assert.fail('fixture failure'));\n");
      const failing = spawnSync(binary, ['--nocapture'], { cwd: f.directory, env: directEnvironment, encoding: 'utf8' });
      assert.ok(Number.isInteger(failing.status) && failing.status !== 0);
      directVerified = true;
    }
    return response;
  } });
  assert.ok(directVerified && result.passingNodeProbe && result.failingNodeProbe);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

for (const [name, mutate] of [
  ['missing Node launch', () => '#[test] fn pipeline_preserves_target_and_artifact_identity() {}'],
  ['incomplete corpus', source => source.replace('.collect::<Vec<_>>();', '.take(1).collect::<Vec<_>>();')],
  ['duplicate corpus arguments', source => source.replace('.args(files)', '.args(&files).args(&files)')],
  ['extra Node option', source => source.replace('.arg("--test")', '.arg("--test").arg("--no-warnings")')],
  ['wrong child working directory', source => source.replace('.current_dir(root)', '.current_dir(root.join("tools"))')],
]) {
  test(`actual Rust launcher refuses ${name}`, t => {
    const f = fixture(t, mutate(actualSource));
    assert.throws(f.run, /passing Node launch unverified|passing launch probe failed/);
    assert.deepEqual(readdirSync(f.temporaryParent), []);
  });
}

test('actual Rust launcher cannot ignore a real failing Node test', t => {
  const source = actualSource.replace('assert!(std::process::Command', 'let _ = std::process::Command').replace('.success());', '.success();');
  const f = fixture(t, source);
  assert.throws(f.run, /failing Node failure propagation invalid/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('real Rust compilation failure refuses and cleans private outputs', t => {
  const f = fixture(t, '#[test] fn broken() { let _: u8 = "invalid type"; }');
  assert.throws(f.run, /compilation failed/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('compile timeout is bounded and cannot leave a reusable executable', t => {
  const f = fixture(t);
  assert.throws(() => f.run({ command(_program, _args, options) {
    assert.equal(options.timeout, 120000); assert.equal(options.maxBuffer, 8 * 1024 ** 2);
    return { error: new Error('timeout fixture'), signal: 'SIGTERM', status: null };
  } }), /compilation failed/);
  assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('oversized compiler output is never executed and private files are removed', t => {
  const f = fixture(t); let calls = 0;
  assert.throws(() => f.run({ command(_program, args) {
    calls++; const binary = args[args.indexOf('-o') + 1];
    writeFileSync(binary, 'x'); truncateSync(binary, 33 * 1024 ** 2);
    return { status: 0, stdout: '', stderr: '' };
  } }), /output bound/);
  assert.equal(calls, 1); assert.deepEqual(readdirSync(f.temporaryParent), []);
});

test('empty or expanded Rust test inventory cannot silently alter the launcher proof', t => {
  for (const source of ['', actualSource + '\n#[test] fn unexpected_extra_test() {}\n']) {
    const f = fixture(t, source);
    assert.throws(f.run, /single expected Rust test/);
    assert.deepEqual(readdirSync(f.temporaryParent), []);
  }
});
