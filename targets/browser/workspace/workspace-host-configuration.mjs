import { createExistingComputerAdapter } from "./creche-existing-computer.mjs";
import { readBodyBoundZip } from "./creche-native-zip.mjs";
import { BROWSER_DISTRIBUTION_PROFILE, BROWSER_RELEASE_PROFILE } from "../deployment/browser/creche-adapter.mjs";
import { acquireConfiguredHostRelease } from "./creche-release-catalog.mjs";
import { buildBrowserBundleImage } from "../deployment/browser/browser-bundle.mjs";
import { createBrowserConfigurationOutfitter as createExactOutfitter } from "./browser-host-configuration.mjs";

const CATALOG_SCHEMA = "conduit.host/browser-capability-intent-catalog@1";
const REVIEW_SCHEMA = "conduit.host/browser-capability-configuration-review@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBrowserConfigurationOutfitter({ host, faceFor, onChange }) {
  const catalog = call(host.runtime, "conduit_browser_host_capability_intent_catalog");
  requireCatalog(catalog);
  const selected = new Set(catalog.defaults);
  let exact = null;

  return Object.freeze({
    required: true,
    render() {
      const root = document.createElement("section");
      root.className = "browser-capability-intent";
      if (exact) {
        const change = document.createElement("button");
        change.type = "button";
        change.textContent = "Change desired capabilities";
        change.addEventListener("click", () => { exact = null; onChange(); });
        root.append(exact.render(), change);
        return root;
      }
      const target = document.createElement("p");
      target.textContent = `Target: Browser page (${catalog.target_id}) · bind reviewed superset`;
      const fieldset = document.createElement("fieldset");
      const legend = document.createElement("legend"); legend.textContent = "What should this host help with?";
      fieldset.append(legend);
      for (const entry of catalog.entries) {
        const label = document.createElement("label");
        const input = document.createElement("input"); input.type = "checkbox";
        input.checked = selected.has(entry.kind);
        input.addEventListener("change", () => input.checked ? selected.add(entry.kind) : selected.delete(entry.kind));
        label.append(input, document.createTextNode(entry.label));
        fieldset.append(label);
      }
      const resolve = document.createElement("button"); resolve.type = "button"; resolve.textContent = "Resolve reviewed Bases";
      resolve.addEventListener("click", () => {
        const review = reviewIntent(host.runtime, catalog.generation, [...selected]);
        exact = createExactOutfitter({ host, faceFor, restoredSelection: review.configuration_selection, onChange });
        onChange();
      });
      root.append(target, fieldset, resolve);
      return root;
    },
    checked: () => exact?.checked() ?? null,
    selection: () => exact?.selection() ?? null,
  });
}

function reviewIntent(runtime, generation, kinds) {
  if (kinds.length < 1 || kinds.length > 32) throw new RangeError("Choose between one and 32 Host capabilities");
  const bytes = encoder.encode(JSON.stringify({ catalog_generation: generation, capabilities: kinds.sort().map(kind => ({ kind })) }));
  if (bytes.length > runtime.conduit_creche_input_capacity()) throw new RangeError("Host capability intent exceeds its admitted bound");
  new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), bytes.length).set(bytes);
  const code = runtime.conduit_browser_host_review_capability_intent(bytes.length);
  if (code < 0) throw outputError(runtime, "Host capability intent review", code);
  const review = readOutput(runtime);
  if (review?.schema !== REVIEW_SCHEMA || !review.configuration_selection) throw new TypeError("Host capability intent review is incompatible");
  return review;
}

function requireCatalog(catalog) {
  if (catalog?.schema !== CATALOG_SCHEMA || !Number.isSafeInteger(catalog.generation)
      || catalog.generation < 1 || typeof catalog.target_id !== "string"
      || !Array.isArray(catalog.defaults) || !Array.isArray(catalog.entries)
      || catalog.entries.length < 1 || catalog.entries.length > 32) {
    throw new TypeError("Host capability intent catalog is malformed or over capacity");
  }
}

function call(runtime, name) {
  const code = runtime[name]();
  if (code < 0) throw outputError(runtime, name, code);
  return readOutput(runtime);
}

function readOutput(runtime) {
  return JSON.parse(decoder.decode(new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_output_ptr(), runtime.conduit_creche_output_len())));
}

