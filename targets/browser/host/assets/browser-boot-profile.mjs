const IMAGE_SCHEMA = "conduit.browser/bundle-image@1";
const BOOT_SCHEMA = "conduit.browser/profile-gated-boot@1";
const MAXIMUM_IMAGE_BYTES = 64 * 1024;
const MAXIMUM_IMPLEMENTATIONS = 64;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export const BROWSER_IMPLEMENTATION_CATALOG = Object.freeze([
  definition("browser/dom@1", "browser/dom@1", []),
  definition("browser/dom-presentation@1", "presentation/graphical", ["resource"]),
  definition("browser/keyboard-events@1", "input/keyboard", ["resource"]),
  definition("browser/pointer-events@1", "input/pointer-source", ["resource"]),
  definition("browser/indexeddb@1", "storage/indexeddb@1", ["secure", "resource"]),
  definition("browser/websocket@1", "line/websocket@1", ["secure", "provider", "endpoint", "authority"]),
  definition("browser/webrtc-datachannel@1", "line/webrtc-datachannel@1", ["secure", "provider", "endpoint", "authority", "signaling", "session-grant"]),
  definition("browser/media-devices-camera@1", "media/acquire-camera@1", ["secure"]),
  definition("browser/media-devices-microphone@1", "media/acquire-microphone@1", ["secure"]),
  definition("browser/webserial@1", "device/webserial@1", ["secure", "permission", "resource"]),
  definition("browser/webusb@1", "device/webusb@1", ["secure", "permission", "resource"]),
]);

const DEFINITIONS = new Map(BROWSER_IMPLEMENTATION_CATALOG.map((item) => [item.implementation_id, item]));

export async function admitBrowserBoot({
  imageBytes,
  expectedImageId,
  expectedProfileId,
  runtimeBytes,
  bootModuleDigest,
  artifactContentDigest,
  bootId,
  availableImplementations,
  observations = {},
  bundleVariant = "superset",
}) {
  const image = await verifyImage(imageBytes, expectedImageId, expectedProfileId);
  await verifyBootArtifacts(image, runtimeBytes, bootModuleDigest);
  requireDigest(artifactContentDigest, "ArtifactIdentityInvalid");
  if (!boundedText(bootId, 256)) refuse("BootIdentityInvalid", "browser Boot identity is missing or unbounded");
  if (!["superset", "reduced-modules"].includes(bundleVariant)) {
    refuse("BundleVariantInvalid", "browser bundle variant is not recognized");
  }
  const available = requireAvailable(availableImplementations);
  return buildSnapshot({ image, artifactContentDigest, bootId, available, observations, bundleVariant, generation: 1 });
}

export function refreshBrowserBootTruth(boot, observations) {
  requireBoot(boot);
  return buildSnapshot({
    image: boot.image,
    artifactContentDigest: boot.artifact_content_digest,
    bootId: boot.boot_id,
    available: new Map(boot.available_implementations.map((item) => [item.id, item.revision])),
    observations,
    bundleVariant: boot.bundle_variant,
    generation: boot.offer_generation + 1,
  });
}

export function bindBrowserOfferRealization(boot, { realizationId, offerId, formId, planId }) {
  requireBoot(boot);
  for (const value of [realizationId, offerId, formId, planId]) {
    if (!boundedText(value, 256)) refuse("RealizationIdentityInvalid", "offer realization identity is missing or unbounded");
  }
  const state = boot.inspection.find((item) => item.offer_id === offerId);
  if (!state?.offered) refuse("OfferUnavailable", `current browser offer ${offerId} is unavailable`);
  return Object.freeze({
    schema: "conduit.browser/offer-realization@1",
    realization_id: realizationId,
    offer_id: offerId,
    implementation_id: state.implementation_id,
    form_id: formId,
    plan_id: planId,
    image_id: boot.image_id,
    boot_id: boot.boot_id,
    admitted_offer_generation: boot.offer_generation,
  });
}

