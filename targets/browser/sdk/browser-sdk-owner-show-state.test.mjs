import assert from 'node:assert/strict';
import test from 'node:test';
import { acknowledgeOwnerFaceShow, clearOwnerFaceShow, ownerVisualPreferences, ownerFaceSnapshotOptions } from './browser-sdk-owner-show-state.mjs';
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

test('personal Look bytes are independent snapshots across Show replacement and explicit clearing', () => {
  const state = {}, input = new Uint8Array([1, 2, 3]);
  const first = ownerVisualPreferences(state, { localLook: input, enhancedContrast: true });
  input[0] = 9;
  assert.deepEqual(first.local_look, [1, 2, 3]);
  assert(Object.isFrozen(first.local_look));
  clearOwnerFaceShow(state);
  assert.deepEqual(ownerVisualPreferences(state), first);
  assert.deepEqual(ownerVisualPreferences(state, { localLook: null, enhancedContrast: false }),
    { minimum_text_scale_percent: 100 });
});

test('invalid preferences refuse atomically without replacing prior preferences', () => {
  const state = {};
  ownerVisualPreferences(state, { localLook: [1], minimumTextScalePercent: 150 });
  const previous = structuredClone(state);
  for (const options of [{ localLook: [] }, { localLook: [256] }, { localLook: [1.2] },
    { localLook: new Uint8Array(65_537) }, { localLook: 'selectors' },
    { enhancedContrast: 'yes' }, { enhancedContrast: null }, { minimumTextScalePercent: 99 }]) {
    assert.throws(() => ownerVisualPreferences(state, options));
    assert.deepEqual(state, previous);
  }
});

test('personal preference preparation and clearing require a full frame without leaking local data', () => {
  const prior = { lastSeenRevision: '9', lastSeenIdentity: 'face/9', localLook: [1], enhancedContrast: true };
  assert.deepEqual(ownerFaceSnapshotOptions({}, prior, {}),
    { lastSeenRevision: '9', lastSeenIdentity: 'face/9' });
  for (const [state, preferences] of [[{}, { local_look: [1] }], [{}, { enhanced_contrast: true }],
    [{ ownerPersonalLookPrepared: true }, {}]]) {
    assert.deepEqual(ownerFaceSnapshotOptions(state, prior, preferences),
      { lastSeenRevision: undefined, lastSeenIdentity: undefined });
  }
});

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
