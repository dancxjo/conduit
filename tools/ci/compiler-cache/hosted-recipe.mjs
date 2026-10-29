import { createHash } from 'node:crypto';
import { canonicalHostedMetadata } from './hosted-metadata.mjs';
import { CacheRefusal } from './refusal.mjs';
const fail = () => { throw new CacheRefusal('hosted-selected-recipe-mismatch'); };
const stable = value => JSON.stringify(value, (_key, item) => item && !Array.isArray(item) && typeof item === 'object'
  ? Object.fromEntries(Object.keys(item).sort().map(key => [key, item[key]])) : item);

// Metadata conservatively includes dev/feature edges. Compare that entire
// selected closure; this is recipe custody, not Cargo unit-graph proof.
export function selectedHostedRecipe(metadata) {
  if (metadata?.version !== 1 || !Array.isArray(metadata.packages) || metadata.packages.length > 2000 ||
      !Array.isArray(metadata.resolve?.nodes) || metadata.resolve.nodes.length > 2000) fail();
  const packages = new Map(metadata.packages.map(pkg => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
  if (packages.size !== metadata.packages.length || nodes.size !== metadata.resolve.nodes.length) fail();
  const roots = metadata.packages.filter(pkg => pkg.name === 'conduit' && pkg.manifest_path === '/source/products/conduit/Cargo.toml');
  if (roots.length !== 1) fail();
  const pending = [roots[0].id], seen = new Set(), selected = [];
  while (pending.length) {
    const id = pending.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    const pkg = packages.get(id), node = nodes.get(id);
    if (!pkg || !node || typeof id !== 'string' || !Array.isArray(node.deps) || node.deps.length > 2000 ||
        !Array.isArray(node.features) || !Array.isArray(pkg.targets) || !pkg.features) fail();
    const deps = node.deps.map(dep => {
      if (typeof dep.name !== 'string' || typeof dep.pkg !== 'string' || !Array.isArray(dep.dep_kinds)) fail();
      pending.push(dep.pkg);
      return { name: dep.name, pkg: dep.pkg, dep_kinds: [...dep.dep_kinds].sort((a, b) => stable(a).localeCompare(stable(b))) };
    }).sort((a, b) => stable(a).localeCompare(stable(b)));
    selected.push({ id, name: pkg.name, version: pkg.version, source: pkg.source,
      edition: pkg.edition, rust_version: pkg.rust_version, links: pkg.links,
      features: pkg.features, targets: [...pkg.targets].sort((a, b) => stable(a).localeCompare(stable(b))),
      resolvedFeatures: [...node.features].sort(), deps });
  }
  return selected.sort((a, b) => a.id.localeCompare(b.id));
}
export function verifyHostedSelectedRecipe(expected, actual) {
  const original = selectedHostedRecipe(expected);
  const normalized = selectedHostedRecipe(canonicalHostedMetadata(actual, {
    sourceRoot: '/source', gitCheckout: '/git-vendor/tongues',
  }));
  const bytes = stable(original);
  if (bytes !== stable(normalized)) fail();
  return { schema: 1, packages: original.length, sha256: createHash('sha256').update(bytes).digest('hex'),
    scope: 'Selected resolved metadata closure; not Cargo unit-graph proof' };
}
