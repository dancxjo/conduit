import { createNativeSporeDownload } from "../../../creche-spore-bundle.mjs";
import { bindBodyProvisionedMedia } from "../../../creche-native-disk.mjs";
import { acquireConduitOsRelease, validateLoaderEvidence } from "./image.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const FAMILY = Object.freeze({ id: "conduit-target-family/conduitos@1", label: "ConduitOS machines" });
const PRODUCT_MODES = modes(true);
const BOUNDS = Object.freeze({ maximumOperations: 16, maximumOperationEvidenceBytes: 32 * 1024, maximumRetainedEvidenceBytes: 128 * 1024 });

function product({ id, label, architecture, machine, profileId, releaseName, firmware, bootEntry, presenter, bounds }) {
  return Object.freeze({
    target: Object.freeze({ id, label, model_id: `conduitos/${architecture}-${machine}@1`, profile_id: profileId }),
    architecture,
    machine,
    manifestPath: new URL(`../../../artifacts/conduitos-${releaseName}-release.json`, import.meta.url).href,
    builderAdapter: `conduit-host-conduitos/build-${architecture}@1`,
    deploymentAdapter: `conduit-host-conduitos/boot-${architecture}@1`,
    bootMechanism: "uefi-limine-hybrid-iso",
    firmware,
    bootEntry,
    presenter,
    bounds: Object.freeze(bounds),
    supportedCarriers: Object.freeze(["downloadable-disk-image"]),
  });
}

export const CONDUITOS_X86_64_PROFILE = product({
  id: "conduitos/x86_64/pc", label: "x86_64 PC · product Host", architecture: "x86_64", machine: "q35",
  profileId: "conduitos-native", releaseName: "x86_64-pc", firmware: "OVMF_CODE.fd", bootEntry: "BOOTX64.EFI",
  presenter: "presenter/native-graphical@1",
  bounds: { static_memory_bytes: 8_388_608, heap_arena_bytes: 16_777_216, queue_items: 1_024, buffered_bytes: 4_194_304, active_instances: 128, operation_slots: 64, timer_slots: 32, line_sessions: 8, evidence_items: 1_024 },
});
export const CONDUITOS_AARCH64_PROFILE = product({
  id: "conduitos/aarch64/virt", label: "AArch64 virt · product Host", architecture: "aarch64", machine: "virt",
  profileId: "conduitos-aarch64-headless", releaseName: "aarch64-virt", firmware: "QEMU_EFI.fd", bootEntry: "BOOTAA64.EFI",
  presenter: "presenter/linear-serial@1",
  bounds: { static_memory_bytes: 8_388_608, heap_arena_bytes: 0, queue_items: 64, buffered_bytes: 16_384, active_instances: 16, operation_slots: 8, timer_slots: 4, line_sessions: 1, evidence_items: 64 },
});
export const CONDUITOS_IA32_PROFILE = product({
  id: "conduitos/ia32/pc", label: "IA-32 PC · product Host", architecture: "ia32", machine: "pc",
  profileId: "conduitos-ia32-headless", releaseName: "ia32-pc", firmware: "OVMF_IA32_CODE.fd", bootEntry: "BOOTIA32.EFI",
  presenter: "presenter/ia32-linear-debugcon@1",
  bounds: { static_memory_bytes: 1_048_576, heap_arena_bytes: 0, queue_items: 64, buffered_bytes: 16_384, active_instances: 16, operation_slots: 8, timer_slots: 4, line_sessions: 1, evidence_items: 64 },
});
export const CONDUITOS_RISCV64_PROFILE = product({
  id: "conduitos/riscv64/virt", label: "RISC-V64 virt · product Host", architecture: "riscv64", machine: "virt",
  profileId: "conduitos-riscv64-headless", releaseName: "riscv64-virt", firmware: "OpenSBI+U-Boot EFI", bootEntry: "BOOTRISCV64.EFI",
  presenter: "presenter/riscv64-linear-sbi-console@1",
  bounds: { static_memory_bytes: 1_048_576, heap_arena_bytes: 0, queue_items: 64, buffered_bytes: 16_384, active_instances: 16, operation_slots: 8, timer_slots: 4, line_sessions: 1, evidence_items: 64 },
});
export const CONDUITOS_LOONGARCH64_PROFILE = product({
  id: "conduitos/loongarch64/virt", label: "LoongArch64 virt · product Host", architecture: "loongarch64", machine: "virt",
  profileId: "conduitos-loongarch64-headless", releaseName: "loongarch64-virt", firmware: "EDK2 QEMU_EFI.fd", bootEntry: "BOOTLOONGARCH64.EFI",
  presenter: "presenter/loongarch64-linear-uart@1",
  bounds: { static_memory_bytes: 1_048_576, heap_arena_bytes: 0, queue_items: 64, buffered_bytes: 16_384, active_instances: 16, operation_slots: 8, timer_slots: 4, line_sessions: 1, evidence_items: 64 },
});

