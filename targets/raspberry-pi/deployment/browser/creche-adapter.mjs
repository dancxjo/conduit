import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_MODES } from "../../../creche-existing-computer.mjs";
import { createNativeSporeDownload } from "../../../creche-spore-bundle.mjs";
import { bindBodyProvisionedMedia } from "../../../creche-native-disk.mjs";
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

function raspberryPiOsProfile({ id, label, model, machine, manifest }) {
  return Object.freeze({
  target: Object.freeze({
    id,
    label,
    model_id: `raspberry-pi/${model}@1`,
    profile_id: "raspios-bookworm-64-aarch64",
  }),
  target_id: id,
  manifest_path: new URL(`../../../artifacts/${manifest}`, import.meta.url).href,
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
    model: machine,
    architecture: "aarch64",
    os: "raspberry-pi-os-bookworm-64",
    package_id: "conduit-host-raspberry-pi@1",
    artifact_format: "native-bundle",
    browser_role: "download Body-bound package only",
    local_helper: "explicit package installer with separately supplied credentials",
    physical_boot_claimed: false,
  }),
});
}

export const RASPBERRY_PI_OS_PROFILE = raspberryPiOsProfile({
  id: "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb",
  label: "Pi 4 Model B rev 1.5 (4 GB) · Raspberry Pi OS Bookworm 64-bit",
  model: "pi-4-model-b-rev-1.5-4gb",
  machine: "raspberry-pi-4-model-b-rev-1.5-4gb",
  manifest: "raspios-bookworm-pi4-model-b-rev-1.5-4gb.json",
});
export const RASPBERRY_PI_ZERO_2_W_OS_PROFILE = raspberryPiOsProfile({
  id: "std/aarch64/raspberry-pi-zero-2-w-rev-1.0",
  label: "Pi Zero 2 W rev 1.0 · Raspberry Pi OS Bookworm 64-bit",
  model: "zero-2-w-rev-1.0",
  machine: "raspberry-pi-zero-2-w-rev-1.0",
  manifest: "raspios-bookworm-zero-2-w-rev-1.0.json",
});
export const RASPBERRY_PI_ZERO_2_WH_OS_PROFILE = raspberryPiOsProfile({
  id: "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0",
  label: "Pi Zero 2 WH rev 1.0 · Raspberry Pi OS Bookworm 64-bit",
  model: "zero-2-wh-rev-1.0",
  machine: "raspberry-pi-zero-2-wh-rev-1.0",
  manifest: "raspios-bookworm-zero-2-wh-rev-1.0.json",
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

function zeroBareMetalProfile({ id, label, model, manifest }) {
  return Object.freeze({
    target: Object.freeze({
      id: `conduitos/armv6/${id}`,
      label: `${label} · ARMv6 · bare-metal ConduitOS`,
      model_id: `raspberry-pi/${model}@1`,
      profile_id: `bcm2835-armv6-${model}-direct-kernel-sd`,
    }),
    manifestPath: new URL(`../../../artifacts/${manifest}`, import.meta.url).href,
    packageId: "conduit-host-raspberry-pi@1",
    builderAdapter: "conduit-host-raspberry-pi/build-sd-image@1",
    deploymentAdapter: "conduit-host-raspberry-pi/flash-removable-media@1",
    architecture: "armv6",
    machine: "BCM2835/ARM1176JZF-S",
    board: id,
    bootMechanism: "raspberry-pi-videocore-firmware-direct-kernel",
  });
}

export const RASPBERRY_PI_ZERO_PROFILE = zeroBareMetalProfile({ id: "raspberry-pi-zero-v1", label: "Pi Zero v1", model: "zero-v1", manifest: "rpi-zero-v1-image.json" });
export const RASPBERRY_PI_ZERO_W_PROFILE = zeroBareMetalProfile({ id: "raspberry-pi-zero-w-v1.1", label: "Pi Zero W v1.1", model: "zero-w-v1.1", manifest: "rpi-zero-w-v1.1-image.json" });
export const RASPBERRY_PI_ZERO_WH_PROFILE = zeroBareMetalProfile({ id: "raspberry-pi-zero-wh-v1.1", label: "Pi Zero WH v1.1", model: "zero-wh-v1.1", manifest: "rpi-zero-wh-v1.1-image.json" });

function piOsContribution(profile) { return Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: profile.target,
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
  target_profile: profile.declaration,
  createAdapter: ({ host }) => createExistingComputerAdapter({ host, profile }),
}); }

