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
  for (const name of ['targets/conduitos/Cargo.toml', 'targets/conduitos/build.rs', 'targets/conduitos/domain/gate.rs', 'targets/conduitos/src/usb.rs', 'targets/conduitos/proof/usb-bot/verify_cancellation.py', 'targets/conduitos/make/xtask/ia32_product_boot.rs', 'targets/conduitos/make/xtask/ia32_firmware.rs', 'targets/conduitos/make/xtask/target_build.rs']) assert.equal(nativeRisk([name]), true);
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
    assert.equal(tests.some(([, args]) => args.includes('--no-fail-fast') && !args.includes('--lib')), !targeted);
    if (targeted) assert.ok(tests.some(([, args]) => args.some(arg => arg.endsWith('kernel-step'))));
  }
});

test('numerical consumer admission retains full library and actual CLI tests, with broad fallback', () => {
  const model = {workspace_members:['host'],packages:[{id:'host',name:'conduit-std-host',manifest_path:'/repo/targets/std/Cargo.toml'}]};
  for (const [extra, narrow] of [ [[],true], [['targets/std/src/hosted_local_model/ollama.rs'],false], [['targets/std/tests/lifecycle.rs'],false] ]) {
    const commands = [];
    runAdmission(['targets/std/src/numerical_host.rs','targets/std/src/lib.rs',...extra], model, '/repo', (...args) => commands.push(args));
    assert.equal(commands.some(([,args]) => args.includes('--lib') && args.includes('conduit-std-host')), narrow);
    assert.equal(commands.some(([,args]) => args.includes('--test') && args.includes('numerical_offers')), narrow);
    assert.equal(commands.some(([,args]) => args.includes('--no-fail-fast') && !args.includes('--lib')), !narrow);
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

test('numerical consumers share one compiler context and actual CLI smoke', () => {
  const model = {workspace_members:[...names,'std','catalog','core'],packages:[...metadata.packages,
    {id:'core',name:'conduit-core',manifest_path:'/repo/architecture/core/Cargo.toml'},
    {id:'std',name:'conduit-std-host',manifest_path:'/repo/targets/std/Cargo.toml'},
    {id:'catalog',name:'conduit-semantic-catalog',manifest_path:'/repo/semantics/catalog/Cargo.toml'}]};
  const paths=['products/conduit/src/product_execution/numerical.rs','targets/std/src/numerical_host.rs','semantics/catalog/src/numerical_realization.rs','architecture/core/src/exact_numeric/realization.rs'];
  for (const [extra, generic] of [[[],false],[['architecture/kernel/src/lib.rs'],true],[['targets/std/src/hosted_local_model/ollama.rs'],true]]) {
    const commands=[];
    runAdmission([...paths,...extra], model, '/repo', (...args)=>commands.push(args));
    const library=commands.filter(([,args])=>args.includes('--lib'));
    assert.equal(library.length,1);
    const args=library[0][1];
    assert.ok(args.includes('conduit') && args.includes('conduit-semantic-catalog'));
    assert.ok(args.includes('conduit-semantic-catalog/kernel-step'));
    assert.ok(args.includes('numerical_realization'));
    assert.equal(commands.some(([,args])=>args.join(' ')==='xtask prove journey std'),generic);
    assert.ok(commands.some(([,args])=>args.includes('alloc') && args.includes('conduit-kernel')));
  }
});


test('parallel broad workspace proof preserves every additional admission command', () => {
  const model = {workspace_members:[...names,'catalog'],packages:[...metadata.packages,
    {id:'catalog',name:'conduit-semantic-catalog',manifest_path:'/repo/semantics/catalog/Cargo.toml'}]};
  for (const paths of [
    ['Cargo.toml'],
    ['Cargo.lock','architecture/kernel/src/lib.rs'],
    ['Cargo.toml','architecture/core/src/exact_numeric/number.rs','targets/browser/src/lib.rs'],
  ]) {
    const ordinary=[], parallel=[];
    runAdmission(paths, model, '/repo', (...args)=>ordinary.push(args));
    runAdmission(paths, model, '/repo', (...args)=>parallel.push(args), {workspaceChecked:true});
    const supplemental=ordinary.filter(([,args])=>args[0]!=='clippy' && !args.includes('--no-fail-fast'));
    assert.deepEqual(parallel,supplemental);
    assert.ok(parallel.some(([,args])=>args.includes('conduit-kernel') && args.includes('alloc')));
    if (paths.some(path=>path.startsWith('architecture/'))) {
      for (const proof of ['std','triple','r1-new-plan-recovery'])
        assert.ok(parallel.some(([,args])=>args.includes(proof)),proof);
    }
    if (paths.some(path=>path.startsWith('targets/browser/')))
      assert.ok(parallel.some(([,args])=>args.includes('wasm32-unknown-unknown')));
    if (paths.some(path=>path.includes('exact_numeric')))
      assert.ok(parallel.some(([,args])=>args.includes('kernel-step') && args.includes('numerical_realization')));
  }
});

test('narrow admission cannot suppress its package proof using the broad route', () => {
  const commands=[];
  assert.throws(()=>runAdmission(['products/conduit/src/main.rs'],metadata,'/repo',
    (...args)=>commands.push(args),{workspaceChecked:true}),/only for broad admission/);
  assert.deepEqual(commands,[]);
});

test('language package admission exercises and lints the optional admitted scorer', () => {
  const language = { id: 'language', name: 'conduit-language', manifest_path: '/repo/semantics/language/Cargo.toml' };
  const model = { workspace_members: [...metadata.workspace_members, language.id], packages: [...metadata.packages, language] };
  for (const paths of [
    ['semantics/language/parser_scorer_features.conduit'],
    ['semantics/language/tests/parser_selection.rs'],
    ['semantics/language/tests/parser_model_admission.rs'],
    ['Cargo.lock'],
  ]) {
    const commands = [];
    runAdmission(paths, model, '/repo', (...args) => commands.push(args));
    const tests = commands.filter(([, args]) => args[0] === 'test' && args.includes('conduit-language'));
    assert.equal(tests.length, 1, 'retain one owning-package test command');
    for (const [, args] of [commands[0], ...tests]) {
      const feature = args.indexOf('--features');
      assert.ok(feature >= 0);
      assert.equal(args[feature + 1], 'conduit-language/parser-model');
      assert.ok(feature < args.indexOf('--'), 'feature belongs to Cargo, not the test harness');
    }
  }
  const parallel = [];
  runAdmission(['Cargo.lock'], model, '/repo', (...args) => parallel.push(args), {workspaceChecked:true});
  assert.equal(parallel.filter(([,args]) => args[0] === 'clippy' && args.includes('conduit-language/parser-model')).length, 1);
  assert.ok(parallel.every(([,args]) => !(args.includes('--no-fail-fast') && args.includes('conduit-language'))), 'foundation owns the already-proven broad package tests');
  const unrelated = [];
  runAdmission(['products/conduit/src/main.rs'], model, '/repo', (...args) => unrelated.push(args));
  assert.ok(unrelated.every(([, args]) => !args.includes('conduit-language/parser-model')));
});


test('narrow admission lints selected packages and broad changes retain workspace lint', () => {
  for (const [paths, expected] of [
    [['products/conduit/tests/installed_screen_free_birth.rs'], ['-p', 'conduit']],
    [['Cargo.toml'], ['--workspace']],
  ]) {
    const commands = [];
    runAdmission(paths, metadata, '/repo', (...args) => commands.push(args));
    const lint = commands.find(([, args]) => args[0] === 'clippy')[1];
    assert.deepEqual(lint, ['clippy', '--locked', ...expected, '--all-targets', '--', '-D', 'warnings']);
  }
});

test('sensitive changes lint every selected boundary package', () => {
  const commands = [];
  runAdmission(['semantics/data/src/number.rs'], metadata, '/repo', (...args) => commands.push(args));
  const lint = commands.find(([, args]) => args[0] === 'clippy')[1];
  for (const name of ['conduit-data', 'conduit-kernel', 'conduit-planner']) assert.ok(lint.includes(name));
  assert.ok(!lint.includes('--workspace'));
});


test('preflight-owned tooling and prose do not compile Rust packages', () => {
  for (const paths of [
    ['tools/ci/pipeline/admission.mjs', 'proof/ci/pipeline-numerical.spec.mjs'],
    ['.github/workflows/candidate.yml'],
    ['targets/std/README.md'],
  ]) {
    assert.deepEqual(plan(paths), {packages: [], broad: false, native: false, smoke: false});
    const commands = [];
    runAdmission(paths, metadata, '/repo', (...args) => commands.push(args));
    assert.deepEqual(commands, []);
  }
});

test('mixed tooling and runtime edits retain the runtime package and its failures', () => {
  const paths = ['tools/ci/pipeline/admission.mjs', 'products/conduit/src/main.rs'];
  assert.deepEqual(plan(paths).packages, ['conduit']);
  const commands = [];
  runAdmission(paths, metadata, '/repo', (...args) => commands.push(args));
  assert.ok(commands.some(([, args]) => args[0] === 'test' && args.includes('conduit')));
  assert.throws(() => runAdmission(paths, metadata, '/repo', () => {throw new Error('runtime lint failure');}), /runtime lint failure/);
  assert.equal(plan(['unknown/tooling-format.bin']).broad, true);
});


test('tooling filenames and prose cannot select unrelated runtime supplements', () => {
  const model = {...metadata, packages: [...metadata.packages, {name: 'conduit-semantic-catalog'}]};
  const commands = [];
  runAdmission(['products/conduit/src/main.rs', 'architecture/README.md', 'proof/ci/pipeline-numerical.spec.mjs'], model, '/repo', (...args) => commands.push(args));
  assert.equal(commands.length, 2);
  assert.deepEqual(commands.map(([, args]) => args[0]), ['clippy', 'test']);
});


test('non-script tooling data retains conservative Rust tooling coverage', () => {
  const model = {workspace_members: [...names, 'xtask'], packages: [...metadata.packages, {id: 'xtask', name: 'xtask', manifest_path: '/repo/tools/xtask/Cargo.toml'}]};
  assert.deepEqual(admissionPlan(['proof/ci/publication-history/receipt.json'], model, '/repo').packages, ['xtask']);
});


test('descriptive measurement receipts use preflight without broadening mixed runtime proof', () => {
  for (const directory of ['build-generators', 'signs', 'catalogue', 'line', 'browser', 'raster']) {
    const receipt = `proof/performance/${directory}/observation.json`;
    assert.equal(plan([receipt]).broad, false);
    assert.deepEqual(plan([receipt]).packages, []);
    assert.deepEqual(plan([receipt, 'products/conduit/src/lib.rs']), plan(['products/conduit/src/lib.rs']));
    const commands = [];
    runAdmission([receipt], metadata, '/repo', (...args) => commands.push(args));
    assert.deepEqual(commands, []);
  }
  for (const fixture of ['proof/performance/runtime/fixtures/input.json', 'proof/performance/line/fixtures/input.json', 'proof/performance/new-format/input.json']) {
    assert.equal(plan([fixture]).broad, true, fixture);
  }
});

test('native owner package admission retains the workspace network feature context', () => {
  const native = { id: 'conduitos', name: 'conduitos', manifest_path: '/repo/targets/conduitos/Cargo.toml' };
  const model = { workspace_members: [...metadata.workspace_members, native.id], packages: [...metadata.packages, native] };
  const commands = [];
  runAdmission(['targets/conduitos/src/native_owner_document.rs'], model, '/repo', (...args) => commands.push(args));
  for (const operation of ['test', 'clippy']) {
    const command = commands.find(([program, args]) => program === 'cargo' && args[0] === operation);
    assert.ok(command, `${operation} is scheduled`);
    const index = command[1].indexOf('--features');
    assert.ok(index > 0);
    assert.deepEqual(command[1][index + 1].split(','), ['conduitos/hosted-tools', 'conduitos/native-compositor']);
  }
  const unrelated = [];
  runAdmission(['products/conduit/src/main.rs'], model, '/repo', (...args) => unrelated.push(args));
  assert.ok(unrelated.every(([, args]) => !args.includes('--features')));
});


test('parallel broad admission retains explicit native lint without repeating package tests', () => {
  const native = { id: 'conduitos', name: 'conduitos', manifest_path: '/repo/targets/conduitos/Cargo.toml' };
  const model = { workspace_members: [...metadata.workspace_members, native.id], packages: [...metadata.packages, native] };
  const commands = [];
  runAdmission(['Cargo.toml'], model, '/repo', (...args) => commands.push(args), { workspaceChecked: true });
  const lint = commands.filter(([program, args]) => program === 'cargo' && args[0] === 'clippy');
  assert.equal(lint.length, 1);
  assert.ok(lint[0][1].includes('conduitos'));
  assert.equal(lint[0][1][lint[0][1].indexOf('--features') + 1], 'conduitos/hosted-tools,conduitos/native-compositor');
  assert.ok(commands.every(([, args]) => !args.includes('--no-fail-fast')));
});

test('shared xtask sources compile both tooling entrances during narrow admission', () => {
  const tools = [
    {id: 'xtask', name: 'xtask', manifest_path: '/repo/tools/xtask/Cargo.toml'},
    {id: 'dispatch', name: 'conduit-xtask-dispatch', manifest_path: '/repo/tools/xtask-dispatch/Cargo.toml'},
  ];
  const model = {workspace_members: [...names, ...tools.map(pkg => pkg.id)], packages: [...metadata.packages, ...tools]};
  for (const source of ['tools/xtask/src/suites/workspace_shard_features_tests.rs', 'tools/xtask/src/suites/host_protocol_shards.rs', 'tools/xtask/src/commands/check.rs']) {
    const selected = admissionPlan([source], model, '/repo');
    assert.deepEqual(selected.packages, ['conduit-xtask-dispatch', 'xtask']);
    assert.equal(selected.broad, false);
    const commands = [];
    runAdmission([source], model, '/repo', (...args) => commands.push(args));
    assert.deepEqual(commands.map(([, args]) => args), [
      ['clippy', '--locked', '-p', 'conduit-xtask-dispatch', '-p', 'xtask', '--all-targets', '--', '-D', 'warnings'],
      ['test', '--locked', '--no-fail-fast', '-p', 'conduit-xtask-dispatch', '-p', 'xtask', '--', '--test-threads=1'],
    ]);
  }
  assert.deepEqual(admissionPlan(['tools/xtask/README.md'], model, '/repo').packages, []);
});
