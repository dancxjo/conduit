import { createNativeSporeDownload } from "../../../creche-spore-bundle.mjs";
import { bindBodyProvisionedMedia } from "../../../creche-native-disk.mjs";
import { acquireOrangePiImage, validateImageWriterEvidence } from "./image.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const FAMILY = Object.freeze({ id: "conduit-target-family/orange-pi@1", label: "Orange Pi computers" });
const MODES = Object.freeze([
  Object.freeze({ id: "fabricate-new", resultKind: "artifact", supported: true }),
  Object.freeze({ id: "install-existing", resultKind: "installation", supported: false }),
  Object.freeze({ id: "attach-running", resultKind: "attachment", supported: false }),
]);
const BOUNDS = Object.freeze({ maximumOperations: 16, maximumOperationEvidenceBytes: 32 * 1024, maximumRetainedEvidenceBytes: 128 * 1024 });

export const ORANGE_PI_5_PROFILE = Object.freeze({
  target: Object.freeze({
    id: "conduitos/aarch64/orange-pi-5-rk3588s",
    label: "Orange Pi 5 · RK3588S · bare-metal ConduitOS",
    model_id: "orange-pi/orange-pi-5@1",
    profile_id: "conduitos-rk3588s-u-boot-sd",
  }),
  manifestPath: new URL("../../../artifacts/orange-pi-5-image.json", import.meta.url).href,
  packageId: "conduit-host-orange-pi@1",
  builderAdapter: "conduit-host-orange-pi/build-conduitos-sd-image@1",
  deploymentAdapter: "conduit-host-orange-pi/flash-removable-media@1",
  architecture: "aarch64",
  machine: "rk3588s",
  board: "orange-pi-5",
  bootMechanism: "rk3588s-bootrom-u-boot-booti-conduitos-image",
});

const declaration = Object.freeze({
  schema: "conduit.orange-pi/creche-bare-metal-profile@1",
  intention: "fabricate-new",
  model: ORANGE_PI_5_PROFILE.board,
  soc: "rockchip-rk3588s",
  cpu: "4xcortex-a76+4xcortex-a55",
  architecture: ORANGE_PI_5_PROFILE.architecture,
  os: null,
  boot_mechanism: ORANGE_PI_5_PROFILE.bootMechanism,
  image_format: "mbr-rk3588-fat32-sd-image",
  carrier: "removable-microsd-card",
  browser_role: "download Body-bound ConduitOS SD image spore only",
  local_helper: "explicit removable-media writer with raw block authority",
  browser_raw_block_authority: false,
  physical_flash_boot_uart_human_gated: true,
});

export const ORANGE_PI_CRECHE_TARGET_CONTRIBUTION = Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: ORANGE_PI_5_PROFILE.target,
  intentions: MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed ConduitOS Orange Pi 5 SD image" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([
      Object.freeze({ id: "conduit-carrier/removable-sd-download@1", label: "Download Body-bound IMG for explicit local SD writer" }),
    ]),
    installation: Object.freeze([]), attachment: Object.freeze([]), observation: Object.freeze([]),
  }),
  bounds: BOUNDS,
  expected_join_contract: "conduit.conduitos/physical-uart-attestation-before-join@1",
  target_profile: declaration,
  createAdapter: ({ host }) => createOrangePiAdapter({ host }),
});

