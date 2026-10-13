import assert from 'node:assert/strict';
import { test } from 'node:test';
import { admissionPlan, runAdmission, nativeRisk } from '../../tools/ci/pipeline/admission.mjs';
const names = ['conduit', 'conduit-data', 'conduit-kernel', 'conduit-planner'];
const directories = ['products/conduit', 'semantics/data', 'architecture/kernel', 'architecture/planner'];
const metadata = { workspace_members: names, packages: names.map((name, i) => ({ id: name, name, manifest_path: `/repo/${directories[i]}/Cargo.toml` })) };
const plan = paths => admissionPlan(paths, metadata, '/repo');

test('test corrections run their owning package without unrelated emulator proof', () => {
  const result = plan(['products/conduit/tests/installed_screen_free_birth.rs']);
  assert.deepEqual(result, { packages: ['conduit'], native: false, smoke: false });
  const commands = [];
  runAdmission(['products/conduit/tests/installed_screen_free_birth.rs'], metadata, '/repo', (...args) => commands.push(args));
  assert.equal(commands.length, 2);
  assert.deepEqual(commands[1][1], ['test', '--locked', '--no-fail-fast', '-p', 'conduit', '--', '--test-threads=1']);
});
test('numerical certificates and cancellation retain shared boundary tests', () => {
  for (const name of ['semantics/data/src/number.rs', 'products/conduit/src/cancellation.rs']) {
    const result = plan([name]);
    for (const pkg of ['conduit-data', 'conduit-kernel', 'conduit-planner']) assert.ok(result.packages.includes(pkg));
    assert.equal(result.native, false);
  }
});
test('native ownership and isolation require native proof, including either rename side', () => {
  for (const name of ['targets/conduitos/Cargo.toml', 'targets/conduitos/build.rs', 'targets/conduitos/domain/gate.rs', 'targets/conduitos/src/usb.rs', 'targets/conduitos/proof/usb-bot/verify_cancellation.py']) assert.equal(nativeRisk([name]), true);
  assert.equal(nativeRisk(['targets/conduitos/domain/old.rs', 'products/new.rs']), true);
  assert.equal(nativeRisk(['products/conduit/tests/installed_screen_free_birth.rs']), false);
});
test('unknown, deleted-package and empty changes retain all workspace tests', () => {
  for (const paths of [[], ['Cargo.lock'], ['deleted/package/src/lib.rs'], ['site/index.html']]) assert.deepEqual(plan(paths).packages, [...names].sort());
});
test('recognized documentation needs preflight alone; runtime needs smoke', () => {
  assert.deepEqual(plan(['docs/contributing/ci.md']).packages, []);
  assert.equal(plan(['architecture/kernel/src/lib.rs']).smoke, true);
  assert.throws(() => plan([null]), TypeError);
});
test('a failed lint cannot be converted into successful admission', () => {
  assert.throws(() => runAdmission(['products/conduit/src/main.rs'], metadata, '/repo', () => { throw new Error('lint failed'); }), /lint failed/);
});

test('boundary admission retains allocation-feature kernel conformance', () => {
  const commands = [];
  runAdmission(['semantics/data/src/number.rs'], metadata, '/repo', (...args) => commands.push(args));
  assert.ok(commands.some(([program, args]) => program === 'cargo' && args.includes('alloc') && args.includes('conduit-kernel')));
});
