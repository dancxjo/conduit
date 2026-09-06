import { parseEsp32Image, sha256Bytes, Esp32ImageRefusal, ESP32_IMAGE_BOUNDS } from "./image.mjs";
import { deployEsp32Rom, Esp32RomRefusal, ESP32_ROM_TARGETS, requiredEsp32Transfers } from "./rom-loader.mjs";
import { enterEsp32RomLoader, Esp32ResetRefusal, ESP32_RESET_OPERATION_COUNTS } from "./reset.mjs";

const SERIAL_RESOURCE_CLASS = "conduit.resource/web-serial-port@1";
const SERIAL_BASE_IMPLEMENTATION = "browser/web-serial@1";
const SERIAL_USE_AUTHORITY = "conduit.authority/use-web-serial@1";
const sealedPlans = new WeakMap();

export class Esp32DeploymentRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "Esp32DeploymentRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new Esp32DeploymentRefusal(code, message, cause);
}

function requireIdentity(value, name) {
  if (typeof value !== "string" || value.length === 0 || value.length > 512) {
    refuse("Identity", `${name} is missing or outside its finite identity bound`);
  }
  return value;
}

function requireBase(base) {
  const methods = ["evidence", "startUse", "read", "write", "setSignals", "close"];
  if (!base || methods.some((name) => typeof base[name] !== "function")) {
    refuse("BaseContract", "ESP32 deployment requires one admitted Web Serial Base session");
  }
}

function exactBaseTruth(base) {
  requireBase(base);
  const truth = base.evidence();
  if (
    truth?.schema !== "conduit.browser/web-serial-base-evidence@1"
    || truth.phase !== "resource-truth"
    || truth.resource_class !== SERIAL_RESOURCE_CLASS
    || truth.base_implementation_id !== SERIAL_BASE_IMPLEMENTATION
    || truth.use_authority_contract !== SERIAL_USE_AUTHORITY
    || !truth.host_id
    || !truth.boot_id
    || !truth.resource_handle
    || !truth.base_instance_id
    || !truth.use_authority_grant
  ) {
    refuse("BaseTruth", "Web Serial Base resource or use-authority truth is missing or stale");
  }
  if (
    truth.configuration?.baud_rate !== 115200
    || truth.configuration.data_bits !== 8
    || truth.configuration.stop_bits !== 1
    || truth.configuration.parity !== "None"
    || truth.transfer_bounds?.maximum_in_flight !== 1
  ) {
    refuse("SerialConfiguration", "ESP32 ROM loader requires an exact admitted 115200 8N1 Base");
  }
  if (
    truth.admitted_reads !== 0
    || truth.admitted_writes !== 0
    || truth.admitted_signal_operations !== 0
  ) {
    refuse("BaseAlreadyUsed", "ESP32 deployment requires an unused exact Base operation budget");
  }
  return truth;
}

function baseIdentity(truth) {
  return Object.freeze({
    hostId: truth.host_id,
    bootId: truth.boot_id,
    resourceHandle: truth.resource_handle,
    baseImplementationId: truth.base_implementation_id,
    baseInstanceId: truth.base_instance_id,
    useAuthorityGrant: truth.use_authority_grant,
    usbVendorId: truth.usb_vendor_id,
    usbProductId: truth.usb_product_id,
  });
}

function sameBase(current, expected) {
  return current.host_id === expected.hostId
    && current.boot_id === expected.bootId
    && current.resource_handle === expected.resourceHandle
    && current.base_implementation_id === expected.baseImplementationId
    && current.base_instance_id === expected.baseInstanceId
    && current.use_authority_grant === expected.useAuthorityGrant
    && current.usb_vendor_id === expected.usbVendorId
    && current.usb_product_id === expected.usbProductId;
}

function copy(value) {
  return JSON.parse(JSON.stringify(value));
}

