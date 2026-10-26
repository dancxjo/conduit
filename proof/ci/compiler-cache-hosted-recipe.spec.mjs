import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyHostedSelectedRecipe } from '../../tools/ci/compiler-cache/hosted-recipe.mjs';
function graph() {
  const root = 'path+file:///source/products/conduit#0.1.0';
  const dependency = 'registry+https://github.com/rust-lang/crates.io-index#example@1.0.0';
  const pkg = (id, name, version, directory, source) => ({ id, name, version, source, edition: '2021',
    rust_version: '1.80', links: null, manifest_path: `${directory}/Cargo.toml`, features: { default: [] },
    targets: [{ name, kind: ['lib'], crate_types: ['lib'], src_path: `${directory}/src/lib.rs`, test: true }] });
  return { version: 1, packages: [pkg(root, 'conduit', '0.1.0', '/source/products/conduit', null),
    pkg(dependency, 'example', '1.0.0', '/vendor/example-1.0.0', 'registry+https://github.com/rust-lang/crates.io-index')],
    resolve: { nodes: [{ id: root, features: [], deps: [{ name: 'example', pkg: dependency, dep_kinds: [{ kind: null, target: null }] }] },
      { id: dependency, features: ['default'], deps: [] }] } };
}
test('selected recipe accepts identical closure with reordered package inventory', () => {
  const expected = graph(), actual = structuredClone(expected); actual.packages.reverse();
  assert.equal(verifyHostedSelectedRecipe(expected, actual).packages, 2);
});
for (const [name, mutate] of [
  ['resolved feature', g => g.resolve.nodes[1].features.push('changed')],
  ['declared feature', g => g.packages[1].features.default.push('changed')],
  ['target source', g => { g.packages[1].targets[0].src_path = '/vendor/example-1.0.0/src/other.rs'; }],
  ['source identity', g => { g.packages[1].source = 'registry+https://other.invalid'; }],
  ['dependency kind', g => { g.resolve.nodes[0].deps[0].dep_kinds[0].kind = 'build'; }],
  ['missing edge', g => { g.resolve.nodes[0].deps = []; }],
  ['missing resolved package', g => { g.resolve.nodes.pop(); }],
  ['duplicate package identity', g => { g.packages.push(g.packages[0]); }],
]) test(`selected recipe refuses changed ${name}`, () => {
  const expected = graph(), actual = structuredClone(expected); mutate(actual);
  assert.throws(() => verifyHostedSelectedRecipe(expected, actual));
});
