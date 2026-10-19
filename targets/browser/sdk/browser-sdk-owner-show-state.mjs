// Private SDK participation-state transitions; runtime acknowledgement remains authoritative.
export function clearOwnerFaceShow(state) {
  state.pendingOwnerShow = null;
  state.lastOwnerShow = null;
  state.preparedOwnerShow = null;
  return state.ownerShowEpoch = {};
}

export async function acknowledgeOwnerFaceShow(state, snapshot, acknowledge, stillCurrent) {
  clearOwnerFaceShow(state);
  const pending = state.pendingOwnerShow = {};
  try {
    await acknowledge();
    if (state.pendingOwnerShow !== pending || !stillCurrent()) {
      throw new Error('Owner Face acknowledgement is no longer current.');
    }
    state.lastOwnerShow = snapshot;
  } finally {
    if (state.pendingOwnerShow === pending) state.pendingOwnerShow = null;
  }
}

export async function selectOwnerFaceComponent(state, select, refresh, stillCurrent) {
  const epoch = clearOwnerFaceShow(state);
  await select();
  if (state.ownerShowEpoch !== epoch || !stillCurrent()) {
    throw new Error('Owner component selection is no longer current.');
  }
  return refresh();
}

export async function prepareOwnerFaceShow(state, epoch, receive, prepare, stillCurrent) {
  const frame = await receive();
  if (state.ownerShowEpoch !== epoch || !stillCurrent()) {
    throw new Error('Owner Face preparation is no longer current.');
  }
  const view = prepare(frame);
  state.preparedOwnerShow = structuredClone(view);
  return view;
}

// Owner Mask preparation and acknowledgement use this same private state seam.
export function validateOwnerFaceAcknowledgement(view, shown) {
  const sameIdentity = view && shown?.body_id === view.body_id &&
    shown?.show_id === view.show_id && shown?.face_id === view.face_id &&
    shown?.face_revision === view.face_revision;
  const admissionTransition = typeof shown?.interactions_admitted === "boolean" &&
    (view?.show_state === "prepared" ? view.interactions_admitted === false
      : view?.show_state === "available" && shown.interactions_admitted === view.interactions_admitted);
  if (!sameIdentity || shown?.show_state !== "available" || !admissionTransition) {
    throw new Error("browser Mask acknowledged another owner Face Show");
  }
  return structuredClone(shown);
}

/** Run the exact owner Face through this Host's checked browser Mask Plot. */
export async function ownerFacePrepare(state, participation, options) {
  const epoch = clearOwnerFaceShow(state);
  const credential = participation.membershipCredential();
  if (participation.presenceState() !== "available" || !credential) {
    throw new Error("current browser presence is required for the owner Face Mask");
  }
  const minimumTextScale = options?.minimumTextScalePercent
    ?? state.minimumOwnerTextScalePercent ?? 100;
  if (!Number.isInteger(minimumTextScale) || minimumTextScale < 100 || minimumTextScale > 200) {
    throw new Error("minimum text scale must be between 100 and 200 percent");
  }
  state.minimumOwnerTextScalePercent = minimumTextScale;
  const hostId = participation.hostId, bootId = participation.bootId, bodyId = credential.body_id;
  return prepareOwnerFaceShow(state, epoch,
    () => participation.ownerFaceSnapshot(options),
    frame => {
      const mediaMatches = query => globalThis.matchMedia?.(query)?.matches === true;
      const view = state.host.bridge.ownerFacePrepare({
        body_id: bodyId, host_id: hostId, boot_id: bootId,
        visual_constraints: {
          forced_colors: mediaMatches('(forced-colors: active)'),
          reduced_motion: mediaMatches('(prefers-reduced-motion: reduce)'),
          minimum_text_scale_percent: minimumTextScale,
        },
      }, frame);
      if (view?.schema === "conduit.browser/owner-face-refusal@1") {
        throw new Error(`owner Face refused: ${view.code}`);
      }
      if (view?.schema !== "conduit.browser/owner-face-mask@1" ||
          view.body_id !== bodyId || !["prepared", "available"].includes(view.show_state) ||
          typeof view.interactions_admitted !== "boolean") {
        throw new Error("owner Face Mask did not prepare one exact Show");
      }
      return view;
    },
    () => participation.presenceState() === "available" && participation.hostId === hostId && participation.bootId === bootId
      && participation.membershipCredential()?.body_id === bodyId);
}
export async function ownerFaceAcknowledge(state, participation, view) {
  const credential = participation.membershipCredential();
  if (participation.presenceState() !== "available" || !credential ||
      view?.body_id !== credential.body_id || !["prepared", "available"].includes(view?.show_state)) {
    throw new Error("owner Face Show no longer has current browser presence");
  }
  const shown = view.show_state === "available"
    ? state.preparedOwnerShow ?? state.lastOwnerShow
    : state.host.bridge.ownerFaceAcknowledge({
      show_id: view.show_id, face_id: view.face_id, face_revision: view.face_revision,
    });
  // Capture before crossing the asynchronous membership boundary: callers
  // may still own and mutate the available view while acknowledgement waits.
  const acknowledged = validateOwnerFaceAcknowledgement(view, shown);
  const hostId = participation.hostId, bootId = participation.bootId;
  await acknowledgeOwnerFaceShow(state, acknowledged,
    () => state.membership.acknowledgeFaceShow(state.host.bridge.ownerFaceShowReceipt()),
    () => participation.presenceState() === "available" && participation.hostId === hostId && participation.bootId === bootId
      && participation.membershipCredential()?.body_id === acknowledged.body_id);
  return shown;
}
/** Retire this browser Mask and its pending/acknowledged local Show authority. */
export function ownerFaceDiscard(state) {
  state.host.bridge.ownerFaceClear();
  clearOwnerFaceShow(state);
}
export async function ownerComponentSelect(state, participation, { look, recipe, minimumTextScalePercent }) {
  if (minimumTextScalePercent !== undefined &&
      (!Number.isInteger(minimumTextScalePercent) || minimumTextScalePercent < 100 || minimumTextScalePercent > 200)) {
    throw new Error("minimum text scale must be between 100 and 200 percent");
  }
  const credential = participation.membershipCredential();
  if (participation.presenceState() !== "available" || !credential || !state.lastOwnerShow) {
    throw new Error("current acknowledged owner Show is required for component selection");
  }
  const showBytes = state.host.bridge.ownerFaceShowReceipt();
  const hostId = participation.hostId, bootId = participation.bootId, bodyId = credential.body_id;
  return selectOwnerFaceComponent(state,
    () => state.membership.selectComponentMask({ showBytes, look, recipe }),
    () => {
      state.host.bridge.ownerFaceClear();
      return participation.prepareOwnerFaceMask({ minimumTextScalePercent });
    },
    () => participation.presenceState() === "available" && participation.hostId === hostId && participation.bootId === bootId
      && participation.membershipCredential()?.body_id === bodyId);
}

