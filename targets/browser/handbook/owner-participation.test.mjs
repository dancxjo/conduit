import test from 'node:test';
import assert from 'node:assert/strict';
import { checkedExpectedBodyId, checkedLoopbackOwnerWindow, checkedOwnerRouteEvidence } from './owner-participation.mjs';

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
