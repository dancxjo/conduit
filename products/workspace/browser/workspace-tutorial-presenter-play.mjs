import { openBrowserRemoteFragment, runBrowserRemoteFragment } from "../../../targets/browser/host/assets/browser-remote-fragment.mjs";

const REQUEST_IMPLEMENTATION = "browser/workspace-tutorial-presenter-request@1";
const MANIFESTATION_IMPLEMENTATION = "browser/workspace-tutorial-manifestation@1";
const PRESENTATION_RESOURCE = "conduit.resource/presentation-slot@1";
const PRESENTATION_POOL = "browser/presentation";
const GENERATED_KIND = "conduit.presentation/generated-manifestation@2";
const decoder = new TextDecoder("utf-8", { fatal: true });

function acquireTutorialPresenter({ window, outputRoot, host, fragment, request }) {
  const requestPlacements = fragment.placements.filter(item => item.implementation_id === REQUEST_IMPLEMENTATION);
  const manifestationPlacements = fragment.placements.filter(item => item.implementation_id === MANIFESTATION_IMPLEMENTATION);
  const slot = manifestationPlacements[0]?.resources?.filter(resource =>
    resource.class_id === PRESENTATION_RESOURCE && resource.pool_id === PRESENTATION_POOL && resource.units === 1) ?? [];
  if (requestPlacements.length !== 1 || manifestationPlacements.length !== 1 || slot.length !== 1 ||
      !outputRoot?.isConnected || fragment.host_id !== host.host_id || fragment.boot_id !== host.boot_id ||
      fragment.offer_generation !== host.offer_generation) {
    throw new Error("tutorial Presenter browser fragment does not own its exact source and presentation slot");
  }
  const output = outputRoot.ownerDocument.createElement("output");
  output.dataset.placementId = manifestationPlacements[0].placement_id;
  output.dataset.tutorialPresenter = "";
  output.setAttribute("aria-label", "Generated tutorial voice");
  outputRoot.append(output);
  let closed = false, activePlayId = null, pendingRequest = null;
  const current = () => {
    if (closed || !output.isConnected || !outputRoot.isConnected) {
      throw new Error("tutorial Presenter browser resources were lost");
    }
  };
  return Object.freeze({
    bind(play) { current(); if (activePlayId) throw new Error("tutorial Presenter already bound"); activePlayId = play; },
    observations() {
      current();
      return [{
        host_id: host.host_id, boot_id: host.boot_id, offer_generation: host.offer_generation,
        pool_id: PRESENTATION_POOL, class_id: PRESENTATION_RESOURCE, health: "Ready",
        unreserved_units: 1, utilized_units: 0,
        sign_id: `browser-resource/${host.boot_id}/${window.crypto.randomUUID()}`,
      }];
    },
    async perform(effect, signal) {
      current();
      if (signal.aborted || effect.active_play_id !== activePlayId || effect.host_id !== host.host_id ||
          effect.boot_id !== host.boot_id) throw new Error("tutorial Presenter effect identity mismatch");
      if (effect.effect_kind === "tutorial-presenter-request") {
        const identity = `tutorial-presenter/${activePlayId}/${effect.request_sequence}`;
        const encoded = await request(identity, effect.request_sequence);
        const value = JSON.parse(decoder.decode(encoded));
        if (value?.request_identity !== identity || !value.semantic_data?.source_presentation_identity ||
            !Number.isSafeInteger(value.semantic_data.source_presentation_revision) ||
            !Number.isSafeInteger(value.bounds?.maximum_output_bytes)) {
          throw new Error("current tutorial Presenter request is malformed");
        }
        pendingRequest = value;
        return encoded;
      }
      if (effect.effect_kind !== "manifestation" || effect.presentation_kind !== GENERATED_KIND ||
          !Array.isArray(effect.canonical_value)) throw new Error("unsupported tutorial Presenter effect");
      const manifestation = JSON.parse(decoder.decode(Uint8Array.from(effect.canonical_value)));
      if (!pendingRequest || manifestation?.request_identity !== pendingRequest.request_identity ||
          manifestation.source_presentation_identity !== pendingRequest.semantic_data.source_presentation_identity ||
          manifestation.source_presentation_revision !== pendingRequest.semantic_data.source_presentation_revision ||
          manifestation.template_contract_revision !== pendingRequest.policy.template_contract_revision ||
          !Array.isArray(manifestation.content) || manifestation.content.length > 8 ||
          !Array.isArray(manifestation.affordances) || manifestation.affordances.length > 32) {
        throw new Error("generated tutorial manifestation is stale or malformed");
      }
      const outputBytes = manifestation.content.reduce((sum, segment) => sum + (segment.bytes?.length ?? 0), 0);
      if (outputBytes > pendingRequest.bounds.maximum_output_bytes || manifestation.affordances.some(item =>
        item.source_presentation_revision !== pendingRequest.semantic_data.source_presentation_revision)) {
        throw new Error("generated tutorial manifestation exceeds its request");
      }
      const content = manifestation.content.map(segment => decoder.decode(Uint8Array.from(segment.bytes ?? [])));
      output.dataset.planId = effect.plan_id;
      output.dataset.activePlayId = activePlayId;
      output.dataset.presentationKind = effect.presentation_kind;
      output.textContent = content.length ? content.join("\n") : `Presenter ${manifestation.disposition}`;
      pendingRequest = null;
      return undefined;
    },
    close() { if (closed) return; closed = true; output.remove(); },
  });
}