function bareMetalDeclaration(profile) { return Object.freeze({
  schema: "conduit.raspberry-pi/creche-bare-metal-profile@1",
  intention: "fabricate-new",
  model: profile.board,
  architecture: profile.architecture,
  machine: profile.machine,
  boot_mechanism: profile.bootMechanism,
  image_format: "mbr-fat32-sd-image",
  boot_files: Object.freeze(["LICENCE.broadcom", "bootcode.bin", "config.txt", "fixup.dat", "kernel.img", "start.elf"]),
  carrier: "removable-sd-card",
  browser_role: "download Body-bound SD image spore only",
  local_helper: "explicit removable-media writer with raw block authority",
  browser_raw_block_authority: false,
  physical_flash_boot_uart_human_gated: true,
}); }

function bareMetalContribution(profile) { return Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: profile.target,
  intentions: BARE_METAL_MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: `Reviewed ConduitOS ${profile.target.label} SD image` }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([
      Object.freeze({ id: "conduit-carrier/removable-sd-download@1", label: "Download Body-bound IMG for explicit local SD writer" }),
    ]),
    installation: Object.freeze([]),
    attachment: Object.freeze([]),
    observation: Object.freeze([]),
  }),
  bounds: BARE_METAL_BOUNDS,
  expected_join_contract: "conduit.conduitos/physical-uart-attestation-before-join@1",
  target_profile: bareMetalDeclaration(profile),
  createAdapter: ({ host }) => createBareMetalAdapter({ host, profile }),
}); }

export const RASPBERRY_PI_CRECHE_TARGET_CONTRIBUTIONS = Object.freeze([
  ...[RASPBERRY_PI_OS_PROFILE, RASPBERRY_PI_ZERO_2_W_OS_PROFILE, RASPBERRY_PI_ZERO_2_WH_OS_PROFILE].map(piOsContribution),
  ...[RASPBERRY_PI_B_PLUS_PROFILE, RASPBERRY_PI_ZERO_PROFILE, RASPBERRY_PI_ZERO_W_PROFILE, RASPBERRY_PI_ZERO_WH_PROFILE].map(bareMetalContribution),
]);