export function reconcileBrowserOfferRealizations(previous, current, realizations) {
  requireBoot(previous);
  requireBoot(current);
  if (previous.image_id !== current.image_id || previous.boot_id !== current.boot_id) {
    refuse("StaleBoot", "offer truth cannot be reconciled across IMAGE or Boot identity");
  }
  if (!Array.isArray(realizations) || realizations.length > MAXIMUM_IMPLEMENTATIONS) {
    refuse("RealizationBound", "browser offer realizations exceed their finite bound");
  }
  const offers = new Set(current.offers.map((item) => item.offer_id));
  return Object.freeze(realizations.flatMap((realization) => offers.has(realization.offer_id) ? [] : [Object.freeze({
    schema: "conduit.browser/offer-loss@1",
    terminal: "CurrentOfferLost",
    realization_id: realization.realization_id,
    offer_id: realization.offer_id,
    form_id: realization.form_id,
    plan_id: realization.plan_id,
    image_id: current.image_id,
    boot_id: current.boot_id,
    prior_offer_generation: realization.admitted_offer_generation,
    current_offer_generation: current.offer_generation,
    authored_form_mutated: false,
    image_mutated: false,
  })]));
}

export async function observeBrowserHostEnvironment(scope = globalThis) {
  const document = scope.document;
  const navigator = scope.navigator;
  const active = !document || document.visibilityState === "visible";
  return Object.freeze({
    "browser/dom@1": observation(Boolean(document), { page_active: active }),
    "browser/dom-presentation@1": observation(Boolean(document), { resource_ready: Boolean(document?.body), page_active: active }),
    "browser/keyboard-events@1": observation(Boolean(scope.KeyboardEvent), { resource_ready: Boolean(document), page_active: active }),
    "browser/pointer-events@1": observation(Boolean(scope.PointerEvent), { resource_ready: Boolean(document), page_active: active }),
    "browser/indexeddb@1": observation(Boolean(scope.indexedDB), { secure_context: Boolean(scope.isSecureContext), resource_ready: Boolean(scope.indexedDB) }),
    "browser/websocket@1": observation(Boolean(scope.WebSocket), { secure_context: Boolean(scope.isSecureContext), provider_ready: true, endpoint_ready: false, authority_ready: false }),
    "browser/webrtc-datachannel@1": observation(Boolean(scope.RTCPeerConnection), { secure_context: Boolean(scope.isSecureContext), provider_ready: true, endpoint_ready: false, authority_ready: false, signaling_ready: false, session_grant_ready: false }),
    "browser/media-devices-camera@1": observation(Boolean(navigator?.mediaDevices?.getUserMedia), { secure_context: Boolean(scope.isSecureContext) }),
    "browser/media-devices-microphone@1": observation(Boolean(navigator?.mediaDevices?.getUserMedia), { secure_context: Boolean(scope.isSecureContext) }),
    "browser/webserial@1": observation(Boolean(navigator?.serial), { secure_context: Boolean(scope.isSecureContext) }),
    "browser/webusb@1": observation(Boolean(navigator?.usb), { secure_context: Boolean(scope.isSecureContext) }),
  });
}

function buildSnapshot({ image, artifactContentDigest, bootId, available, observations, bundleVariant, generation }) {
  const inspection = image.implementations.map((implementation) => inspect(implementation, available, observations[implementation.id]));
  const offers = inspection.filter((item) => item.offered).map((item) => Object.freeze({
    offer_id: item.offer_id,
    implementation_id: item.implementation_id,
    implementation_revision: item.implementation_revision,
    resource_identity: item.resource_identity,
  }));
  return Object.freeze({
    schema: BOOT_SCHEMA,
    image_id: image.image_id,
    profile_id: image.profile_id,
    build_id: image.build_id,
    boot_id: bootId,
    artifact_content_digest: artifactContentDigest,
    bundle_variant: bundleVariant,
    offer_generation: generation,
    image,
    available_implementations: Object.freeze([...available].map(([id, revision]) => Object.freeze({ id, revision }))),
    implementation_registry: Object.freeze(inspection.filter((item) => item.admitted).map((item) => Object.freeze({ id: item.implementation_id, revision: item.implementation_revision }))),
    offers: Object.freeze(offers),
    inspection: Object.freeze(inspection),
  });
}

