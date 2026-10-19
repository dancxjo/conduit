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
