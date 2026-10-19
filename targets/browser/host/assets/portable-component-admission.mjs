// Host-only admission for the single built-in portable Face component Back.
const IDENTITY = 'conduit.browser/portable-face-component@1';
const SCHEMA = 'conduit.browser/portable-face-component-back@1';
const ROLES = ['controls', 'model', 'combobox', 'component', 'style'];
const PATHS = ROLES.map(role => role === 'style' ? 'portable-face-look.css' : `portable-face-${role}.mjs`);
const LIMITS = [16384, 8192, 8192, 8192, 16384];
const DIGEST = /^[0-9a-f]{64}$/;
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const modules = new Map();

export function portableComponentBundleBytes(resources) {
  const chunks = [encoder.encode('conduit.browser/portable-face-bundle@1\n')];
  for (const { role, bytes } of resources) {
    const name = encoder.encode(role);
    for (const value of [name, bytes]) {
      const length = new Uint8Array(4);
      new DataView(length.buffer).setUint32(0, value.byteLength, false);
      chunks.push(length, value);
    }
  }
  const output = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  let offset = 0;
  for (const chunk of chunks) { output.set(chunk, offset); offset += chunk.byteLength; }
  return output;
}

function checkDescriptor(descriptor) {
  if (descriptor?.schema !== SCHEMA || descriptor.identity !== IDENTITY || descriptor.version !== 1
    || descriptor.tag !== 'conduit-portable-face' || descriptor.maximum_nodes !== 2048
    || descriptor.maximum_depth !== 12 || descriptor.maximum_in_flight_events !== 1
    || descriptor.maximum_bundle_bytes !== 65536
    || descriptor.implementation_id !== 'implementation/browser-portable-face-component@1'
    || descriptor.capability_id !== 'capability/browser-portable-face-component@1'
    || descriptor.artifact_id !== `artifact/browser-portable-face-component@1/sha256/${descriptor.bundle_sha256}`
    || !DIGEST.test(descriptor.bundle_sha256 ?? '') || !Array.isArray(descriptor.resources)
    || descriptor.resources.length !== ROLES.length) throw new Error('Portable component descriptor refused');
  return descriptor.resources.map((resource, index) => {
    if (resource?.role !== ROLES[index] || resource.path !== PATHS[index]
      || !Number.isSafeInteger(resource.bytes) || resource.bytes < 1 || resource.bytes > LIMITS[index]
      || !DIGEST.test(resource.sha256 ?? '')) throw new Error('Portable component resource descriptor refused');
    return Object.freeze({ ...resource });
  });
}

async function importComponent(source) {
  const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));
  try { return await import(url); } finally { URL.revokeObjectURL(url); }
}

export function createPortableComponentAdmission({ packageRoot, byRole, admittedBytes, boundedFetch, packageUrl, sha256,
  importModule = importComponent, registry = globalThis.customElements }) {
  return async function admitPortableFaceComponent({ descriptor, render }) {
    try {
      if (typeof render !== 'function') throw new Error('Portable component renderer refused');
      descriptor = { ...descriptor, resources: descriptor?.resources?.map(resource => ({ ...resource })) };
      const resources = checkDescriptor(descriptor);
      const content = [];
      for (const [index, resource] of resources.entries()) {
        let bytes;
        if (index < 3) {
          const role = `portable-face-${resource.role}`;
          const admitted = byRole.get(role);
          if (!admitted || admitted.kind !== 'module' || admitted.path !== resource.path
            || admitted.sha256 !== `sha256:${resource.sha256}`) throw new Error('Portable component application graph differs');
          bytes = admittedBytes.get(role);
        } else {
          bytes = await boundedFetch(packageUrl(resource.path, packageRoot, 'portable component path'), resource.bytes, 'portable component resource');
        }
        if (!(bytes instanceof Uint8Array) || bytes.byteLength !== resource.bytes
          || (await sha256(bytes)).replace(/^sha256:/, '') !== resource.sha256) throw new Error('Portable component resource changed identity');
        content.push({ role: resource.role, bytes });
      }
      if (content.reduce((sum, { bytes }) => sum + bytes.byteLength, 0) > descriptor.maximum_bundle_bytes
        || (await sha256(portableComponentBundleBytes(content))).replace(/^sha256:/, '') !== descriptor.bundle_sha256) {
        throw new Error('Portable component bundle changed identity');
      }
      const source = decoder.decode(content[3].bytes);
      const styleText = decoder.decode(content[4].bytes);
      if (/\bimport\b|\bfrom\s*["']/.test(source)) throw new Error('Portable component module is not self-contained');
      if (/@import\b|url\s*\(/i.test(styleText)) throw new Error('Portable component style refers to external resources');
      let pending = modules.get(descriptor.bundle_sha256);
      if (!pending) {
        if (modules.size >= 4) throw new Error('Portable component module cache capacity');
        pending = importModule(source);
        modules.set(descriptor.bundle_sha256, pending);
        pending.catch(() => { if (modules.get(descriptor.bundle_sha256) === pending) modules.delete(descriptor.bundle_sha256); });
      }
      const module = await pending;
      const contract = module.PORTABLE_FACE_COMPONENT;
      for (const key of ['identity', 'version', 'tag', 'maximum_nodes', 'maximum_depth', 'maximum_in_flight_events']) {
        if (contract?.[key] !== descriptor[key]) throw new Error('Portable component module contract differs');
      }
      if (typeof module.registerPortableFaceComponent !== 'function') throw new Error('Portable component registration entrance unavailable');
      const constructor = module.registerPortableFaceComponent({ render, styleText });
      if (typeof constructor !== 'function' || registry?.get(descriptor.tag) !== constructor
        || constructor.componentIdentity !== descriptor.identity) throw new Error('Portable component registration differs');
      return Object.freeze({ selected: true, constructor, identity: descriptor.identity, bundle_sha256: descriptor.bundle_sha256 });
    } catch (error) {
      return Object.freeze({ selected: false, reason: error instanceof Error ? error.message : 'Portable component admission refused' });
    }
  };
}
