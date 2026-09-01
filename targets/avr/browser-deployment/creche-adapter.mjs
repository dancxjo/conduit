import { createNativeSporeDownload } from "../../../creche-spore-bundle.mjs";
import { acquireProMicroRelease, bindAvrBodySpore, validateExternalProgrammerEvidence } from "./image.mjs";

const ADAPTER_SCHEMA = "conduit.creche/physical-host-target-adapter@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const MODES = Object.freeze([
  Object.freeze({ id: "fabricate-new", resultKind: "artifact", supported: true }),
  Object.freeze({ id: "install-existing", resultKind: "installation", supported: false }),
  Object.freeze({ id: "attach-running", resultKind: "attachment", supported: false }),
]);
const BOUNDS = Object.freeze({
  maximumOperations: 16,
  maximumOperationEvidenceBytes: 32 * 1024,
  maximumRetainedEvidenceBytes: 128 * 1024,
});
const FAMILY = Object.freeze({
  id: "conduit-target-family/sparkfun-pro-micro@1",
  label: "SparkFun Pro Micro",
});

export const AVR_PRO_MICRO_PROFILE = Object.freeze({
  target: Object.freeze({
    id: "avr/avr5/sparkfun-pro-micro-atmega32u4-5v-16mhz",
    label: "Pro Micro · ATmega32U4 · 5 V / 16 MHz",
    model_id: "sparkfun/pro-micro-atmega32u4@1",
    profile_id: "atmega32u4-5v-16mhz-caterina-avr109",
  }),
  manifestPath: new URL(
    "../../../artifacts/avr-promicro-atmega32u4-5v-16mhz.json",
    import.meta.url,
  ).href,
  packageId: "conduit-host-avr-promicro@1",
  builderAdapter: "conduit-host-avr/build-intel-hex@1",
  board: "sparkfun-pro-micro-5v-16mhz",
  fqbn: "SparkFun:avr:promicro:cpu=16MHzatmega32U4",
  mcu: "atmega32u4",
  clockHz: 16_000_000,
  bootloader: "caterina",
  protocol: "avr109",
  resetTransition: "1200-baud-touch-then-fresh-port",
});

const declaration = Object.freeze({
  schema: "conduit.avr/creche-target-profile@1",
  board: AVR_PRO_MICRO_PROFILE.board,
  fqbn: AVR_PRO_MICRO_PROFILE.fqbn,
  mcu: AVR_PRO_MICRO_PROFILE.mcu,
  clock_hz: AVR_PRO_MICRO_PROFILE.clockHz,
  voltage_mv: 5_000,
  flash: Object.freeze({
    total_bytes: 32_768,
    application_bytes: 28_672,
    boot_region: Object.freeze({ start: 28_672, bytes: 4_096, protected: true }),
    spore_region: Object.freeze({ start: 27_648, bytes: 1_024, body_bound: true }),
  }),
  sram_bytes: 2_560,
  artifact_format: "intel-hex",
  bootloader: Object.freeze({
    name: AVR_PRO_MICRO_PROFILE.bootloader,
    protocol: AVR_PRO_MICRO_PROFILE.protocol,
    reset_transition: AVR_PRO_MICRO_PROFILE.resetTransition,
  }),
  browser_deployment: "unavailable: exact Caterina reset and fresh-port contract is not implemented",
  external_carrier: "download Body-bound spore for an explicit external programmer",
  join_behavior: "fresh Boot and exact ATTEST must be observed before a later explicit authenticated admission",
  authenticated_join_implemented: false,
});

export const AVR_PRO_MICRO_CRECHE_TARGET_CONTRIBUTION = Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: AVR_PRO_MICRO_PROFILE.target,
  intentions: MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed generic Pro Micro Intel HEX" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([
      Object.freeze({ id: "conduit-carrier/external-programmer-download@1", label: "Download spore for explicit external programmer" }),
    ]),
    installation: Object.freeze([]),
    attachment: Object.freeze([]),
    observation: Object.freeze([]),
  }),
  bounds: BOUNDS,
  expected_join_contract: "conduit.avr/external-cdc-attestation-before-join@1",
  target_profile: declaration,
  createAdapter: ({ host }) => createAvrProMicroCrecheAdapter({ host }),
});

