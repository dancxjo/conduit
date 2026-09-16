import { openBrowserRemoteFragment, runBrowserRemoteFragment } from "../../../targets/browser/host/assets/browser-remote-fragment.mjs";
import { acquireBrowserRemoteVoice } from "../../../targets/browser/host/assets/browser-remote-voice.mjs";

/** Prepare one exact two-Host spoken-conversation Plan without changing Body
 * lifecycle truth. The caller remains responsible for composing this fragment
 * with the owning Body Play before exposing it as awake. */
export async function prepareWorkspaceVoicePlay({
  api,
  localAdvertisement,
  joined,
  plan,
  outputRoot,
  window = outputRoot?.ownerDocument?.defaultView,
  acquireVoice = acquireBrowserRemoteVoice,
  openRemote = openBrowserRemoteFragment,
  driveRemote = runBrowserRemoteFragment,
}) {
  const fragments = Array.isArray(plan?.fragments) ? plan.fragments : [];
  const local = fragments.filter(fragment => fragment.host_id === localAdvertisement?.host_id &&
    fragment.boot_id === localAdvertisement?.boot_id);
  const peer = fragments.filter(fragment => fragment.host_id === joined?.host_id &&
    fragment.boot_id === joined?.boot_id);
  if (fragments.length !== 2 || local.length !== 1 || peer.length !== 1 ||
      joined?.advertisement?.host_id !== joined.host_id ||
      joined?.advertisement?.boot_id !== joined.boot_id ||
      joined?.line?.schema !== "conduit.creche/joined-host-line@1") {
    throw new Error("spoken conversation does not name one exact browser and Voice Host");
  }
  const voice = acquireVoice({ window, outputRoot, host: localAdvertisement, fragment: local[0] });
  let prepared = false, remote = null;
  try {
    const preparation = await joined.line.prepareRemote(plan);
    prepared = true;
    remote = openRemote({ api, plan, host: localAdvertisement, preparation,
      observations: voice.observations() });
    voice.bind(remote.identity.active_play_id);
  } catch (error) {
    try { remote?.close(); } catch {}
    await voice.close();
    if (prepared) await joined.line.releaseRemote().catch(() => {});
    throw error;
  }
  const controller = new AbortController();
  let running = null, settled = false, closed = false;
  return Object.freeze({
    identity: remote.identity,
    run() {
      if (closed || running) throw new Error("Workspace voice Play may run exactly once");
      running = driveRemote({ remote, line: joined.line,
        perform: (effect, signal) => voice.perform(effect, signal), signal: controller.signal })
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
        try { await running; }
        catch (error) { runError = error; }
      }
      try { remote.close(); }
      finally {
        await voice.close();
        await joined.line.releaseRemote();
      }
      if (runError && !cancelledForClose) throw runError;
    },
  });
}
