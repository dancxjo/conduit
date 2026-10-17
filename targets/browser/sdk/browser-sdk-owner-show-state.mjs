// Private SDK participation-state transitions; runtime acknowledgement remains authoritative.
export function clearOwnerFaceShow(state) {
  state.pendingOwnerShow = null;
  state.lastOwnerShow = null;
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
