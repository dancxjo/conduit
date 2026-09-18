import {
  acquireBrowserPcmAudio,
  PCM_CAPTURE_POOL,
  PCM_CAPTURE_RESOURCE,
  PCM_PLAY_POOL,
  PCM_PLAY_RESOURCE,
} from "./browser-pcm-audio.mjs";

const pools = new Map([
  [PCM_CAPTURE_RESOURCE, PCM_CAPTURE_POOL],
  [PCM_PLAY_RESOURCE, PCM_PLAY_POOL],
]);

/** Acquire exactly the browser microphone and safe playback owned by one
 * planned distributed spoken conversation fragment. */
export function acquireBrowserRemoteVoice({
  window,
  outputRoot,
  host,
  fragment,
  acquirePcm = acquireBrowserPcmAudio,
}) {
  if (!window?.crypto || !outputRoot?.isConnected ||
      fragment?.host_id !== host?.host_id || fragment?.boot_id !== host?.boot_id ||
      fragment.offer_generation !== host.offer_generation ||
      !Array.isArray(fragment.placements) || fragment.placements.length < 1) {
    throw new Error("invalid browser remote voice acquisition");
  }
  const placements = new Map();
  const demand = new Map();
  for (const placement of fragment.placements) {
    if (typeof placement?.placement_id !== "string" || !placement.placement_id ||
        !Array.isArray(placement.resources)) {
      throw new Error("invalid browser remote voice placement");
    }
    const classes = new Set();
    for (const resource of placement.resources) {
      if (pools.get(resource.class_id) !== resource.pool_id || resource.units !== 1 ||
          classes.has(resource.class_id)) {
        throw new Error("unsupported browser remote voice resource binding");
      }
      classes.add(resource.class_id);
      demand.set(resource.class_id, (demand.get(resource.class_id) ?? 0) + 1);
      placements.set(placement.placement_id, resource.class_id);
    }
  }
  if (demand.get(PCM_CAPTURE_RESOURCE) !== 1 || demand.get(PCM_PLAY_RESOURCE) !== 1 ||
      demand.size !== 2) {
    throw new Error("browser remote voice requires one microphone turn and one audio output");
  }
  const pushToTalk = outputRoot.ownerDocument.createElement("button");
  pushToTalk.type = "button";
  pushToTalk.textContent = "Hold to talk";
  pushToTalk.dataset.conduitPushToTalk = "";
  outputRoot.append(pushToTalk);
  let pcm = null, activePlayId = null, closed = false;
  try {
    pcm = acquirePcm({ window, pushToTalkTarget: pushToTalk });
    if (pcm.capacity?.capture !== 1 || pcm.capacity?.playback !== 1) {
      throw new Error("browser PCM acquisition differs from the planned voice demand");
    }
  } catch (error) {
    pushToTalk.remove();
    throw error;
  }
  const current = () => {
    if (closed || !outputRoot.isConnected || !pushToTalk.isConnected) {
      throw new Error("browser remote voice resources were lost");
    }
  };
  return Object.freeze({
    observations() {
      current();
      if (activePlayId) throw new Error("browser remote voice resources are reserved");
      return [PCM_CAPTURE_RESOURCE, PCM_PLAY_RESOURCE].map(class_id => ({
        host_id: host.host_id,
        boot_id: host.boot_id,
        offer_generation: host.offer_generation,
        pool_id: pools.get(class_id),
        class_id,
        health: "Ready",
        unreserved_units: 1,
        utilized_units: 0,
        sign_id: `browser-resource/${host.boot_id}/${window.crypto.randomUUID()}`,
      }));
    },
    bind(playId) {
      current();
      if (activePlayId || typeof playId !== "string" || !playId) {
        throw new Error("browser remote voice Play binding refused");
      }
      activePlayId = playId;
    },
    async perform(effect, signal) {
      current();
      if (!activePlayId || effect?.host_id !== host.host_id || effect?.boot_id !== host.boot_id ||
          effect.active_play_id !== activePlayId || !(signal instanceof AbortSignal)) {
        throw new Error("browser remote voice effect identity mismatch");
      }
      const expected = effect.effect_kind === "audio-capture" ? PCM_CAPTURE_RESOURCE
        : effect.effect_kind === "pcm-playback" ? PCM_PLAY_RESOURCE : null;
      if (!expected || placements.get(effect.placement_id) !== expected) {
        throw new Error("browser remote voice effect differs from its planned resource");
      }
      return pcm.perform(effect, signal);
    },
    async close() {
      if (closed) return;
      closed = true;
      try { await pcm.close(); }
      finally { pushToTalk.remove(); }
    },
  });
}
