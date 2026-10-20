import path from 'node:path';

// These owned directories retain descriptive measurement outputs, never runtime
// fixtures. Their JSON syntax is checked in preflight; checks still execute from
// source and do not trust these records as passing results.
export function isMeasurementReceipt(name) {
  return /^proof\/performance\/(?:build-generators|signs|catalogue|line|browser|raster)\/[^/]+\.json$/.test(name);
}

function preflightOnly(name) {
  return isMeasurementReceipt(name) || name.endsWith('.md') || name === 'LICENSE'
    || /^(?:tools\/ci\/|proof\/ci\/).+\.(?:mjs|js)$/.test(name)
    || /^\.github\/workflows\/[^/]+\.ya?ml$/.test(name);
}

export function nativeRisk(paths) {
  // The native kernel owns isolation, device ownership and preemption. Its
  // standalone sources and product build/proof collectors cannot be covered
  // by workspace package tests alone.
  return paths.some(name => /^targets\/conduitos\/(?:(?:src|domain|build|proof|make\/xtask)\/|(?:Cargo\.(?:toml|lock)|build\.rs)$)/.test(name));
}

export function admissionPlan(paths, metadata, root) {
  if (!Array.isArray(paths) || paths.some(name => typeof name !== 'string')) throw new TypeError('invalid changed paths');
  const runtimePaths = paths.filter(name => !preflightOnly(name));
  const members = metadata.packages.filter(pkg => metadata.workspace_members.includes(pkg.id));
  const owners = members.map(pkg => ({ pkg, directory: path.relative(root, path.dirname(pkg.manifest_path)).split(path.sep).join('/') }));
  const selected = new Set();
  let broad = paths.length === 0;
  for (const name of runtimePaths) {
    // A dependency lock update accompanies its owning manifest; workspace lint
    // is exhaustive during Integration. A standalone lock change remains broad.
    if (name === 'Cargo.lock' && paths.some(item => item.endsWith('/Cargo.toml'))) continue;
    const owner = owners.filter(item => name.startsWith(`${item.directory}/`)).sort((a, b) => b.directory.length - a.directory.length)[0];
    if (/^(?:tools\/ci\/|proof\/ci\/|\.github\/)/.test(name)) {
      for (const item of owners.filter(item => item.directory.startsWith('tools/xtask'))) selected.add(item.pkg.name);
    }
    else if (owner) {
      selected.add(owner.pkg.name);
      // The dispatcher includes shared xtask modules by source path, outside
      // Cargo's dependency graph. Compile both entrances for these edits so a
      // module-layout error cannot pass Candidate and fail Integration later.
      if (owner.directory === 'tools/xtask') {
        for (const item of owners.filter(item => item.directory === 'tools/xtask-dispatch')) selected.add(item.pkg.name);
      }
    }
    else if (name.startsWith('plots/')) {
      // Authored plots and their inventory are consumed by the product and
      // xtask catalog commands, rather than every workspace package.
      for (const item of owners.filter(item => item.directory === 'products/conduit' || item.directory.startsWith('tools/xtask'))) selected.add(item.pkg.name);
    }
    else broad = true; // Deleted packages, shared source and unknown paths fail closed.
  }
  if (broad) for (const pkg of members) selected.add(pkg.name);
  const sensitive = runtimePaths.some(name => /^(?:architecture\/(?:core|kernel|planner|assigned-plan|protected-line|composite)\/|semantics\/data\/)/.test(name)
    || /(?:usb|xhci|cancel|certificate|numerical|exact_numeric|binary64|isolation|capability)/i.test(name));
  if (sensitive) {
    for (const name of ['conduit-core', 'conduit-kernel', 'conduit-planner', 'conduit-assigned-plan', 'conduit-protected-line', 'conduit-composite', 'conduit-data']) {
      if (members.some(pkg => pkg.name === name)) selected.add(name);
    }
  }
  return { packages: [...selected].sort(), broad, native: nativeRisk(runtimePaths), smoke: runtimePaths.some(name => /^(?:architecture|targets\/std|targets\/browser)\//.test(name)) };
}

export function runAdmission(paths, metadata, root, run, { workspaceChecked = false } = {}) {
  const plan = admissionPlan(paths, metadata, root);
  // In CI this corpus runs in independent unit jobs. The final workflow gate
  // requires their success; this flag delegates work rather than trusting a cache.
  if (workspaceChecked && !plan.broad) throw new Error('parallel workspace proof is valid only for broad admission');
  if (paths.length && paths.every(preflightOnly)) return;
  const runtimePaths = paths.filter(name => !preflightOnly(name));
  console.log(`Admission packages: ${plan.packages.join(', ')}`);
  const enabledFeatures = [
    ...(plan.packages.includes('conduit-language') ? ['conduit-language/parser-model'] : []),
    ...(plan.packages.includes('conduitos') ? ['conduitos/hosted-tools', 'conduitos/native-compositor'] : []),
  ];
  const featureArguments = enabledFeatures.length ? ['--features', enabledFeatures.join(',')] : [];
  const lintPackages = plan.broad ? ['--workspace'] : plan.packages.flatMap(name => ['-p', name]);
  if (!workspaceChecked && lintPackages.length) run('cargo', ['clippy', '--locked', ...lintPackages, '--all-targets', ...featureArguments, '--', '-D', 'warnings']);
  else if (featureArguments.length) {
    // Explicit consumer features must also be linted after parallel package proof.
    run('cargo', ['clippy', '--locked', ...['conduit-language', 'conduitos'].filter(name => plan.packages.includes(name)).flatMap(name => ['-p', name]), '--all-targets', ...featureArguments, '--', '-D', 'warnings']);
  }
  const catalogChanges = runtimePaths.filter(name => name.startsWith('semantics/catalog/') && !name.endsWith('.md'));
  const numericalCatalog = !plan.broad && catalogChanges.length > 0 && catalogChanges.every(name =>
    /^semantics\/catalog\/(?:src\/numerical_realization(?:_back)?\.rs|numerical\/|tests\/numerical_realization(?:\.rs|\/))/.test(name));
  // Numerical registration changes affect the whole consumer library, but do
  // not require compiling every unrelated UI/network integration-test binary.
  // Any other source or test path falls back to the complete package suite.
  const consumers = [
    { name: 'conduit', directory: 'products/conduit/', allowed: /^products\/conduit\/(?:src\/(?:lib|main|plot_source|product_execution|product_execution_tests)\.rs|src\/product_execution\/numerical\.rs|tests\/numerical_realization\.rs)$/, test: 'numerical_realization' },
    { name: 'conduit-std-host', directory: 'targets/std/', allowed: /^targets\/std\/(?:src\/(?:lib|installed_std|numerical_host)\.rs|src\/installed_std\/(?:back_capacity|back_kind|back_step|catalog|numerical_back|numerical_back_tests|structured_values_back)\.rs|tests\/numerical_offers\.rs)$/, test: 'numerical_offers' },
  ].filter(consumer => {
    const changes = runtimePaths.filter(name => name.startsWith(consumer.directory) && !name.endsWith('.md') && !name.startsWith('targets/std/offers/'));
    return !plan.broad && plan.packages.includes(consumer.name) && changes.some(name => name.includes('numerical')) && changes.every(name => consumer.allowed.test(name));
  });
  const packages = plan.packages.filter(name => !(numericalCatalog && name === 'conduit-semantic-catalog') && !consumers.some(consumer => consumer.name === name));
  if (packages.length && !workspaceChecked) run('cargo', ['test', '--locked', '--no-fail-fast', ...packages.flatMap(name => ['-p', name]), ...featureArguments, '--', '--test-threads=1']);
  const libraries = [...consumers.map(consumer => consumer.name), ...(numericalCatalog ? ['conduit-semantic-catalog'] : [])];
  if (libraries.length) {
    // One feature-unified compiler context avoids rebuilding the same Host and
    // semantic dependencies separately for each consumer library/test binary.
    const tests = [...new Set([...consumers.map(consumer => consumer.test), ...(numericalCatalog ? ['numerical_realization'] : [])])];
    run('cargo', ['test', '--locked', '--no-fail-fast', ...libraries.flatMap(name => ['-p', name]), '--lib',
      ...tests.flatMap(name => ['--test', name]),
      ...(numericalCatalog ? ['--features', 'conduit-semantic-catalog/kernel-step'] : []), '--', '--test-threads=1']);
  }
  if (plan.packages.includes('conduit-kernel')) run('cargo', ['test', '--locked', '-p', 'conduit-kernel', '--features', 'alloc', '--', '--test-threads=1']);
  if (runtimePaths.some(name => /(?:numerical|exact_numeric|binary64)/.test(name))
      && metadata.packages.some(pkg => pkg.name === 'conduit-semantic-catalog') && !numericalCatalog) {
    run('cargo', ['test', '--locked', '-p', 'conduit-semantic-catalog', '--features', 'kernel-step', '--test', 'numerical_realization', '--', '--test-threads=1']);
  }
  const actualNumericalCli = consumers.some(consumer => consumer.name === 'conduit');
  const stdConsumer = consumers.find(consumer => consumer.name === 'conduit-std-host');
  const otherRuntime = runtimePaths.some(name => !name.endsWith('.md') && (
    (name.startsWith('architecture/') && !/^architecture\/core\/(?:src\/exact_numeric\/|tests\/numerical_)/.test(name))
    || (name.startsWith('targets/std/') && !name.startsWith('targets/std/offers/') && !stdConsumer?.allowed.test(name))
    || name.startsWith('targets/browser/')));
  // The numerical integration binary executes the actual ordinary Std CLI.
  // An additional Hello journey establishes no new numerical property.
  if (plan.smoke && (!actualNumericalCli || otherRuntime)) {
    run('cargo', ['xtask', 'prove', 'journey', 'std']);
    if (runtimePaths.some(name => name.startsWith('architecture/'))) {
      run('cargo', ['xtask', 'prove', 'journey', 'triple']);
      run('cargo', ['xtask', 'prove', 'r1-new-plan-recovery']);
    }
    if (runtimePaths.some(name => name.startsWith('targets/browser/'))) {
      run('cargo', ['build', '--locked', '-p', 'conduit-browser-runtime', '--target', 'wasm32-unknown-unknown']);
    }
  }
}
