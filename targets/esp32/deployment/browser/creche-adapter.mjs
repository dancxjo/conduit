import { createBrowserDeviceBase } from "../../../device-base.mjs";
import {
  PHYSICAL_SPAWN_STREAM_BOUNDS,
  requestPhysicalSpawnJoin,
} from "../../rp2040/browser-deployment/spawn.mjs";
import { createEsp32BrowserDeploymentAdapter, ESP32_BROWSER_DEPLOYMENT } from "./deployment.mjs";
import { bindEsp32BodySpore, parseEsp32Image } from "./image.mjs";
import { createNativeSporeDownload } from "../../../creche-spore-bundle.mjs";

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
  maximumArtifactBytes: ESP32_BROWSER_DEPLOYMENT.imageBounds.maximumImageBytes,
});
const FAMILY = Object.freeze({ id: "conduit-target-family/esp32@1", label: "ESP32 boards" });

const PROFILES = Object.freeze([
  profile({
    id: "esp32/riscv32imc/usb-dcf8355d-esp32-c3",
    label: "ESP32-C3",
    modelId: "espressif/esp32-c3@1",
    profileId: "usb-dcf8355d-esp32-c3",
    releaseName: "c3",
    chip: "esp32c3",
    chipId: 5,
    architecture: "riscv32imc",
    flashBytes: 4 * 1024 * 1024,
    imageOffset: 0,
    resetStrategy: "usb-jtag",
    usb: [0x10c4, 0xea60],
    transport: "ROM UART0 via CP2102N at 115200 8N1",
  }),
  profile({
    id: "esp32/xtensa-lx7/usb-54e2006398-esp32-s3",
    label: "ESP32-S3",
    modelId: "espressif/esp32-s3@1",
    profileId: "usb-54e2006398-esp32-s3",
    releaseName: "s3",
    chip: "esp32s3",
    chipId: 9,
    architecture: "xtensa-lx7",
    flashBytes: 16 * 1024 * 1024,
    imageOffset: 0,
    resetStrategy: "usb-jtag",
    usb: [0x1a86, 0x55d3],
    transport: "ROM UART0 via CH343 at 115200 8N1",
  }),
  profile({
    id: "esp32/xtensa-lx6/hw-463-esp-wroom-32",
    label: "ESP32-WROOM-32 · HW-463",
    modelId: "espressif/esp-wroom-32@1",
    profileId: "hw-463-esp-wroom-32",
    releaseName: "wroom",
    chip: "esp32",
    chipId: 0,
    architecture: "xtensa-lx6",
    flashBytes: 4 * 1024 * 1024,
    imageOffset: 0x1000,
    resetStrategy: "classic",
    usb: [0x10c4, 0xea60],
    transport: "ROM UART0 via CP2102 at 115200 8N1",
  }),
]);

export const ESP32_CRECHE_TARGET_CONTRIBUTIONS = Object.freeze(PROFILES.map((targetProfile) => Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: targetProfile.target,
  intentions: MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed generic release IMAGE" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([{ id: "conduit-carrier/browser-web-serial-rom@1", label: "Browser-attended ESP ROM loader" }]),
    installation: Object.freeze([]),
    attachment: Object.freeze([]),
    observation: Object.freeze([{ id: "conduit-carrier/browser-serial-spawn@1", label: "Browser-attended spawn observation" }]),
  }),
  bounds: BOUNDS,
  expected_join_contract: "conduit.esp32/browser-spawn-observation@1",
  target_profile: targetProfile.declaration,
  createAdapter: ({ host }) => createEsp32CrecheTargetAdapter({ host, targetProfile }),
})));

