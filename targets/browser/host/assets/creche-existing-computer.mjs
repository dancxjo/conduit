import { acquireHostRelease, releaseArtifact } from "./creche-release-bundle.mjs";
import { packageSporeBundle } from "./creche-spore-bundle.mjs";

const ADAPTER_SCHEMA = "conduit.creche/physical-host-target-adapter@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const MODES = Object.freeze([
  Object.freeze({ id: "fabricate-new", resultKind: "artifact", supported: false }),
  Object.freeze({ id: "install-existing", resultKind: "installation", supported: true }),
  Object.freeze({ id: "attach-running", resultKind: "attachment", supported: false }),
]);
const BOUNDS = Object.freeze({
  maximumOperations: 12,
  maximumOperationEvidenceBytes: 32 * 1024,
  maximumRetainedEvidenceBytes: 128 * 1024,
});

export function createExistingComputerAdapter({ host, profile }) {
  requireProfile(profile);
  let loadedHost = null;

  function createOptions({ mode }) {
    const note = document.createElement("p");
    note.className = "target-option-note";
    if (mode === "install-existing") {
      note.textContent = profile.browser_carrier
        ? "Download the reviewed browser Host release, bind it into this Body's spore, then load that exact bundle in an isolated browser Host."
        : "Download the reviewed native Host release and bind it into this Body's spore. Installation and start wait for an explicit native helper; no SSH, credential, address, package-manager, or container authority is assumed.";
    } else if (mode === "attach-running") {
      note.textContent = "No authenticated already-running connection carrier is implemented for this exact target.";
    } else {
      note.textContent = "This is existing machinery; the Crèche downloads a reviewed generic release instead of fabricating a machine.";
    }
    return note;
  }

  async function obtain({ mode, signal }) {
    requireMode(mode, "obtain", profile);
    requireCurrent(signal, mode, "obtain", profile);
    try {
      const release = await acquireHostRelease(profile, signal);
      requireCurrent(signal, mode, "obtain", profile);
      return Object.freeze({
        resultKind: "installation",
        private: Object.freeze({ release }),
        evidence: Object.freeze({
          schema: "conduit.creche/existing-computer-obtainment@1",
          mode,
          result_kind: "installation",
          target_id: profile.target_id,
          os: profile.os,
          architecture: profile.architecture,
          package_id: release.manifest.fabrication_package_id,
          output: release.manifest.output,
          source_identity: release.manifest.source_identity,
          bundle_sha256: release.manifest.bundle_sha256,
          files: release.manifest.files.map(({ path, bytes, sha256, media_type }) => ({ path, bytes, sha256, media_type })),
          carrier: "conduit-carrier/browser-release-download@1",
          does_not_prove: ["body-binding", "installation", "start", "boot-observation", "join", "membership"],
        }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(profile, mode, "obtain", error?.code ?? "ArtifactUnavailable", "generic Host release acquisition terminated without success", error);
    }
  }

  async function bind({ mode, obtainment, nowMillis, signal }) {
    requireMode(mode, "bind", profile);
    requireCurrent(signal, mode, "bind", profile);
    const release = obtainment?.private?.release;
    if (!release || release.manifest?.bundle_sha256 !== obtainment.evidence?.bundle_sha256) {
      refuse(profile, mode, "bind", "MissingArtifact", "exact generic Host release truth is missing before Body binding");
    }
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(profile.target_id);
    const digestBytes = encoder.encode(release.manifest.bundle_sha256);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy);
      input.set(targetBytes, entropy.length);
      input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(targetBytes.length, digestBytes.length, BigInt(nowMillis));
      if (code < 0) throw outputError(host.runtime, "existing-computer spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== profile.target_id
        || prepared.image_content_digest !== release.manifest.bundle_sha256
        || prepared.output !== profile.output
        || prepared.fabrication_package_id !== profile.package_id
        || prepared.deployment_adapter !== profile.deployment_adapter) {
        refuse(profile, mode, "bind", "BindingIdentity", "Body binding lost the exact generic Host release selection");
      }
      requireCurrent(signal, mode, "bind", profile);
      const download = packageSporeBundle({
        prepared,
        artifact: releaseArtifact(release),
        filename: `${prepared.spore_id.replaceAll(":", "-")}.spore`,
      });
      return Object.freeze({
        prepared,
        download,
        evidence: Object.freeze({ ...prepared, invitation_secret: "redacted" }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(profile, mode, "bind", error?.code ?? "BindingFailed", "generic Host Body binding terminated without success", error);
    } finally {
      entropy.fill(0);
    }
  }

  async function realize({ mode, obtainment, binding, signal }) {
    requireMode(mode, "realize", profile);
    requireCurrent(signal, mode, "realize", profile);
    if (!profile.browser_carrier) {
      const terminal = profile.credentials_required ? "UnavailableCredentials" : "ExplicitInstallerRequired";
      const message = profile.credentials_required
        ? "the Body-bound package is downloadable, but no explicit Raspberry Pi OS installation credentials were supplied to a local helper"
        : "the native spore is downloadable, but no reviewed native installer/start carrier is implemented";
      refuse(profile, mode, "realize", terminal, message, undefined, {
        spore_id: binding.prepared.spore_id,
        offered_carriers: ["conduit-carrier/browser-release-download@1"],
        unavailable_carriers: ["explicit-helper", "ssh", "package-manager", "container"],
        ambient_credentials_used: false,
        ambient_addresses_used: false,
        external_work_started: false,
      });
    }
    const release = obtainment.private.release;
    const runtime = release.payloads.find(({ path }) => path === "runtime.wasm");
    if (!runtime) refuse(profile, mode, "realize", "StaleArtifact", "browser bundle omitted its exact runtime");
    try {
      const instance = await WebAssembly.instantiate(runtime.bytes, {});
      const api = instance.instance.exports;
      requireMembershipAbi(api);
      const hostId = `browser/${crypto.randomUUID()}`;
      const bootId = `browser-boot/${crypto.randomUUID()}`;
      initializeMembership(api, hostId, bootId);
      requireCurrent(signal, mode, "realize", profile);
      loadedHost = Object.freeze({ api, hostId, bootId });
      return Object.freeze({
        terminal: "BrowserBundleLoaded",
        evidence: Object.freeze({
          schema: "conduit.browser/creche-bundle-load@1",
          terminal: "BrowserBundleLoaded",
          target_id: profile.target_id,
          package_id: profile.package_id,
          bundle_sha256: release.manifest.bundle_sha256,
          spore_id: binding.prepared.spore_id,
          carrier: "conduit-carrier/browser-local-sandbox@1",
          host_id: hostId,
          boot_id: bootId,
          boot_observed: false,
          join_created: false,
        }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(profile, mode, "realize", error?.code ?? "BrowserLoadFailed", "browser bundle load terminated without success", error);
    }
  }

  async function observe({ mode, binding, signal }) {
    requireMode(mode, "observe", profile);
    requireCurrent(signal, mode, "observe", profile);
    if (!profile.browser_carrier || !loadedHost) {
      refuse(profile, mode, "observe", "NoRunningCarrier", "no authenticated running Host carrier is available for observation");
    }
    const { api, hostId, bootId } = loadedHost;
    if (api.conduit_browser_membership_advertisement() < 0) {
      refuse(profile, mode, "observe", "AdvertisementFailed", "loaded browser Host did not export current offers");
    }
    const advertisement = readMembershipOutput(api, true);
    if (advertisement.host_id !== hostId || advertisement.boot_id !== bootId) {
      refuse(profile, mode, "observe", "WrongBoot", "browser Host advertisement lost the loaded Host or Boot identity");
    }
    const prepared = binding.prepared;
    const envelope = encoder.encode(JSON.stringify({
      claim: {
        invitation_id: prepared.invitation_id,
        body_id: prepared.body_id,
        nonce: prepared.invitation_nonce,
        expires_at_millis: prepared.invitation_expires_at_millis,
      },
      secret: prepared.invitation_secret,
    }));
    writeMembershipInput(api, envelope);
    const code = api.conduit_browser_membership_prove_spawn(envelope.length);
    envelope.fill(0);
    prepared.invitation_secret.fill(0);
    if (code < 0) refuse(profile, mode, "observe", "JoinProofFailed", `loaded browser Host refused invitation proof ${code}`);
    const signature = Array.from(readMembershipOutput(api, false));
    requireCurrent(signal, mode, "observe", profile);
    const join = Object.freeze({
      spore_id: prepared.spore_id,
      image_id: prepared.image_id,
      advertisement,
      invitation_id: prepared.invitation_id,
      body_id: prepared.body_id,
      host_id: hostId,
      boot_id: bootId,
      nonce: prepared.invitation_nonce,
      signature,
      observed_at_millis: Date.now(),
    });
    return Object.freeze({
      join,
      evidence: Object.freeze({ schema: "conduit.browser/creche-spawn-observation@1", ...join }),
    });
  }

  async function cancel({ mode, operation }) {
    loadedHost = null;
    return Object.freeze({ schema: "conduit.creche/existing-computer-cancellation@1", target_id: profile.target_id, mode, operation, terminal: "Cancelled" });
  }

  return Object.freeze({ schema: ADAPTER_SCHEMA, target: profile.target, modes: MODES, bounds: BOUNDS, createOptions, obtain, bind, realize, observe, cancel });
}

export { MODES as EXISTING_COMPUTER_MODES, BOUNDS as EXISTING_COMPUTER_BOUNDS };

function initializeMembership(api, hostId, bootId) {
  const host = encoder.encode(hostId);
  const boot = encoder.encode(bootId);
  const seed = crypto.getRandomValues(new Uint8Array(32));
  const frame = new Uint8Array(host.length + boot.length + seed.length);
  frame.set(host); frame.set(boot, host.length); frame.set(seed, host.length + boot.length);
  writeMembershipInput(api, frame);
  const code = api.conduit_browser_membership_initialize(host.length, boot.length);
  seed.fill(0); frame.fill(0);
  if (code < 0) throw new Error(`browser Host initialization failed ${code}`);
}

function requireMembershipAbi(api) {
  const required = ["memory", "conduit_browser_membership_input_ptr", "conduit_browser_membership_input_capacity", "conduit_browser_membership_output_ptr", "conduit_browser_membership_output_len", "conduit_browser_membership_initialize", "conduit_browser_membership_advertisement", "conduit_browser_membership_prove_spawn"];
  if (required.some((name) => !(name in api)) || api.conduit_browser_membership_input_capacity() !== 4096) {
    const error = new Error("browser release membership ABI is incomplete"); error.code = "IncompatibleRelease"; throw error;
  }
}

function writeMembershipInput(api, bytes) {
  if (bytes.length > api.conduit_browser_membership_input_capacity()) throw new RangeError("browser membership input exceeds its finite bound");
  new Uint8Array(api.memory.buffer, api.conduit_browser_membership_input_ptr(), bytes.length).set(bytes);
}

function readMembershipOutput(api, json) {
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_browser_membership_output_ptr(), api.conduit_browser_membership_output_len());
  return json ? JSON.parse(decoder.decode(bytes)) : new Uint8Array(bytes);
}

function requireMode(mode, operation, profile) {
  if (mode === "install-existing") return;
  const code = mode === "attach-running" ? "AttachRunningUnsupported" : "FabricateNewUnsupported";
  refuse(profile, mode, operation, code, `${profile.target.label} does not offer ${mode}`, undefined, { authority_requested: false, external_work_started: false });
}

function requireCurrent(signal, mode, operation, profile) {
  if (signal?.aborted) refuse(profile, mode, operation, "Cancelled", "existing-computer operation was cancelled");
}

function requireProfile(profile) {
  if (!profile?.target || profile.target.id !== profile.target_id || typeof profile.browser_carrier !== "boolean") {
    throw new TypeError("existing-computer adapter profile is incomplete");
  }
}

function readOutput(api) {
  return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len())));
}

function outputError(api, label, code) {
  const error = new Error(`${label} failed ${code}: ${JSON.stringify(readOutput(api))}`); error.code = "RuntimeRefusal"; return error;
}

function refuse(profile, mode, operation, code, message, cause, detail = {}) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.code = code;
  error.evidence = Object.freeze({ schema: "conduit.creche/existing-computer-failure@1", target_id: profile.target_id, mode, operation, terminal: code, message, ...detail });
  throw error;
}
