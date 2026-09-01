import { deployPicoboot, PicobootRefusal, requiredPicobootTransfers } from "./picoboot.mjs";
import { parseRp2040Uf2, Rp2040ImageRefusal, sha256ContentId } from "./uf2.mjs";

const TARGET_ID = "conduitos/thumbv6m/pico-w";
const USB_VENDOR_ID = 0x2e8a;
const USB_PRODUCT_ID = 0x0003;
const USB_RESOURCE_CLASS = "conduit.resource/web-usb-device@1";
const USB_BASE_IMPLEMENTATION = "browser/web-usb@1";
const USB_USE_AUTHORITY = "conduit.authority/use-web-usb@1";
const sealedPlans = new WeakMap();

export class Rp2040DeploymentRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "Rp2040DeploymentRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new Rp2040DeploymentRefusal(code, message, cause);
}

function requireIdentity(value, name) {
  if (typeof value !== "string" || value.length === 0 || value.length > 512) {
    refuse("Identity", `${name} is missing or outside its finite identity bound`);
  }
  return value;
}

function requireBase(base) {
  const methods = [
    "evidence",
    "startUse",
    "transferIn",
    "transferOut",
    "controlTransferIn",
    "controlTransferOut",
    "close",
  ];
  if (!base || methods.some((name) => typeof base[name] !== "function")) {
    refuse("BaseContract", "RP2040 deployment requires one admitted WebUSB Base session");
  }
}

function exactBaseTruth(base) {
  requireBase(base);
  const truth = base.evidence();
  if (
    truth?.schema !== "conduit.browser/web-usb-base-evidence@1"
    || truth.phase !== "resource-truth"
    || truth.resource_class !== USB_RESOURCE_CLASS
    || truth.base_implementation_id !== USB_BASE_IMPLEMENTATION
    || truth.use_authority_contract !== USB_USE_AUTHORITY
    || !truth.host_id
    || !truth.boot_id
    || !truth.resource_handle
    || !truth.base_instance_id
    || !truth.use_authority_grant
  ) {
    refuse("BaseTruth", "WebUSB Base resource or use-authority truth is missing or stale");
  }
  if (truth.vendor_id !== USB_VENDOR_ID || truth.product_id !== USB_PRODUCT_ID) {
    refuse("WrongTarget", "selected WebUSB device is not an RP2040 BOOTSEL device");
  }
  if (
    truth.configuration?.configuration_value !== 1
    || truth.configuration.interface_number !== 1
    || truth.configuration.alternate_setting !== 0
    || truth.configuration.in_endpoint !== 4
    || truth.configuration.out_endpoint !== 3
  ) {
    refuse("PicobootInterface", "selected Base does not own the RP2040 PICOBOOT vendor interface");
  }
  if (truth.admitted_in_transfers !== 0 || truth.admitted_out_transfers !== 0) {
    refuse("BaseAlreadyUsed", "RP2040 deployment requires an unused exact Base transfer budget");
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
    interfaceNumber: truth.configuration.interface_number,
  });
}

function sameBase(current, expected) {
  return current.host_id === expected.hostId
    && current.boot_id === expected.bootId
    && current.resource_handle === expected.resourceHandle
    && current.base_implementation_id === expected.baseImplementationId
    && current.base_instance_id === expected.baseInstanceId
    && current.use_authority_grant === expected.useAuthorityGrant
    && current.configuration?.interface_number === expected.interfaceNumber;
}

function copyEvidence(value) {
  return JSON.parse(JSON.stringify(value));
}

