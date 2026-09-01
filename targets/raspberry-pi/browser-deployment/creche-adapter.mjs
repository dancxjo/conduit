import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_MODES } from "../../../creche-existing-computer.mjs";
import { packageSporeBundle } from "../../../creche-spore-bundle.mjs";
import { acquireRaspberryPiImage, validateImageWriterEvidence } from "./image.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const FAMILY = Object.freeze({ id: "conduit-target-family/raspberry-pi@1", label: "Raspberry Pi computers" });
const BARE_METAL_MODES = Object.freeze([
  Object.freeze({ id: "fabricate-new", resultKind: "artifact", supported: true }),
  Object.freeze({ id: "install-existing", resultKind: "installation", supported: false }),
  Object.freeze({ id: "attach-running", resultKind: "attachment", supported: false }),
]);
const BARE_METAL_BOUNDS = Object.freeze({
  maximumOperations: 16,
  maximumOperationEvidenceBytes: 32 * 1024,
  maximumRetainedEvidenceBytes: 128 * 1024,
});

export const RASPBERRY_PI_OS_PROFILE = Object.freeze({
  target: Object.freeze({
    id: "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb",
    label: "Pi 4 Model B rev 1.5 (4 GB) · Raspberry Pi OS Bookworm 64-bit",
    model_id: "raspberry-pi/pi-4-model-b-rev-1.5-4gb@1",
    profile_id: "raspios-bookworm-64-aarch64",
  }),
  target_id: "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb",
  manifest_path: new URL(
    "../../../artifacts/raspios-bookworm-pi4-model-b-rev-1.5-4gb.json",
    import.meta.url,
  ).href,
  package_id: "conduit-host-raspberry-pi@1",
  output: "native-bundle",
  builder_adapter: "conduit-host-raspberry-pi/build-raspios-native@1",
  deployment_adapter: "conduit-host-raspberry-pi/install-raspios-package@1",
  os: "raspberry-pi-os-bookworm-64",
  architecture: "aarch64",
  browser_carrier: false,
  credentials_required: true,
  declaration: Object.freeze({
    schema: "conduit.raspberry-pi/creche-os-profile@1",
    intention: "install-existing",
    model: "raspberry-pi-4-model-b-rev-1.5-4gb",
    architecture: "aarch64",
    os: "raspberry-pi-os-bookworm-64",
    package_id: "conduit-host-raspberry-pi@1",
    artifact_format: "native-bundle",
    browser_role: "download Body-bound package only",
    local_helper: "explicit package installer with separately supplied credentials",
    physical_boot_claimed: false,
  }),
});

export const RASPBERRY_PI_B_PLUS_PROFILE = Object.freeze({
  target: Object.freeze({
    id: "conduitos/armv6/raspberry-pi-model-b-plus-v1.2",
    label: "Model B+ v1.2 · ARMv6 · bare-metal ConduitOS",
    model_id: "raspberry-pi/model-b-plus-v1.2@1",
    profile_id: "bcm2835-armv6-direct-kernel-sd",
  }),
  manifestPath: new URL("../../../artifacts/rpi-b-plus-image.json", import.meta.url).href,
  packageId: "conduit-host-raspberry-pi@1",
  builderAdapter: "conduit-host-raspberry-pi/build-sd-image@1",
  deploymentAdapter: "conduit-host-raspberry-pi/flash-removable-media@1",
  architecture: "armv6",
  machine: "BCM2835/ARM1176JZF-S",
  board: "raspberry-pi-model-b-plus-v1.2",
  bootMechanism: "raspberry-pi-videocore-firmware-direct-kernel",
});

const piOsContribution = Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: RASPBERRY_PI_OS_PROFILE.target,
  intentions: EXISTING_COMPUTER_MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed Raspberry Pi OS aarch64 package" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([]),
    installation: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-release-download@1", label: "Download Body-bound Raspberry Pi OS ZIP" }),
    ]),
    attachment: Object.freeze([]),
    observation: Object.freeze([]),
  }),
  bounds: EXISTING_COMPUTER_BOUNDS,
  expected_join_contract: "conduit.host/native-spawn-observation@1",
  target_profile: RASPBERRY_PI_OS_PROFILE.declaration,
  createAdapter: ({ host }) => createExistingComputerAdapter({ host, profile: RASPBERRY_PI_OS_PROFILE }),
});

