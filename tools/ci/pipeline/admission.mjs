import path from 'node:path';

export function nativeRisk(paths) {
  // The native kernel owns isolation, device ownership and preemption. Its
  // standalone sources cannot be covered by workspace package tests alone.
  return paths.some(name => /^targets\/conduitos\/(?:(?:src|domain|build|proof)\/|(?:Cargo\.(?:toml|lock)|build\.rs)$)/.test(name));
}

export function admissionPlan(paths, metadata, root) {
  if (!Array.isArray(paths) || paths.some(name => typeof name !== 'string')) throw new TypeError('invalid changed paths');
  const members = metadata.packages.filter(pkg => metadata.workspace_members.includes(pkg.id));
  const owners = members.map(pkg => ({ pkg, directory: path.relative(root, path.dirname(pkg.manifest_path)).split(path.sep).join('/') }));
  const selected = new Set();
  let broad = paths.length === 0;
  for (const name of paths) {
    if (/^(?:docs\/.+\.md|[^/]+\.md|LICENSE)$/.test(name)) continue;
    const owner = owners.filter(item => name.startsWith(`${item.directory}/`)).sort((a, b) => b.directory.length - a.directory.length)[0];
    if (/^(?:tools\/ci\/|proof\/ci\/|\.github\/)/.test(name)) {
      for (const item of owners.filter(item => item.directory.startsWith('tools/xtask'))) selected.add(item.pkg.name);
    }
    else if (owner) selected.add(owner.pkg.name);
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
  return { packages: [...selected].sort(), native: nativeRisk(paths), smoke: paths.some(name => /^(?:architecture|targets\/std|targets\/browser)\//.test(name)) };
}

export function runAdmission(paths, metadata, root, run) {
  const plan = admissionPlan(paths, metadata, root);
  if (paths.length && paths.every(name => /^(?:docs\/.+\.md|[^/]+\.md|LICENSE)$/.test(name))) return;
  console.log(`Admission packages: ${plan.packages.join(', ')}`);
  run('cargo', ['clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings']);
  if (plan.packages.length) run('cargo', ['test', '--locked', '--no-fail-fast', ...plan.packages.flatMap(name => ['-p', name]), '--', '--test-threads=1']);
  if (plan.packages.includes('conduit-kernel')) run('cargo', ['test', '--locked', '-p', 'conduit-kernel', '--features', 'alloc', '--', '--test-threads=1']);
  if (paths.some(name => /(?:numerical|exact_numeric|binary64)/.test(name))
      && metadata.packages.some(pkg => pkg.name === 'conduit-semantic-catalog')) {
    run('cargo', ['test', '--locked', '-p', 'conduit-semantic-catalog', '--features', 'kernel-step', '--test', 'numerical_realization', '--', '--test-threads=1']);
  }
  if (plan.smoke) run('cargo', ['xtask', 'integrate']);
}