function outputError(runtime, operation, code) {
  let message = `${operation} refused (${code})`;
  try { message = readOutput(runtime).message ?? message; } catch {}
  return new Error(message);
}

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
    retire: () => request("ReleaseStemAfterAdmission", owner),
    cancel,
  });
}

export function openWorkspaceStem({ host, session, faceFor, root, wake, onChanged, onError }) {
  let eligible = false;
  let preparing = false;
  let provider, ready = false;
  let pendingEffect;
  const update = () => {
    root.hidden = !eligible;
    if (eligible && !preparing) root.replaceChildren(outfitter.render(ready));
  };
  const outfitter = createExactOutfitter({
    host, faceFor, onChange: update,
    async onPrepareStem(selection) {
      if (!eligible || preparing || session.current()?.state !== "LULLED") return;
      preparing = true;
      root.replaceChildren();
      try {
        const signal = AbortSignal.timeout(120_000);
        const distribution = await acquireConfiguredHostRelease(host, BROWSER_DISTRIBUTION_PROFILE, signal);
        if (!eligible || session.current()?.state !== "LULLED") return;
        provider = await session.prepareStem(selection, distribution, signal);
        await wake(Object.freeze({
          ...provider,
          perform(effect, signal) {
            pendingEffect = provider.perform(effect, signal);
            return pendingEffect;
          },
          completed() {
            const result = provider.result();
            ready = Boolean(result.artifact_prepared && result.kernel_outcome);
            onChanged();
            if (!ready) onError(new Error("Stem did not prepare a Seed"));
          },
          async afterLull() {
            await pendingEffect?.catch(() => {});
            if (provider && !provider.result().delivery_committed) {
              provider.cancel();
              provider = null;
              await session.save();
            }
            ready = false;
            update();
          },
        }));
        if (session.current()?.state === "LULLED") {
          provider.cancel();
          provider = null;
          await session.save();
        }
      } catch (error) {
        onError(error);
      } finally {
        preparing = false;
        update();
      }
    },
    async onLaunchStem() {
      if (!eligible || preparing || !ready || session.current()?.state === "LULLED") return;
      preparing = true;
      root.replaceChildren();
      try {
        await launchSeed(host, session, provider);
        provider.retire();
        provider = null;
        ready = false;
        onChanged();
      } catch (error) {
        onError(error);
      } finally {
        preparing = false;
        update();
      }
    },
  });
  return Object.freeze({
    show(selected) {
      const visible = selected && (session.current()?.state === "LULLED" || ready);
      if (eligible === visible) return;
      eligible = visible;
      update();
    },
  });
}

async function launchSeed(host, session, provider) {
  const { artifact } = provider.beginDelivery();
  const start = host.runtime.conduit_stem_seed_output_ptr();
  const length = host.runtime.conduit_stem_seed_output_len();
  const buffer = host.runtime.memory.buffer;
  if (!Number.isSafeInteger(start) || start <= 0 || length !== artifact.artifact_bytes ||
      length < 22 || length > 64 * 1024 * 1024 || start + length > buffer.byteLength) {
    throw new Error("Retained Stem Seed differs from its bounded package receipt");
  }
  const bytes = new Uint8Array(buffer, start, length).slice();
  try {
    const { provision } = readBodyBoundZip(bytes);
    const binding = {
      prepared: { ...provision.spore, invitation_id: provision.invitation_provision.invitation_id,
        invitation_nonce: provision.invitation_provision.nonce },
      nativeSpore: { bytes, content_digest: artifact.artifact_content_sha256 },
    };
    const adapter = createExistingComputerAdapter({ host, profile: BROWSER_RELEASE_PROFILE });
    const mode = "install-existing";
    const signal = AbortSignal.timeout(120_000);
    await adapter.realize({ mode, binding, signal, obtainment: { private: { release: { manifest: provision.release_manifest } } } });
    const { join } = await adapter.observe({ mode, binding, signal });
    const { advertisement, observed_at_millis, invitation_id, body_id, host_id, boot_id, nonce, signature } = join;
    await session.admitInvitation(advertisement, { invitation_id, body_id, host_id, boot_id, nonce, signature }, observed_at_millis);
  } finally {
    bytes.fill(0);
  }
}