export function createAvrProMicroCrecheAdapter({ host, externalProgrammer } = {}) {
  function createOptions({ mode }) {
    const note = document.createElement("p");
    note.className = "target-option-note";
    note.textContent = mode === "fabricate-new"
      ? "Conduit downloads the exact reviewed Intel HEX and binds it into a spore. Flashing requires an explicit external programmer; browser AVR109 flashing is not offered."
      : "This exact Pro Micro profile does not install or attach already-running machinery.";
    return note;
  }

  async function obtain({ mode, signal }) {
    requireMode(mode, "obtain");
    requireCurrent(signal, mode, "obtain");
    const release = await acquireProMicroRelease(AVR_PRO_MICRO_PROFILE, signal);
    return Object.freeze({
      private: release,
      resultKind: "artifact",
      evidence: Object.freeze({
        schema: "conduit.avr/creche-obtainment@1",
        target_id: AVR_PRO_MICRO_PROFILE.target.id,
        result_kind: "artifact",
        image_id: release.manifest.image_id,
        source_identity: release.manifest.source_identity,
        artifact_sha256: release.digest,
        artifact_bytes: release.bytes.byteLength,
        programmed_bytes: release.parsed.programmedBytes,
        maximum_address: release.parsed.maximumAddress,
        format: "intel-hex",
        browser_deployment_offered: false,
        does_not_prove: Object.freeze(["flash", "boot", "join", "membership", "offers"]),
      }),
    });
  }

  async function bind({ mode, body, obtainment, nowMillis, signal }) {
    requireMode(mode, "bind");
    requireCurrent(signal, mode, "bind");
    const release = obtainment?.private;
    if (!release?.bytes || typeof release.digest !== "string") refuse(mode, "bind", "MissingArtifact", "exact Pro Micro artifact truth is missing before invitation binding");
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(AVR_PRO_MICRO_PROFILE.target.id);
    const digestBytes = encoder.encode(release.digest);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy);
      input.set(targetBytes, entropy.length);
      input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(targetBytes.length, digestBytes.length, BigInt(nowMillis));
      if (code < 0) throw outputError(host.runtime, "Pro Micro spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== AVR_PRO_MICRO_PROFILE.target.id || prepared.image_content_digest !== release.digest
        || prepared.output !== "intel-hex" || prepared.fabrication_package_id !== AVR_PRO_MICRO_PROFILE.packageId
        || prepared.deployment_adapter !== null) {
        refuse(mode, "bind", "BindingIdentity", "prepared invitation lost the exact Pro Micro target, artifact, or external-carrier truth");
      }
      const nativeSpore = await bindAvrBodySpore(release.bytes, prepared);
      prepared.invitation_secret.fill(0);
      const filename = `${friendlyFilename(body?.friendly_name ?? "body")}-pro-micro.hex`;
      const download = await createNativeSporeDownload({
        prepared,
        bytes: nativeSpore.bytes,
        contentDigest: nativeSpore.content_id,
        filename,
        format: "intel-hex",
        mediaType: "application/vnd.conduit.intel-hex",
      });
      return Object.freeze({
        prepared,
        nativeSpore,
        download,
        evidence: Object.freeze({
          ...prepared,
          invitation_secret: "embedded in native HEX; redacted",
          spore_artifact: Object.freeze({
            format: nativeSpore.format,
            filename,
            bytes: nativeSpore.bytes.byteLength,
            content_digest: nativeSpore.content_id,
            image_content_digest: nativeSpore.image_content_id,
            programmed_bytes: nativeSpore.programmed_bytes,
            maximum_address: nativeSpore.maximum_address,
            bootstrap_bytes: nativeSpore.bootstrap_bytes,
            bootstrap_flash_address: nativeSpore.bootstrap_flash_address,
          }),
        }),
      });
    } finally {
      entropy.fill(0);
    }
  }

  async function realize({ mode, binding, signal }) {
    requireMode(mode, "realize");
    requireCurrent(signal, mode, "realize");
    const evidence = externalProgrammer
      ? await externalProgrammer({ profile: AVR_PRO_MICRO_PROFILE, binding, signal })
      : null;
    let receipt;
    try {
      receipt = validateExternalProgrammerEvidence(evidence, AVR_PRO_MICRO_PROFILE, binding);
    } catch (error) {
      refuse(mode, "realize", error?.code ?? "ProgrammerEvidenceInvalid", error instanceof Error ? error.message : String(error));
    }
    return Object.freeze({
      terminal: "ExternalProgrammerReceiptAccepted",
      evidence: Object.freeze({
        schema: "conduit.avr/creche-external-programmer@1",
        terminal: "ExternalProgrammerReceiptAccepted",
        receipt,
        boot_observed: false,
        join_created: false,
      }),
    });
  }

  async function observe({ mode }) {
    requireMode(mode, "observe");
    refuse(mode, "observe", "AutomatedJoinUnavailable", "this slice does not promote Pro Micro ATTEST into authenticated join truth");
  }

  async function cancel({ mode, operation }) {
    return Object.freeze({ schema: "conduit.avr/creche-cancellation@1", target_id: AVR_PRO_MICRO_PROFILE.target.id, mode, operation, terminal: "Cancelled" });
  }

  return Object.freeze({
    schema: ADAPTER_SCHEMA,
    target: AVR_PRO_MICRO_PROFILE.target,
    modes: MODES,
    bounds: BOUNDS,
    createOptions,
    obtain,
    bind,
    realize,
    observe,
    cancel,
  });
}

function requireMode(mode, operation) {
  if (mode === "fabricate-new") return;
  refuse(mode, operation, mode === "install-existing" ? "InstallExistingUnsupported" : "AttachRunningUnsupported", `Pro Micro target does not offer ${mode}`);
}

function requireCurrent(signal, mode, operation) {
  if (signal?.aborted) refuse(mode, operation, "Cancelled", "Pro Micro target operation was cancelled");
}

function refuse(mode, operation, terminal, message) {
  const error = new Error(message);
  error.code = terminal;
  error.evidence = Object.freeze({
    schema: "conduit.avr/creche-operation-refusal@1",
    target_id: AVR_PRO_MICRO_PROFILE.target.id,
    mode,
    operation,
    terminal,
    message,
    authority_requested: false,
    external_work_started: false,
  });
  throw error;
}

function readOutput(api) {
  return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len())));
}

function friendlyFilename(value) {
  const stem = String(value).normalize("NFKD").replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
  return stem || "body";
}

function outputError(api, operation, code) {
  const output = readOutput(api);
  const error = new Error(`${operation} refused (${code}): ${output.message ?? "unknown refusal"}`);
  error.code = "RuntimeRefusal";
  error.output = output;
  return error;
}
