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
