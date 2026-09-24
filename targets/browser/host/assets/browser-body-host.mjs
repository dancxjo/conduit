import { acquireBrowserAudioCue, AUDIO_CUE_RESOURCE, AUDIO_CUE_POOL } from "./browser-audio-cue.mjs";
import { acquireBrowserPcmAudio, PCM_CAPTURE_RESOURCE, PCM_CAPTURE_POOL, PCM_PLAY_RESOURCE, PCM_PLAY_POOL } from "./browser-pcm-audio.mjs";
import { createBodyInputRouting } from "./browser-body-input.mjs";
import { openBrowserHumanInput } from "./browser-human-input.mjs";
import { createPitchTonePerformer, drainBrowserEffects } from "./browser-form-effects.mjs";
import { manifestApplicationView } from "./application-presentation.mjs";
import { bindBrowserRuntimeBridge } from "./browser-runtime-bridge.mjs";

const PRESENTATION = "conduit.resource/presentation-slot@1";
const INPUT = "conduit.resource/browser-window-input@1";
const TIMER = "conduit.resource/timer-slot@1";
const TEMPLATE = "conduit.resource/named-pattern-storage-slot@1";
const CLOCK = "conduit.resource/monotonic-millisecond-timer-slot@1";
const pools = new Map([
  [AUDIO_CUE_RESOURCE, AUDIO_CUE_POOL],
  [PCM_CAPTURE_RESOURCE, PCM_CAPTURE_POOL], [PCM_PLAY_RESOURCE, PCM_PLAY_POOL],
  [PRESENTATION, "browser/presentation"], [INPUT, "browser/window-input"],
  [TEMPLATE, "browser/named-pattern-storage"], [TIMER, "browser/timer"], [CLOCK, "browser/monotonic-millisecond-timer"],
]);

function readOutput(bridge) {
  return bridge.decodeJson(bridge.readOutputBytes({
    pointerExport: "conduit_browser_form_output_ptr",
    lengthExport: "conduit_browser_form_output_len",
    minimum: 1,
    maximum: 256 * 1024,
    label: "browser Body output",
  }));
}

function refuseUnavailableExecutionLine(proposal, fragment) {
  const error = new Error("Body proposal selected an admitted host without a current execution Line");
  error.code = "ExecutionLineUnavailable";
  error.refusal = Object.freeze({
    schema: "conduit.body/wake-refusal@1",
    rejections: Object.freeze([Object.freeze({
      reason_code: "execution.line-unavailable",
      category: "Connectivity",
      stage: "Browser Host acquisition",
      resource: "remote-fragment-execution-line",
      required: 1,
      available: 0,
      host_id: proposal.authority_host_id,
      boot_id: proposal.authority_boot_id,
      plan_id: proposal.plan.plan_id,
      checked_form_ids: proposal.plan.forms.map(({ form }) => form.checked_form_id),
      selected_host_id: fragment.host_id,
      selected_boot_id: fragment.boot_id,
    })]),
  });
  throw error;
}

/** Acquire this page host's local resources before coordinator start admission.
 * The exact proposal is still only a proposal. No WASM Play is started here.
 * One owner per WASM instance prevents duplicate page-side resource ownership.
 */
