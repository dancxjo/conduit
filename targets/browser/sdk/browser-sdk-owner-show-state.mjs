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

// Independent canonical intent is scoped to a membership occurrence, never
// recovered from the rendered component or its routed Plan.
export function ownerHistoryLook(state, basis) {
  const custody = state.ownerHistoryLook;
  if (!custody) return undefined;
  if (!basis || ['body_id','host_id','boot_id','credential_id','part_id'].some(key => custody.basis[key] !== basis[key])) {
    state.ownerHistoryLook = null;
    return undefined;
  }
  return custody.bytes.slice();
}
export function retainOwnerHistoryLook(state, basis, bytes) {
  if (!(bytes instanceof Uint8Array) || !bytes.length || bytes.length > 64 * 1024) {
    throw new Error('history Look exceeds its canonical input bound');
  }
  state.ownerHistoryLook = {basis:{body_id:basis.body_id,host_id:basis.host_id,boot_id:basis.boot_id,credential_id:basis.credential_id,part_id:basis.part_id},bytes:bytes.slice()};
}
