import test from "node:test";
import assert from "node:assert/strict";
import { openWorkspacePlay } from "../../targets/browser/workspace/workspace-play.mjs";

const terminal = disposition => ({ schema: "conduit.tour/manifestation-receipt@3",
  disposition, active_play_id: "body-play", terminal_sign_id: `sign/${disposition}` });

test("ordinary body wake and lull own one exact externally managed voice Plot", async () => {
  const events = [], states = [];
  const proposal = { schema: "conduit.body/execution-proposal@1",
    plan: { plots: [{ plan: { plan_id: "plan/local", fragments: [{}] } },
      { plan: { plan_id: "plan/voice", fragments: [{}, {}] } }] } };
  const session = {
    propose: async () => proposal,
    started: async value => events.push(`started:${value.play.active_play_id}`),
    lull: async play => events.push(`lull:${play?.active_play_id ?? "none"}`),
    current: () => ({ state: "AWAKE", workload_revision: 1 }),
    evidence: () => ({ realization: {} }), persistenceFailure: () => null,
  };
  const external = { planId: "plan/voice", identity: { active_play_id: "voice-play" },
    run: async () => { events.push("voice-run"); return { disposition: "completed", active_play_id: "voice-play" }; },
    close: async () => events.push("voice-close") };
  const adapter = { start: () => ({ play: { active_play_id: "body-play" } }),
    run: async () => ({ schema: "conduit.browser/pending-effects@1",
      disposition: "quiescent_awaiting_input", active_play_id: "body-play", pending_effects: 0 }),
    close: () => { events.push("body-close"); return { receipt: terminal("cancelled") }; }, evidence: () => null };
  const play = openWorkspacePlay({ host: { runtime: {}, hostId: "host/browser", bootId: "boot/browser" },
    session, source: "catalog", planningLines: () => [], inputTarget: {}, outputRoot: {}, foregroundPlot: () => "checked/local",
    prepareExternal: async supplied => { assert.equal(supplied, proposal); events.push("voice-prepare"); return external; },
    acquireBody: options => { assert.deepEqual(options.externallyManagedPlanIds, ["plan/voice"]); events.push("body-acquire"); return adapter; },
    onState: state => states.push(state.state) });

  await play.wake(true);
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.deepEqual(events.slice(0, 4), ["voice-prepare", "body-acquire", "started:body-play", "voice-run"]);
  assert.equal(states.at(-1), "Idle");
  await play.lull();
  assert.deepEqual(events.slice(-3), ["voice-close", "body-close", "lull:body-play"]);
  assert.equal(states.at(-1), "Lulled");
});

test("finishing retires a play first and publishes one irreversible terminal state", async () => {
  const events = [], states = [];
  let state = "AWAKE";
  const session = {
    propose: async () => ({ plan: { plots: [] } }),
    started: async () => events.push("started"),
    lull: async () => { events.push("lull"); state = "LULLED"; },
    fulfill: async () => { assert.equal(state, "LULLED"); events.push("fulfill"); state = "FULFILLED"; },
    current: () => ({ state, workload_revision: 0 }),
    evidence: () => ({ realization: {} }), persistenceFailure: () => null,
  };
  const adapter = {
    start: () => ({ play: { active_play_id: "body-play" } }),
    run: async () => new Promise(() => {}),
    close: () => ({ receipt: terminal("cancelled") }), evidence: () => null,
  };
  const play = openWorkspacePlay({ host: { runtime: {}, hostId: "host/browser", bootId: "boot/browser" },
    session, source: "catalog", planningLines: () => [], inputTarget: {}, outputRoot: {}, foregroundPlot: () => null,
    acquireBody: () => adapter, onState: value => states.push(value.state) });

  await play.wake();
  await play.fulfill();
  assert.deepEqual(events, ["started", "lull", "fulfill"]);
  assert.equal(state, "FULFILLED");
  assert.equal(states.at(-1), "Fulfilled");
});

