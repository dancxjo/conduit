import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createPortableComponentAdmission, portableComponentBundleBytes } from '../../targets/browser/host/assets/portable-component-admission.mjs';

const encode = text => new TextEncoder().encode(text);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
function fixture() {
  const roles = ['controls', 'model', 'combobox', 'component', 'style'];
  const contents = ['controls', 'model', 'combobox', 'export const component = 1;', ':host { color: CanvasText; }'].map(encode);
  const resources = roles.map((role, i) => ({ role, path: role === 'style' ? 'portable-face-look.css' : `portable-face-${role}.mjs`, bytes: contents[i].length, sha256: digest(contents[i]) }));
  const descriptor = {
    schema: 'conduit.browser/portable-face-component-back@1',
    identity: 'conduit.browser/portable-face-component@1', version: 1, tag: 'conduit-portable-face',
    maximum_nodes: 2048, maximum_depth: 12, maximum_in_flight_events: 1, maximum_bundle_bytes: 65536,
    resources, bundle_sha256: digest(portableComponentBundleBytes(resources.map((r, i) => ({ role: r.role, bytes: contents[i] })))),
  };
  descriptor.implementation_id = 'implementation/browser-portable-face-component@1';
  descriptor.capability_id = 'capability/browser-portable-face-component@1';
  descriptor.artifact_id = `artifact/browser-portable-face-component@1/sha256/${descriptor.bundle_sha256}`;
  const byRole = new Map(resources.slice(0, 3).map(r => [`portable-face-${r.role}`, { kind: 'module', path: r.path, sha256: `sha256:${r.sha256}` }]));
  const admittedBytes = new Map(resources.slice(0, 3).map((r, i) => [`portable-face-${r.role}`, contents[i]]));
  const requested = [];
  let calls = 0;
  class Component {}
  Object.defineProperty(Component, 'componentIdentity', { value: descriptor.identity });
  const module = {
    PORTABLE_FACE_COMPONENT: descriptor,
    registerPortableFaceComponent({ render, styleText }) { assert.equal(typeof render, 'function'); assert.equal(styleText, new TextDecoder().decode(contents[4])); return Component; },
  };
  const dependencies = {
    packageRoot: new URL('https://example.test/app/'), byRole, admittedBytes,
    packageUrl(path, base) { return new URL(path, base); },
    async boundedFetch(url, maximum) { requested.push(url.pathname); const index = resources.findIndex(r => url.pathname.endsWith(r.path)); assert.equal(maximum, resources[index].bytes); return contents[index]; },
    async sha256(bytes) { return `sha256:${digest(bytes)}`; },
    async importModule() { calls++; return module; }, registry: { get() { return Component; } },
  };
  return { descriptor, dependencies, requested, contents, byRole, module, calls: () => calls };
}

test('bundle uses length framing, fixed role order and exact content', () => {
  const bytes = portableComponentBundleBytes([{ role: 'x', bytes: Uint8Array.of(1, 2) }]);
  const prefix = encode('conduit.browser/portable-face-bundle@1\n');
  assert.deepEqual(bytes.slice(prefix.length), Uint8Array.of(0, 0, 0, 1, 120, 0, 0, 0, 2, 1, 2));
});

test('exact optional built-in admission uses admitted graph and caches namespace', async () => {
  const f = fixture(); const admit = createPortableComponentAdmission(f.dependencies);
  const first = await admit({ descriptor: f.descriptor, render() {} });
  assert.equal(first.selected, true); assert.equal(first.bundle_sha256, f.descriptor.bundle_sha256);
  assert.equal((await admit({ descriptor: f.descriptor, render() {} })).constructor, first.constructor);
  assert.equal(f.calls(), 1);
  assert.deepEqual(f.requested, ['/app/portable-face-component.mjs', '/app/portable-face-look.css', '/app/portable-face-component.mjs', '/app/portable-face-look.css']);
});

test('missing optional bytes, changed styles, foreign graph and descriptor limits choose generic', async () => {
  for (const mutation of [
    f => { f.dependencies.boundedFetch = async () => { throw new Error('unavailable'); }; },
    f => { f.contents[4][0] ^= 1; },
    f => { f.byRole.get('portable-face-controls').sha256 = `sha256:${'0'.repeat(64)}`; },
    f => { f.descriptor.version = 2; },
    f => { f.descriptor.maximum_nodes = 4096; },
    f => { f.descriptor.resources[3].path = '../foreign.mjs'; },
    f => { f.descriptor.bundle_sha256 = '0'.repeat(64); f.descriptor.artifact_id = `artifact/browser-portable-face-component@1/sha256/${f.descriptor.bundle_sha256}`; },
    f => { f.descriptor.implementation_id = 'foreign'; },
  ]) {
    const f = fixture(); mutation(f);
    const result = await createPortableComponentAdmission(f.dependencies)({ descriptor: f.descriptor, render() {} });
    assert.equal(result.selected, false); assert.equal(typeof result.reason, 'string'); assert.equal(f.calls(), 0);
  }
});

test('resource-exact external CSS and importing component are refused before evaluation', async () => {
  for (const [index, source] of [[4, '@import "remote.css";'], [4, 'a{background:url(x)}'], [3, 'import "x";']]) {
    const f = fixture(); f.contents[index] = encode(source);
    Object.assign(f.descriptor.resources[index], { bytes: f.contents[index].length, sha256: digest(f.contents[index]) });
    f.descriptor.bundle_sha256 = digest(portableComponentBundleBytes(f.descriptor.resources.map((r, i) => ({ role: r.role, bytes: f.contents[i] }))));
    f.descriptor.artifact_id = `artifact/browser-portable-face-component@1/sha256/${f.descriptor.bundle_sha256}`;
    const result = await createPortableComponentAdmission(f.dependencies)({ descriptor: f.descriptor, render() {} });
    assert.equal(result.selected, false); assert.equal(f.calls(), 0);
  }
});

test('foreign custom-element registry cannot become the selected constructor', async () => {
  const f = fixture(); f.dependencies.registry = { get() { return class Foreign {}; } };
  assert.equal((await createPortableComponentAdmission(f.dependencies)({ descriptor: f.descriptor, render() {} })).selected, false);
});