export function createBareMetalAdapter({ host, imageWriter, profile = RASPBERRY_PI_B_PLUS_PROFILE } = {}) {
  function createOptions({ mode }) {
    const note = document.createElement("p");
    note.className = "target-option-note";
    note.textContent = mode === "fabricate-new"
      ? `Conduit downloads the exact reviewed ${profile.target.label} SD image and binds it into a spore. A separate local writer must hold explicit raw block-device authority; this browser does not.`
      : "This exact bare-metal substrate is fabricated as a new Host; it is not an existing OS installation or an already-running attachment.";
    return note;
  }

  async function obtain({ mode, signal }) {
    requireMode(profile, mode, "obtain"); requireCurrent(profile, signal, mode, "obtain");
    const release = await acquireRaspberryPiImage(profile, signal);
    return Object.freeze({
      resultKind: "artifact",
      private: release,
      evidence: Object.freeze({
        schema: "conduit.raspberry-pi/creche-image-obtainment@1",
        target_id: profile.target.id,
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

  async function bind({ mode, body, obtainment, nowMillis, signal }) {
    requireMode(profile, mode, "bind"); requireCurrent(profile, signal, mode, "bind");
    const release = obtainment?.private;
    if (!release?.bytes || release.digest !== obtainment.evidence?.image_sha256) refuse(profile, mode, "bind", "MissingArtifact", "exact SD image truth is missing before Body binding");
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(profile.target.id);
    const digestBytes = encoder.encode(release.digest);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy); input.set(targetBytes, entropy.length); input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(targetBytes.length, digestBytes.length, BigInt(nowMillis));
      if (code < 0) throw outputError(host.runtime, "Raspberry Pi spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== profile.target.id || prepared.image_content_digest !== release.digest
        || prepared.output !== "sd-image" || prepared.fabrication_package_id !== profile.packageId
        || prepared.deployment_adapter !== profile.deploymentAdapter) {
        refuse(profile, mode, "bind", "BindingIdentity", "prepared invitation lost exact Raspberry Pi board, SD image, or writer-adapter truth");
      }
      const filename = `${friendlyFilename(body?.friendly_name ?? "body")}-conduitos-${friendlyFilename(profile.board)}.img`;
      const nativeSpore = await bindBodyProvisionedMedia({
        prepared,
        imageBytes: release.bytes,
        filename,
        format: "img",
        mediaType: "application/x-raw-disk-image",
      });
      prepared.invitation_secret.fill(0);
      const download = await createNativeSporeDownload({
        prepared,
        bytes: nativeSpore.bytes,
        contentDigest: nativeSpore.content_digest,
        filename,
        format: nativeSpore.format,
        mediaType: nativeSpore.media_type,
      });
      return Object.freeze({
        prepared,
        nativeSpore,
        download,
        evidence: Object.freeze({
          ...prepared,
          invitation_secret: "embedded in native IMG; redacted",
          spore_artifact: Object.freeze({
            format: nativeSpore.format,
            filename,
            media_type: nativeSpore.media_type,
            bytes: nativeSpore.bytes.byteLength,
            content_digest: nativeSpore.content_digest,
            image_content_digest: nativeSpore.image_content_digest,
            image_bytes: nativeSpore.image_bytes,
            provision_offset: nativeSpore.provision_offset,
            provision_bytes: nativeSpore.provision_bytes,
          }),
        }),
      });
    } finally { entropy.fill(0); }
  }

  async function realize({ mode, binding, signal }) {
    requireMode(profile, mode, "realize"); requireCurrent(profile, signal, mode, "realize");
    const supplied = imageWriter ? await imageWriter({ profile, binding, signal }) : null;
    let receipt;
    try { receipt = validateImageWriterEvidence(supplied, profile, binding); }
    catch (error) { refuse(profile, mode, "realize", error?.code ?? "WriterEvidenceInvalid", error instanceof Error ? error.message : String(error)); }
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
    requireMode(profile, mode, "observe");
    refuse(profile, mode, "observe", "BootObservationUnavailable", `physical ${profile.target.label} boot and UART observation remain a separate human-gated proof`);
  }

  async function cancel({ mode, operation }) {
    return Object.freeze({ schema: "conduit.raspberry-pi/creche-cancellation@1", target_id: profile.target.id, mode, operation, terminal: "Cancelled" });
  }
  return Object.freeze({ schema: "conduit.creche/physical-host-target-adapter@1", target: profile.target, modes: BARE_METAL_MODES, bounds: BARE_METAL_BOUNDS, createOptions, obtain, bind, realize, observe, cancel });
}

function requireMode(profile, mode, operation) {
  if (mode === "fabricate-new") return;
  refuse(profile, mode, operation, mode === "install-existing" ? "InstallExistingUnsupported" : "AttachRunningUnsupported", `bare-metal ${profile.target.label} target does not offer ${mode}`);
}
function requireCurrent(profile, signal, mode, operation) { if (signal?.aborted) refuse(profile, mode, operation, "Cancelled", "Raspberry Pi operation was cancelled"); }
function refuse(profile, mode, operation, terminal, message) {
  const error = new Error(message); error.code = terminal;
  error.evidence = Object.freeze({ schema: "conduit.raspberry-pi/creche-operation-refusal@1", target_id: profile.target.id, mode, operation, terminal, message, browser_raw_block_authority_claimed: false, external_work_started: false });
  throw error;
}
function readOutput(api) { return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len()))); }
function outputError(api, operation, code) { const output = readOutput(api); const error = new Error(`${operation} refused (${code}): ${output.message ?? "unknown refusal"}`); error.code = "RuntimeRefusal"; error.output = output; return error; }
function friendlyFilename(value) { const normalized = value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); return normalized.slice(0, 80) || "body"; }
