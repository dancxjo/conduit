import assert from 'node:assert/strict';
import { test } from 'node:test';
import { admissionPlan, runAdmission, nativeRisk } from '../../tools/ci/pipeline/admission.mjs';
const names = ['conduit', 'conduit-data', 'conduit-kernel', 'conduit-planner'];
const directories = ['products/conduit', 'semantics/data', 'architecture/kernel', 'architecture/planner'];
const metadata = { workspace_members: names, packages: names.map((name, i) => ({ id: name, name, manifest_path: `/repo/${directories[i]}/Cargo.toml` })) };
const plan = paths => admissionPlan(paths, metadata, '/repo');

test('test corrections run their owning package without unrelated emulator proof', () => {
  const result = plan(['products/conduit/tests/installed_screen_free_birth.rs']);
  assert.deepEqual(result, { packages: ['conduit'], broad: false, native: false, smoke: false });
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

test('numerical changes exercise the actual Plan/Play Back behind its feature gate', () => {
  const model = { ...metadata, packages: [...metadata.packages, { name: 'conduit-semantic-catalog' }] };
  const commands = [];
  runAdmission(['architecture/core/src/exact_numeric/realization.rs'], model, '/repo', (...args) => commands.push(args));
  assert.ok(commands.some(([program, args]) => program === 'cargo' && args.includes('kernel-step') && args.includes('numerical_realization')));
});

 test('plot inventories select their consumers without unrelated workspace tests', () => {
  assert.deepEqual(plan(['plots/inventory.toml', 'plots/numerics/main.conduit', 'plots/numerics/README.md']).packages, ['conduit']);
 });
 test('lock updates with owned manifests select consumers; standalone locks remain broad', () => {
  assert.deepEqual(plan(['Cargo.lock', 'products/conduit/Cargo.toml']).packages, ['conduit']);
  assert.deepEqual(plan(['Cargo.lock']).packages, [...names].sort());
 });

test('Std changes use a hosted smoke without unrelated browser compilation', () => {
  const commands = [];
  runAdmission(['targets/std/src/lib.rs'], metadata, '/repo', (...args) => commands.push(args));
  assert.ok(commands.some(([, args]) => args.join(' ') === 'xtask prove journey std'));
  assert.ok(!commands.some(([, args]) => args.includes('integrate') || args.includes('wasm32-unknown-unknown')));
});
test('architecture smoke retains multi-placement and recovery; browser changes build WASM', () => {
  for (const [path, expected] of [['architecture/kernel/src/lib.rs', 'r1-new-plan-recovery'], ['targets/browser/src/lib.rs', 'wasm32-unknown-unknown']]) {
    const commands = [];
    runAdmission([path], metadata, '/repo', (...args) => commands.push(args));
    assert.ok(commands.some(([, args]) => args.includes(expected)));
  }
});
test('numerical catalogue changes select numerical tests; other catalogue edits retain full tests', () => {
  const model = {workspace_members: [...names, 'catalog'], packages: [...metadata.packages, {id: 'catalog', name:'conduit-semantic-catalog', manifest_path:'/repo/semantics/catalog/Cargo.toml'}]};
  for (const [path, targeted] of [['semantics/catalog/src/numerical_realization_back.rs', true], ['semantics/catalog/src/lib.rs', false]]) {
    const commands = [];
    runAdmission([path], model, '/repo', (...args) => commands.push(args));
    const tests = commands.filter(([, args]) => args[0] === 'test' && args.includes('conduit-semantic-catalog'));
    assert.equal(tests.some(([, args]) => args.includes('--lib')), targeted);
    assert.equal(tests.some(([, args]) => args.includes('--no-fail-fast')), !targeted);
    if (targeted) assert.ok(tests.some(([, args]) => args.includes('kernel-step')));
  }
});

test('numerical consumer admission retains full library and actual CLI tests, with broad fallback', () => {
  const model = {workspace_members:['host'],packages:[{id:'host',name:'conduit-std-host',manifest_path:'/repo/targets/std/Cargo.toml'}]};
  for (const [extra, narrow] of [ [[],true], [['targets/std/src/hosted_local_model/ollama.rs'],false], [['targets/std/tests/lifecycle.rs'],false] ]) {
    const commands = [];
    runAdmission(['targets/std/src/numerical_host.rs','targets/std/src/lib.rs',...extra], model, '/repo', (...args) => commands.push(args));
    assert.equal(commands.some(([,args]) => args.includes('--lib') && args.includes('conduit-std-host')), narrow);
    assert.equal(commands.some(([,args]) => args.includes('--test') && args.includes('numerical_offers')), narrow);
    assert.equal(commands.some(([,args]) => args.includes('--no-fail-fast')), !narrow);
  }
  const commands = [];
  runAdmission(['products/conduit/src/product_execution/numerical.rs','products/conduit/src/main.rs'], metadata, '/repo', (...args) => commands.push(args));
  assert.ok(commands.some(([,args]) => args.includes('--lib') && args.includes('conduit')));
  assert.ok(commands.some(([,args]) => args.includes('--test') && args.includes('numerical_realization')));
});

test('unknown or global changes disable narrowed numerical consumer admission', () => {
  const model = {workspace_members:[...names,'catalog'],packages:[...metadata.packages,{id:'catalog',name:'conduit-semantic-catalog',manifest_path:'/repo/semantics/catalog/Cargo.toml'}]};
  const commands = [];
  runAdmission(['Cargo.toml','products/conduit/src/product_execution/numerical.rs','semantics/catalog/src/numerical_realization.rs'], model, '/repo', (...args)=>commands.push(args));
  assert.ok(commands.some(([,args])=>args.includes('--no-fail-fast') && args.includes('conduit') && args.includes('conduit-semantic-catalog')));
  assert.ok(!commands.some(([,args])=>args.includes('--lib')));
});