/** Exact retained Face bytes for observation; this does not refresh or acknowledge. */
export function ownerFaceCanonicalSnapshot(state, participation) {
  if (participation.presenceState() !== "available" || !participation.membershipCredential()
      || !state.lastOwnerShow) {
    throw new Error("current acknowledged owner Show is required for canonical observation");
  }
  return state.host.bridge.ownerFaceCanonicalSnapshot();
}

const ownerComponentModules = new Map();
const ownerEncoder = new TextEncoder();
const ownerDecoder = new TextDecoder('utf-8', { fatal: true });
const ownerResources = [
  ['component', 'owner-face-component.mjs', 8192],
  ['presentation', 'owner-face-presentation.mjs', 49152],
  ['style', 'owner-face-component-look.css', 16384],
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
      || descriptor.tag !== 'conduit-owner-face' || descriptor.maximum_bundle_bytes !== 65536
      || !/^[0-9a-f]{64}$/.test(descriptor.bundle_sha256 ?? '')
      || descriptor.artifact_id !== `artifact/browser-owner-face-component@1/sha256/${descriptor.bundle_sha256}`
      || descriptor.implementation_id !== 'implementation/browser-owner-face-component@1'
      || descriptor.capability_id !== 'capability/browser-owner-face-component@1'
      || !Array.isArray(descriptor.resources) || descriptor.resources.length !== 3) throw new Error('Owner component descriptor refused');
    const byRole = new Map(application.manifest.resources.map(resource => [resource.role, resource]));
    const presentation = byRole.get('owner-face-presentation');
    const style = byRole.get('owner-face-look');
    if (presentation?.kind !== 'module' || presentation.path !== ownerResources[1][1]
      || presentation.dependencies.length !== 0 || style?.kind !== 'style' || style.path !== 'owner-face-look.css') {
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
    if (content.reduce((total, resource) => total + resource.bytes.byteLength, 0) > 65536
      || await sha256(ownerBundleBytes(content)) !== `sha256:${descriptor.bundle_sha256}`) throw new Error('Owner component bundle identity differs');
    const presentationSource = ownerDecoder.decode(content[1].bytes);
    const styleText = ownerDecoder.decode(content[2].bytes);
    const source = ownerDecoder.decode(content[0].bytes);
    const marker = '"./owner-face-presentation.mjs"';
    if (source.split(marker).length !== 2 || /\bimport\s*(?:\(|["'])|\bfrom\s*["']/.test(source.replace(marker, ''))
      || /\bimport\s*(?:\(|["'])|\bfrom\s*["']/.test(presentationSource)
      || /@import\b|url\s*\(/i.test(styleText)) throw new Error('Owner component has an unpinned dependency');
    let pending = ownerComponentModules.get(descriptor.bundle_sha256);
    if (!pending) {
      if (ownerComponentModules.size >= 4 || globalThis.customElements?.get(descriptor.tag)) throw new Error('Owner component registration differs');
      const dependencyUrl = URL.createObjectURL(new Blob([presentationSource], { type: 'text/javascript' }));
      const moduleUrl = URL.createObjectURL(new Blob([source.replace(marker, JSON.stringify(dependencyUrl))], { type: 'text/javascript' }));
      pending = (async () => {
        try { return await importModule(moduleUrl); }
        finally { URL.revokeObjectURL(moduleUrl); URL.revokeObjectURL(dependencyUrl); }
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
