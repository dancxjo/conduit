// Private SDK participation-state transitions; runtime acknowledgement remains authoritative.
export function clearOwnerFaceShow(state) {
  state.pendingOwnerShow = null;
  state.lastOwnerShow = null;
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
  return prepare(frame);
}