function inspect(implementation, available, input = {}) {
  const configured = true;
  const definition = DEFINITIONS.get(implementation.id);
  if (!definition) return state(implementation, null, { configured, reason: "UnknownImplementation" });
  if (available.get(implementation.id) !== implementation.revision) {
    return state(implementation, definition, { configured, reason: "ImplementationCodeAbsent" });
  }
  const admitted = true;
  if (input.initialization_failure) return state(implementation, definition, { configured, admitted, reason: "InitializationFailed" });
  if (input.api_supported !== true) return state(implementation, definition, { configured, admitted, reason: "UnsupportedApi" });
  const initialized = true;
  if (input.page_active === false) return state(implementation, definition, { configured, admitted, initialized, reason: "PageInactive" });
  if (definition.prerequisites.includes("secure") && input.secure_context !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "InsecureContext" });
  }
  if (definition.prerequisites.includes("activation") && input.user_activation !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "UserActivationAbsent" });
  }
  if (definition.prerequisites.includes("permission") && input.permission !== "granted") {
    return state(implementation, definition, { configured, admitted, initialized, reason: input.permission === "denied" ? "PermissionDenied" : "PermissionNotAcquired" });
  }
  if (definition.prerequisites.includes("provider") && input.provider_ready !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "ProviderUnavailable" });
  }
  if (definition.prerequisites.includes("endpoint") && input.endpoint_ready !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "EndpointUnavailable" });
  }
  if (definition.prerequisites.includes("authority") && input.authority_ready !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "EndpointAuthorityAbsent" });
  }
  if (definition.prerequisites.includes("signaling") && input.signaling_ready !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "SignalingBootstrapAbsent" });
  }
  if (definition.prerequisites.includes("session-grant") && input.session_grant_ready !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: "SessionGrantAbsent" });
  }
  if (definition.prerequisites.includes("resource") && input.resource_ready !== true) {
    return state(implementation, definition, { configured, admitted, initialized, reason: input.resource_lost ? "ResourceLost" : "ResourceNotAcquired" });
  }
  return state(implementation, definition, {
    configured, admitted, initialized, resourceReady: true, offered: true,
    resourceIdentity: boundedText(input.resource_identity, 256) ? input.resource_identity : null,
  });
}

function state(implementation, definition, values) {
  return Object.freeze({
    implementation_id: implementation.id,
    implementation_revision: implementation.revision,
    offer_id: definition?.offer_id ?? null,
    configured: values.configured,
    admitted: values.admitted ?? false,
    initialized: values.initialized ?? false,
    resource_ready: values.resourceReady ?? false,
    offered: values.offered ?? false,
    resource_identity: values.resourceIdentity ?? null,
    refusal: values.reason ?? null,
  });
}

