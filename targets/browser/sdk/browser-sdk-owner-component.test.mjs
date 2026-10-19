// Deterministic Host admission boundary; actual module execution is Chromium proof.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { checkedComponentMaskRequestFrame } from '../host/assets/browser-membership.js';
import { admitOwnerFaceComponentBack } from './browser-sdk-owner-show-state.mjs';

const encoder = new TextEncoder();
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
let sequence = 0;
function fixture(t, change = () => {}) {
  const sources = [
    `import { OwnerFacePresentation } from "./owner-face-presentation.mjs";\n// fixture ${++sequence}`,
    'export class OwnerFacePresentation {}', ':host { display: block; }',
  ];
  const roles = ['component', 'presentation', 'style'];
  const paths = ['owner-face-component.mjs', 'owner-face-presentation.mjs', 'owner-face-component-look.css'];
  const limits = [8192, 49152, 16384];
  const bytes = sources.map(value => encoder.encode(value));
  const chunks = [Buffer.from('conduit.browser/owner-face-bundle@1\n')];
  for (let index = 0; index < roles.length; index++) {
    for (const value of [Buffer.from(roles[index]), bytes[index]]) {
      const length = Buffer.alloc(4); length.writeUInt32BE(value.byteLength);
      chunks.push(length, value);
    }
  }
  const bundle = digest(Buffer.concat(chunks));
  const descriptor = { schema: 'conduit.browser/owner-face-component-back@1',
    identity: 'conduit.browser/owner-face-component@1', version: 1, tag: 'conduit-owner-face',
    maximum_bundle_bytes: 65536, bundle_sha256: bundle,
    artifact_id: `artifact/browser-owner-face-component@1/sha256/${bundle}`,
    implementation_id: 'implementation/browser-owner-face-component@1',
    capability_id: 'capability/browser-owner-face-component@1',
    resources: roles.map((role, index) => ({ role, path: paths[index], maximum_bytes: limits[index],
      bytes: bytes[index].byteLength, sha256: digest(bytes[index]) })) };
  const resources = [1, 2].map(index => ({ role: index === 1 ? 'owner-face-presentation' : 'owner-face-look',
    kind: index === 1 ? 'module' : 'style', path: index === 1 ? paths[index] : 'owner-face-look.css', dependencies: [],
    sha256: `sha256:${digest(bytes[index])}`, url: new URL(paths[index], 'https://component.test/handbook/') }));
  const application = { manifest: { resources }, bytes: role => bytes[role === 'owner-face-presentation' ? 1 : 2].slice() };
  const memory = new WebAssembly.Memory({ initial: 2 });
  let outputLength = 0, admissionStatus = 0, registered;
  const calls = { admissions: [], imports: 0, fetches: 0, styles: [] };
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'customElements');
  Object.defineProperty(globalThis, 'customElements', { configurable: true,
    value: { get: () => registered } });
  t.after(() => { if (previous) Object.defineProperty(globalThis, 'customElements', previous); else delete globalThis.customElements; });
  const api = { memory,
    conduit_browser_membership_output_ptr: () => 8192,
    conduit_browser_membership_output_len: () => outputLength,
    conduit_browser_membership_input_ptr: () => 0,
    conduit_browser_membership_input_capacity: () => 4096,
    conduit_browser_membership_owner_component_descriptor: () => {
      const value = encoder.encode(JSON.stringify(descriptor)); outputLength = value.byteLength;
      new Uint8Array(memory.buffer, 8192, value.byteLength).set(value); return 0;
    },
    conduit_browser_membership_owner_component_admit: length => {
      calls.admissions.push(JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, 0, length))));
      return admissionStatus;
    },
  };
  class OwnerFaceElement {}
  const services = { sha256: async value => `sha256:${digest(value)}`,
    fetchAsset: async (root, path, maximum) => {
      calls.fetches++; assert.equal(root.href, 'https://component.test/handbook/');
      const index = path === paths[0] ? 0 : 2;
      assert.equal(path, paths[index]); assert.equal(maximum, limits[index]); return bytes[index].slice();
    },
    importModule: async url => {
      calls.imports++; assert.match(url, /^blob:/); registered = OwnerFaceElement;
      return { OwnerFaceElement, admitOwnerFaceStyle: style => { calls.styles.push(style); return OwnerFaceElement; } };
    } };
  const state = { api, hostId: 'host/current', boot: { boot_id: 'boot/current' } };
  const context = { descriptor, application, state, services, calls,
    registerForeign: () => { registered = class Foreign {}; },
    freezeProfile: () => { admissionStatus = -265; } };
  change(context);
  return { ...context, run: () => admitOwnerFaceComponentBack(state, application, services) };
}

test('exact assets bind one current incarnation before advertising; repeat is immutable', async t => {
  const value = fixture(t);
  const selected = await value.run();
  assert.equal(selected.selected, true);
  assert.equal(selected.artifactId, value.descriptor.artifact_id);
  assert.deepEqual(value.calls.admissions, [{ host_id: 'host/current', boot_id: 'boot/current',
    component: { identity: value.descriptor.identity, version: 1, tag: value.descriptor.tag,
      bundle_sha256: value.descriptor.bundle_sha256 } }]);
  assert.equal(value.calls.imports, 1);
  assert.equal(value.calls.styles.length, 1);
  assert.equal(await value.run(), selected);
  assert.equal(value.calls.fetches, 2);
});

for (const [name, change] of [
  ['foreign descriptor', value => { value.descriptor.identity += '/foreign'; }],
  ['wrong asset digest', value => { value.descriptor.resources[0].sha256 = '0'.repeat(64); }],
  ['unadmitted generic dependency', value => { value.application.manifest.resources[0].dependencies.push({ role: 'invented' }); }],
  ['missing optional wrapper', value => { value.services.fetchAsset = async () => { throw new Error('absent'); }; }],
  ['missing optional stylesheet', value => { const fetchAsset = value.services.fetchAsset; value.services.fetchAsset = async (...args) => { if (args[1].endsWith('.css')) throw new Error('absent style'); return fetchAsset(...args); }; }],
  ['unknown registered element', value => value.registerForeign()],
]) {
  test(`${name} leaves the component unadvertised`, async t => {
    const value = fixture(t, change);
    const result = await value.run();
    assert.equal(result.selected, false);
    assert.equal(value.calls.admissions.length, 0);
    assert.equal(value.state.ownerComponentBack, undefined);
  });
}

test('an already advertised Host refuses a late component installation', async t => {
  const value = fixture(t, value => value.freezeProfile());
  const result = await value.run();
  assert.equal(result.selected, false);
  assert.match(result.reason, /already frozen/);
  assert.equal(value.calls.admissions.length, 1);
  assert.equal(value.state.ownerComponentBack, undefined);
});

// A reviewed root is a closed choice; the receipt remains exact WASM bytes.
test('required-item selection preserves the canonical Show integers and rejects invented roots', () => {
  const show = '{"revision":18446744073709551615}';
  const options = { requestId: 'browser-component:1', request: {}, showBytes: encoder.encode(show),
    look: 'compact', recipe: 'required-item' };
  const frame = new TextDecoder().decode(checkedComponentMaskRequestFrame(options));
  assert.match(frame, /"recipe":"required-item"/);
  assert.ok(frame.endsWith('"show":' + show + '}'));
  for (const recipe of ['requireditem', 'required-item/forged', '__proto__']) {
    assert.throws(() => checkedComponentMaskRequestFrame({ ...options, recipe }), /finite fields/);
  }
});
