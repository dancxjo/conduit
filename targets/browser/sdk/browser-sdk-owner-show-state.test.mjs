import assert from 'node:assert/strict';
import test from 'node:test';
import { acknowledgeOwnerFaceShow, clearOwnerFaceShow } from './browser-sdk-owner-show-state.mjs';
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

test('clearing a Show fences an acknowledgement already awaiting the owner', async () => {
  const state = { lastOwnerShow: { show_id: 'previous' } }, gate = deferred();
  const pending = acknowledgeOwnerFaceShow(state, { show_id: 'pending' }, () => gate.promise, () => true);
  clearOwnerFaceShow(state);
  const refused = assert.rejects(pending, /no longer current/);
  gate.resolve(); await refused;
  assert.equal(state.lastOwnerShow, null);
});

for (const newestFirst of [false, true]) {
  test(`newer acknowledgement supersedes old completion (newest first: ${newestFirst})`, async () => {
    const state = {}, first = deferred(), second = deferred();
    const older = acknowledgeOwnerFaceShow(state, { show_id: 'old' }, () => first.promise, () => true);
    const newer = acknowledgeOwnerFaceShow(state, { show_id: 'new' }, () => second.promise, () => true);
    const refused = assert.rejects(older, /no longer current/);
    if (newestFirst) { second.resolve(); await newer; first.resolve(); await refused; }
    else { first.resolve(); await refused; assert.equal(state.lastOwnerShow, null); second.resolve(); await newer; }
    assert.deepEqual(state.lastOwnerShow, { show_id: 'new' });
  });
}

test('lost presence and owner refusal never publish an acknowledged snapshot', async () => {
  const state = { lastOwnerShow: { show_id: 'previous' } }, gate = deferred();
  let available = true;
  const pending = acknowledgeOwnerFaceShow(state, { show_id: 'pending' }, () => gate.promise, () => available);
  available = false;
  const refused = assert.rejects(pending, /no longer current/);
  gate.resolve(); await refused;
  assert.equal(state.lastOwnerShow, null);
  await assert.rejects(acknowledgeOwnerFaceShow(state, {}, async () => { throw new Error('owner refused'); }, () => true), /owner refused/);
  assert.equal(state.lastOwnerShow, null);
});

test('an old owner refusal cannot clear a newer pending acknowledgement', async () => {
  const state = {}, first = deferred(), second = deferred();
  const older = acknowledgeOwnerFaceShow(state, { show_id: 'old' }, () => first.promise, () => true);
  const newer = acknowledgeOwnerFaceShow(state, { show_id: 'new' }, () => second.promise, () => true);
  const refused = assert.rejects(older, /old owner refused/);
  first.reject(new Error('old owner refused')); await refused;
  second.resolve(); await newer;
  assert.deepEqual(state.lastOwnerShow, { show_id: 'new' });
  assert.equal(state.pendingOwnerShow, null);
});

for (const invalidate of ['new-show', 'lost-presence']) {
  test(`component selection cannot refresh after ${invalidate}`, async () => {
    const { selectOwnerFaceComponent } = await import('./browser-sdk-owner-show-state.mjs');
    const state = { lastOwnerShow: { show_id: 'old' } }, gate = deferred();
    let available = true, refreshes = 0;
    const pending = selectOwnerFaceComponent(state, () => gate.promise, () => { refreshes++; }, () => available);
    if (invalidate === 'new-show') {
      clearOwnerFaceShow(state); state.lastOwnerShow = { show_id: 'new' };
    } else available = false;
    const refused = assert.rejects(pending, /selection is no longer current/);
    gate.resolve(); await refused;
    assert.equal(refreshes, 0);
    assert.deepEqual(state.lastOwnerShow, invalidate === 'new-show' ? { show_id: 'new' } : null);
  });
}

test('current component selection refreshes once and preserves owner refusal', async () => {
  const { selectOwnerFaceComponent } = await import('./browser-sdk-owner-show-state.mjs');
  const state = { lastOwnerShow: { show_id: 'old' } };
  let refreshes = 0;
  assert.equal(await selectOwnerFaceComponent(state, async () => {}, () => { refreshes++; return 'prepared'; }, () => true), 'prepared');
  assert.equal(refreshes, 1); assert.equal(state.lastOwnerShow, null);
  await assert.rejects(selectOwnerFaceComponent(state, async () => { throw new Error('selection refused'); },
    () => { refreshes++; }, () => true), /selection refused/);
  assert.equal(refreshes, 1);
});

for (const newestFirst of [false, true]) {
  test(`older Face frame never reaches preparation (newest first: ${newestFirst})`, async () => {
    const { prepareOwnerFaceShow } = await import('./browser-sdk-owner-show-state.mjs');
    const state = {}, first = deferred(), second = deferred(), prepared = [];
    const prepare = frame => { prepared.push(frame); return frame; };
    const older = prepareOwnerFaceShow(state, clearOwnerFaceShow(state), () => first.promise, prepare, () => true);
    const newer = prepareOwnerFaceShow(state, clearOwnerFaceShow(state), () => second.promise, prepare, () => true);
    const refused = assert.rejects(older, /preparation is no longer current/);
    if (newestFirst) { second.resolve('new'); await newer; first.resolve('old'); await refused; }
    else { first.resolve('old'); await refused; second.resolve('new'); await newer; }
    assert.deepEqual(prepared, ['new']);
  });
}