// Actual renderer/checked argument path over an injected admitted owner. These
// are adapter controls, not browser/WASM execution or a fabricated owner graph.
function patchbayFixture({ visible = true, onPatchbayPublication } = {}) {
  const states = [], acknowledgements = [], interactions = [], refreshes = [];
  let options, component, status, revision = 1;
  const view = () => ({ body_id: 'body', face_id: `face/${revision}`, face_revision: String(revision),
    show_id: `show/${revision}`, show_state: 'prepared', interactions_admitted: false,
    subjects: [], actions: [{ identity: 'select', target: 'plot/exact', name: 'Select',
      intent: 'patchbay/select-plot@1', availability: 'available', arguments: [] }] });
  let shown = view();
  const observation = () => ({ schema: 'conduit.browser/patchbay-observation@1',
    body_plan_id: 'body-plan', active_play_id: 'body-play', owner_checked_plot_id: 'checked/patchbay',
    mask_show: null, interaction_admitted: false, face: { basis: { body_id: 'body' } },
    publication: { view: shown, effect: { show_id: shown.show_id, manifestation_id: 'manifestation',
      mask_plan_id: 'mask-plan', mask_play: { active_play_id: 'mask-play' }, placement_id: 'slot',
      face_id: shown.face_id, face_revision: revision } } });
  const root = { isConnected: true,
    querySelector: selector => selector === 'conduit-owner-face' ? component : status,
    replaceChildren: (face, message) => { component = face; status = message; },
    ownerDocument: { defaultView: {}, createElement: tag => tag === 'conduit-owner-face' ? {
      isConnected: true, disabled: true, views: [],
      show(value, handlers) { this.views.push(value); this.handlers = handlers; },
      checkVisibility: () => visible,
    } : { dataset: {}, setAttribute() {}, textContent: '' } },
  };
  const adapter = {
    start: () => ({ play: { body_id: 'body', plan_id: 'body-plan', active_play_id: 'body-play' } }),
    run() {
      return Promise.resolve(options.onPatchbayPublication({ observation: observation(), root,
        signal: new AbortController().signal })).then(() => new Promise(() => {}));
    },
    patchbayObservation: observation,
    acknowledgePatchbayFace(message) {
      assert.equal(component.views.at(-1).show_state, 'prepared', 'render precedes ACK');
      acknowledgements.push(message);
      shown = { ...shown, show_state: 'available', interactions_admitted: true };
      return observation();
    },
    async submitPatchbayInteraction(message) {
      interactions.push(message); revision++;
      shown = view();
      return observation();
    },
    refreshPatchbayFace(message) { refreshes.push(message); return observation(); },
    close: () => ({ receipt: terminal('cancelled') }), evidence: () => null,
  };
  const session = {
    propose: async () => ({ plan: { plots: [] } }), started: async () => {}, lull: async () => {},
    current: () => ({ state: 'LULLED', workload_revision: 1 }),
    evidence: () => ({ realization: {} }), persistenceFailure: () => null,
  };
  const handler = () => { throw new Error('no owner effect is inferred by this fixture'); };
  const play = openWorkspacePlay({ host: { runtime: {}, hostId: 'host', bootId: 'boot' },
    session, source: 'catalog', planningLines: () => [], inputTarget: {}, outputRoot: {},
    foregroundPlot: () => 'checked/patchbay', onState: value => states.push(value),
    onPatchbayOwnerRequest: handler, ...(onPatchbayPublication !== undefined ? { onPatchbayPublication } : {}),
    acquireBody: supplied => { options = supplied; return adapter; } });
  return { play, adapter, states, acknowledgements, interactions, refreshes, handler,
    options: () => options, observation, root, component: () => component,
    setObservation(value) { adapter.patchbayObservation = () => value; } };
}
const settle = () => new Promise(resolve => setTimeout(resolve, 0));

