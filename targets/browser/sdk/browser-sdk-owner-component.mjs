const ownerComponentModules = new Map();
const ownerEncoder = new TextEncoder();
const ownerDecoder = new TextDecoder('utf-8', { fatal: true });
const ownerResources = [
  ['component', 'owner-face-component.mjs', 8192],
  ['presentation', 'owner-face-presentation.mjs', 65536],
  ['pointer', 'owner-face-pointer.mjs', 8192],
  ['style', 'owner-face-look.css', 16384],
  ['icons', 'owner-face-icons.mjs', 32768],
  ['look', 'application-look.mjs', 32768],
];

function ownerBundleBytes(resources) {
  const chunks = [ownerEncoder.encode('conduit.browser/owner-face-bundle@1\n')];
  for (const { role, bytes } of resources) {
    for (const value of [ownerEncoder.encode(role), bytes]) {
      const length = new Uint8Array(4);
      new DataView(length.buffer).setUint32(0, value.byteLength, false);
      chunks.push(length, value);
    }
  }
  const result = new Uint8Array(chunks.reduce((total, chunk) => total + chunk.byteLength, 0));
  let offset = 0;
  for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.byteLength; }
  return result;
}

// Trusted Host admission over the application's already verified generic DOM
// module and style, plus one optional, digest-pinned custom-element sidecar.
export async function admitOwnerFaceComponentBack(state, application, { fetchAsset, sha256, importModule }) {
  if (state.ownerComponentBack) return state.ownerComponentBack;
  try {
    const api = state.api;
    const read = () => {
      const length = api.conduit_browser_membership_output_len();
      if (!Number.isSafeInteger(length) || length < 1 || length > 8192) throw new Error('Owner component descriptor bound');
      return new Uint8Array(api.memory.buffer, api.conduit_browser_membership_output_ptr(), length).slice();
    };
    if (typeof api.conduit_browser_membership_owner_component_descriptor !== 'function'
      || typeof api.conduit_browser_membership_owner_component_admit !== 'function'
      || api.conduit_browser_membership_owner_component_descriptor() < 0) throw new Error('Owner component admission unavailable');
    const descriptor = JSON.parse(ownerDecoder.decode(read()));
    if (descriptor.schema !== 'conduit.browser/owner-face-component-back@1'
      || descriptor.identity !== 'conduit.browser/owner-face-component@1' || descriptor.version !== 1
      || descriptor.tag !== 'conduit-owner-face' || descriptor.maximum_bundle_bytes !== 131072
      || !/^[0-9a-f]{64}$/.test(descriptor.bundle_sha256 ?? '')
      || descriptor.artifact_id !== `artifact/browser-owner-face-component@1/sha256/${descriptor.bundle_sha256}`
      || descriptor.implementation_id !== 'implementation/browser-owner-face-component@1'
      || descriptor.capability_id !== 'capability/browser-owner-face-component@1'
      || !Array.isArray(descriptor.resources) || descriptor.resources.length !== 6) throw new Error('Owner component descriptor refused');
    const byRole = new Map(application.manifest.resources.map(resource => [resource.role, resource]));
    const presentation = byRole.get('owner-face-presentation');
    const style = byRole.get('owner-face-look');
    const expectedDependencies = [
      ['owner-face-icons', './owner-face-icons.mjs'],
      ['application-look', '../host/assets/application-look.mjs'],
      ['owner-face-pointer', './owner-face-pointer.mjs'],
    ];
    if (presentation?.kind !== 'module' || presentation.path !== ownerResources[1][1]
      || !Array.isArray(presentation.dependencies)
      || JSON.stringify(presentation.dependencies.map(({role, specifier}) => [role, specifier])) !== JSON.stringify(expectedDependencies)
      || !['style', 'content'].includes(style?.kind) || style.path !== 'owner-face-look.css') {
      throw new Error('Owner component generic dependency graph differs');
    }
    const packageRoot = new URL('./', presentation.url);
    const content = [];
    for (const [index, [role, path, maximum]] of ownerResources.entries()) {
      const resource = descriptor.resources[index];
      if (resource?.role !== role || resource.path !== path || resource.maximum_bytes !== maximum
        || !Number.isSafeInteger(resource.bytes) || resource.bytes < 1 || resource.bytes > maximum
        || !/^[0-9a-f]{64}$/.test(resource.sha256 ?? '')) throw new Error('Owner component resource descriptor refused');
      const bytes = index !== 1 ? await fetchAsset(packageRoot, path, maximum)
        : application.bytes('owner-face-presentation');
      if (!(bytes instanceof Uint8Array) || bytes.byteLength !== resource.bytes
        || await sha256(bytes) !== `sha256:${resource.sha256}`
        || (index === 1 && presentation.sha256 !== `sha256:${resource.sha256}`)) {
        throw new Error('Owner component resource identity differs');
      }
      content.push({ role, bytes });
    }
    if (content.reduce((total, resource) => total + resource.bytes.byteLength, 0) > 131072
      || await sha256(ownerBundleBytes(content)) !== `sha256:${descriptor.bundle_sha256}`) throw new Error('Owner component bundle identity differs');
    const styleText = ownerDecoder.decode(content[3].bytes);
    if (/@import\b|url\s*\(/i.test(styleText)) throw new Error('Owner component has an unpinned style dependency');
    const dependencyGraph = new Map([
      [0, [[1, '"./owner-face-presentation.mjs"'], [2, '"./owner-face-pointer.mjs"']]],
      [1, [[5, '"../host/assets/application-look.mjs"'], [4, '"./owner-face-icons.mjs"'], [2, '"./owner-face-pointer.mjs"']]],
      [2, []], [4, []], [5, []],
    ]);
    const sources = new Map();
    for (const [index, dependencies] of dependencyGraph) {
      const source = ownerDecoder.decode(content[index].bytes);
      let remainder = source;
      for (const [, marker] of dependencies) {
        if (remainder.split(marker).length !== 2) throw new Error('Owner component dependency identity differs');
        remainder = remainder.replace(marker, '');
      }
      if (/\bimport\s*(?:\(|["'])|\bfrom\s*["']/.test(remainder)) throw new Error('Owner component has an unpinned dependency');
      sources.set(index, source);
    }
    let pending = ownerComponentModules.get(descriptor.bundle_sha256);
    if (!pending) {
      if (ownerComponentModules.size >= 4 || globalThis.customElements?.get(descriptor.tag)) throw new Error('Owner component registration differs');
      const urls = new Map();
      const moduleUrl = index => {
        if (urls.has(index)) return urls.get(index);
        let source = sources.get(index);
        for (const [dependency, marker] of dependencyGraph.get(index)) source = source.replace(marker, JSON.stringify(moduleUrl(dependency)));
        const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
        urls.set(index, url);
        return url;
      };
      pending = (async () => {
        try { return await importModule(moduleUrl(0)); }
        finally { for (const url of urls.values()) URL.revokeObjectURL(url); }
      })();
      ownerComponentModules.set(descriptor.bundle_sha256, pending);
      pending.catch(() => { if (ownerComponentModules.get(descriptor.bundle_sha256) === pending) ownerComponentModules.delete(descriptor.bundle_sha256); });
    }
    const module = await pending;
    if (typeof module.OwnerFaceElement !== 'function' || typeof module.admitOwnerFaceStyle !== 'function'
      || globalThis.customElements?.get(descriptor.tag) !== module.OwnerFaceElement
      || module.admitOwnerFaceStyle(styleText) !== module.OwnerFaceElement) throw new Error('Owner component registration refused');
    const component = { identity: descriptor.identity, version: descriptor.version, tag: descriptor.tag, bundle_sha256: descriptor.bundle_sha256 };
    const request = ownerEncoder.encode(JSON.stringify({ host_id: state.hostId, boot_id: state.boot.boot_id, component }));
    if (request.byteLength > api.conduit_browser_membership_input_capacity()) throw new Error('Owner component admission input bound');
    new Uint8Array(api.memory.buffer, api.conduit_browser_membership_input_ptr(), request.byteLength).set(request);
    if (api.conduit_browser_membership_owner_component_admit(request.byteLength) < 0) throw new Error('Owner component Host profile is already frozen or differs');
    return state.ownerComponentBack = Object.freeze({ selected: true, constructor: module.OwnerFaceElement,
      identity: descriptor.identity, artifactId: descriptor.artifact_id, bundleSha256: descriptor.bundle_sha256 });
  } catch (error) {
    return Object.freeze({ selected: false, reason: error instanceof Error ? error.message : 'Owner component admission refused' });
  }
}