const bareMetalDeclaration = Object.freeze({
  schema: "conduit.raspberry-pi/creche-bare-metal-profile@1",
  intention: "fabricate-new",
  model: RASPBERRY_PI_B_PLUS_PROFILE.board,
  architecture: RASPBERRY_PI_B_PLUS_PROFILE.architecture,
  machine: RASPBERRY_PI_B_PLUS_PROFILE.machine,
  boot_mechanism: RASPBERRY_PI_B_PLUS_PROFILE.bootMechanism,
  image_format: "mbr-fat32-sd-image",
  boot_files: Object.freeze(["LICENCE.broadcom", "bootcode.bin", "config.txt", "fixup.dat", "kernel.img", "start.elf"]),
  carrier: "removable-sd-card",
  browser_role: "download Body-bound SD image spore only",
  local_helper: "explicit removable-media writer with raw block authority",
  browser_raw_block_authority: false,
  physical_flash_boot_uart_human_gated: true,
});

const bareMetalContribution = Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: RASPBERRY_PI_B_PLUS_PROFILE.target,
  intentions: BARE_METAL_MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed ConduitOS Model B+ SD image" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([
      Object.freeze({ id: "conduit-carrier/removable-sd-download@1", label: "Download spore for explicit local SD writer" }),
    ]),
    installation: Object.freeze([]),
    attachment: Object.freeze([]),
    observation: Object.freeze([]),
  }),
  bounds: BARE_METAL_BOUNDS,
  expected_join_contract: "conduit.conduitos/physical-uart-attestation-before-join@1",
  target_profile: bareMetalDeclaration,
  createAdapter: ({ host }) => createBareMetalAdapter({ host }),
});

export const RASPBERRY_PI_CRECHE_TARGET_CONTRIBUTIONS = Object.freeze([piOsContribution, bareMetalContribution]);

