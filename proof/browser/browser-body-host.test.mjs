import test from "node:test";
import assert from "node:assert/strict";
import { acquireBrowserBodyHost } from "../../targets/browser/host/assets/browser-body-host.mjs";

function fixture({ timer = false, timerEffects = timer ? 1 : 0, timerDuration = 10_000, inputUnits = 0, text = "hello", quiescent = false, immediate = false } = {}) {
  const memory = { buffer: new ArrayBuffer(512 * 1024) };
  let length = 0, starts = 0, cancels = 0, polls = 0, completions = 0, request;
  const output = value => {
    const bytes = new TextEncoder().encode(JSON.stringify(value));
    new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);length = bytes.length;
  };
  const effect = index => ({ host_id: "host", boot_id: "boot", active_play_id: "play", placement_id: `placement-${index}`, plan_id: "partition", request_sequence: index,
    ...(timerEffects ? { effect_kind: "timer", duration_millis: timerDuration } : { effect_kind: "manifestation", presentation_kind: "presentation/text", text }) });
  const api = {
    memory,
    conduit_browser_form_human_machinery() { output({ schema: "conduit.browser/selected-human-machinery@1", limits: { maximum_gears: 32 }, implementations: [{ id: "browser/dom-presentation@1", revision: 1 }, { id: "browser/pointer-events@1", revision: 1 }] });return 0; },
    conduit_browser_form_output_ptr: () => 0, conduit_browser_form_output_len: () => length,
    conduit_browser_body_input_ptr: () => 256 * 1024, conduit_browser_body_input_capacity: () => 256 * 1024,
    conduit_browser_body_start(length) {
      starts++;request = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, 256 * 1024, length)));
      output({ schema: "conduit.browser/body-started@1", play: { active_play_id: "play" }, progress: immediate
        ? { schema: "conduit.tour/manifestation-receipt@3", disposition: "completed", active_play_id: "play" } : effect(0) });return 0;
    },
    conduit_browser_form_pending_capacity: () => 16,
    conduit_browser_form_poll_effect() {
      output(++polls < timerEffects ? effect(polls) : { disposition: "waiting" });return 0;
    },
    conduit_browser_form_input_ptr: () => 128 * 1024, conduit_browser_form_input_capacity: () => 64 * 1024,
    conduit_browser_form_complete_effect() {
      completions++;
      output(quiescent
        ? { schema: "conduit.browser/pending-effects@1", disposition: "quiescent_awaiting_input", active_play_id: "play" }
        : completions < timerEffects ? { disposition: "waiting" }
          : { schema: "conduit.tour/manifestation-receipt@3", disposition: "completed", active_play_id: "play" });return 0;
    },
    conduit_tour_cancel() { cancels++;output({ schema: "conduit.tour/manifestation-receipt@3", disposition: "cancelled", active_play_id: "play" });return 0; },
  };
  const window = { setTimeout, clearTimeout, performance, crypto };
  const document = { defaultView: window, createElement() { return { dataset: {}, setAttribute() {}, remove() { this.isConnected = false; } }; } };
  const outputRoot = { isConnected: true, ownerDocument: document, children: [], append(element) { this.children.push(element);element.isConnected = true; } };
  const inputTarget = Object.assign(new EventTarget(), {
    isConnected: true,
    nodeType: 1,
    ownerDocument: document,
    getBoundingClientRect: () => ({ width: 1, height: 1, left: 0, top: 0 }),
  });
  const resource = inputUnits > 0
    ? { pool_id: "browser/window-input", class_id: "conduit.resource/browser-window-input@1", units: inputUnits }
    : timerEffects ? { pool_id: "browser/timer", class_id: "conduit.resource/timer-slot@1", units: 1 }
      : { pool_id: "browser/presentation", class_id: "conduit.resource/presentation-slot@1", units: 1 };
  const placements = Array.from({ length: timerEffects || 1 }, (_, index) => ({ placement_id: `placement-${index}`, gear_id: `gear-${index}`, resources: [resource] }));
  const proposal = { schema: "conduit.patchbay/body-execution-proposal@1", wake: { lifecycle: "AwaitingPlan", plans: [] }, plan: { forms: [{ plan: { fragments: [{ host_id: "host", boot_id: "boot", offer_generation: 1, placements }] } }] } };
  return { api, proposal, outputRoot, inputTarget, hostId: "host", bootId: "boot", count: () => ({ starts, cancels }), request: () => request, output };
}

test("one acquired window-input adapter reports every admitted logical input unit", () => {
  const f = fixture({ inputUnits: 16 });
  const owner = acquireBrowserBodyHost(f);
  assert.equal(owner.observations()[0].unreserved_units, 16);
  owner.close();
});

