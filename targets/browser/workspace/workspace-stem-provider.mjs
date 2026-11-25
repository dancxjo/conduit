import { BROWSER_RELEASE_PROFILE } from "../deployment/browser/creche-adapter.mjs";
import { createBrowserConfigurationOutfitter } from "./browser-host-configuration.mjs";
import { acquireConfiguredHostRelease } from "./creche-release-catalog.mjs";
import { buildBrowserBundleImage } from "../deployment/browser/browser-bundle.mjs";

const encoder = new TextEncoder();
const chunkBytes = 64 * 1024;

export async function prepareWorkspaceStemProvider({ request, here, selection, distribution, signal }) {
  signal.throwIfAborted();
  const manifestBytes = distribution.manifestBytes;
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", manifestBytes));
  const manifestDigest = "sha256:" + Array.from(hash, byte => byte.toString(16).padStart(2, "0")).join("");
  const { manifest, payloads } = distribution;
  const now = Date.now();
  signal.throwIfAborted();
  const secret = crypto.getRandomValues(new Uint8Array(32));
  const reservation = {
    ...here, target_id: "browser/wasm32/page", selection,
    secret: Array.from(secret), nonce: Array.from(crypto.getRandomValues(new Uint8Array(32))),
    now_millis: now, expires_at_millis: now + 10 * 60_000,
    expected_release_digest: manifest.bundle_sha256,
    release_pin: { manifest_digest: manifestDigest, source_identity: manifest.source_identity },
  };
  let reserved;
  try {
    reserved = request("ReserveStem", { reservation });
  } finally {
    secret.fill(0);
    reservation.secret.fill(0);
  }
  const owner = { ...here, reservation_id: reserved.reservation_id };
  const stage = operation => request("StageStemRelease", { stage: { ...owner, operation } });
  const cancel = () => request("CancelStemReservation", owner);
  try {
    stage({ kind: "Begin", manifest: Array.from(manifestBytes) });
    for (const [file, { bytes }] of payloads.entries()) {
      for (let offset = 0; offset < bytes.byteLength; offset += chunkBytes) {
        signal.throwIfAborted();
        stage({ kind: "Chunk", file, offset, bytes: Array.from(bytes.subarray(offset, offset + chunkBytes)) });
      }
    }
    stage({ kind: "Finish" });
  } catch (error) {
    cancel();
    throw error;
  }
  return Object.freeze({
    observe: placement => request("ObserveStemResource", { ...owner, placement }),
    async perform(effect, effectSignal) {
      effectSignal.throwIfAborted();
      const { active_play_id, placement_id, request_sequence } = effect;
      const fields = () => ({ ...owner, active_play_id, placement_id, request_sequence, now_millis: Date.now() });
      const job = request("BeginStemHostJob", { call: fields() });
      try {
        if (job.release_manifest_digest !== manifestDigest || job.release_source_identity !== manifest.source_identity ||
            job.distribution_digest !== manifest.bundle_sha256) {
          throw new Error("Stem provider job differs from the staged distribution");
        }
        const release = await buildBrowserBundleImage({ checked: job.checked, distribution });
        effectSignal.throwIfAborted();
        const image = release.payloads.find(file => file.path === "conduit-browser-image.json");
        request("CompleteStemHostJob", {
          call: fields(),
          manifest: Array.from(encoder.encode(JSON.stringify(release.manifest))),
          image: Array.from(image.bytes),
        });
      } catch (error) {
        request("FailStemHostJob", { call: fields(), cancelled: effectSignal.aborted });
        if (effectSignal.aborted) throw error;
      }
      return Uint8Array.from(request("RunStemProvider", { call: fields() }).outcome);
    },
    result: () => request("StemResult", owner),
    beginDelivery: () => request("BeginStemDelivery", { ...owner, now_millis: Date.now() }),
    cancel,
  });
}

export function openWorkspaceStem({ host, session, faceFor, root, wake, onError }) {
  let eligible = false;
  let preparing = false;
  const update = () => {
    root.hidden = !eligible;
    if (eligible && !preparing) root.replaceChildren(outfitter.render());
  };
  const outfitter = createBrowserConfigurationOutfitter({
    host, faceFor, onChange: update,
    async onPrepareStem(selection) {
      if (!eligible || preparing || session.current()?.state !== "LULLED") return;
      preparing = true;
      root.replaceChildren();
      let provider;
      try {
        const signal = AbortSignal.timeout(120_000);
        const distribution = await acquireConfiguredHostRelease(host, BROWSER_RELEASE_PROFILE, signal);
        if (!eligible || session.current()?.state !== "LULLED") return;
        provider = await session.prepareStem(selection, distribution, signal);
        await wake(provider);
        if (session.current()?.state === "LULLED") {
          provider.cancel();
          await session.save();
        }
      } catch (error) {
        onError(error);
      } finally {
        preparing = false;
        update();
      }
    },
  });
  return Object.freeze({
    show(visible) {
      if (eligible === visible) return;
      eligible = visible;
      update();
    },
  });
}
