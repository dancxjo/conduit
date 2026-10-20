import path from 'node:path';

export function nativeRisk(paths) {
  // The native kernel owns isolation, device ownership and preemption. Its
  // standalone sources and product build/proof collectors cannot be covered
  // by workspace package tests alone.
  return paths.some(name => /^targets\/conduitos\/(?:(?:src|domain|build|proof|make\/xtask)\/|(?:Cargo\.(?:toml|lock)|build\.rs)$)/.test(name));
}

export function admissionPlan(paths, metadata, root) {
  if (!Array.isArray(paths) || paths.some(name => typeof name !== 'string')) throw new TypeError('invalid changed paths');
  const members = metadata.packages.filter(pkg => metadata.workspace_members.includes(pkg.id));
  const owners = members.map(pkg => ({ pkg, directory: path.relative(root, path.dirname(pkg.manifest_path)).split(path.sep).join('/') }));
  const selected = new Set();
  let broad = paths.length === 0;
  for (const name of paths) {
    if (name.endsWith('.md') || name === 'LICENSE') continue;
    // A dependency lock update accompanies its owning manifest; workspace lint
    // still compiles every consumer. A standalone lock change remains broad.
    if (name === 'Cargo.lock' && paths.some(item => item.endsWith('/Cargo.toml'))) continue;
    const owner = owners.filter(item => name.startsWith(`${item.directory}/`)).sort((a, b) => b.directory.length - a.directory.length)[0];
    if (/^(?:tools\/ci\/|proof\/ci\/|\.github\/)/.test(name)) {
      for (const item of owners.filter(item => item.directory.startsWith('tools/xtask'))) selected.add(item.pkg.name);
    }
    else if (owner) selected.add(owner.pkg.name);
    else if (name.startsWith('plots/')) {
      // Authored plots and their inventory are consumed by the product and
      // xtask catalog commands, rather than every workspace package.
      for (const item of owners.filter(item => item.directory === 'products/conduit' || item.directory.startsWith('tools/xtask'))) selected.add(item.pkg.name);
    }
    else broad = true; // Deleted packages, shared source and unknown paths fail closed.
  }
  if (broad) for (const pkg of members) selected.add(pkg.name);
  const sensitive = paths.some(name => /^(?:architecture\/(?:core|kernel|planner|assigned-plan|protected-line|composite)\/|semantics\/data\/)/.test(name)
    || /(?:usb|xhci|cancel|certificate|numerical|exact_numeric|binary64|isolation|capability)/i.test(name));
  if (sensitive) {
    for (const name of ['conduit-core', 'conduit-kernel', 'conduit-planner', 'conduit-assigned-plan', 'conduit-protected-line', 'conduit-composite', 'conduit-data']) {
      if (members.some(pkg => pkg.name === name)) selected.add(name);
    }
  }
  return { packages: [...selected].sort(), broad, native: nativeRisk(paths), smoke: paths.some(name => /^(?:architecture|targets\/std|targets\/browser)\//.test(name)) };
}

export function runAdmission(paths, metadata, root, run, { workspaceChecked = false } = {}) {
  const plan = admissionPlan(paths, metadata, root);
  if (workspaceChecked && !plan.broad) throw new Error('parallel workspace proof is valid only for broad admission');
  if (paths.length && paths.every(name => /^(?:docs\/.+\.md|[^/]+\.md|LICENSE)$/.test(name))) return;
  console.log(`Admission packages: ${plan.packages.join(', ')}`);
  const languageFeatures = plan.packages.includes('conduit-language') ? ['--features', 'conduit-language/parser-model'] : [];
  if (!workspaceChecked) run('cargo', ['clippy', '--locked', '--workspace', '--all-targets', ...languageFeatures, '--', '-D', 'warnings']);
  else if (languageFeatures.length) {
    // Parallel workspace lint does not enable this optional consumer feature.
    run('cargo', ['clippy', '--locked', '-p', 'conduit-language', '--all-targets', ...languageFeatures, '--', '-D', 'warnings']);
  }
  const catalogChanges = paths.filter(name => name.startsWith('semantics/catalog/') && !name.endsWith('.md'));
  const numericalCatalog = !plan.broad && catalogChanges.length > 0 && catalogChanges.every(name =>
    /^semantics\/catalog\/(?:src\/numerical_realization(?:_back)?\.rs|numerical\/|tests\/numerical_realization(?:\.rs|\/))/.test(name));
  // Numerical registration changes affect the whole consumer library, but do
  // not require compiling every unrelated UI/network integration-test binary.
  // Any other source or test path falls back to the complete package suite.
  const consumers = [
    { name: 'conduit', directory: 'products/conduit/', allowed: /^products\/conduit\/(?:src\/(?:lib|main|plot_source|product_execution|product_execution_tests)\.rs|src\/product_execution\/numerical\.rs|tests\/numerical_realization\.rs)$/, test: 'numerical_realization' },
    { name: 'conduit-std-host', directory: 'targets/std/', allowed: /^targets\/std\/(?:src\/(?:lib|installed_std|numerical_host)\.rs|src\/installed_std\/(?:back_capacity|back_kind|back_step|catalog|numerical_back|numerical_back_tests|structured_values_back)\.rs|tests\/numerical_offers\.rs)$/, test: 'numerical_offers' },
  ].filter(consumer => {
    const changes = paths.filter(name => name.startsWith(consumer.directory) && !name.endsWith('.md') && !name.startsWith('targets/std/offers/'));
    return !plan.broad && plan.packages.includes(consumer.name) && changes.some(name => name.includes('numerical')) && changes.every(name => consumer.allowed.test(name));
  });
  const packages = plan.packages.filter(name => !(numericalCatalog && name === 'conduit-semantic-catalog') && !consumers.some(consumer => consumer.name === name));
  if (packages.length && !workspaceChecked) run('cargo', ['test', '--locked', '--no-fail-fast', ...packages.flatMap(name => ['-p', name]), ...languageFeatures, '--', '--test-threads=1']);
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
  if (paths.some(name => /(?:numerical|exact_numeric|binary64)/.test(name))
      && metadata.packages.some(pkg => pkg.name === 'conduit-semantic-catalog') && !numericalCatalog) {
    run('cargo', ['test', '--locked', '-p', 'conduit-semantic-catalog', '--features', 'kernel-step', '--test', 'numerical_realization', '--', '--test-threads=1']);
  }
  const actualNumericalCli = consumers.some(consumer => consumer.name === 'conduit');
  const stdConsumer = consumers.find(consumer => consumer.name === 'conduit-std-host');
  const otherRuntime = paths.some(name => !name.endsWith('.md') && (
    (name.startsWith('architecture/') && !/^architecture\/core\/(?:src\/exact_numeric\/|tests\/numerical_)/.test(name))
    || (name.startsWith('targets/std/') && !name.startsWith('targets/std/offers/') && !stdConsumer?.allowed.test(name))
    || name.startsWith('targets/browser/')));
  // The numerical integration binary executes the actual ordinary Std CLI.
  // An additional Hello journey establishes no new numerical property.
  if (plan.smoke && (!actualNumericalCli || otherRuntime)) {
    run('cargo', ['xtask', 'prove', 'journey', 'std']);
    if (paths.some(name => name.startsWith('architecture/'))) {
      run('cargo', ['xtask', 'prove', 'journey', 'triple']);
      run('cargo', ['xtask', 'prove', 'r1-new-plan-recovery']);
    }
    if (paths.some(name => name.startsWith('targets/browser/'))) {
      run('cargo', ['build', '--locked', '-p', 'conduit-browser-runtime', '--target', 'wasm32-unknown-unknown']);
    }
  }
}