test("three admitted timer slots run independently through the page Host", async () => {
  const f = fixture({ timerEffects: 3, timerDuration: 0 });
  const owner = acquireBrowserBodyHost(f);
  assert.equal(owner.observations()[0].unreserved_units, 3);
  owner.start(1);
  const receipt = await owner.run();
  assert.equal(receipt.disposition, "completed");
  owner.close();
});

test("a synchronously completed Body retains its receipt before dispatch or retirement", () => {
  const f = fixture({ immediate: true }), owner = acquireBrowserBodyHost(f);
  const started = owner.start(1);
  const closed = owner.close();
  assert.deepEqual(closed.receipt, started.progress);
  assert.equal(closed.receipt.disposition, "completed");
  assert.equal(f.count().cancels, 0);
});

test("acquisition reports owned slots without starting a play or copying offer capacity", async () => {
  const f = fixture();
  const owner = acquireBrowserBodyHost(f);
  assert.equal(f.count().starts, 0);
  assert.equal(f.outputRoot.children.length, 1);
  assert.equal(owner.observations()[0].unreserved_units, 1);
  assert.throws(() => acquireBrowserBodyHost(f), /already acquired/);
  const original = structuredClone(f.proposal);
  f.proposal.plan.forms.length = 0;
  owner.start(1);
  assert.deepEqual(f.request().plan, original.plan);
  assert.throws(() => owner.observations(), /reserved/);
  const receipt = await owner.run();
  assert.equal(receipt.disposition, "completed");
  assert.equal(f.outputRoot.children[0].textContent, "hello");
  assert.equal(f.outputRoot.children[0].dataset.activePlayId, "play");
  assert.throws(() => owner.run(), /exactly once/);
  const closed = owner.close();
  assert.equal(closed.status, null);
  assert.equal(closed.receipt, receipt);
  assert.equal(f.count().cancels, 0);
  assert.equal(f.outputRoot.children[0].isConnected, false);
});

test("wrong Boot, excessive demand, unsupported pools, and lost slots refuse", () => {
  for (const change of [
    f => { f.bootId = "wrong"; },
    f => { f.proposal.plan.forms[0].plan.fragments[0].placements[0].resources[0].units = 33; },
    f => { f.proposal.plan.forms[0].plan.fragments[0].placements[0].resources[0].pool_id = "other"; },
  ]) {
    const f = fixture();change(f);
    assert.throws(() => acquireBrowserBodyHost(f));
    assert.equal(f.count().starts, 0);
  }
  const f = fixture(), owner = acquireBrowserBodyHost(f);
  f.outputRoot.children[0].remove();
  assert.throws(() => owner.start(1), /resources lost/);
  assert.equal(f.count().starts, 0);
  owner.close();
});

test("a plan selecting another admitted host retains an exact pre-play Line refusal", () => {
  const f = fixture();
  f.proposal.plan.plan_id = "body-plan";
  f.proposal.plan.forms[0].form = { checked_form_id: "checked/form" };
  f.proposal.plan.forms[0].plan.plan_id = "partition";
  const fragment = f.proposal.plan.forms[0].plan.fragments[0];
  fragment.host_id = "host/remote";
  fragment.boot_id = "boot/remote";
  assert.throws(() => acquireBrowserBodyHost(f), error => {
    assert.equal(error.code, "ExecutionLineUnavailable");
    assert.deepEqual(error.refusal.rejections, [{
      reason_code: "execution.line-unavailable",
      category: "Connectivity",
      stage: "Browser Host acquisition",
      resource: "remote-fragment-execution-line",
      required: 1,
      available: 0,
      host_id: "host",
      boot_id: "boot",
      plan_id: "body-plan",
      checked_form_ids: ["checked/form"],
      selected_host_id: "host/remote",
      selected_boot_id: "boot/remote",
    }]);
    return true;
  });
  assert.equal(f.count().starts, 0);
});

test("Body placement admission follows the runtime envelope instead of a duplicated page constant", () => {
  const f = fixture();
  const fragment = f.proposal.plan.forms[0].plan.fragments[0];
  fragment.placements = Array.from({ length: 32 }, (_, index) => ({
    placement_id: `placement-${index}`,
    gear_id: `gear-${index}`,
    resources: [],
  }));
  const owner = acquireBrowserBodyHost(f);
  owner.close();
  fragment.placements.push({ placement_id: "placement-overflow", gear_id: "gear-overflow", resources: [] });
  assert.throws(() => acquireBrowserBodyHost(f), /placement bound exceeded/);
});