export function createOrangePiAdapter({ host, imageWriter } = {}) {
  function createOptions({ mode }) {
    const note = document.createElement("p");
    note.className = "target-option-note";
    note.textContent = mode === "fabricate-new"
      ? "Conduit downloads the exact reviewed Orange Pi 5 ConduitOS image and binds it into a spore. A separate local writer must hold explicit raw block-device authority; this browser does not."
      : "This exact bare-metal substrate is fabricated as a new Host; no existing operating-system installation is used.";
    return note;
  }

  async function obtain({ mode, signal }) {
    requireMode(mode, "obtain"); requireCurrent(signal, mode, "obtain");
    const release = await acquireOrangePiImage(ORANGE_PI_5_PROFILE, signal);
    return Object.freeze({
      resultKind: "artifact", private: release,
      evidence: Object.freeze({
        schema: "conduit.orange-pi/creche-image-obtainment@1",
        target_id: ORANGE_PI_5_PROFILE.target.id, result_kind: "artifact",
        image_id: release.manifest.image_id, source_identity: release.manifest.source_identity,
        image_sha256: release.digest, image_bytes: release.bytes.byteLength,
        image_format: release.manifest.artifact.format,
        boot_files: [release.manifest.bootloader_asset, release.manifest.kernel, release.manifest.boot_script],
        carrier: "conduit-carrier/removable-sd-download@1",
        browser_raw_block_authority_claimed: false,
        does_not_prove: Object.freeze(["image-write", "boot", "uart-observation", "join", "membership"]),
      }),
    });
  }

  async function bind({ mode, body, obtainment, nowMillis, signal }) {
    requireMode(mode, "bind"); requireCurrent(signal, mode, "bind");
    const release = obtainment?.private;
    if (!release?.bytes || release.digest !== obtainment.evidence?.image_sha256) refuse(mode, "bind", "MissingArtifact", "exact Orange Pi 5 SD image truth is missing before Body binding");
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(ORANGE_PI_5_PROFILE.target.id);
    const digestBytes = encoder.encode(release.digest);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy); input.set(targetBytes, entropy.length); input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(targetBytes.length, digestBytes.length, BigInt(nowMillis));
      if (code < 0) throw outputError(host.runtime, "Orange Pi spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== ORANGE_PI_5_PROFILE.target.id || prepared.image_content_digest !== release.digest
        || prepared.output !== "sd-image" || prepared.fabrication_package_id !== ORANGE_PI_5_PROFILE.packageId
        || prepared.deployment_adapter !== ORANGE_PI_5_PROFILE.deploymentAdapter) {
        refuse(mode, "bind", "BindingIdentity", "prepared invitation lost exact Orange Pi 5, ConduitOS image, or writer-adapter truth");
      }
      const filename = `${friendlyFilename(body?.friendly_name ?? "body")}-conduitos-orange-pi-5.img`;
      const nativeSpore = await bindBodyProvisionedMedia({ prepared, imageBytes: release.bytes, filename, format: "img", mediaType: "application/x-raw-disk-image" });
      prepared.invitation_secret.fill(0);
      const download = await createNativeSporeDownload({ prepared, bytes: nativeSpore.bytes, contentDigest: nativeSpore.content_digest, filename, format: nativeSpore.format, mediaType: nativeSpore.media_type });
      return Object.freeze({
        prepared, nativeSpore, download,
        evidence: Object.freeze({
          ...prepared, invitation_secret: "embedded in native IMG; redacted",
          spore_artifact: Object.freeze({
            format: nativeSpore.format, filename, media_type: nativeSpore.media_type,
            bytes: nativeSpore.bytes.byteLength, content_digest: nativeSpore.content_digest,
            image_content_digest: nativeSpore.image_content_digest, image_bytes: nativeSpore.image_bytes,
            provision_offset: nativeSpore.provision_offset, provision_bytes: nativeSpore.provision_bytes,
          }),
        }),
      });
    } finally { entropy.fill(0); }
  }

  async function realize({ mode, binding, signal }) {
    requireMode(mode, "realize"); requireCurrent(signal, mode, "realize");
    const supplied = imageWriter ? await imageWriter({ profile: ORANGE_PI_5_PROFILE, binding, signal }) : null;
    let receipt;
    try { receipt = validateImageWriterEvidence(supplied, ORANGE_PI_5_PROFILE, binding); }
    catch (error) { refuse(mode, "realize", error?.code ?? "WriterEvidenceInvalid", error instanceof Error ? error.message : String(error)); }
    return Object.freeze({ terminal: "ImageWritten", evidence: Object.freeze({ schema: "conduit.orange-pi/creche-image-write@1", terminal: "ImageWritten", receipt, package_install_completed: false, boot_observed: false, join_created: false }) });
  }

  async function observe({ mode }) { requireMode(mode, "observe"); refuse(mode, "observe", "BootObservationUnavailable", "physical Orange Pi 5 boot and UART2 observation remain a separate human-gated proof"); }
  async function cancel({ mode, operation }) { return Object.freeze({ schema: "conduit.orange-pi/creche-cancellation@1", target_id: ORANGE_PI_5_PROFILE.target.id, mode, operation, terminal: "Cancelled" }); }
  return Object.freeze({ schema: "conduit.creche/physical-host-target-adapter@1", target: ORANGE_PI_5_PROFILE.target, modes: MODES, bounds: BOUNDS, createOptions, obtain, bind, realize, observe, cancel });
}

function requireMode(mode, operation) { if (mode === "fabricate-new") return; refuse(mode, operation, mode === "install-existing" ? "InstallExistingUnsupported" : "AttachRunningUnsupported", `bare-metal Orange Pi 5 target does not offer ${mode}`); }
function requireCurrent(signal, mode, operation) { if (signal?.aborted) refuse(mode, operation, "Cancelled", "Orange Pi operation was cancelled"); }
function refuse(mode, operation, terminal, message) { const error = new Error(message); error.code = terminal; error.evidence = Object.freeze({ schema: "conduit.orange-pi/creche-operation-refusal@1", target_id: ORANGE_PI_5_PROFILE.target.id, mode, operation, terminal, message, browser_raw_block_authority_claimed: false, external_work_started: false }); throw error; }
function readOutput(api) { return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len()))); }
function outputError(api, operation, code) { const output = readOutput(api); const error = new Error(`${operation} refused (${code}): ${output.message ?? "unknown refusal"}`); error.code = "RuntimeRefusal"; error.output = output; return error; }
function friendlyFilename(value) { const normalized = value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); return normalized.slice(0, 80) || "body"; }