const owners = new WeakSet();
export function acquireBrowserBodyHost({ api, hostId, bootId, proposal: suppliedProposal, inputTarget, outputRoot, foregroundForm,
  presentationRootFor, onApplicationEvent, onTutorialPresenterRequest, externallyManagedPlanIds = [] }) {
  const proposal = structuredClone(suppliedProposal);
  const external = new Set(externallyManagedPlanIds);
  if (owners.has(api)) throw new Error("browser Body resources already acquired");
  if ([hostId, bootId].some(identity => typeof identity !== "string" || identity.length < 1 || identity.length > 256) ||
      proposal?.schema !== "conduit.patchbay/body-execution-proposal@1" ||
      proposal.wake?.lifecycle !== "AwaitingPlan" || proposal.wake.plans.length !== 0 ||
      !Array.isArray(proposal.plan?.forms) || proposal.plan.forms.length < 1 || proposal.plan.forms.length > 16 ||
      !Array.isArray(externallyManagedPlanIds) || external.size !== externallyManagedPlanIds.length ||
      externallyManagedPlanIds.some(identity => typeof identity !== "string" || !identity) ||
      !outputRoot?.isConnected || !inputTarget?.isConnected) {
    throw new Error("invalid browser Body acquisition inputs");
  }
  const bridge = bindBrowserRuntimeBridge(api, {
    context: "browser Body host runtime",
    requiredExports: [
      "conduit_browser_form_output_ptr", "conduit_browser_form_output_len",
      "conduit_browser_form_input_ptr", "conduit_browser_form_input_capacity",
      "conduit_browser_body_input_ptr", "conduit_browser_body_input_capacity", "conduit_browser_body_start",
    ],
  });
  if (api.conduit_browser_form_human_machinery() < 0) throw new Error("browser machinery unavailable");
  const machinery = readOutput(bridge);
  const maximumPlacements = machinery?.limits?.maximum_gears;
  if (machinery.schema !== "conduit.browser/selected-human-machinery@1" || !Array.isArray(machinery.implementations) || machinery.implementations.length > 64 ||
      !Number.isSafeInteger(maximumPlacements) || maximumPlacements < 1) throw new Error("invalid browser machinery");
  const placements = [];
  const placementForms = new Map();
  const matchedExternal = new Set();
  for (const form of proposal.plan.forms) {
    if (external.has(form.plan.plan_id)) {
      const local = form.plan.fragments.filter(fragment => fragment.host_id === hostId && fragment.boot_id === bootId);
      if (form.plan.fragments.length < 2 || local.length !== 1) {
        throw new Error("external Body Form does not name one exact browser fragment");
      }
      matchedExternal.add(form.plan.plan_id);
      continue;
    }
    if (form.plan.fragments.length !== 1) throw new Error("distributed Body Form requires an external manager");
    const fragment = form.plan.fragments[0];
    if (fragment.host_id !== hostId || fragment.boot_id !== bootId || fragment.offer_generation !== 1) {
      refuseUnavailableExecutionLine({ ...proposal, authority_host_id: hostId, authority_boot_id: bootId }, fragment);
    }
    for (const placement of fragment.placements) {
      if (placementForms.has(placement.placement_id)) throw new Error("duplicate browser Body placement identity");
      placementForms.set(placement.placement_id, form.form?.checked_form_id ?? form.plan.checked_form_id);
      placements.push(placement);
    }
    if (placements.length > maximumPlacements) throw new Error("browser Body placement bound exceeded");
  }
  if (matchedExternal.size !== external.size) throw new Error("external Body Form is absent from the proposal");
  if (!placements.length) throw new Error("browser Body requires at least one locally managed Form");
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
    const capacity = [PRESENTATION, INPUT, AUDIO_CUE_RESOURCE].includes(kind) ? 16
      : kind === TIMER ? maximumPlacements : 1;
    if (units > capacity) throw new Error("browser Body resource demand exceeds local bounds");
  }
  const boot = { host_id: hostId, boot_id: bootId, offer_generation: 1, implementation_registry: machinery.implementations };
  const routing = foregroundForm ? createBodyInputRouting({ forms: proposal.plan.forms, foreground: foregroundForm, maximumPlacements }) : null;
  const slots = new Map();
  const applicationChannels = new Map();
  const applicationChannel = checkedFormId => {
    let channel = applicationChannels.get(checkedFormId);
    if (!channel) {
      channel = { queue: [], bytes: 0, waiter: null, manifestation: null };
      applicationChannels.set(checkedFormId, channel);
    }
    return channel;
  };
  const publishApplicationEvent = (checkedFormId, event) => {
    const channel = applicationChannel(checkedFormId);
    channel.manifestation?.nextEvent();
    if (channel.waiter) {
      const waiter = channel.waiter;
      channel.waiter = null;
      waiter.resolve(event.encoded);
    } else if (channel.queue.length >= 8 || channel.bytes + event.encoded.length > 131072) {
      throw new Error("browser application event queue pressure");
    } else {
      channel.queue.push(event.encoded);
      channel.bytes += event.encoded.length;
    }
    onApplicationEvent?.(Object.freeze({ checkedFormId, event }));
  };
  const nextApplicationEvent = (checkedFormId, signal) => {
    const channel = applicationChannel(checkedFormId);
    const queued = channel.queue.shift();
    if (queued) {
      channel.bytes -= queued.length;
      return Promise.resolve(queued);
    }
    if (channel.waiter) throw new Error("browser application event request already pending");
    return new Promise((resolve, reject) => {
      const abort = () => {
        if (channel.waiter?.abort === abort) channel.waiter = null;
        reject(new Error("browser application event cancelled"));
      };
      channel.waiter = { resolve: value => { signal.removeEventListener("abort", abort); resolve(value); }, abort };
      signal.addEventListener("abort", abort, { once: true });
      if (signal.aborted) abort();
    });
  };
  // The single WASM owner leases this local preparation slot. Its bounded
  // template storage is initialized by ordinary kernel preparation before Play.
  const templateSlots = new Set();
  const presentationTimers = new Map();
  const elements = [];
  let audio = null;
  let pcmAudio = null, pushToTalk = null;
  let input = null, timerSlots = [], clock = false, closed = false, started = null, completion = null, startAccepted = false, terminal = null;
  let startOutcome = "not-attempted";
  const window = outputRoot.ownerDocument.defaultView;
  const tone = createPitchTonePerformer(window);
  owners.add(api);
  try {
    if (demand.has(AUDIO_CUE_RESOURCE)) {
      if (!machinery.implementations.some(entry => entry.id === "browser/audio-cue@1")) throw new Error("audio cue machinery is not installed");
      audio = acquireBrowserAudioCue({ api, window, placements: placements.filter(item => item.resources.some(resource => resource.class_id === AUDIO_CUE_RESOURCE)) });
      if (audio.capacity !== demand.get(AUDIO_CUE_RESOURCE)) throw new Error("audio acquisition differs from demand");
    }
    if (demand.has(PCM_CAPTURE_RESOURCE) || demand.has(PCM_PLAY_RESOURCE)) {
      pushToTalk = outputRoot.ownerDocument.createElement("button");
      pushToTalk.type = "button";
      pushToTalk.textContent = "Hold to talk";
      pushToTalk.hidden = !demand.has(PCM_CAPTURE_RESOURCE);
      pushToTalk.dataset.conduitPushToTalk = "";
      outputRoot.append(pushToTalk);
      elements.push(pushToTalk);
      pcmAudio = acquireBrowserPcmAudio({ window, pushToTalkTarget: pushToTalk });
    }
    for (const placement of placements) {
      if (placement.resources.some(resource => resource.class_id === TEMPLATE)) templateSlots.add(placement.placement_id);
    }
    if (templateSlots.size !== (demand.get(TEMPLATE) ?? 0)) throw new Error("template storage acquisition differs from demand");
    if (demand.has(INPUT)) {
      input = openBrowserHumanInput({ target: inputTarget, boot, routeInput: routing?.capture });
      routing?.attach(input);
    }
    if (demand.has(TIMER) || demand.has(CLOCK)) {
      if (typeof window.setTimeout !== "function" || typeof window.performance?.now !== "function" ||
          !Number.isFinite(window.performance.now())) throw new Error("browser timer unavailable");
      timerSlots = Array.from({ length: demand.get(TIMER) ?? 0 }, () => ({ pending: null, cancel: null }));
      clock = demand.has(CLOCK);
    }
    for (const placement of placements.filter(item => item.resources.some(resource => resource.class_id === PRESENTATION))) {
      if (!machinery.implementations.some(entry => entry.id === "browser/dom-presentation@1")) throw new Error("browser presentation not installed");
      const output = outputRoot.ownerDocument.createElement("output");
      output.dataset.placementId = placement.placement_id;
      output.setAttribute("aria-label", placement.gear_id);
      const selectedRoot = presentationRootFor?.(Object.freeze({
        checkedFormId: placementForms.get(placement.placement_id),
        placementId: placement.placement_id,
        gearId: placement.gear_id,
      })) ?? outputRoot;
      if (!selectedRoot?.isConnected || selectedRoot.ownerDocument !== outputRoot.ownerDocument) {
        throw new Error("browser presentation root is unavailable");
      }
      selectedRoot.append(output);
      elements.push(output);
      slots.set(placement.placement_id, output);
      presentationTimers.set(placement.placement_id, { pending: null, cancel: null });
    }
    if (slots.size !== (demand.get(PRESENTATION) ?? 0)) throw new Error("presentation acquisition does not match demand");
  } catch (error) {
    audio?.close();pcmAudio?.close();routing?.close();input?.close();elements.forEach(element => element.remove());owners.delete(api);throw error;
  }

  const assertCurrent = () => {
    if (closed || !outputRoot.isConnected || !inputTarget.isConnected ||
        elements.some(element => !element.isConnected)) throw new Error("browser Body resources lost");
  };
  const observations = () => {
    assertCurrent();
    if (startAccepted) throw new Error("browser Body resources are reserved by its play");
    return [...demand.keys()].map(class_id => ({
      host_id: hostId, boot_id: bootId, offer_generation: 1,
      pool_id: pools.get(class_id), class_id, health: "Ready",
      // Counts come from acquired adapter state, not advertised capacities.
      unreserved_units: class_id === AUDIO_CUE_RESOURCE ? audio.capacity : class_id === PCM_CAPTURE_RESOURCE ? pcmAudio.capacity.capture : class_id === PCM_PLAY_RESOURCE ? pcmAudio.capacity.playback : class_id === PRESENTATION ? slots.size : class_id === INPUT ? demand.get(INPUT) : class_id === TEMPLATE ? templateSlots.size : class_id === TIMER ? timerSlots.length : Number(clock),
      utilized_units: 0, sign_id: `browser-resource/${bootId}/${window.crypto.randomUUID()}`,
    }));
  };
  const delay = (duration, signal, timerOwner) => new Promise((resolve, reject) => {
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
    if (effect.effect_kind === "audio-cue") {
      if (!audio) throw new Error("audio cue slot not acquired");
      return audio.perform(effect, signal);
    }
    if (effect.effect_kind === "audio-capture" || effect.effect_kind === "pcm-playback") {
      if (!pcmAudio) throw new Error("browser PCM audio slot not acquired");
      return pcmAudio.perform(effect, signal);
    }
    if (effect.effect_kind === "pitch-tone") return tone(effect, signal);
    if (effect.effect_kind === "timer") {
      const availableTimer = timerSlots.find(slot => slot.pending === null);
      if (!availableTimer) throw new Error("browser timer slot was not acquired");
      return delay(effect.duration_millis, signal, availableTimer);
    }
    if (effect.effect_kind === "clock-observation") {
      if (!clock) throw new Error("browser clock not acquired");
      const bytes = new Uint8Array(8);
      new DataView(bytes.buffer).setBigUint64(0, BigInt(Math.floor(window.performance.now() * 1000)), true);
      return bytes;
    }
    if (effect.effect_kind === "pointer-event") {
      if (!input || !routing) throw new Error("routed pointer input not acquired");
      const event = await routing.next("pointer", effect.placement_id, signal);
      assertCurrent();
      const status = api.conduit_browser_form_encode_pointer(event.position_x, event.position_y,
        event.delta_x, event.delta_y, event.primary_pressed ? 1 : 0, event.coalesced,
        event.dropped, event.queue_capacity, event.sequence);
      if (status < 0) throw new Error("pointer encoding refused");
      return bridge.readOutputBytes({
        pointerExport: "conduit_browser_form_output_ptr",
        lengthExport: "conduit_browser_form_output_len",
        minimum: 1,
        maximum: 256 * 1024,
        label: "pointer encoding output",
      });
    }
    if (effect.effect_kind === "key-event" || effect.effect_kind === "button-transition") {
      if (!input) throw new Error("browser input not acquired");
      const abort = () => { if (!routing) input.cancelPending(); };
      signal.addEventListener("abort", abort, { once: true });
      try {
        if (signal.aborted) throw new Error("browser input cancelled");
        if (effect.effect_kind === "key-event") return (await (routing ? routing.next("keyboard", effect.placement_id, signal) : input.nextKeyboard())).canonical_bytes;
        const event = await (routing ? routing.next("button", effect.placement_id, signal) : input.nextButton());
        assertCurrent();
        if (api.conduit_tour_encode_button_transition(event.pressed ? 1 : 0, BigInt(event.sequence)) < 0) throw new Error("button encoding refused");
        return bridge.readOutputBytes({
          pointerExport: "conduit_browser_form_output_ptr",
          lengthExport: "conduit_browser_form_output_len",
          minimum: 1,
          maximum: 256 * 1024,
          label: "button encoding output",
        });
      } finally { signal.removeEventListener("abort", abort); }
    }
    if (effect.effect_kind === "application-event") {
      return nextApplicationEvent(effect.checked_form_id, signal);
    }
    if (effect.effect_kind === "tutorial-presenter-request") {
      if (typeof onTutorialPresenterRequest !== "function") throw new Error("tutorial Presenter request owner is unavailable");
      return onTutorialPresenterRequest(effect);
    }
    if (effect.effect_kind === "manifestation") {
      const output = slots.get(effect.placement_id);
      if (!output) throw new Error("browser presentation slot not acquired");
      output.dataset.planId = effect.plan_id;
      output.dataset.activePlayId = effect.active_play_id;
      output.dataset.presentationKind = effect.presentation_kind;
      if (effect.presentation_kind === "presentation/application-view" && Array.isArray(effect.application_view)) {
        const channel = applicationChannel(effect.checked_form_id);
        channel.manifestation = manifestApplicationView(Uint8Array.from(effect.application_view), output, {
          eventCapacity: 8,
          eventByteCapacity: 131072,
          choiceScope: effect.active_play_id,
          onEvent: event => publishApplicationEvent(effect.checked_form_id, event),
        });
      } else if (typeof effect.text === "string") output.textContent = effect.text;
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
    evidence() {
      if (terminal) return terminal.kernel_signs ?? null;
      if (!started || closed) return null;
      if (api.conduit_browser_form_signs() < 0) throw new Error("active body observation is unavailable");
      return readOutput(bridge);
    },
    start(playSequence) {
      assertCurrent();
      if (startAccepted || !Number.isSafeInteger(playSequence) || playSequence < 1) throw new Error("browser Body start refused");
      const request = bridge.encodeJson({
        wake: proposal.wake,
        plan: proposal.plan,
        local_host_id: hostId,
        local_boot_id: bootId,
        externally_managed_plan_ids: [...external],
        body_evidence: proposal.body_evidence ?? null,
        source: proposal.source ?? "",
        foreground_checked_form_id: foregroundForm?.() ?? proposal.plan.forms[0].form?.checked_form_id ?? proposal.plan.forms[0].plan.checked_form_id,
        play_sequence: playSequence,
        observations: observations(),
      });
      startOutcome = "unknown";
      const { status: startStatus } = bridge.start({
        inputBytes: request,
        input: {
          pointerExport: "conduit_browser_body_input_ptr",
          capacityExport: "conduit_browser_body_input_capacity",
          minimum: 1,
          label: "browser Body input",
        },
        invoke: (length) => api.conduit_browser_body_start(length),
        retireInput: true,
        label: "browser Body",
      });
      if (startStatus < 0) {
        startOutcome = "refused-before-play";
        const refusal = api.conduit_browser_form_output_len() > 0 ? readOutput(bridge) : null;
        const rejection = refusal?.rejections?.[0];
        const detail = rejection
          ? `${rejection.stage} could not fit ${rejection.resource}: required ${rejection.required}, available ${rejection.available} on Host ${rejection.host_id} / Boot ${rejection.boot_id}`
          : refusal?.message ?? "no refusal detail";
        const error = new Error(`browser Body admission refused (${startStatus}): ${detail}`);
        error.status = startStatus;error.refusal = refusal;
        throw error;
      }
      startOutcome = "accepted";
      startAccepted = true;
      started = readOutput(bridge);
      if (started.schema !== "conduit.browser/body-started@1" ||
          typeof started.play?.active_play_id !== "string" || !started.play.active_play_id ||
          started.play.plan_id !== proposal.plan.plan_id || started.play.wake_id !== proposal.wake.wake_id ||
          started.play.body_id !== proposal.plan.body_id) throw new Error("invalid browser Body start output");
      if (started.progress?.schema === "conduit.tour/manifestation-receipt@3") terminal = started.progress;
      return started;
    },
    run() {
      assertCurrent();
      if (!started || completion) throw new Error("browser Body must be started exactly once before dispatch");
      completion = drainBrowserEffects({ api, initialProgress: started.progress, readOutput: (_runtime) => readOutput(bridge), perform, bridge })
        .then(receipt => {
          if (receipt?.schema === "conduit.tour/manifestation-receipt@3") terminal = receipt;
          return receipt;
        });
      return completion;
    },
    close() {
      if (closed) return;
      closed = true;
      audio?.close();
      pcmAudio?.close();
      routing?.close();
      input?.close();
      for (const timer of timerSlots) timer.cancel?.();
      for (const timer of presentationTimers.values()) timer.cancel?.();
      const status = (startAccepted || startOutcome === "unknown") && !terminal ? api.conduit_tour_cancel() : null;
      try {
        const receipt = status !== null && status >= 0 ? readOutput(bridge) : terminal;
        return { status, receipt, startOutcome };
      } finally { elements.forEach(element => element.remove());owners.delete(api); }
    },
  });
}
