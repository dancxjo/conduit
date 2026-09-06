import { createBrowserDeviceBase } from "../../../device-base.mjs";
import { createBrowserUsbDeviceBase } from "../../../usb-device-base.mjs";
import { createRp2040BrowserDeploymentAdapter, RP2040_BROWSER_DEPLOYMENT } from "./deployment.mjs";
import {
  bindRp2040BodySpore,
  createRp2040BrowserFabricationAdapter,
  RP2040_BROWSER_FABRICATION,
} from "./fabrication.mjs";
import { PHYSICAL_SPAWN_STREAM_BOUNDS, requestRp2040SpawnJoin } from "./spawn.mjs";
import { createNativeSporeDownload } from "../../../creche-spore-bundle.mjs";

const ADAPTER_SCHEMA = "conduit.creche/physical-host-target-adapter@1";
const TARGET_ID = RP2040_BROWSER_DEPLOYMENT.targetId;
const PACKAGED_MANIFEST_PATH = new URL(
  "../../../artifacts/pico-w-signal-pico-local.json",
  import.meta.url,
).href;
const BUILD_ID = "conduit-pico-w-signal:4ccd179a7ddf32c17ba8b7f948a1f528e6cf8d78:thumbv6m-none-eabi:release:pico-local";
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const TARGET = Object.freeze({
  id: TARGET_ID,
  label: "Raspberry Pi Pico W · RP2040",
  model_id: "raspberry-pi/pico-w@1",
  profile_id: "pico-local",
});
const MODES = Object.freeze([
  Object.freeze({ id: "fabricate-new", resultKind: "artifact", supported: true }),
  Object.freeze({ id: "install-existing", resultKind: "installation", supported: false }),
  Object.freeze({ id: "attach-running", resultKind: "attachment", supported: false }),
]);
const BOUNDS = Object.freeze({
  maximumOperations: 16,
  maximumOperationEvidenceBytes: 32 * 1024,
  maximumRetainedEvidenceBytes: 128 * 1024,
  maximumArtifactBytes: RP2040_BROWSER_FABRICATION.maximumArtifactBytes,
});

export const RP2040_CRECHE_TARGET_CONTRIBUTION = Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: Object.freeze({ id: "conduit-target-family/rp2040@1", label: "RP2040 boards" }),
  target: TARGET,
  intentions: MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "packaged-exact", label: "Reviewed packaged IMAGE" }),
    Object.freeze({ id: "template-specialized", label: "Reviewed template + bounded Body label" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-picoboot@1", label: "Browser-attended Picoboot deployment" }),
    ]),
    installation: Object.freeze([]),
    attachment: Object.freeze([]),
    observation: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-serial-spawn@1", label: "Browser-attended spawn observation" }),
    ]),
  }),
  bounds: BOUNDS,
  expected_join_contract: "conduit.rp2040/browser-spawn-observation@1",
  target_profile: Object.freeze({
    schema: "conduit.rp2040/creche-target-profile@1",
    chip: "RP2040",
    artifact_layout: "UF2 family RP2040; 512-byte blocks",
    fabrication_strategy: "reviewed packaged IMAGE or bounded template specialization",
    browser_transport: "WebUSB Picoboot",
    loader_behavior: "BOOTSEL Picoboot command protocol then reboot",
    expected_post_flash_join: "bounded Web Serial spawn protocol 2",
  }),
  createAdapter: createRp2040CrecheTargetAdapter,
});