for (const invalidation of ['clear', 'presence']) {
  test(`pending Face frame cannot prepare after ${invalidation}`, async () => {
    const { prepareOwnerFaceShow } = await import('./browser-sdk-owner-show-state.mjs');
    const state = {}, gate = deferred(); let available = true, preparations = 0;
    const pending = prepareOwnerFaceShow(state, clearOwnerFaceShow(state), () => gate.promise,
      () => { preparations++; }, () => available);
    if (invalidation === 'clear') clearOwnerFaceShow(state); else available = false;
    const refused = assert.rejects(pending, /preparation is no longer current/);
    gate.resolve('frame'); await refused;
    assert.equal(preparations, 0);
  });
}

test('current Face frame prepares once; failed download never invokes preparation', async () => {
  const { prepareOwnerFaceShow } = await import('./browser-sdk-owner-show-state.mjs');
  const state = {}; let preparations = 0;
  const prepare = frame => { preparations++; return frame; };
  assert.equal(await prepareOwnerFaceShow(state, clearOwnerFaceShow(state), async () => 'frame', prepare, () => true), 'frame');
  await assert.rejects(prepareOwnerFaceShow(state, clearOwnerFaceShow(state), async () => { throw new Error('download refused'); },
    prepare, () => true), /download refused/);
  assert.equal(preparations, 1);
});

test('prepared Show custody is detached from returned views and cleared on invalidation', async () => {
  const { prepareOwnerFaceShow } = await import('./browser-sdk-owner-show-state.mjs');
  const state = {}, view = { show_id: 'show/current', show_state: 'available',
    actions: [{ arguments: [{ choices: ['yes'] }] }] };
  await prepareOwnerFaceShow(state, clearOwnerFaceShow(state), async () => 'frame', () => view, () => true);
  view.actions[0].arguments[0].choices.push('forged');
  assert.deepEqual(state.preparedOwnerShow.actions[0].arguments[0].choices, ['yes']);
  clearOwnerFaceShow(state);
  assert.equal(state.preparedOwnerShow, null);
});

test('history intent custody copies both inputs and outputs and clears on membership replacement', async () => {
  const {ownerHistoryLook,retainOwnerHistoryLook} = await import('./browser-sdk-owner-show-state.mjs');
  const state = {}, basis = {body_id:'body/one',host_id:'host/one',boot_id:'boot/one',credential_id:'credential/one',part_id:'part/one'};
  const source = Uint8Array.of(1,2,3);
  retainOwnerHistoryLook(state,basis,source); source[0]=9;
  const first = ownerHistoryLook(state,basis); assert.deepEqual([...first],[1,2,3]);
  first[1]=9; assert.deepEqual([...ownerHistoryLook(state,basis)],[1,2,3]);
  clearOwnerFaceShow(state); assert.deepEqual([...ownerHistoryLook(state,basis)],[1,2,3]);
  for (const key of ['body_id','host_id','boot_id','credential_id','part_id']) {
    retainOwnerHistoryLook(state,basis,Uint8Array.of(1));
    assert.equal(ownerHistoryLook(state,{...basis,[key]:'different'}),undefined);
    assert.equal(ownerHistoryLook(state,basis),undefined);
  }
  assert.throws(() => retainOwnerHistoryLook(state,basis,new Uint8Array()));
  assert.throws(() => retainOwnerHistoryLook(state,basis,new Uint8Array(65537)));
});

test('history input is retained only after accepted current selection and survives ordinary refresh', async () => {
  const {selectOwnerFaceComponent,ownerHistoryLook,retainOwnerHistoryLook} = await import('./browser-sdk-owner-show-state.mjs');
  const basis = {body_id:'body/one',host_id:'host/one',boot_id:'boot/one',credential_id:'credential/one',part_id:'part/one'};
  for (const outcome of ['accepted','refused','superseded','presence-lost']) {
    const state = {}, gate = deferred(); let current = true;
    retainOwnerHistoryLook(state,basis,Uint8Array.of(1));
    const pending = selectOwnerFaceComponent(state,() => gate.promise,() => {
      retainOwnerHistoryLook(state,basis,Uint8Array.of(2));
      return ownerHistoryLook(state,basis);
    },() => current);
    const refused = outcome === 'accepted' ? null : assert.rejects(pending);
    if (outcome === 'superseded') clearOwnerFaceShow(state);
    if (outcome === 'presence-lost') current = false;
    if (outcome === 'refused') gate.reject(new Error('owner refused')); else gate.resolve();
    if (refused) { await refused; assert.deepEqual([...ownerHistoryLook(state,basis)],[1]); }
    else { assert.deepEqual([...(await pending)],[2]); clearOwnerFaceShow(state); assert.deepEqual([...ownerHistoryLook(state,basis)],[2]); }
  }
});
