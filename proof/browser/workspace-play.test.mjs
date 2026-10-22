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

test("Workspace canonical renderer acknowledges only its actually started current Play", async () => {
  const identity = {body_id:"body/current", plan_id:"plan/current", active_play_id:"body-play"};
  const effect = {show_id:"show", manifestation_id:"manifestation", mask_plan_id:"mask-plan",
    mask_play:{active_play_id:"mask-play"}, placement_id:"slot", face_id:"face", face_revision:1};
  const prepared = {schema:"conduit.browser/patchbay-observation@1", mask_show:null,
    interaction_admitted:false, body_plan_id:identity.plan_id, active_play_id:identity.active_play_id,
    face:{basis:{body_id:identity.body_id}}, publication:{effect, view:{show_state:"prepared", show_id:"show"}}};
  let render, acknowledgement;
  const controller = new AbortController();
  const component = {isConnected:true, disabled:true, checkVisibility:()=>true,
    show(view) { this.view = view; }};
  const root = {isConnected:true, querySelector:selector=>selector==="conduit-owner-face"?component:{},
    ownerDocument:{defaultView:{}}};
  const request = {observation:prepared, root, signal:controller.signal};
  const session = {current:()=>({state:"AWAKE"}), propose:async()=>({}), started:async()=>{},
    lull:async()=>{}, evidence:()=>({}), persistenceFailure:()=>null};
  const adapter = {start:()=>({play:identity}),
    acknowledgePatchbayFace(message) { acknowledgement = message;
      return {...prepared, publication:{effect, view:{show_state:"available", show_id:"show"}}}; },
    run:async()=>{ await render(request); return {disposition:"quiescent_awaiting_input"}; },
    close:()=>({receipt:terminal("cancelled")})};
  const play = openWorkspacePlay({host:{runtime:{},hostId:"host",bootId:"boot"}, session,
    source:"catalog", planningLines:()=>[], inputTarget:{}, outputRoot:{}, onState:()=>{},
    acquireBody(options) { render = options.onPatchbayPublication;
      assert.equal(options.onPatchbayOwnerRequest, undefined, "Workspace gains no editor handler");
      assert.throws(()=>render(request), /no current started/, "preparation cannot acknowledge before start");
      return adapter; }});
  await play.wake(); await new Promise(resolve=>setTimeout(resolve,0));
  assert.equal(component.view.show_state, "available");
  assert.equal(acknowledgement.show_id, effect.show_id);
  assert.equal(acknowledgement.active_play_id, effect.mask_play.active_play_id);
  await play.lull();
  assert.throws(()=>render(request), /no current started/);
});