export function createRp2040CrecheTargetAdapter({ host }) {
  let strategy = "packaged-exact";
  let activeDeployment = null;
  let activeBase = null;

  const descriptor = Object.freeze({
    schema: ADAPTER_SCHEMA,
    target: TARGET,
    modes: MODES,
    bounds: BOUNDS,
  });

  function createOptions({ mode, onChange }) {
    if (mode !== "fabricate-new") {
      const explanation = document.createElement("p");
      explanation.className = "target-option-note";
      explanation.textContent = mode === "install-existing"
        ? "This target adapter does not install onto an existing computer."
        : "This target adapter does not attach an already running Host without a prepared invitation.";
      return explanation;
    }
    const label = document.createElement("label");
    label.textContent = "Fabrication strategy";
    const wrapper = document.createElement("span");
    wrapper.className = "select-field";
    const select = document.createElement("select");
    select.className = "fabrication-strategy";
    for (const [value, text] of [
      ["packaged-exact", "Reviewed packaged IMAGE"],
      ["template-specialized", "Reviewed template + bounded Body label"],
    ]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = text;
      option.selected = value === strategy;
      select.append(option);
    }
    select.addEventListener("change", () => { strategy = select.value; onChange(); });
    wrapper.append(select);
    label.append(wrapper);
    return label;
  }

  async function obtain({ mode, body, signal }) {
    requireMode(mode, "obtain");
    requireCurrent(signal, mode, "obtain");
    try {
      const fabrication = await createRp2040BrowserFabricationAdapter().fabricate({
        strategy,
        selection: {
          targetId: "conduit-target/rp2040-pico-w@1",
          profileId: "pico-local",
          buildId: BUILD_ID,
          imageId: "conduit-image/pico-w-signal-b7@1",
          manifestPath: PACKAGED_MANIFEST_PATH,
        },
        configuration: strategy === "template-specialized" ? { body_label: body?.friendly_name ?? "unborn" } : {},
      });
      requireCurrent(signal, mode, "obtain");
      return Object.freeze({
        resultKind: "artifact",
        private: Object.freeze({ imageBytes: fabrication.bytes, imageDigest: fabrication.content_id }),
        evidence: Object.freeze({
          schema: "conduit.rp2040/creche-obtainment@1",
          mode,
          result_kind: "artifact",
          target_id: TARGET_ID,
          artifact: Object.freeze({
            ...fabrication.provenance,
            content_digest: fabrication.content_id,
            bytes: fabrication.bytes.length,
          }),
          fabrication: Object.freeze({
            ...fabrication,
            bytes: `${fabrication.bytes.length} bytes retained outside evidence`,
          }),
          does_not_prove: Object.freeze(["invitation", "deployment", "boot", "join", "membership", "offers"]),
        }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(mode, "obtain", error?.code ?? "FabricationFailed", "RP2040 artifact fabrication terminated without success", error);
    }
  }

  async function bind({ mode, body, obtainment, nowMillis, signal }) {
    requireMode(mode, "bind");
    requireCurrent(signal, mode, "bind");
    const digest = obtainment?.private?.imageDigest;
    if (typeof digest !== "string" || !obtainment.private.imageBytes) {
      refuse(mode, "bind", "MissingArtifact", "exact RP2040 artifact truth is missing before invitation binding");
    }
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const digestBytes = encoder.encode(digest);
    try {
      const input = new Uint8Array(
        host.runtime.memory.buffer,
        host.runtime.conduit_creche_input_ptr(),
        entropy.length + digestBytes.length,
      );
      input.set(entropy);
      input.set(digestBytes, entropy.length);
      const code = host.runtime.conduit_creche_prepare_selected_physical_spore(
        digestBytes.length,
        BigInt(nowMillis),
      );
      if (code < 0) throw outputError(host.runtime, "spore preparation", code);
      const prepared = readOutput(host.runtime);
      if (prepared.image_content_digest !== digest || prepared.target_id !== TARGET_ID) {
        refuse(mode, "bind", "BindingIdentity", "prepared invitation lost the selected RP2040 target or artifact identity");
      }
      requireCurrent(signal, mode, "bind");
      const nativeSpore = await bindRp2040BodySpore(obtainment.private.imageBytes, prepared);
      prepared.invitation_secret.fill(0);
      const filename = `${friendlyFilename(body?.friendly_name ?? "body")}-pico-w.uf2`;
      const download = await createNativeSporeDownload({
        prepared,
        bytes: nativeSpore.bytes,
        contentDigest: nativeSpore.content_id,
        filename,
        format: "uf2",
        mediaType: "application/x-uf2",
      });
      return Object.freeze({
        prepared,
        nativeSpore,
        download,
        evidence: Object.freeze({
          ...prepared,
          invitation_secret: "embedded in native UF2; redacted",
          spore_artifact: Object.freeze({
            format: nativeSpore.format,
            filename,
            bytes: nativeSpore.bytes.byteLength,
            content_digest: nativeSpore.content_id,
            image_content_digest: nativeSpore.image_content_id,
            bootstrap_bytes: nativeSpore.bootstrap_bytes,
            bootstrap_flash_address: nativeSpore.bootstrap_flash_address,
          }),
        }),
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(mode, "bind", error?.code ?? "BindingFailed", "RP2040 invitation binding terminated without success", error);
    } finally {
      entropy.fill(0);
    }
  }

  async function realize({ mode, obtainment, binding, signal }) {
    requireMode(mode, "realize");
    requireCurrent(signal, mode, "realize");
    let deployment = null;
    try {
      const usb = createBrowserUsbDeviceBase({
        api: host.runtime,
        hostId: host.hostId,
        bootId: host.bootId,
        selectedImplementations: ["browser/webusb@1"],
        status: null,
        output: null,
      });
      activeBase = await usb.acquireUsb({
        configurationValue: RP2040_BROWSER_DEPLOYMENT.configurationValue,
        interfaceNumber: RP2040_BROWSER_DEPLOYMENT.interfaceNumber,
        alternateSetting: RP2040_BROWSER_DEPLOYMENT.alternateSetting,
        inEndpoint: RP2040_BROWSER_DEPLOYMENT.inEndpoint,
        outEndpoint: RP2040_BROWSER_DEPLOYMENT.outEndpoint,
        maximumTransferBytes: RP2040_BROWSER_DEPLOYMENT.maximumTransferBytes,
        maximumInTransfers: RP2040_BROWSER_DEPLOYMENT.maximumInTransfers,
        maximumOutTransfers: RP2040_BROWSER_DEPLOYMENT.maximumOutTransfers,
      });
      deployment = createRp2040BrowserDeploymentAdapter({ base: activeBase });
      activeDeployment = deployment;
      const prepared = binding.prepared;
      const plan = await deployment.sealDeployment({
        deploymentPlanId: `deployment-plan/${prepared.spore_id}`,
        deploymentOperationId: `deployment/${prepared.spore_id}`,
        targetId: prepared.target_id,
        sporeId: prepared.spore_id,
        imageId: prepared.image_id,
        imageContentId: prepared.image_content_digest,
        sporeContentId: binding.nativeSpore.content_id,
        imageBytes: binding.nativeSpore.bytes,
        explicitAction: true,
      });
      const evidence = await deployment.deploy(plan, { signal });
      requireCurrent(signal, mode, "realize");
      activeDeployment = null;
      activeBase = null;
      return Object.freeze({ terminal: evidence.terminal, evidence });
    } catch (error) {
      const targetEvidence = deployment ? {
        ...deployment.evidence(),
        failure_chain: errorChain(error),
      } : null;
      const permissionHint = /access denied/i.test(error?.message ?? "")
        ? " On Linux, run `sudo targets/rp2040/tools/install-pico-headless-flash.sh` and reconnect the Pico in BOOTSEL mode."
        : "";
      refuse(
        mode,
        "realize",
        error?.code ?? error?.name ?? "DeploymentFailed",
        `RP2040 deployment terminated without success.${permissionHint} This USB acquisition is terminal; use a fresh browser Host before trying again.`,
        error,
        targetEvidence,
      );
    }
  }

  async function observe({ mode, binding, signal }) {
    requireMode(mode, "observe");
    requireCurrent(signal, mode, "observe");
    try {
      const devices = createBrowserDeviceBase({
        api: host.runtime,
        hostId: host.hostId,
        bootId: host.bootId,
        selectedImplementations: ["browser/webserial@1"],
        status: null,
        output: null,
      });
      activeBase = await devices.acquireSerial({
        maximumTransferBytes: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumTransferBytes,
        maximumReads: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumReads,
        maximumWrites: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumWrites,
        maximumSignalOperations: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumSignalOperations,
      });
      const evidence = await requestRp2040SpawnJoin({ base: activeBase, prepared: binding.prepared });
      requireCurrent(signal, mode, "observe");
      activeBase = null;
      return Object.freeze({
        join: Object.freeze({
          spore_id: evidence.spore_id,
          image_id: evidence.image_id,
          advertisement: evidence.advertisement,
          invitation_id: evidence.invitation_id,
          body_id: evidence.body_id,
          host_id: evidence.host_id,
          boot_id: evidence.boot_id,
          nonce: evidence.nonce,
          signature: evidence.signature,
          observed_at_millis: evidence.observed_at_millis,
        }),
        evidence,
      });
    } catch (error) {
      if (error?.evidence) throw error;
      refuse(mode, "observe", error?.code ?? error?.name ?? "ObservationFailed", "RP2040 Boot/join observation terminated without success", error);
    }
  }

  async function cancel({ mode, operation }) {
    const deployment = activeDeployment;
    const base = activeBase;
    activeDeployment = null;
    activeBase = null;
    try {
      if (deployment) await deployment.close();
      else if (base?.close) await base.close();
    } catch (error) {
      return Object.freeze({
        schema: "conduit.rp2040/creche-cancellation@1",
        target_id: TARGET_ID,
        mode,
        operation,
        terminal: "CloseFailed",
        message: error instanceof Error ? error.message : String(error),
      });
    }
    return Object.freeze({
      schema: "conduit.rp2040/creche-cancellation@1",
      target_id: TARGET_ID,
      mode,
      operation,
      terminal: "Cancelled",
    });
  }

  return Object.freeze({ ...descriptor, createOptions, obtain, bind, realize, observe, cancel });
}

function friendlyFilename(value) {
  const name = String(value).normalize("NFKD").toLowerCase()
    .replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 64);
  return name || "body";
}

function requireMode(mode, operation) {
  if (mode === "fabricate-new") return;
  const code = mode === "install-existing" ? "InstallExistingUnsupported" : "AttachRunningUnsupported";
  const resultKind = mode === "install-existing" ? "installation" : "attachment";
  refuse(
    mode,
    operation,
    code,
    `RP2040 target adapter does not offer ${mode}`,
    undefined,
    Object.freeze({
      schema: "conduit.rp2040/creche-unsupported-intention@1",
      target_id: TARGET_ID,
      mode,
      result_kind: resultKind,
      supported: false,
    }),
  );
}

function requireCurrent(signal, mode, operation) {
  if (signal?.aborted) refuse(mode, operation, "Cancelled", "RP2040 target operation was cancelled");
}

function refuse(mode, operation, code, message, cause = undefined, targetEvidence = null) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.name = "Rp2040CrecheTargetRefusal";
  error.code = code;
  error.evidence = Object.freeze({
    schema: "conduit.rp2040/creche-operation-refusal@1",
    target_id: TARGET_ID,
    mode,
    operation,
    terminal: code,
    message,
    failure_chain: Object.freeze(errorChain(error)),
    target_evidence: targetEvidence,
  });
  throw error;
}

function errorChain(error) {
  const messages = [];
  for (let current = error; current && messages.length < 4; current = current.cause) {
    const code = typeof current.code === "string" ? `${current.code}: ` : "";
    messages.push(`${code}${current instanceof Error ? current.message : String(current)}`);
  }
  return messages;
}

function readOutput(api) {
  return JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer,
    api.conduit_creche_output_ptr(),
    api.conduit_creche_output_len(),
  )));
}

function outputError(api, operation, code) {
  const evidence = api.conduit_creche_output_len() > 0 ? readOutput(api) : null;
  return Object.assign(new Error(evidence?.message ?? `${operation} refused (${code})`), { code: "BodyRefusal" });
}