export function createEsp32CrecheTargetAdapter({ host, targetProfile, acquireSerial, observeSpawn }) {
  if (!PROFILES.includes(targetProfile)) throw new TypeError("ESP32 Crèche adapter requires one package-owned exact profile");
  let activeBase = null;
  let activeDeployment = null;

  function createOptions({ mode, onChange }) {
    const note = document.createElement("p");
    note.className = "target-option-note";
    if (mode !== "fabricate-new") {
      note.textContent = "This target does not install or attach existing machinery in this adapter.";
      return note;
    }
    note.textContent = "The reviewed generic release IMAGE is downloaded first; Body binding then produces a downloadable spore. Browser flashing is optional.";
    return note;
  }

  async function obtain({ mode, signal }) {
    requireMode(mode, "obtain", targetProfile);
    requireCurrent(signal, mode, "obtain", targetProfile);
    try {
      const release = await downloadRelease(targetProfile, signal);
      requireCurrent(signal, mode, "obtain", targetProfile);
      const parsed = await parseEsp32Image({
        targetId: targetProfile.target.id,
        segments: release.segments,
        maximumTransferBytes: ESP32_BROWSER_DEPLOYMENT.maximumTransferBytes,
      });
      return Object.freeze({
        resultKind: "artifact",
        private: Object.freeze({ segments: parsed.segments, imageDigest: parsed.contentId }),
        evidence: Object.freeze({
          schema: "conduit.esp32/creche-obtainment@1",
          target_id: targetProfile.target.id,
          result_kind: "artifact",
          artifact: Object.freeze({
            image_id: release.manifest.image_id,
            content_digest: parsed.contentId,
            bytes: parsed.totalBytes,
            segments: parsed.segments.length,
            layout: release.manifest.artifact_layout,
            release_source_identity: release.manifest.source_identity,
            release_artifact_sha256: release.manifest.artifact_sha256,
          }),
          fabrication_strategy: "reviewed-generic-release-download",
          does_not_prove: Object.freeze(["invitation", "deployment", "boot", "join", "membership", "offers"]),
        }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(targetProfile, mode, "obtain", error?.code ?? "FabricationFailed", "ESP32 IMAGE validation terminated without success", error);
    }
  }

  async function bind({ mode, body, obtainment, nowMillis, signal }) {
    requireMode(mode, "bind", targetProfile);
    requireCurrent(signal, mode, "bind", targetProfile);
    const digest = obtainment?.private?.imageDigest;
    if (typeof digest !== "string" || !Array.isArray(obtainment?.private?.segments)) {
      refuse(targetProfile, mode, "bind", "MissingArtifact", "exact ESP32 artifact truth is missing before invitation binding");
    }
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const targetBytes = encoder.encode(targetProfile.target.id);
    const digestBytes = encoder.encode(digest);
    try {
      const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), entropy.length + targetBytes.length + digestBytes.length);
      input.set(entropy);
      input.set(targetBytes, entropy.length);
      input.set(digestBytes, entropy.length + targetBytes.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore_for_target(
        targetBytes.length,
        digestBytes.length,
        BigInt(nowMillis),
      );
      if (code < 0) throw outputError(host.runtime, "ESP32 spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.target_id !== targetProfile.target.id || prepared.image_content_digest !== digest
        || prepared.output !== "esp32-image" || prepared.fabrication_package_id !== "conduit-host-esp32@1") {
        refuse(targetProfile, mode, "bind", "BindingIdentity", "prepared invitation lost the selected ESP32 target or artifact identity");
      }
      requireCurrent(signal, mode, "bind", targetProfile);
      const nativeSpore = await bindEsp32BodySpore({
        targetId: targetProfile.target.id,
        segments: obtainment.private.segments,
        prepared,
      });
      const filename = `${friendlyFilename(body?.friendly_name ?? "body")}-${targetProfile.releaseName}.bin`;
      const download = await createNativeSporeDownload({
        prepared,
        bytes: nativeSpore.bytes,
        contentDigest: nativeSpore.content_id,
        filename,
        format: "ESP32 image",
      });
      return Object.freeze({
        prepared,
        nativeSpore,
        download,
        evidence: Object.freeze({
          ...prepared,
          invitation_secret: "embedded in native ESP32 image; redacted",
          spore_artifact: Object.freeze({
            format: nativeSpore.format,
            filename,
            bytes: nativeSpore.bytes.byteLength,
            content_digest: nativeSpore.content_id,
            deployment_content_digest: nativeSpore.deployment_content_id,
            image_content_digest: nativeSpore.image_content_id,
            bootstrap_bytes: nativeSpore.bootstrap_bytes,
            bootstrap_flash_address: nativeSpore.bootstrap_flash_address,
          }),
        }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(targetProfile, mode, "bind", error?.code ?? "BindingFailed", "ESP32 invitation binding terminated without success", error);
    } finally {
      entropy.fill(0);
    }
  }

  async function realize({ mode, host: currentHost, binding, signal }) {
    requireMode(mode, "realize", targetProfile);
    requireCurrent(signal, mode, "realize", targetProfile);
    try {
      activeBase = acquireSerial
        ? await acquireSerial({ host: currentHost, targetProfile, signal })
        : await browserSerial(currentHost, targetProfile);
      requirePort(activeBase.evidence(), targetProfile);
      activeDeployment = createEsp32BrowserDeploymentAdapter({ base: activeBase });
      const prepared = binding.prepared;
      const nativeSpore = binding.nativeSpore;
      if (!nativeSpore?.segments || nativeSpore.spore_id !== prepared.spore_id) {
        refuse(targetProfile, mode, "realize", "MissingArtifact", "exact Body-bound ESP32 Spore is missing before deployment");
      }
      const plan = await activeDeployment.sealDeployment({
        deploymentPlanId: `deployment-plan/${prepared.spore_id}`,
        deploymentOperationId: `deployment/${prepared.spore_id}`,
        targetId: prepared.target_id,
        imageId: prepared.image_id,
        imageContentId: nativeSpore.deployment_content_id,
        artifactContentId: nativeSpore.content_id,
        sporeId: prepared.spore_id,
        segments: nativeSpore.segments,
        resetStrategy: targetProfile.resetStrategy,
        explicitAction: true,
      });
      const evidence = await activeDeployment.deploy(plan);
      await activeDeployment.close();
      activeDeployment = null;
      activeBase = null;
      requireCurrent(signal, mode, "realize", targetProfile);
      return Object.freeze({ terminal: evidence.terminal, evidence });
    } catch (error) {
      await closeActive();
      if (error?.evidence) throw error;
      refuse(targetProfile, mode, "realize", error?.code ?? error?.name ?? "DeploymentFailed", "ESP32 deployment terminated without success", error);
    }
  }

  async function observe({ mode, host: currentHost, binding, signal }) {
    requireMode(mode, "observe", targetProfile);
    requireCurrent(signal, mode, "observe", targetProfile);
    try {
      activeBase = acquireSerial
        ? await acquireSerial({ host: currentHost, targetProfile, signal, observation: true })
        : await browserSerial(currentHost, targetProfile, true);
      requirePort(activeBase.evidence(), targetProfile);
      const evidence = observeSpawn
        ? await observeSpawn({ base: activeBase, prepared: binding.prepared, targetProfile, signal })
        : await requestPhysicalSpawnJoin({
          base: activeBase,
          prepared: binding.prepared,
          evidenceSchema: "conduit.esp32/browser-spawn-observation@1",
          usePlanPrefix: "esp32-spawn",
          subject: targetProfile.target.label,
        });
      activeBase = null;
      requireCurrent(signal, mode, "observe", targetProfile);
      return Object.freeze({ join: joinFrom(evidence), evidence });
    } catch (error) {
      await closeActive();
      if (error?.evidence) throw error;
      refuse(targetProfile, mode, "observe", error?.code ?? error?.name ?? "ObservationFailed", "ESP32 Boot/join observation terminated without success", error);
    }
  }

  async function closeActive() {
    const deployment = activeDeployment;
    const base = activeBase;
    activeDeployment = null;
    activeBase = null;
    try { if (deployment) await deployment.close(); else if (base?.close) await base.close(); } catch {}
  }

  async function cancel({ mode, operation }) {
    await closeActive();
    return Object.freeze({ schema: "conduit.esp32/creche-cancellation@1", target_id: targetProfile.target.id, mode, operation, terminal: "Cancelled" });
  }

  return Object.freeze({ schema: ADAPTER_SCHEMA, target: targetProfile.target, modes: MODES, bounds: BOUNDS, createOptions, obtain, bind, realize, observe, cancel });
}

function profile(values) {
  const target = Object.freeze({ id: values.id, label: values.label, model_id: values.modelId, profile_id: values.profileId });
  return Object.freeze({
    ...values,
    artifactManifest: new URL(
      `../../../artifacts/esp32-${values.releaseName}-generic-release.json`,
      import.meta.url,
    ).href,
    target,
    declaration: Object.freeze({
      schema: "conduit.esp32/creche-target-profile@1",
      chip: values.chip,
      chip_id: values.chipId,
      architecture: values.architecture,
      artifact_layout: Object.freeze({ format: "espressif-merged-image", flash_offset: 0, maximum_bytes: ESP32_BROWSER_DEPLOYMENT.imageBounds.maximumImageBytes }),
      flash: Object.freeze({
        bytes: values.flashBytes,
        transfer_block_bytes: ESP32_BROWSER_DEPLOYMENT.imageBounds.flashBlockBytes,
        spore_region: Object.freeze({ start: 4 * 1024 * 1024 - 4096, bytes: 4096, body_bound: true }),
      }),
      fabrication_strategy: "reviewed generic release IMAGE download, then Body binding into one merged ESP32 flash image",
      browser_transport: values.transport,
      reset_strategy: values.resetStrategy,
      rom_loader: "Espressif serial ROM loader with exact chip observation and MD5 verification",
      expected_post_flash_join: "bounded serial spawn protocol 2",
    }),
  });
}

async function downloadRelease(targetProfile, signal) {
  let response;
  try {
    response = await fetch(targetProfile.artifactManifest, { signal, cache: "no-store" });
  } catch (error) {
    refuse(targetProfile, "fabricate-new", "obtain", "ArtifactUnavailable", "generic ESP32 release manifest is unavailable", error, {
      authority_requested: false,
      artifact_work_started: false,
    });
  }
  if (!response.ok) {
    refuse(targetProfile, "fabricate-new", "obtain", "ArtifactUnavailable", `generic ESP32 release manifest returned HTTP ${response.status}`, undefined, {
      authority_requested: false,
      artifact_work_started: false,
    });
  }
  const manifest = await response.json();
  if (manifest?.schema !== "conduit.release/target-artifact@1"
    || manifest.target_id !== targetProfile.target.id
    || typeof manifest.source_identity !== "string"
    || typeof manifest.image_id !== "string"
    || !Array.isArray(manifest.segments) || manifest.segments.length < 1 || manifest.segments.length > 8) {
    refuse(targetProfile, "fabricate-new", "obtain", "StaleArtifact", "generic ESP32 release manifest does not match the exact target profile");
  }
  const segments = [];
  let total = 0;
  for (const segment of manifest.segments) {
    if (!Number.isSafeInteger(segment.offset) || segment.offset < 0 || typeof segment.path !== "string") {
      refuse(targetProfile, "fabricate-new", "obtain", "StaleArtifact", "generic ESP32 release segment layout is malformed");
    }
    const artifactResponse = await fetch(new URL(segment.path, response.url), { signal, cache: "no-store" });
    if (!artifactResponse.ok) refuse(targetProfile, "fabricate-new", "obtain", "ArtifactUnavailable", `generic ESP32 release artifact returned HTTP ${artifactResponse.status}`);
    const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
    total += bytes.byteLength;
    if (bytes.byteLength !== segment.bytes || total > BOUNDS.maximumArtifactBytes) {
      refuse(targetProfile, "fabricate-new", "obtain", "ArtifactBound", "generic ESP32 release artifact violated its sealed byte bounds");
    }
    segments.push(Object.freeze({ offset: segment.offset, bytes }));
  }
  const raw = new Uint8Array(total);
  let cursor = 0;
  for (const segment of segments) { raw.set(segment.bytes, cursor); cursor += segment.bytes.byteLength; }
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", raw));
  const rawDigest = `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  if (manifest.artifact_sha256 !== rawDigest || manifest.bytes !== total) {
    refuse(targetProfile, "fabricate-new", "obtain", "StaleArtifact", "generic ESP32 release artifact identity does not match its reviewed manifest");
  }
  return Object.freeze({ manifest: Object.freeze(manifest), segments: Object.freeze(segments) });
}

async function browserSerial(host, targetProfile, observation = false) {
  const devices = createBrowserDeviceBase({
    api: host.runtime,
    hostId: host.hostId,
    bootId: host.bootId,
    selectedImplementations: ["browser/webserial@1"],
    status: null,
    output: null,
  });
  return devices.acquireSerial({
    baudRate: ESP32_BROWSER_DEPLOYMENT.baudRate,
    dataBits: ESP32_BROWSER_DEPLOYMENT.dataBits,
    stopBits: ESP32_BROWSER_DEPLOYMENT.stopBits,
    parity: ESP32_BROWSER_DEPLOYMENT.parity,
    bufferSize: ESP32_BROWSER_DEPLOYMENT.bufferSize,
    maximumTransferBytes: observation
      ? PHYSICAL_SPAWN_STREAM_BOUNDS.maximumTransferBytes
      : ESP32_BROWSER_DEPLOYMENT.maximumTransferBytes,
    maximumReads: observation ? PHYSICAL_SPAWN_STREAM_BOUNDS.maximumReads : ESP32_BROWSER_DEPLOYMENT.maximumReads,
    maximumWrites: observation ? PHYSICAL_SPAWN_STREAM_BOUNDS.maximumWrites : ESP32_BROWSER_DEPLOYMENT.maximumWrites,
    maximumSignalOperations: observation
      ? PHYSICAL_SPAWN_STREAM_BOUNDS.maximumSignalOperations
      : ESP32_BROWSER_DEPLOYMENT.maximumSignalOperations,
  });
}

function requirePort(evidence, targetProfile) {
  const [vendor, product] = targetProfile.usb;
  if (evidence?.usb_vendor_id !== vendor || evidence.usb_product_id !== product) {
    const error = new Error("selected Web Serial port does not match the exact ESP32 profile");
    error.code = "WrongPort";
    throw error;
  }
}

function requireMode(mode, operation, targetProfile) {
  if (mode === "fabricate-new") return;
  refuse(targetProfile, mode, operation, mode === "install-existing" ? "InstallExistingUnsupported" : "AttachRunningUnsupported", `ESP32 target adapter does not offer ${mode}`);
}

function requireCurrent(signal, mode, operation, targetProfile) {
  if (signal?.aborted) refuse(targetProfile, mode, operation, "Cancelled", "ESP32 target operation was cancelled");
}

function joinFrom(evidence) {
  return Object.freeze({
    spore_id: evidence.spore_id, image_id: evidence.image_id, advertisement: evidence.advertisement,
    invitation_id: evidence.invitation_id, body_id: evidence.body_id, host_id: evidence.host_id,
    boot_id: evidence.boot_id, nonce: evidence.nonce, signature: evidence.signature,
    observed_at_millis: evidence.observed_at_millis,
  });
}

function friendlyFilename(value) {
  const stem = String(value).normalize("NFKD").replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
  return stem || "body";
}

function refuse(targetProfile, mode, operation, code, message, cause = undefined, details = {}) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.name = "Esp32CrecheTargetRefusal";
  error.code = code;
  error.evidence = Object.freeze({ schema: "conduit.esp32/creche-operation-refusal@1", target_id: targetProfile.target.id, mode, operation, terminal: code, message, ...details });
  throw error;
}

function readOutput(api) {
  return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len())));
}

function outputError(api, operation, code) {
  const output = readOutput(api);
  const error = new Error(`${operation} refused (${code}): ${output.message ?? "unknown refusal"}`);
  error.code = "RuntimeRefusal";
  error.output = output;
  return error;
}