export function createBareMetalAdapter({ host, imageWriter } = {}) {
  function createOptions({ mode }) {
    const note = document.createElement("p");
    note.className = "target-option-note";
    note.textContent = mode === "fabricate-new"
      ? "Conduit downloads the exact reviewed Model B+ SD image and binds it into a spore. A separate local writer must hold explicit raw block-device authority; this browser does not."
      : "This exact bare-metal substrate is fabricated as a new Host; it is not an existing OS installation or an already-running attachment.";
    return note;
  }

  async function obtain({ mode, signal }) {
    requireMode(mode, "obtain"); requireCurrent(signal, mode, "obtain");
    const release = await acquireRaspberryPiImage(RASPBERRY_PI_B_PLUS_PROFILE, signal);
    return Object.freeze({
      resultKind: "artifact",
      private: release,
      evidence: Object.freeze({
        schema: "conduit.raspberry-pi/creche-image-obtainment@1",
        target_id: RASPBERRY_PI_B_PLUS_PROFILE.target.id,
        result_kind: "artifact",
        image_id: release.manifest.image_id,
        source_identity: release.manifest.source_identity,
        image_sha256: release.digest,
        image_bytes: release.bytes.byteLength,
        image_format: release.manifest.artifact.format,
        boot_files: release.manifest.boot_files.map(({ path, bytes, sha256 }) => ({ path, bytes, sha256 })),
        carrier: "conduit-carrier/removable-sd-download@1",
        browser_raw_block_authority_claimed: false,
        does_not_prove: Object.freeze(["image-write", "boot", "uart-observation", "join", "membership"]),
      }),
    });
  }

  async function bind({ mode, obtainment, nowMillis, signal }) {
    requireMode(mode, "bind"); requireCurrent(signal, mode, "bind");
    const release = obtainment?.private;
    if (!release?.bytes || release.digest !== obtainment.evidence?.image_sha256) refuse(mode, "bind", "MissingArtifact", "exact SD image truth is missing before Body binding");
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(RASPBERRY_PI_B_PLUS_PROFILE.target.id);
    const digestBytes = encoder.encode(release.digest);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy); input.set(targetBytes, entropy.length); input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(targetBytes.length, digestBytes.length, BigInt(nowMillis));
      if (code < 0) throw outputError(host.runtime, "Raspberry Pi spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== RASPBERRY_PI_B_PLUS_PROFILE.target.id || prepared.image_content_digest !== release.digest
        || prepared.output !== "sd-image" || prepared.fabrication_package_id !== RASPBERRY_PI_B_PLUS_PROFILE.packageId
        || prepared.deployment_adapter !== RASPBERRY_PI_B_PLUS_PROFILE.deploymentAdapter) {
        refuse(mode, "bind", "BindingIdentity", "prepared invitation lost exact Model B+, SD image, or writer-adapter truth");
      }
      const download = packageSporeBundle({
        prepared,
        artifact: Object.freeze({ layout: Object.freeze({ format: "mbr-fat32-sd-image", release: release.manifest }), payloads: [release.bytes] }),
        filename: `${prepared.spore_id.replaceAll(":", "-")}.spore`,
      });
      return Object.freeze({ prepared, download, evidence: Object.freeze({ ...prepared, invitation_secret: "redacted" }) });
    } finally { entropy.fill(0); }
  }

  async function realize({ mode, binding, signal }) {
    requireMode(mode, "realize"); requireCurrent(signal, mode, "realize");
    const supplied = imageWriter ? await imageWriter({ profile: RASPBERRY_PI_B_PLUS_PROFILE, binding, signal }) : null;
    let receipt;
    try { receipt = validateImageWriterEvidence(supplied, RASPBERRY_PI_B_PLUS_PROFILE, binding); }
    catch (error) { refuse(mode, "realize", error?.code ?? "WriterEvidenceInvalid", error instanceof Error ? error.message : String(error)); }
    return Object.freeze({
      terminal: "ImageWritten",
      evidence: Object.freeze({
        schema: "conduit.raspberry-pi/creche-image-write@1",
        terminal: "ImageWritten",
        receipt,
        package_install_completed: false,
        boot_observed: false,
        join_created: false,
      }),
    });
  }

  async function observe({ mode }) {
    requireMode(mode, "observe");
    refuse(mode, "observe", "BootObservationUnavailable", "physical Model B+ boot and UART observation remain a separate human-gated proof");
  }

  async function cancel({ mode, operation }) {
    return Object.freeze({ schema: "conduit.raspberry-pi/creche-cancellation@1", target_id: RASPBERRY_PI_B_PLUS_PROFILE.target.id, mode, operation, terminal: "Cancelled" });
  }
  return Object.freeze({ schema: "conduit.creche/physical-host-target-adapter@1", target: RASPBERRY_PI_B_PLUS_PROFILE.target, modes: BARE_METAL_MODES, bounds: BARE_METAL_BOUNDS, createOptions, obtain, bind, realize, observe, cancel });
}

function requireMode(mode, operation) {
  if (mode === "fabricate-new") return;
  refuse(mode, operation, mode === "install-existing" ? "InstallExistingUnsupported" : "AttachRunningUnsupported", `bare-metal Model B+ target does not offer ${mode}`);
}
function requireCurrent(signal, mode, operation) { if (signal?.aborted) refuse(mode, operation, "Cancelled", "Raspberry Pi operation was cancelled"); }
function refuse(mode, operation, terminal, message) {
  const error = new Error(message); error.code = terminal;
  error.evidence = Object.freeze({ schema: "conduit.raspberry-pi/creche-operation-refusal@1", target_id: RASPBERRY_PI_B_PLUS_PROFILE.target.id, mode, operation, terminal, message, browser_raw_block_authority_claimed: false, external_work_started: false });
  throw error;
}
function readOutput(api) { return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len()))); }
function outputError(api, operation, code) { const output = readOutput(api); const error = new Error(`${operation} refused (${code}): ${output.message ?? "unknown refusal"}`); error.code = "RuntimeRefusal"; error.output = output; return error; }
