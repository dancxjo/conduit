import test from 'node:test';
import assert from 'node:assert/strict';
import { renderPatchbayPublication } from './browser-sdk-patchbay.mjs';

function fixture({ visible = true } = {}) {
  let component, status, acknowledgements = 0, visibilityCallback, observing = false;
  const root = { isConnected: true,
    querySelector: selector => selector === 'conduit-owner-face' ? component : status,
    replaceChildren: (face, message) => { component = face; status = message; },
    ownerDocument: { defaultView: { ResizeObserver: class {
      constructor(callback) { visibilityCallback = callback; }
      observe() { observing = true; }
      disconnect() { observing = false; }
    } }, createElement: tag => tag === 'conduit-owner-face' ? {
      isConnected: true, disabled: true, views: [],
      show(view, options) { this.views.push(view); this.options = options; },
      checkVisibility: () => visible,
    } : { dataset: {}, setAttribute() {}, textContent: '' } },
  };
  const observation = { publication: { view: { show_state: 'prepared', show_id: 'show' },
    effect: { show_id: 'show', manifestation_id: 'manifestation', mask_plan_id: 'mask-plan',
      mask_play: { active_play_id: 'mask-play' }, placement_id: 'slot', face_id: 'face', face_revision: 9 } } };
  const play = { state: 'playing', acknowledgePatchbayFace(message) {
    assert.equal(component.views.at(-1).show_state, 'prepared');
    assert.equal(component.isConnected, true);
    assert.equal(message.active_play_id, 'mask-play');
    assert.equal(message.face_revision, 9);
    acknowledgements++;
    return { publication: { view: { show_state: 'available', show_id: 'show' } } };
  } };
  play.patchbayObservation = () => observation;
  return { reveal() { visible = true; if (observing) visibilityCallback(); }, observing: () => observing,
    root, observation, play, signal: new AbortController().signal,
    component: () => component, status: () => status, acknowledgements: () => acknowledgements };
}

test('default Patchbay presentation renders before acknowledgement and restricts supported actions', async () => {
  const f = fixture();
  await renderPatchbayPublication(f);
  assert.equal(f.acknowledgements(), 1);
  assert.equal(f.component().views.at(-1).show_state, 'available');
  assert.equal(f.component().disabled, false);
  assert.equal(f.component().options.supportsAction({ intent: 'body/stop@1' }), false);
  assert.equal(f.component().options.supportsAction({ intent: 'patchbay/edit-subject@1' }), true);
});

test('default Patchbay presentation leaves a hidden resident unacknowledged', async () => {
  const f = fixture({ visible: false });
  await renderPatchbayPublication(f);
  assert.equal(f.acknowledgements(), 0);
  assert.equal(f.component().disabled, true);
  assert.equal(f.component().views.at(-1).show_state, 'prepared');
});

test('default Patchbay input failure shows current truth without replaying or acknowledging again', async () => {
  const f = fixture();
  let submissions = 0;
  f.play.submitPatchbayFaceAction = async () => { submissions++; throw new Error('owner refused'); };
  f.play.patchbayObservation = () => ({ publication: { view: { show_state: 'available', show_id: 'latest' } } });
  await renderPatchbayPublication(f);
  await f.component().options.invoke({ view: {}, action: { identity: 'edit', target: 'plot' }, arguments: [] });
  assert.equal(submissions, 1);
  assert.equal(f.acknowledgements(), 1);
  assert.equal(f.component().views.at(-1).show_id, 'latest');
  assert.match(f.status().textContent, /owner refused/);
});


test('a hidden retained Show is acknowledged on one visibility wake without replaying an action', async () => {
  const f = fixture({ visible: false });
  f.play.submitPatchbayFaceAction = () => { throw new Error('visibility must not submit'); };
  await renderPatchbayPublication(f);
  assert.equal(f.observing(), true);
  f.reveal(); await Promise.resolve();
  assert.equal(f.acknowledgements(), 1);
  assert.equal(f.observing(), false);
  f.reveal(); await Promise.resolve();
  assert.equal(f.acknowledgements(), 1);
});


test('retiring a hidden Play disconnects its visibility wake and cannot acknowledge later', async () => {
  const f = fixture({ visible: false });
  const controller = new AbortController();
  f.signal = controller.signal;
  await renderPatchbayPublication(f);
  assert.equal(f.observing(), true);
  controller.abort();
  assert.equal(f.observing(), false);
  f.reveal(); await Promise.resolve();
  assert.equal(f.acknowledgements(), 0);
});
