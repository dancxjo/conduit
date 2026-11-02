import test from 'node:test';
import assert from 'node:assert/strict';
import { publicKeyBase64, checkedOwnerWindowHint, prefillOwnerWindowHint, checkedExpectedBodyId, checkedLoopbackOwnerWindow, checkedOwnerRouteEvidence, projectFaceCollections } from './owner-participation.mjs';

test('Handbook owner window accepts only the local admitted carrier', () => {
  assert.equal(checkedLoopbackOwnerWindow('ws://127.0.0.1:41000/conduit'), 'ws://127.0.0.1:41000/conduit');
  for (const url of [
    'ws://localhost:41000/conduit', 'ws://192.168.1.2:41000/conduit',
    'wss://127.0.0.1:41000/conduit', 'ws://127.0.0.1:41000/other',
    'ws://127.0.0.1:41000/?token=secret', 'ws://user@127.0.0.1:41000/',
  ]) assert.throws(() => checkedLoopbackOwnerWindow(url), /exact ws:\/\/127\.0\.0\.1/);
});

test('route inspection requires the acknowledged Show on this exact browser Boot', () => {
  const view = {
    body_id: 'body/one', face_id: 'face/one', face_revision: '9',
    mask_plot_id: 'plot/browser', mask_plan_id: 'plan/mask', mask_play_id: 'play/mask',
    show_id: 'show/one', show_state: 'available', interactions_admitted: true,
    route: { plan_id: 'plan/route', owner_host_id: 'host/linux', owner_boot_id: 'boot/linux',
      mask_host_id: 'host/browser', mask_boot_id: 'boot/browser' },
  };
  assert.deepEqual(checkedOwnerRouteEvidence(view, 'host/browser', 'boot/browser'), {
    body_id: 'body/one', face_id: 'face/one', face_revision: '9',
    selected_mask_plot_id: 'plot/browser', route_plan_id: 'plan/route',
    owner_host_id: 'host/linux', owner_boot_id: 'boot/linux',
    mask_host_id: 'host/browser', mask_boot_id: 'boot/browser',
    mask_plan_id: 'plan/mask', mask_play_id: 'play/mask', show_id: 'show/one',
    show_state: 'available', interactions_admitted: true,
  });
  assert.throws(() => checkedOwnerRouteEvidence(view, 'host/browser', 'boot/new'), /current sealed browser route/);
  assert.throws(() => checkedOwnerRouteEvidence({ ...view, show_state: 'prepared' }, 'host/browser', 'boot/browser'), /current sealed browser route/);
  assert.throws(() => checkedOwnerRouteEvidence({ ...view, route: null }, 'host/browser', 'boot/browser'), /current sealed browser route/);
});

test('Handbook requires an exact bounded owner Body identity', () => {
  assert.equal(checkedExpectedBodyId(' body/owner '), 'body/owner');
  assert.throws(() => checkedExpectedBodyId(''), /exact Body ID/);
  assert.throws(() => checkedExpectedBodyId('x'.repeat(129)), /exact Body ID/);
});


test('simple collection projection folds only redundant count status', () => {
  const collection = { identity: 'list', role: 'Collection', disclosure: 'Primary', properties: [], values: [] };
  const item = { identity: 'item', role: 'Item', disclosure: 'Primary', text: [], properties: [], values: [],
    flags: [{ name: 'complete', value: false }] };
  const status = { identity: 'status', role: 'Status', text: ['1 thing left · 0 completed'], properties: [], values: [] };
  const view = { subjects: [collection, item, status], actions: [], relationships: [item, status]
    .map(subject => ({ kind: 'Contains', source: collection.identity, target: subject.identity })) };
  const projected = projectFaceCollections(view);
  assert.equal(projected.collections.length, 1);
  assert.deepEqual(projected.collections[0].open, [item]);
  assert.deepEqual(projected.collections[0].completed, []);
  assert.equal(projected.collections[0].count, status.text[0]);
  assert.deepEqual([...projected.placed], ['list', 'item', 'status']);
  const actionable = projectFaceCollections({ ...view, actions: [{ identity: 'inspect', target: status.identity }] });
  assert.equal(actionable.collections.length, 1);
  assert.equal(actionable.placed.has(status.identity), false);
});


test('owner link hints require one bounded complete pair and no credentials or remote window', () => {
  const query = values => `?${new URLSearchParams(values)}`;
  const exact = { participate: 'owner', body: 'body/one', window: 'ws://127.0.0.1:46156/conduit' };
  assert.deepEqual(checkedOwnerWindowHint(query(exact)), { body: exact.body, window: exact.window });
  assert.equal(checkedOwnerWindowHint('?participate=owner'), null);
  for (const search of [
    '?body=body/one', '?window=ws%3A%2F%2F127.0.0.1%3A46156%2Fconduit',
    `${query(exact)}&body=body/two`, `${query(exact)}&window=${encodeURIComponent(exact.window)}`,
    `${query(exact)}&participate=owner`, `${query(exact)}&token=private`,
    query({ ...exact, body: '' }), query({ ...exact, body: 'x'.repeat(129) }),
    query({ ...exact, window: 'ws://192.168.1.1:46156/conduit' }),
    query({ ...exact, window: 'ws://user:secret@127.0.0.1:46156/conduit' }),
    query({ ...exact, window: 'ws://127.0.0.1:46156/conduit?token=private' }),
    query({ ...exact, window: 'ws://127.0.0.1:46156/conduit#secret' }),
    `?${'x'.repeat(4097)}`,
  ]) assert.throws(() => checkedOwnerWindowHint(search));
});

test('checked prefill mutates only editable fields and never submits or connects', () => {
  const fields = { '[name="body"]': { value: '' }, '[name="window"]': { value: '' } };
  const form = { querySelector(selector) { assert(selector in fields); return fields[selector]; },
    submit() { assert.fail('prefill cannot submit'); }, requestSubmit() { assert.fail('prefill cannot request submission'); } };
  const search = `?${new URLSearchParams({ participate: 'owner', body: 'body/one', window: 'ws://127.0.0.1:46156/conduit' })}`;
  assert.equal(prefillOwnerWindowHint(form, '?participate=owner'), false);
  assert.equal(fields['[name="body"]'].value, '');
  assert.throws(() => prefillOwnerWindowHint(form, `${search}&window=bad`));
  assert.equal(fields['[name="body"]'].value, '');
  assert.equal(prefillOwnerWindowHint(form, search), true);
  assert.equal(fields['[name="body"]'].value, 'body/one');
  assert.equal(fields['[name="window"]'].value, 'ws://127.0.0.1:46156/conduit');
});

test("public key clipboard form is standard Base64 with unchanged bytes", () => {
  const key = Array.from({ length: 32 }, (_, index) => index);
  const encoded = publicKeyBase64(key);
  assert.equal(encoded, "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=");
  assert.deepEqual([...Buffer.from(encoded, "base64")], key);
  assert.throws(() => publicKeyBase64(key.slice(1)), /32 bytes/);
  assert.throws(() => publicKeyBase64([...key.slice(1), 256]), /32 bytes/);
});