/** Prepare the exact browser → joined Presenter Host → browser Form. */
export async function prepareWorkspaceTutorialPresenterPlay({
  api, localAdvertisement, joined, plan, outputRoot, request,
  window = outputRoot?.ownerDocument?.defaultView,
  acquire = acquireTutorialPresenter,
  openRemote = openBrowserRemoteFragment,
  driveRemote = runBrowserRemoteFragment,
}) {
  const fragments = Array.isArray(plan?.fragments) ? plan.fragments : [];
  const local = fragments.filter(fragment => fragment.host_id === localAdvertisement?.host_id &&
    fragment.boot_id === localAdvertisement?.boot_id);
  const peer = fragments.filter(fragment => fragment.host_id === joined?.host_id && fragment.boot_id === joined?.boot_id);
  if (fragments.length !== 2 || local.length !== 1 || peer.length !== 1 ||
      local[0].offer_generation !== localAdvertisement?.offer_generation ||
      peer[0].offer_generation !== joined?.advertisement?.offer_generation ||
      joined?.line?.schema !== "conduit.creche/joined-host-line@1" || typeof request !== "function") {
    throw new Error("tutorial Presenter does not name one exact browser and Presenter Host");
  }
  const presenter = acquire({ window, outputRoot, host: localAdvertisement, fragment: local[0], request });
  let prepared = false, remote = null;
  try {
    const preparation = await joined.line.prepareRemote(plan);
    prepared = true;
    remote = openRemote({ api, plan, host: localAdvertisement, preparation,
      observations: presenter.observations() });
    presenter.bind(remote.identity.active_play_id);
  } catch (error) {
    try { remote?.close(); } catch {}
    presenter.close();
    if (prepared) await joined.line.releaseRemote().catch(() => {});
    throw error;
  }
  const controller = new AbortController();
  let running = null, settled = false, closed = false;
  return Object.freeze({
    identity: remote.identity,
    run() {
      if (closed || running) throw new Error("Workspace tutorial Presenter Play may run exactly once");
      running = driveRemote({ remote, line: joined.line,
        perform: (effect, signal) => presenter.perform(effect, signal), signal: controller.signal })
        .finally(() => { settled = true; });
      return running;
    },
    async close() {
      if (closed) return;
      closed = true;
      const cancelledForClose = Boolean(running && !settled);
      let runError = null;
      if (running) {
        if (cancelledForClose) controller.abort();
        try { await running; } catch (error) { runError = error; }
      }
      try { remote.close(); }
      finally { presenter.close(); await joined.line.releaseRemote(); }
      if (runError && !cancelledForClose) throw runError;
    },
  });
}
