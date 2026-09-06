import { openBrowserHumanInput } from "./browser-human-input.mjs";
import { drainBrowserEffects } from "./browser-form-effects.mjs";

const PRESENTATION = "conduit.resource/presentation-slot@1";
const INPUT = "conduit.resource/browser-window-input@1";
const TIMER = "conduit.resource/timer-slot@1";
const CLOCK = "conduit.resource/monotonic-millisecond-timer-slot@1";
const pools = new Map([
  [PRESENTATION, "browser/presentation"], [INPUT, "browser/window-input"],
  [TIMER, "browser/timer"], [CLOCK, "browser/monotonic-millisecond-timer"],
]);

function readOutput(api) {
  const length = api.conduit_browser_form_output_len();
  if (!Number.isSafeInteger(length) || length < 1 || length > 256 * 1024) {
    throw new Error("invalid browser Body output bound");
  }
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(
    new Uint8Array(api.memory.buffer, api.conduit_browser_form_output_ptr(), length),
  ));
}

/** Acquire this page Host's local resources before coordinator start admission.
 * The exact proposal is still only a proposal. No WASM Play is started here.
 * One owner per WASM instance prevents duplicate page-side resource ownership.
 */
const owners = new WeakSet();
export function acquireBrowserBodyHost({ api, hostId, bootId, proposal: suppliedProposal, inputTarget, outputRoot }) {
  const proposal = structuredClone(suppliedProposal);
  if (owners.has(api)) throw new Error("browser Body resources already acquired");
  if ([hostId, bootId].some(identity => typeof identity !== "string" || identity.length < 1 || identity.length > 256) ||
      proposal?.schema !== "conduit.patchbay/body-execution-proposal@1" ||
      proposal.wake?.lifecycle !== "AwaitingPlan" || proposal.wake.plans.length !== 0 ||
      !Array.isArray(proposal.plan?.forms) || proposal.plan.forms.length < 1 || proposal.plan.forms.length > 16 ||
      !outputRoot?.isConnected || !inputTarget?.isConnected) {
    throw new Error("invalid browser Body acquisition inputs");
  }
  const placements = [];
  for (const form of proposal.plan.forms) {
    if (form.plan.fragments.length !== 1) throw new Error("browser Body requires local partitions");
    const fragment = form.plan.fragments[0];
    if (fragment.host_id !== hostId || fragment.boot_id !== bootId || fragment.offer_generation !== 1) {
      throw new Error("Body proposal does not name this browser Host and Boot");
    }
    placements.push(...fragment.placements);
    if (placements.length > 16) throw new Error("browser Body placement bound exceeded");
  }
  const demand = new Map();
  for (const placement of placements) {
    if (!Array.isArray(placement.resources) || placement.resources.length > 64) throw new Error("browser resource binding bound exceeded");
    for (const resource of placement.resources) {
      if (pools.get(resource.class_id) !== resource.pool_id ||
          !Number.isSafeInteger(resource.units) || resource.units < 1) {
        throw new Error("unsupported browser Body resource binding");
      }
      demand.set(resource.class_id, (demand.get(resource.class_id) ?? 0) + resource.units);
    }
  }
  for (const [kind, units] of demand) {
    if (units > (kind === PRESENTATION ? 16 : 1)) throw new Error("browser Body resource demand exceeds local bounds");
  }
  if (api.conduit_browser_form_human_machinery() < 0) throw new Error("browser machinery unavailable");
  const machinery = readOutput(api);
  if (machinery.schema !== "conduit.browser/selected-human-machinery@1") throw new Error("invalid browser machinery");
  const boot = { host_id: hostId, boot_id: bootId, offer_generation: 1, implementation_registry: machinery.implementations };
  const slots = new Map();
  const presentationTimers = new Map();
  const elements = [];
  let input = null, timer = null, closed = false, started = null, completion = null, startAccepted = false, terminal = null;
  const window = outputRoot.ownerDocument.defaultView;
  owners.add(api);
  try {
    if (demand.has(INPUT)) input = openBrowserHumanInput({ target: inputTarget, boot });
    if (demand.has(TIMER) || demand.has(CLOCK)) {
      if (typeof window.setTimeout !== "function" || typeof window.performance?.now !== "function" ||
          !Number.isFinite(window.performance.now())) throw new Error("browser timer unavailable");
      timer = { pending: null, cancel: null };
    }
    for (const placement of placements.filter(item => item.resources.some(resource => resource.class_id === PRESENTATION))) {
      if (!machinery.implementations.includes("browser/dom-presentation@1")) throw new Error("browser presentation not installed");
      const output = outputRoot.ownerDocument.createElement("output");
      output.dataset.placementId = placement.placement_id;
      output.setAttribute("aria-label", placement.gear_id);
      outputRoot.append(output);
      elements.push(output);
      slots.set(placement.placement_id, output);
      presentationTimers.set(placement.placement_id, { pending: null, cancel: null });
    }
    if (slots.size !== (demand.get(PRESENTATION) ?? 0)) throw new Error("presentation acquisition does not match demand");
  } catch (error) {
    input?.close();elements.forEach(element => element.remove());owners.delete(api);throw error;
  }

  const assertCurrent = () => {
    if (closed || !outputRoot.isConnected || !inputTarget.isConnected ||
        elements.some(element => !element.isConnected)) throw new Error("browser Body resources lost");
  };
  const observations = () => {
    assertCurrent();
    if (startAccepted) throw new Error("browser Body resources are reserved by its Play");
    return [...demand.keys()].map(class_id => ({
      host_id: hostId, boot_id: bootId, offer_generation: 1,
      pool_id: pools.get(class_id), class_id, health: "Ready",
      // Counts come from acquired adapter state, not advertised capacities.
      unreserved_units: class_id === PRESENTATION ? slots.size : class_id === INPUT ? Number(input !== null) : Number(timer !== null),
      utilized_units: 0, sign_id: `browser-resource/${bootId}/${window.crypto.randomUUID()}`,
    }));
  };
  const delay = (duration, signal, timerOwner = timer) => new Promise((resolve, reject) => {
    const timer = timerOwner;
    if (!timer || timer.pending !== null || !Number.isSafeInteger(duration) || duration < 0 || duration > 60_000) {
      reject(new Error("browser timer request exceeds acquisition"));return;
    }
    const finish = error => {
      window.clearTimeout(timer.pending);timer.pending = null;timer.cancel = null;
      signal.removeEventListener("abort", abort);
      error ? reject(error) : resolve();
    };
    const abort = () => finish(new Error("browser timer cancelled"));
    timer.cancel = abort;
    timer.pending = window.setTimeout(() => finish(), duration);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
  });
  const perform = async (effect, signal) => {
    assertCurrent();
    if (effect.host_id !== hostId || effect.boot_id !== bootId ||
        effect.active_play_id !== started.play.active_play_id) throw new Error("browser effect identity mismatch");
    if (effect.effect_kind === "timer") return delay(effect.duration_millis, signal);
    if (effect.effect_kind === "clock-observation") {
      if (!timer) throw new Error("browser clock not acquired");
      const bytes = new Uint8Array(8);
      new DataView(bytes.buffer).setBigUint64(0, BigInt(Math.floor(window.performance.now() * 1000)), true);
      return bytes;
    }
    if (effect.effect_kind === "key-event" || effect.effect_kind === "button-transition") {
      if (!input) throw new Error("browser input not acquired");
      const abort = () => input.cancelPending();
      signal.addEventListener("abort", abort, { once: true });
      try {
        if (signal.aborted) throw new Error("browser input cancelled");
        if (effect.effect_kind === "key-event") return (await input.nextKeyboard()).canonical_bytes;
        const event = await input.nextButton();
        assertCurrent();
        if (api.conduit_tour_encode_button_transition(event.pressed ? 1 : 0, BigInt(event.sequence)) < 0) throw new Error("button encoding refused");
        return new Uint8Array(api.memory.buffer, api.conduit_browser_form_output_ptr(), api.conduit_browser_form_output_len()).slice();
      } finally { signal.removeEventListener("abort", abort); }
    }
    if (effect.effect_kind === "manifestation") {
      const output = slots.get(effect.placement_id);
      if (!output) throw new Error("browser presentation slot not acquired");
      output.dataset.planId = effect.plan_id;
      output.dataset.activePlayId = effect.active_play_id;
      output.dataset.presentationKind = effect.presentation_kind;
      if (typeof effect.text === "string") output.textContent = effect.text;
      else if (effect.presentation_kind === "presentation/indicator" && Array.isArray(effect.segments) && effect.segments.length <= 256) {
        for (const segment of effect.segments) {
          assertCurrent();
          output.textContent = segment.level ? "on" : "off";
          await delay(segment.units * effect.unit_millis, signal, presentationTimers.get(effect.placement_id));
        }
        output.textContent = "off";
      } else throw new Error("unsupported browser manifestation");
      return;
    }
    throw new Error(`unsupported browser Body effect ${effect.effect_kind}`);
  };
  return Object.freeze({
    observations,
    start(playSequence) {
      assertCurrent();
      if (startAccepted || !Number.isSafeInteger(playSequence) || playSequence < 1) throw new Error("browser Body start refused");
      const request = new TextEncoder().encode(JSON.stringify({ wake: proposal.wake, plan: proposal.plan, play_sequence: playSequence, observations: observations() }));
      if (request.length > api.conduit_browser_body_input_capacity()) throw new Error("browser Body input bound exceeded");
      new Uint8Array(api.memory.buffer, api.conduit_browser_body_input_ptr(), request.length).set(request);
      if (api.conduit_browser_body_start(request.length) < 0) throw new Error("browser Body admission refused");
      startAccepted = true;
      started = readOutput(api);
      if (started.schema !== "conduit.browser/body-started@1" ||
          typeof started.play?.active_play_id !== "string" || !started.play.active_play_id ||
          started.play.plan_id !== proposal.plan.plan_id || started.play.wake_id !== proposal.wake.wake_id ||
          started.play.body_id !== proposal.plan.body_id) throw new Error("invalid browser Body start output");
      return started;
    },
    run() {
      assertCurrent();
      if (!started || completion) throw new Error("browser Body must be started exactly once before dispatch");
      completion = drainBrowserEffects({ api, initialProgress: started.progress, readOutput, perform })
        .then(receipt => { terminal = receipt;return receipt; });
      return completion;
    },
    close() {
      if (closed) return;
      closed = true;
      input?.close();
      timer?.cancel?.();
      for (const timer of presentationTimers.values()) timer.cancel?.();
      const status = startAccepted && !terminal ? api.conduit_tour_cancel() : null;
      try {
        const receipt = status !== null && status >= 0 ? readOutput(api) : terminal;
        return { status, receipt };
      } finally { elements.forEach(element => element.remove());owners.delete(api); }
    },
  });
}
