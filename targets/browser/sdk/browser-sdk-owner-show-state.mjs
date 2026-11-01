// Private SDK participation-state transitions; runtime acknowledgement remains authoritative.
// Snapshot caller-bound preference data before awaiting an owner response.
// These fields never cross the owner Line or grant interaction authority.
export function ownerVisualPreferences(state, options) {
  const minimum = options?.minimumTextScalePercent ?? state.minimumOwnerTextScalePercent ?? 100;
  if (!Number.isInteger(minimum) || minimum < 100 || minimum > 200) {
    throw new Error('minimum text scale must be between 100 and 200 percent');
  }
  const enhanced = options?.enhancedContrast === undefined
    ? state.ownerEnhancedContrast ?? false : options.enhancedContrast;
  if (typeof enhanced !== 'boolean') throw new Error('enhanced contrast must be Boolean');
  const input = options?.localLook === undefined ? state.ownerLocalLookBytes : options.localLook;
  let local = null;
  if (input !== undefined && input !== null) {
    const length = input.length;
    if (!(Array.isArray(input) || input instanceof Uint8Array) || !Number.isInteger(length)
        || length < 1 || length > 65_536) {
      throw new Error('local Look must be bounded canonical bytes');
    }
    const bytes = [];
    for (let index = 0; index < length; index++) {
      const value = input[index];
      if (!Number.isInteger(value) || value < 0 || value > 255) throw new Error('local Look must be bounded canonical bytes');
      bytes.push(value);
    }
    local = Object.freeze(bytes);
  }
  state.minimumOwnerTextScalePercent = minimum;
  state.ownerEnhancedContrast = enhanced;
  state.ownerLocalLookBytes = local;
  return { minimum_text_scale_percent: minimum,
    ...(local ? { local_look: local } : {}), ...(enhanced ? { enhanced_contrast: true } : {}) };
}

export function ownerFaceSnapshotOptions(state, options, preferences) {
  // A conditional response would reuse the old Mask without applying personal
  // preferences, including a clear after a previously prepared personal Look.
  const full = preferences.local_look || preferences.enhanced_contrast || state.ownerPersonalLookPrepared;
  return { lastSeenRevision: full ? undefined : options?.lastSeenRevision,
    lastSeenIdentity: full ? undefined : options?.lastSeenIdentity };
}

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