export function createEsp32BrowserDeploymentAdapter({ base, cryptoApi = globalThis.crypto, wait }) {
  requireBase(base);
  let phase = "available";
  let evidence = {
    schema: "conduit.esp32/browser-deployment-evidence@1",
    phase,
    terminal: null,
    deployment_plan_id: null,
    deployment_operation_id: null,
    target_id: null,
    chip: null,
    chip_magic: null,
    image_id: null,
    image_content_id: null,
    spore_id: null,
    artifact_content_id: null,
    host_id: null,
    boot_id: null,
    resource_handle: null,
    base_implementation_id: null,
    base_instance_id: null,
    use_authority_grant: null,
    usb_vendor_id: null,
    usb_product_id: null,
    reset_strategy: null,
    admitted_signal_operations: 0,
    completed_signal_operations: 0,
    admitted_commands: 0,
    completed_commands: 0,
    admitted_image_bytes: 0,
    completed_image_bytes: 0,
    verification: "not-started",
    reboot_requested: false,
    resource_terminal: null,
    runtime_truth_created: false,
  };
  const publish = (changes) => {
    evidence = { ...evidence, ...changes };
    phase = evidence.phase;
  };

  async function sealDeployment({
    deploymentPlanId,
    deploymentOperationId,
    targetId,
    imageId,
    imageContentId,
    sporeId,
    artifactContentId,
    segments,
    resetStrategy = "classic",
    explicitAction = false,
  }) {
    if (phase !== "available") refuse("WrongPhase", "one ESP32 deployment is already owned");
    if (explicitAction !== true) refuse("ExplicitAction", "deployment requires an explicit operator action");
    requireIdentity(deploymentPlanId, "deployment Plan identity");
    requireIdentity(deploymentOperationId, "deployment operation identity");
    requireIdentity(imageId, "IMAGE identity");
    requireIdentity(imageContentId, "IMAGE content identity");
    requireIdentity(sporeId, "Spore identity");
    requireIdentity(artifactContentId, "Spore artifact content identity");
    if (!ESP32_ROM_TARGETS[targetId]) refuse("WrongTarget", "selected ESP32 target is unsupported");
    if (!(resetStrategy in ESP32_RESET_OPERATION_COUNTS)) refuse("ResetStrategy", "selected reset strategy is unsupported");

    const truth = exactBaseTruth(base);
    const image = await parseEsp32Image({
      targetId,
      segments,
      maximumTransferBytes: truth.transfer_bounds.maximum_transfer_bytes,
      cryptoApi,
    });
    if (image.contentId !== imageContentId) {
      refuse("ContentIdentity", "selected IMAGE segments do not match their sealed SHA-256 identity");
    }
    if (image.segments.length !== 1 || image.segments[0].offset !== 0) {
      refuse("ArtifactLayout", "deployable ESP32 Spore must be one merged image at flash offset zero");
    }
    const observedArtifactContentId = await sha256Bytes(image.segments[0].bytes, cryptoApi);
    if (observedArtifactContentId !== artifactContentId) {
      refuse("ArtifactContentIdentity", "deployable ESP32 Spore bytes do not match their sealed SHA-256 identity");
    }
    const required = requiredEsp32Transfers(image);
    const signalOperations = ESP32_RESET_OPERATION_COUNTS[resetStrategy];
    if (
      truth.transfer_bounds.maximum_reads < required.maximumReads
      || truth.transfer_bounds.maximum_writes < required.maximumWrites
      || truth.transfer_bounds.maximum_signal_operations < signalOperations
    ) {
      refuse("OperationBudget", "Web Serial Base did not admit the complete ESP32 deployment budget");
    }

    const identity = baseIdentity(truth);
    const plan = Object.freeze({
      schema: "conduit.esp32/browser-deployment-plan@1",
      deploymentPlanId,
      deploymentOperationId,
      targetId,
      imageId,
      imageContentId,
      sporeId,
      artifactContentId,
      hostId: identity.hostId,
      bootId: identity.bootId,
      resourceHandle: identity.resourceHandle,
      baseInstanceId: identity.baseInstanceId,
      resetStrategy,
      requiredReads: required.maximumReads,
      requiredWrites: required.maximumWrites,
      requiredSignalOperations: signalOperations,
      imageBytes: image.totalBytes,
      segmentCount: image.segments.length,
    });
    sealedPlans.set(plan, { image, identity });
    publish({
      phase: "sealed",
      deployment_plan_id: deploymentPlanId,
      deployment_operation_id: deploymentOperationId,
      target_id: targetId,
      image_id: imageId,
      image_content_id: imageContentId,
      spore_id: sporeId,
      artifact_content_id: artifactContentId,
      host_id: identity.hostId,
      boot_id: identity.bootId,
      resource_handle: identity.resourceHandle,
      base_implementation_id: identity.baseImplementationId,
      base_instance_id: identity.baseInstanceId,
      use_authority_grant: identity.useAuthorityGrant,
      usb_vendor_id: identity.usbVendorId,
      usb_product_id: identity.usbProductId,
      reset_strategy: resetStrategy,
      admitted_signal_operations: signalOperations,
      admitted_commands: required.commands,
      admitted_image_bytes: image.totalBytes,
    });
    return plan;
  }

  async function deploy(plan) {
    const sealed = sealedPlans.get(plan);
    if (phase !== "sealed" || !sealed) refuse("WrongPlan", "deployment Plan is missing or was not sealed here");
    const current = exactBaseTruth(base);
    if (!sameBase(current, sealed.identity)) refuse("StaleBase", "sealed browser Base identity is no longer current");
    publish({ phase: "resetting" });
    try {
      base.startUse(plan.deploymentPlanId);
      const reset = await enterEsp32RomLoader({ base, strategy: plan.resetStrategy, wait });
      publish({ phase: "deploying", completed_signal_operations: reset.operations });
      const result = await deployEsp32Rom({
        base,
        image: sealed.image,
        targetId: plan.targetId,
        progress: ({ commands, completedBytes }) => publish({
          completed_commands: commands,
          completed_image_bytes: completedBytes ?? evidence.completed_image_bytes,
        }),
      });
      publish({
        phase: "terminal",
        terminal: "RebootRequested",
        chip: result.chip,
        chip_magic: result.chipMagic,
        completed_commands: result.commands,
        completed_image_bytes: result.completedBytes,
        verification: "matched",
        reboot_requested: true,
      });
      return Object.freeze({ ...copy(evidence), schema: "conduit.esp32/browser-deployment-receipt@1" });
    } catch (error) {
      const code = error instanceof Esp32RomRefusal || error instanceof Esp32ImageRefusal || error instanceof Esp32ResetRefusal
        ? error.code
        : "DeploymentFailed";
      publish({ phase: "terminal", terminal: code, verification: code === "VerificationFailed" ? "mismatch" : evidence.verification });
      if (error instanceof Esp32DeploymentRefusal) throw error;
      refuse(code, "ESP32 deployment terminated without success", error);
    }
  }

  function observeResourceTerminal() {
    const truth = base.evidence();
    if (truth?.phase === "terminal" && truth.terminal) publish({ resource_terminal: truth.terminal });
    return Object.freeze(copy(evidence));
  }

  async function close() {
    try {
      await base.close();
    } finally {
      observeResourceTerminal();
    }
    return Object.freeze(copy(evidence));
  }

  return Object.freeze({ sealDeployment, deploy, observeResourceTerminal, close, evidence: () => Object.freeze(copy(evidence)) });
}

export const ESP32_BROWSER_DEPLOYMENT = Object.freeze({
  targetIds: Object.freeze(Object.keys(ESP32_ROM_TARGETS)),
  baudRate: 115200,
  dataBits: 8,
  stopBits: 1,
  parity: "none",
  bufferSize: 4096,
  maximumTransferBytes: 4096,
  maximumReads: 40000,
  maximumWrites: 8192,
  maximumSignalOperations: 32,
  imageBounds: ESP32_IMAGE_BOUNDS,
});