test("an exact externally managed distributed Form stays in the body Plan but outside the local scheduler", () => {
  const f = fixture();
  f.proposal.plan.forms[0].plan.plan_id = "plan/local";
  f.proposal.plan.forms.push({ plan: { plan_id: "plan/voice", fragments: [
    { host_id: "host", boot_id: "boot", offer_generation: 1, placements: [] },
    { host_id: "host/voice", boot_id: "boot/voice", offer_generation: 4, placements: [] },
  ] } });
  assert.throws(() => acquireBrowserBodyHost(f), /requires an external manager/);
  const owner = acquireBrowserBodyHost({ ...f, externallyManagedPlanIds: ["plan/voice"] });
  owner.start(1);
  assert.deepEqual(f.request().externally_managed_plan_ids, ["plan/voice"]);
  assert.equal(f.request().plan.forms.length, 2);
  owner.close();
});

test("closing pending timer work settles the dispatcher and releases the owner", async () => {
  const f = fixture({ timer: true }), owner = acquireBrowserBodyHost(f);
  owner.start(1);
  const running = owner.run();
  const closed = owner.close();
  assert.equal(closed.receipt.disposition, "cancelled");
  await assert.rejects(running, /timer cancelled/);
  assert.equal(f.count().cancels, 1);
  owner.close();
  assert.equal(f.count().cancels, 1);
  const replacement = acquireBrowserBodyHost(f);
  replacement.close();
});

test("closing a quiescent Play cancels it instead of mistaking pending effects for a terminal receipt", async () => {
  const f = fixture({ quiescent: true }), owner = acquireBrowserBodyHost(f);
  owner.start(1);
  const pending = await owner.run();
  assert.equal(pending.disposition, "quiescent_awaiting_input");
  const closed = owner.close();
  assert.equal(closed.receipt.schema, "conduit.tour/manifestation-receipt@3");
  assert.equal(closed.receipt.disposition, "cancelled");
  assert.equal(f.count().cancels, 1);
});

test("adapter failure is not converted into successful execution", async () => {
  const f = fixture({ text: null }), owner = acquireBrowserBodyHost(f);
  owner.start(1);
  await assert.rejects(owner.run(), /unsupported browser manifestation/);
  owner.close();
  assert.equal(f.count().cancels, 1);
});

test("pre-start refusal and an unknown WASM start outcome remain distinct", () => {
  for (const [start, outcome, cancels] of [
    [() => -1, "refused-before-play", 0],
    [() => { throw new Error("WASM trap"); }, "unknown", 1],
  ]) {
    const f = fixture();f.api.conduit_browser_body_start = start;
    const owner = acquireBrowserBodyHost(f);
    assert.throws(() => owner.start(1));
    const result = owner.close();
    assert.equal(result.startOutcome, outcome);
    assert.equal(f.count().cancels, cancels);
    if (outcome === "unknown") assert.equal(result.receipt.disposition, "cancelled");
    else assert.equal(result.receipt, null);
  }
  const f = fixture(), owner = acquireBrowserBodyHost(f);
  assert.throws(() => owner.start(0));
  assert.equal(owner.close().startOutcome, "not-attempted");
});

test("first access to the body input arena may replace the WASM memory buffer", () => {
  const f = fixture();
  f.api.conduit_browser_body_input_ptr = () => {
    const next = new ArrayBuffer(1024 * 1024);
    new Uint8Array(next).set(new Uint8Array(f.api.memory.buffer));
    f.api.memory.buffer = next;
    return 256 * 1024;
  };
  const owner = acquireBrowserBodyHost(f);
  owner.start(1);
  assert.equal(f.request().play_sequence, 1);
  owner.close();
});

test("malformed successful start output still retires the acquired session", () => {
  const f = fixture();
  const start = f.api.conduit_browser_body_start;
  f.api.conduit_browser_body_start = length => {
    start(length);f.output({ schema: "wrong-start-envelope" });return 0;
  };
  const owner = acquireBrowserBodyHost(f);
  assert.throws(() => owner.start(1), /invalid browser Body start output/);
  assert.throws(() => owner.observations(), /reserved/);
  assert.equal(owner.close().receipt.disposition, "cancelled");
  assert.equal(f.count().cancels, 1);
});

test('the kernel-local template slot has one preparation owner and is released on close', () => {
  const f = fixture();
  const resource = f.proposal.plan.forms[0].plan.fragments[0].placements[0].resources[0];
  Object.assign(resource, { pool_id: 'browser/named-pattern-storage', class_id: 'conduit.resource/named-pattern-storage-slot@1' });
  const owner = acquireBrowserBodyHost(f);
  assert.equal(owner.observations()[0].unreserved_units, 1);
  assert.equal(f.count().starts, 0);
  assert.throws(() => acquireBrowserBodyHost(f), /already acquired/);
  owner.close();
  assert.throws(() => owner.observations(), /resources lost/);
  const next = acquireBrowserBodyHost(f);
  next.close();
  resource.units = 2;
  assert.throws(() => acquireBrowserBodyHost(f), /exceeds local bounds/);
});