test('Workspace binds the existing canonical renderer and typed current action without a second owner', async () => {
  const f = patchbayFixture();
  await f.play.wake(); await settle();
  assert.equal(f.states.at(-1).state, 'Playing');
  assert.equal(typeof f.options().onPatchbayPublication, 'function');
  assert.equal(f.options().onPatchbayOwnerRequest, f.handler);
  assert.equal(f.acknowledgements.length, 1);
  assert.equal(f.acknowledgements[0].show_id, 'show/1');
  assert.equal(f.acknowledgements[0].mask_plan_id, 'mask-plan');
  assert.equal(f.acknowledgements[0].active_play_id, 'mask-play');
  const component = f.component(), current = component.views.at(-1);
  await component.handlers.invoke({ view: current, action: current.actions[0], arguments: [] });
  assert.deepEqual(f.interactions, [{ show_id: 'show/1', face_id: 'face/1', face_revision: 1,
    action_id: 'select', target: 'plot/exact', arguments: [], sequence: 1 }]);
  assert.equal(f.refreshes.length, 1);
  assert.equal(f.acknowledgements.length, 2);
  assert.equal(component.views.at(-1).show_id, 'show/2');
});

test('Workspace canonical publication preserves hidden prepared Show without false acknowledgement', async () => {
  const f = patchbayFixture({ visible: false });
  await f.play.wake(); await settle();
  assert.equal(f.acknowledgements.length, 0);
  assert.equal(f.component().views.at(-1).show_state, 'prepared');
  assert.equal(f.component().disabled, true);
  assert.equal(f.interactions.length, 0);
});

test('Workspace refuses foreign observation and missing renderer instead of claiming canonical success', async () => {
  let callback;
  const f = patchbayFixture({ onPatchbayPublication: request => { callback = request.play; } });
  await f.play.wake(); await settle();
  const current = f.observation();
  f.setObservation({ ...current, body_plan_id: 'foreign' });
  assert.throws(() => callback.patchbayObservation(), /differs from this Body Play/);
  assert.equal(f.interactions.length, 0);
  assert.equal(f.acknowledgements.length, 0);
  const refused = patchbayFixture({ onPatchbayPublication: null });
  await refused.play.wake();
  assert.equal(refused.states.at(-1).state, 'Refused');
  assert.match(refused.states.at(-1).detail, /requires a selected browser Face renderer/);
  assert.equal(refused.options(), undefined, 'missing renderer refuses before resource acquisition');
  assert.equal(refused.interactions.length, 0);
  assert.equal(refused.acknowledgements.length, 0);
});

test('Workspace stale Show and retired adapter cannot dispatch or acknowledge after re-Wake', async () => {
  let renderer;
  const f = patchbayFixture({ onPatchbayPublication: request => { renderer = request.play; } });
  await f.play.wake(); await settle();
  const first = renderer;
  const current = { ...f.observation().publication.view, show_state: 'available', interactions_admitted: true };
  f.setObservation({ ...f.observation(), publication: { ...f.observation().publication, view: current } });
  await assert.rejects(first.submitPatchbayFaceAction({ view: { ...current, show_id: 'retired' },
    actionId: 'select', target: 'plot/exact', arguments: [], sequence: 1 }), error => error.code === 'StaleShow');
  assert.equal(f.interactions.length, 0);
  const oldRender = f.options().onPatchbayPublication;
  await f.play.lull();
  assert.equal(first.state, 'retired');
  assert.throws(() => first.acknowledgePatchbayFace({}), /unavailable/);
  await f.play.wake(); await settle();
  assert.notEqual(renderer, first);
  assert.throws(() => oldRender({ observation: f.observation(), root: f.root,
    signal: new AbortController().signal }), /unavailable/);
  f.play.close();
  assert.equal(renderer.state, 'retired');
  assert.throws(() => renderer.refreshPatchbayFace({}), /unavailable/);
});