export function createRp2040BrowserDeploymentAdapter({ base, cryptoApi = globalThis.crypto }) {
  requireBase(base);
  let phase = "available";
  let evidence = {
    schema: "conduit.rp2040/browser-deployment-evidence@1",
    phase,
    terminal: null,
    deployment_plan_id: null,
    deployment_operation_id: null,
    target_id: null,
    spore_id: null,
    image_id: null,
    image_content_id: null,
    spore_content_id: null,
    host_id: null,
    boot_id: null,
    resource_handle: null,
    base_implementation_id: null,
    base_instance_id: null,
    use_authority_grant: null,
    admitted_commands: 0,
    admitted_image_bytes: 0,
    completed_image_bytes: 0,
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
    sporeId,
    imageId,
    imageContentId,
    sporeContentId,
    imageBytes,
    explicitAction = false,
  }) {
    if (phase !== "available") refuse("WrongPhase", "one RP2040 deployment is already owned");
    if (explicitAction !== true) refuse("ExplicitAction", "deployment requires an explicit operator action");
    requireIdentity(deploymentPlanId, "deployment Plan identity");
    requireIdentity(deploymentOperationId, "deployment operation identity");
    requireIdentity(sporeId, "spore identity");
    requireIdentity(imageId, "IMAGE identity");
    requireIdentity(imageContentId, "IMAGE content identity");
    requireIdentity(sporeContentId, "Spore content identity");
    if (targetId !== TARGET_ID) refuse("WrongTarget", "IMAGE target is not the selected Pico W target");

    const truth = exactBaseTruth(base);
    const maximumTransferBytes = truth.transfer_bounds?.maximum_transfer_bytes;
    const image = parseRp2040Uf2(imageBytes, maximumTransferBytes);
    const actualContentId = await sha256ContentId(imageBytes, cryptoApi);
    if (actualContentId !== sporeContentId) {
      refuse("ContentIdentity", "selected native Spore bytes do not match their sealed SHA-256 identity");
    }
    const required = requiredPicobootTransfers(image.chunks.length);
    if (
      truth.transfer_bounds.maximum_in_flight !== 1
      || truth.transfer_bounds.maximum_in_transfers < required.maximumInTransfers
      || truth.transfer_bounds.maximum_out_transfers < required.maximumOutTransfers
    ) {
      refuse("TransferBudget", "WebUSB Base did not admit the complete RP2040 deployment budget");
    }

    const identity = baseIdentity(truth);
    const plan = Object.freeze({
      schema: "conduit.rp2040/browser-deployment-plan@1",
      deploymentPlanId,
      deploymentOperationId,
      targetId,
      sporeId,
      imageId,
      imageContentId,
      sporeContentId,
      hostId: identity.hostId,
      bootId: identity.bootId,
      resourceHandle: identity.resourceHandle,
      baseInstanceId: identity.baseInstanceId,
      requiredInTransfers: required.maximumInTransfers,
      requiredOutTransfers: required.maximumOutTransfers,
      imageBytes: image.imageBytes,
      chunkCount: image.chunks.length,
    });
    sealedPlans.set(plan, { image, identity });
    publish({
      phase: "sealed",
      deployment_plan_id: deploymentPlanId,
      deployment_operation_id: deploymentOperationId,
      target_id: targetId,
      spore_id: sporeId,
      image_id: imageId,
      image_content_id: imageContentId,
      spore_content_id: sporeContentId,
      host_id: identity.hostId,
      boot_id: identity.bootId,
      resource_handle: identity.resourceHandle,
      base_implementation_id: identity.baseImplementationId,
      base_instance_id: identity.baseInstanceId,
      use_authority_grant: identity.useAuthorityGrant,
      admitted_image_bytes: image.imageBytes,
    });
    return plan;
  }

  async function deploy(plan, { signal } = {}) {
    const sealed = sealedPlans.get(plan);
    if (phase !== "sealed" || !sealed) refuse("WrongPlan", "deployment Plan is missing or was not sealed here");
    const current = exactBaseTruth(base);
    if (!sameBase(current, sealed.identity)) refuse("StaleBase", "sealed browser Base identity is no longer current");
    publish({ phase: "deploying" });
    try {
      base.startUse(plan.deploymentPlanId);
      const result = await deployPicoboot({
        base,
        image: sealed.image,
        interfaceNumber: sealed.identity.interfaceNumber,
        signal,
        progress: ({ command }) => publish({
          admitted_commands: evidence.admitted_commands + 1,
          completed_image_bytes: command === 0x05
            ? Math.min(evidence.admitted_image_bytes, evidence.completed_image_bytes + 4096)
            : evidence.completed_image_bytes,
        }),
      });
      publish({
        phase: "terminal",
        terminal: "RebootRequested",
        admitted_commands: result.commands,
        completed_image_bytes: evidence.admitted_image_bytes,
        reboot_requested: true,
      });
      return Object.freeze({
        ...copyEvidence(evidence),
        schema: "conduit.rp2040/browser-deployment-receipt@1",
      });
    } catch (error) {
      const code = error instanceof PicobootRefusal || error instanceof Rp2040ImageRefusal
        ? error.code
        : "DeploymentFailed";
      publish({ phase: "terminal", terminal: code });
      if (error instanceof Rp2040DeploymentRefusal) throw error;
      refuse(code, "RP2040 deployment terminated without success", error);
    }
  }

  function observeResourceTerminal() {
    const truth = base.evidence();
    if (truth?.phase === "terminal" && truth.terminal) {
      publish({ resource_terminal: truth.terminal });
    }
    return Object.freeze(copyEvidence(evidence));
  }

  async function close() {
    try {
      await base.close();
    } finally {
      observeResourceTerminal();
    }
    return Object.freeze(copyEvidence(evidence));
  }

  return Object.freeze({
    sealDeployment,
    deploy,
    observeResourceTerminal,
    close,
    evidence: () => Object.freeze(copyEvidence(evidence)),
  });
}

export const RP2040_BROWSER_DEPLOYMENT = Object.freeze({
  targetId: TARGET_ID,
  usbVendorId: USB_VENDOR_ID,
  usbProductId: USB_PRODUCT_ID,
  configurationValue: 1,
  interfaceNumber: 1,
  alternateSetting: 0,
  inEndpoint: 4,
  outEndpoint: 3,
  maximumTransferBytes: 4096,
  maximumInTransfers: 2048,
  maximumOutTransfers: 2048,
});