async function verifyImage(bytes, expectedImageId, expectedProfileId) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_IMAGE_BYTES) {
    refuse("ImageManifestBound", "browser IMAGE manifest violates its finite byte bound");
  }
  let image;
  try { image = JSON.parse(decoder.decode(bytes)); } catch (error) { refuse("ImageManifestMalformed", "browser IMAGE manifest is malformed", error); }
  const { image_id: imageId, ...payload } = image;
  if (payload.schema !== IMAGE_SCHEMA || !digestIdentity(imageId) || imageId !== expectedImageId
    || payload.target_id !== "browser/wasm32/page" || payload.profile_id !== expectedProfileId
    || !digestIdentity(payload.build_id) || !digestIdentity(payload.source_configuration_id)
    || payload.reviewed_distribution?.runtime_abi !== "conduit.browser/runtime-abi@1"
    || !digestIdentity(payload.reviewed_distribution?.distribution_digest)
    || !Array.isArray(payload.implementations) || payload.implementations.length > MAXIMUM_IMPLEMENTATIONS) {
    refuse("ImageBindingMismatch", "browser Boot did not receive its exact bound PROFILE/IMAGE");
  }
  if (`image:${await sha256(encoder.encode(JSON.stringify(payload)))}` !== imageId) {
    refuse("ImageDigestMismatch", "browser IMAGE identity cannot be recomputed");
  }
  const seen = new Set();
  for (const item of payload.implementations) {
    if (!boundedText(item?.id, 256) || item.revision !== 1 || seen.has(item.id)) {
      refuse("ImplementationBindingInvalid", "browser IMAGE implementation table is invalid");
    }
    seen.add(item.id);
  }
  if (!Array.isArray(payload.files) || payload.files.length < 1 || payload.files.length > 16
    || payload.boot_module?.role !== "profile-gated-boot" || payload.boot_module.path !== "browser-boot-profile.mjs"
    || !digestIdentity(payload.boot_module.sha256)) {
    refuse("ArtifactBindingInvalid", "browser IMAGE artifact table or Boot entry is invalid");
  }
  return Object.freeze({ ...payload, image_id: imageId, implementations: Object.freeze(payload.implementations.map((item) => Object.freeze({ ...item }))) });
}

async function verifyBootArtifacts(image, runtimeBytes, bootModuleDigest) {
  if (!(runtimeBytes instanceof Uint8Array) || runtimeBytes.byteLength < 1 || runtimeBytes.byteLength > 16 * 1024 * 1024) {
    refuse("RuntimeArtifactBound", "browser runtime violates its finite IMAGE bound");
  }
  const runtime = image.files.find((item) => item.path === "runtime.wasm");
  const boot = image.files.find((item) => item.path === image.boot_module.path);
  if (!runtime || runtime.bytes !== runtimeBytes.byteLength || runtime.sha256 !== await sha256(runtimeBytes)) {
    refuse("RuntimeArtifactMismatch", "browser runtime does not match the exact IMAGE");
  }
  if (!boot || boot.sha256 !== image.boot_module.sha256 || boot.sha256 !== bootModuleDigest) {
    refuse("BootModuleBindingMismatch", "profile-gated Boot entry does not match the exact IMAGE");
  }
}

function requireAvailable(input) {
  if (!Array.isArray(input) || input.length > MAXIMUM_IMPLEMENTATIONS) refuse("ImplementationBound", "available implementation table exceeds its finite bound");
  const result = new Map();
  for (const item of input) {
    if (!boundedText(item?.id, 256) || item.revision !== 1 || result.has(item.id)) {
      refuse("AvailableImplementationInvalid", "available implementation table is malformed or ambiguous");
    }
    result.set(item.id, item.revision);
  }
  return result;
}

function requireBoot(boot) {
  if (boot?.schema !== BOOT_SCHEMA || !digestIdentity(boot.image_id) || !boundedText(boot.boot_id, 256)) {
    refuse("BootTruthInvalid", "browser Boot truth is malformed");
  }
}

function definition(implementationId, offerId, prerequisites) {
  return Object.freeze({ implementation_id: implementationId, revision: 1, offer_id: offerId, prerequisites: Object.freeze(prerequisites) });
}
function observation(apiSupported, values = {}) { return Object.freeze({ api_supported: apiSupported, ...values }); }
function requireDigest(value, code) { if (!/^sha256:[0-9a-f]{64}$/.test(value ?? "")) refuse(code, "digest identity is malformed"); }
function digestIdentity(value) { return /^(?:(?:build|image):)?sha256:[0-9a-f]{64}$/.test(value ?? ""); }
function boundedText(value, maximum) { return typeof value === "string" && value.length > 0 && value.length <= maximum; }
async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
function refuse(code, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.code = code;
  error.evidence = Object.freeze({ schema: "conduit.browser/profile-gated-boot-failure@1", terminal: code, message });
  throw error;
}
