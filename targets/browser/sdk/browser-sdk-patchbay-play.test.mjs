import test from 'node:test';
import assert from 'node:assert/strict';
import { createPatchbayPlay } from './browser-sdk-patchbay-play.mjs';

function fixture() {
  const identity = { body_id: 'body', plan_id: 'plan', active_play_id: 'play' };
  const view = { body_id: 'body', face_id: 'face', face_revision: '7', show_id: 'show',
    show_state: 'available', interactions_admitted: true,
    actions: [{ identity: 'edit', target: 'plot', intent: 'patchbay/edit-subject@1', availability: 'available',
      arguments: [{ name: 'subject', value_kind: 'value/text', maximum_bytes: 32, choices: ['gear/current'] }] }] };
  let observation = { schema: 'conduit.browser/patchbay-observation@1', mask_show: null,
    interaction_admitted: false, body_plan_id: 'plan', active_play_id: 'play',
    face: { basis: { body_id: 'body' } }, publication: { view }, owner_checked_plot_id: 'owner',
    pending_owner_request: { application_revision: 2, request: { kind: 'EditCurrent', subject_identity: 'gear/current' } } };
  let current = true;
  const calls = [];
  const adapter = {
    patchbayObservation() { calls.push('observe'); return observation; },
    acknowledgePatchbayFace(message) { calls.push(['ack', message]); return observation; },
    submitPatchbayInteraction(message) { calls.push(['interact', message]); return observation; },
    refreshPatchbayFace(message) { calls.push(['refresh', message]); return observation; },
    patchbayRequestDisposition(message) { calls.push(['disposition', message]); return observation; },
  };
  const play = createPatchbayPlay({ adapter, identity, isCurrent: () => current });
  return { play, identity, view, calls, adapter, get observation() { return observation; },
    replace(value) { observation = value; }, retire() { current = false; } };
}

test('shared facade forwards exact current typed action and rereads runtime truth', async () => {
  const f = fixture();
  await f.play.submitPatchbayFaceAction({ view: f.view, actionId: 'edit', target: 'plot',
    arguments: [{ name: 'subject', value: 'gear/current' }], sequence: 1 });
  const message = f.calls.find(call => call[0] === 'interact')[1];
  assert.deepEqual(message, { show_id: 'show', face_id: 'face', face_revision: 7,
    action_id: 'edit', target: 'plot', arguments: [{ name: 'subject', value_kind: 'value/text', value: Array.from(new TextEncoder().encode('gear/current')) }], sequence: 1 });
  const next = { ...f.observation, publication: { view: { ...f.view, show_id: 'replacement' } } };
  f.replace(next);
  assert.equal(f.play.patchbayObservation(), next);
  await assert.rejects(f.play.submitPatchbayFaceAction({view:f.view, actionId:'edit', target:'plot',
    arguments:[{name:'subject', value:'gear/current'}], sequence:2}), /acknowledged/);
});

test('forged action inputs and foreign Body/Plan/Play cannot reach adapter effects', async () => {
  const f = fixture();
  await assert.rejects(f.play.submitPatchbayFaceAction({view:f.view, actionId:'edit', target:'plot',
    arguments:[{name:'subject', value:'forged'}], sequence:1}), /declared|supported/);
  assert.equal(f.calls.some(call => call[0] === 'interact'), false);
  for (const change of [{active_play_id:'other'}, {body_plan_id:'other'}, {face:{basis:{body_id:'other'}}}]) {
    f.replace({...f.observation, ...change});
    assert.throws(() => f.play.patchbayObservation(), /differs/);
    f.replace({...f.observation, active_play_id:'play', body_plan_id:'plan', face:{basis:{body_id:'body'}}});
  }
});

test('identity copy and current guard fence retired/replaced/aborted presentations', async () => {
  const f = fixture();
  f.identity.active_play_id = 'mutated-proposal';
  assert.equal(f.play.patchbayObservation().active_play_id, 'play');
  f.retire();
  assert.equal(f.play.state, 'terminal');
  const before = f.calls.length;
  assert.throws(() => f.play.patchbayObservation(), /unavailable/);
  assert.throws(() => f.play.acknowledgePatchbayFace({}), /unavailable/);
  assert.throws(() => f.play.refreshPatchbayFace({}), /unavailable/);
  await assert.rejects(f.play.submitPatchbayInteraction({}), /unavailable/);
  assert.equal(f.calls.length, before);
});

test('acknowledgement, refresh and owner disposition keep exact pending request identity', () => {
  const f = fixture();
  f.play.acknowledgePatchbayFace({show_id:'show'});
  f.play.refreshPatchbayFace({show_id:'show'});
  const prior = f.observation;
  f.play.reportPatchbayRequestDisposition({observation:prior, outcome:'Completed', evidenceId:'editor/2'});
  assert.deepEqual(f.calls.find(call => call[0] === 'disposition')[1], {
    body_id:'body', body_plan_id:'plan', active_play_id:'play', owner_checked_plot_id:'owner',
    application_revision:2, outcome:'Completed', evidence_id:'editor/2',
  });
  assert.throws(() => f.play.reportPatchbayRequestDisposition({
    observation:{...prior, pending_owner_request:{...prior.pending_owner_request, application_revision:1}},
    outcome:'Completed', evidenceId:'editor/1'}), /exact pending/);
});


test('checked DOM text becomes exact declared UTF8 wire bytes and raw wire input is preserved', async () => {
  const f = fixture();
  const value = 'gear/Crème雪';
  f.view.actions[0].arguments[0].choices = [value];
  await f.play.submitPatchbayFaceAction({ view: f.view, actionId: 'edit', target: 'plot',
    arguments: [{ name: 'subject', value }], sequence: 2 });
  const wire = f.calls.find(call => call[0] === 'interact')[1];
  assert.deepEqual(wire.arguments, [{ name: 'subject', value_kind: 'value/text',
    value: [103, 101, 97, 114, 47, 67, 114, 195, 168, 109, 101, 233, 155, 170] }]);
  await f.play.submitPatchbayInteraction(wire);
  assert.equal(f.calls.filter(call => call[0] === 'interact')[1][1], wire,
    'already typed raw byte envelope is not encoded again');
  const effects = f.calls.filter(call => call[0] === 'interact').length;
  await assert.rejects(f.play.submitPatchbayFaceAction({ view: f.view, actionId: 'edit', target: 'plot',
    arguments: [{ name: 'subject', value: '\ud800' }], sequence: 3 }), { code: 'MalformedEncoding' });
  assert.equal(f.calls.filter(call => call[0] === 'interact').length, effects);
  f.view.show_state = 'prepared';
  await assert.rejects(f.play.submitPatchbayFaceAction({ view: f.view, actionId: 'edit', target: 'plot',
    arguments: [{ name: 'subject', value }], sequence: 3 }), /acknowledged/);
  assert.equal(f.calls.filter(call => call[0] === 'interact').length, effects);
});