const PRODUCT_PROFILES = Object.freeze([
  CONDUITOS_X86_64_PROFILE,
  CONDUITOS_AARCH64_PROFILE,
  CONDUITOS_IA32_PROFILE,
  CONDUITOS_RISCV64_PROFILE,
  CONDUITOS_LOONGARCH64_PROFILE,
]);

export const CONDUITOS_CRECHE_TARGET_CONTRIBUTIONS = Object.freeze([
  ...PRODUCT_PROFILES.map((profile) => productContribution(profile)),
]);

function productContribution(profile) {
  return Object.freeze({
    schema: "conduit.creche/physical-host-target-entry@1", family: FAMILY, target: profile.target,
    intentions: PRODUCT_MODES,
    fabrication_strategies: Object.freeze([{ id: "reviewed-generic-release-download", label: "Reviewed generic ConduitOS product IMAGE" }]),
    carriers: Object.freeze({
      deployment: Object.freeze([{ id: "conduit-carrier/downloadable-disk-image@1", label: "Download Body-bound ISO" }]),
      installation: Object.freeze([]), attachment: Object.freeze([]), observation: Object.freeze([]),
    }),
    bounds: BOUNDS,
    expected_join_contract: "conduit.conduitos/boot-attestation-before-join@1",
    target_profile: Object.freeze({
      schema: "conduit.conduitos/creche-target-profile@1", architecture: profile.architecture, machine: profile.machine,
      artifact_role: "product-host", output: "disk-image", boot_mechanism: profile.bootMechanism,
      firmware: profile.firmware, boot_entry: profile.bootEntry,
      expected_offers: Object.freeze(["conduit.host/present@1"]), expected_presenter: profile.presenter,
      bounds: profile.bounds, expected_join_behavior: "fresh Boot attestation must precede explicit authenticated admission",
      browser_role: "download Body-bound spore only", supported_carriers: profile.supportedCarriers,
      unavailable_carriers: Object.freeze(["browser-raw-disk-writer", "browser-vm-launcher", "network-boot"]),
      physical_boot_claimed: false,
    }),
    createAdapter: ({ host }) => createConduitOsAdapter({ host, profile }),
  });
}

