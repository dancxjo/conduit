import test from "node:test";
import assert from "node:assert/strict";
import { openWorkspacePlay } from "../../products/workspace/browser/workspace-play.mjs";

const terminal = disposition => ({ schema: "conduit.tour/manifestation-receipt@3",
  disposition, active_play_id: "body-play", terminal_sign_id: `sign/${disposition}` });

test("ordinary body wake and lull own one exact externally managed voice Form", async () => {
  const events = [], states = [];
  const proposal = { schema: "conduit.patchbay/body-execution-proposal@1",
    plan: { forms: [{ plan: { plan_id: "plan/local", fragments: [{}] } },
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
    session, source: "catalog", planningLines: () => [], inputTarget: {}, outputRoot: {}, foregroundForm: () => "checked/local",
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
    propose: async () => ({ plan: { forms: [] } }),
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
    session, source: "catalog", planningLines: () => [], inputTarget: {}, outputRoot: {}, foregroundForm: () => null,
    acquireBody: () => adapter, onState: value => states.push(value.state) });

  await play.wake();
  await play.fulfill();
  assert.deepEqual(events, ["started", "lull", "fulfill"]);
  assert.equal(state, "FULFILLED");
  assert.equal(states.at(-1), "Fulfilled");
});