export function createConduitOsAdapter({ host, profile, loader } = {}) {
  function createOptions() {
    const note = document.createElement("p"); note.className = "target-option-note";
    note.textContent = "Conduit downloads the exact reviewed product IMAGE and binds it into a spore. A separate explicitly authorized local disk or VM loader is required; this browser does not write disks or launch VMs.";
    return note;
  }
  async function obtain({ mode, signal }) {
    requireFabrication(profile, mode, "obtain"); requireCurrent(profile, signal, mode, "obtain");
    const release = await acquireConduitOsRelease(profile, signal);
    return Object.freeze({ resultKind: "artifact", private: release, evidence: Object.freeze({ schema: "conduit.conduitos/creche-obtainment@1", target_id: profile.target.id, result_kind: "artifact", artifact_role: "product-host", profile_id: release.manifest.profile_id, build_id: release.manifest.build_id, image_id: release.manifest.image_id, image_sha256: release.digest, image_bytes: release.bytes.byteLength, carrier: "conduit-carrier/downloadable-disk-image@1", does_not_prove: Object.freeze(["load", "boot", "join", "membership"]) }) });
  }
  async function bind({ mode, body, obtainment, nowMillis, signal }) {
    requireFabrication(profile, mode, "bind"); requireCurrent(profile, signal, mode, "bind");
    const release = obtainment?.private;
    if (!release?.bytes || release.digest !== obtainment.evidence?.image_sha256) refuse(profile, mode, "bind", "MissingArtifact", "exact ConduitOS IMAGE truth is missing before Body binding");
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(profile.target.id); const digestBytes = encoder.encode(release.digest);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy); input.set(targetBytes, entropy.length); input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(targetBytes.length, digestBytes.length, BigInt(nowMillis));
      if (code < 0) throw outputError(host.runtime, "ConduitOS spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== profile.target.id || prepared.image_content_digest !== release.digest || prepared.output !== "disk-image" || prepared.fabrication_package_id !== "conduitos-image@1" || prepared.deployment_adapter !== profile.deploymentAdapter) {
        refuse(profile, mode, "bind", "BindingIdentity", "prepared invitation lost exact ConduitOS target, IMAGE, or loader-adapter truth");
      }
      const filename = `${friendlyFilename(body?.friendly_name ?? "body")}-${profile.target.profile_id}.iso`;
      const nativeSpore = await bindBodyProvisionedMedia({ prepared, imageBytes: release.bytes, filename, format: "iso", mediaType: "application/x-iso9660-image" });
      prepared.invitation_secret.fill(0);
      const download = await createNativeSporeDownload({ prepared, bytes: nativeSpore.bytes, contentDigest: nativeSpore.content_digest, filename, format: nativeSpore.format, mediaType: nativeSpore.media_type });
      return Object.freeze({ prepared, nativeSpore, download, evidence: Object.freeze({
        ...prepared,
        invitation_secret: "embedded in native ISO; redacted",
        spore_artifact: Object.freeze({
          format: nativeSpore.format, filename, media_type: nativeSpore.media_type,
          bytes: nativeSpore.bytes.byteLength, content_digest: nativeSpore.content_digest,
          image_content_digest: nativeSpore.image_content_digest, image_bytes: nativeSpore.image_bytes,
          provision_offset: nativeSpore.provision_offset, provision_bytes: nativeSpore.provision_bytes,
        }),
      }) });
    } finally { entropy.fill(0); }
  }
  async function realize({ mode, binding, signal }) {
    requireFabrication(profile, mode, "realize"); requireCurrent(profile, signal, mode, "realize");
    let receipt;
    try { receipt = validateLoaderEvidence(loader ? await loader({ profile, binding, signal }) : null, profile, binding); }
    catch (error) { refuse(profile, mode, "realize", error?.code ?? "LoaderEvidenceInvalid", error instanceof Error ? error.message : String(error)); }
    return Object.freeze({ terminal: "ImageLoaded", evidence: Object.freeze({ schema: "conduit.conduitos/creche-image-load@1", terminal: "ImageLoaded", receipt, boot_observed: false, join_created: false }) });
  }
  async function observe({ mode }) { requireFabrication(profile, mode, "observe"); refuse(profile, mode, "observe", "PhysicalProofAbsent", "Boot and join observation remain a separate physical or VM execution proof"); }
  async function cancel({ mode, operation }) { return Object.freeze({ schema: "conduit.conduitos/creche-cancellation@1", target_id: profile.target.id, mode, operation, terminal: "Cancelled" }); }
  return Object.freeze({ schema: "conduit.creche/physical-host-target-adapter@1", target: profile.target, modes: PRODUCT_MODES, bounds: BOUNDS, createOptions, obtain, bind, realize, observe, cancel });
}

function modes(fabricate) { return Object.freeze([{ id: "fabricate-new", resultKind: "artifact", supported: fabricate }, { id: "install-existing", resultKind: "installation", supported: false }, { id: "attach-running", resultKind: "attachment", supported: false }].map(Object.freeze)); }
function requireFabrication(profile, mode, operation) { if (mode !== "fabricate-new") refuse(profile, mode, operation, "UnsupportedCombination", `ConduitOS product target does not offer ${mode}`); }
function requireCurrent(profile, signal, mode, operation) { if (signal?.aborted) refuse(profile, mode, operation, "Cancelled", "ConduitOS operation was cancelled"); }
function refuse(profile, mode, operation, terminal, message) { const error = new Error(message); error.code = terminal; error.evidence = Object.freeze({ schema: "conduit.conduitos/creche-operation-refusal@1", target_id: profile.target.id, mode, operation, terminal, message, browser_device_authority_requested: false, external_work_started: false }); throw error; }
function readOutput(api) { return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len()))); }
function outputError(api, operation, code) { const output = readOutput(api); const error = new Error(`${operation} refused (${code}): ${output.message ?? "unknown refusal"}`); error.code = "RuntimeRefusal"; error.output = output; return error; }
function friendlyFilename(value) { const normalized = value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); return normalized.slice(0, 80) || "body"; }
