const DISTRIBUTION_SCHEMA = "conduit.browser/reviewed-distribution@1";
const IMAGE_SCHEMA = "conduit.browser/bundle-image@1";
const RELEASE_SCHEMA = "conduit.release/host-bundle@1";
const TARGET = "browser/wasm32/page";
const BUILDER = "conduit-host-browser/bind-prebuilt@1";
const DEPLOYMENT = "conduit-host-browser/load@1";
const BINDING_PATH = "conduit-browser-image.json";
const BOOT_MODULE_PATH = "browser-boot-profile.mjs";
const MAXIMUM_FILES = 16;
const MAXIMUM_IMPLEMENTATIONS = 64;
const MAXIMUM_MODULES = 32;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export async function buildBrowserBundleImage({ checked, distribution }) {
  requireChecked(checked);
  const reviewed = requireDistribution(distribution);
  const selected = resolveImplementations(checked.selected_implementations, reviewed);
  const files = await verifyDistributionPayloads(distribution, reviewed);
  const buildBasis = {
    schema: "conduit.browser/bundle-build-basis@1",
    target_id: TARGET,
    profile_id: checked.profile_id,
    source_configuration_id: checked.configuration_id,
    distribution_id: reviewed.distribution_id,
    distribution_digest: distribution.manifest.bundle_sha256,
    runtime_abi: reviewed.runtime_abi,
    implementations: selected,
  };
  const buildId = `build:${await sha256(encodeCanonical(buildBasis))}`;
  const imagePayload = {
    schema: IMAGE_SCHEMA,
    build_id: buildId,
    target_id: TARGET,
    profile_id: checked.profile_id,
    source_configuration_id: checked.configuration_id,
    reviewed_distribution: {
      distribution_id: reviewed.distribution_id,
      distribution_digest: distribution.manifest.bundle_sha256,
      runtime_abi: reviewed.runtime_abi,
      toolchain_identity: reviewed.toolchain_identity,
      source_commit: reviewed.source_commit,
    },
    implementations: selected,
    boot_module: requireBootModule(files),
    files: files.map(({ path, bytes, sha256: digest, media_type }) => ({ path, bytes: bytes.byteLength, sha256: digest, media_type })),
  };
  const imageId = `image:${await sha256(encodeCanonical(imagePayload))}`;
  const binding = encoder.encode(JSON.stringify({ ...imagePayload, image_id: imageId }, null, 2));
  const bindingDigest = await sha256(binding);
  const payloads = [...files, Object.freeze({ path: BINDING_PATH, bytes: binding, sha256: bindingDigest, media_type: "application/vnd.conduit.browser-image+json" })];
  const bundleDigest = await digestFileIdentities(payloads);
  const release = Object.freeze({
    manifest: Object.freeze({
      schema: RELEASE_SCHEMA,
      target_id: TARGET,
      fabrication_package_id: "browser-wasm@1",
      output: "browser-bundle",
      builder_adapter: BUILDER,
      deployment_adapter: DEPLOYMENT,
      source_identity: reviewed.source_commit,
      bundle_sha256: bundleDigest,
      browser_image_id: imageId,
      browser_build_id: buildId,
      browser_profile_id: checked.profile_id,
      browser_configuration_id: checked.configuration_id,
      distribution_id: reviewed.distribution_id,
      distribution_sha256: distribution.manifest.bundle_sha256,
      files: Object.freeze(payloads.map(({ path, bytes, sha256: digest, media_type }) => Object.freeze({ path, bytes: bytes.byteLength, sha256: digest, media_type }))),
    }),
    payloads: Object.freeze(payloads),
    totalBytes: payloads.reduce((sum, item) => sum + item.bytes.byteLength, 0),
  });
  await verifyBrowserBundleImage({ checked, release, distribution });
  return release;
}

export async function verifyBrowserBundleImage({ checked, release, distribution }) {
  requireChecked(checked);
  const reviewed = requireDistribution(distribution);
  if (release?.manifest?.schema !== RELEASE_SCHEMA || release.manifest.target_id !== TARGET
    || release.manifest.builder_adapter !== BUILDER || release.manifest.deployment_adapter !== DEPLOYMENT
    || release.manifest.browser_profile_id !== checked.profile_id
    || release.manifest.browser_configuration_id !== checked.configuration_id
    || release.manifest.distribution_id !== reviewed.distribution_id
    || release.manifest.distribution_sha256 !== distribution.manifest.bundle_sha256) {
    refuse("ImageBindingMismatch", "BrowserBundle manifest lost its exact configuration/Profile/distribution binding");
  }
  const expectedPaths = new Set([...distribution.payloads.map((item) => item.path), BINDING_PATH]);
  if (!Array.isArray(release.payloads) || release.payloads.length !== expectedPaths.size
    || release.payloads.some((item) => !expectedPaths.delete(item.path)) || expectedPaths.size !== 0) {
    refuse("UnexpectedAsset", "BrowserBundle contains an undeclared executable or static asset");
  }
  for (const payload of release.payloads) {
    if (await sha256(payload.bytes) !== payload.sha256) refuse("DigestMismatch", `BrowserBundle asset ${payload.path} changed identity`);
  }
  if (await digestFileIdentities(release.payloads) !== release.manifest.bundle_sha256) {
    refuse("DigestMismatch", "BrowserBundle aggregate identity is stale");
  }
  let image;
  try { image = JSON.parse(decoder.decode(release.payloads.find((item) => item.path === BINDING_PATH).bytes)); }
  catch (error) { refuse("ImageManifestMalformed", "BrowserBundle IMAGE manifest is malformed", error); }
  const { image_id: imageId, ...payload } = image;
  if (imageId !== release.manifest.browser_image_id || `image:${await sha256(encodeCanonical(payload))}` !== imageId
    || payload.build_id !== release.manifest.browser_build_id) {
    refuse("ImageBindingMismatch", "BrowserBundle IMAGE identity cannot be recomputed");
  }
  const selected = resolveImplementations(checked.selected_implementations, reviewed);
  if (JSON.stringify(payload.implementations) !== JSON.stringify(selected)) {
    refuse("ImplementationBindingMismatch", "BrowserBundle selected implementation closure is stale");
  }
  const bootModule = release.payloads.find((item) => item.path === BOOT_MODULE_PATH);
  if (!bootModule || payload.boot_module?.role !== "profile-gated-boot"
    || payload.boot_module.path !== BOOT_MODULE_PATH || payload.boot_module.sha256 !== bootModule.sha256) {
    refuse("BootModuleBindingMismatch", "BrowserBundle Boot entry is absent or stale");
  }
  return Object.freeze({ build_id: payload.build_id, image_id: imageId, profile_id: payload.profile_id, configuration_id: payload.source_configuration_id });
}

function requireBootModule(files) {
  const module = files.find((item) => item.path === BOOT_MODULE_PATH);
  if (!module) refuse("BootModuleMissing", "reviewed browser distribution omitted its profile-gated Boot entry");
  return Object.freeze({ role: "profile-gated-boot", path: BOOT_MODULE_PATH, sha256: module.sha256 });
}

function requireChecked(checked) {
  if (checked?.schema !== "conduit.creche/checked-browser-configuration@1"
    || checked.target_id !== TARGET || !digestIdentity(checked.profile_id) || !digestIdentity(checked.configuration_id)
    || !Array.isArray(checked.selected_implementations) || checked.selected_implementations.length > MAXIMUM_IMPLEMENTATIONS) {
    refuse("ProfileInvalid", "checked browser PROFILE is malformed or targets different machinery");
  }
}

function requireDistribution(distribution) {
  const manifest = distribution?.manifest;
  const reviewed = manifest?.reviewed_distribution;
  if (manifest?.schema !== RELEASE_SCHEMA || manifest.target_id !== TARGET
    || manifest.fabrication_package_id !== "browser-wasm@1" || manifest.output !== "browser-bundle"
    || !digestIdentity(manifest.bundle_sha256)) {
    refuse("MissingReviewedDistribution", "exact reviewed browser distribution is unavailable");
  }
  if (reviewed?.schema !== DISTRIBUTION_SCHEMA || reviewed.distribution_id !== "conduit.browser/reviewed-distribution@1"
    || reviewed.runtime_abi !== "conduit.browser/runtime-abi@1"
    || !boundedText(reviewed.toolchain_identity, 256) || !boundedText(reviewed.source_commit, 256)
    || !Number.isSafeInteger(reviewed.maximum_bundle_bytes) || reviewed.maximum_bundle_bytes < 1
    || !Array.isArray(reviewed.implementations) || reviewed.implementations.length < 1 || reviewed.implementations.length > MAXIMUM_IMPLEMENTATIONS
    || !Array.isArray(reviewed.modules) || reviewed.modules.length > MAXIMUM_MODULES) {
    refuse("IncompatibleRuntimeAbi", "reviewed browser distribution metadata is missing or incompatible");
  }
  if (!reviewed.targets?.includes(TARGET)) refuse("UnsupportedBrowserTarget", "reviewed distribution does not support the selected browser target");
  const providers = new Set();
  for (const item of reviewed.implementations) {
    if (!boundedText(item?.id, 256) || !Number.isSafeInteger(item.revision) || item.revision < 1
      || !boundedText(item.artifact, 256) || providers.has(item.id)) {
      refuse(providers.has(item?.id) ? "ConflictingProvider" : "WrongImplementationRevision", "reviewed browser implementation table is invalid");
    }
    providers.add(item.id);
  }
  const paths = new Set(manifest.files.map((item) => item.path));
  for (const module of reviewed.modules) {
    if (!paths.has(module.path) || !Array.isArray(module.dependencies) || module.dependencies.some((path) => !paths.has(path))) {
      refuse("ModuleDependencyMissing", "reviewed browser module dependency closure is incomplete");
    }
  }
  return reviewed;
}

function resolveImplementations(identities, reviewed) {
  const providers = new Map(reviewed.implementations.map((item) => [item.id, item]));
  return [...identities].sort().map((id) => {
    const item = providers.get(id);
    if (!item) refuse("ImplementationAbsent", `selected implementation ${id} is absent from the reviewed distribution`);
    if (item.revision !== 1) refuse("WrongImplementationRevision", `selected implementation ${id} has an incompatible revision`);
    return Object.freeze({ id, revision: item.revision, artifact: item.artifact });
  });
}

async function verifyDistributionPayloads(distribution, reviewed) {
  if (!Array.isArray(distribution.payloads) || distribution.payloads.length < 1 || distribution.payloads.length >= MAXIMUM_FILES) {
    refuse("ArtifactBound", "reviewed browser distribution file count exceeds its finite bound");
  }
  let total = 0;
  const files = [];
  const declarations = new Map(distribution.manifest.files.map((item) => [item.path, item]));
  for (const payload of distribution.payloads) {
    const declared = declarations.get(payload.path);
    total += payload.bytes?.byteLength ?? 0;
    if (total > reviewed.maximum_bundle_bytes) refuse("ArtifactBound", "reviewed browser distribution exceeds its finite bundle bound");
    if (!declared) refuse("UnexpectedAsset", `reviewed browser distribution contains undeclared asset ${payload.path}`);
    if (payload.bytes.byteLength !== declared.bytes || await sha256(payload.bytes) !== declared.sha256) {
      refuse("DigestMismatch", `reviewed browser artifact ${payload.path} failed admission`);
    }
    files.push(Object.freeze({ ...declared, bytes: new Uint8Array(payload.bytes) }));
    declarations.delete(payload.path);
  }
  if (declarations.size) refuse("ArtifactMissing", "reviewed browser distribution omitted a declared artifact");
  for (const implementation of reviewed.implementations) {
    const artifact = implementation.artifact === "browser-runtime-superset.wasm" ? "runtime.wasm" : implementation.artifact;
    if (!files.some((item) => item.path === artifact)) refuse("ArtifactMissing", `implementation ${implementation.id} artifact is unavailable`);
  }
  return files;
}

async function digestFileIdentities(files) {
  let source = "conduit.release/host-bundle-content@1\0";
  for (const file of files) source += `${file.path}\0${file.sha256}\n`;
  return sha256(encoder.encode(source));
}

function encodeCanonical(value) { return encoder.encode(JSON.stringify(value)); }
function digestIdentity(value) { return typeof value === "string" && /^(?:sha256|(?:build|image):sha256):[0-9a-f]{64}$/.test(value); }
function boundedText(value, maximum) { return typeof value === "string" && value.length > 0 && value.length <= maximum; }
async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
function refuse(code, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined); error.code = code;
  error.evidence = Object.freeze({ schema: "conduit.browser/bundle-build-failure@1", terminal: code, message, compiler_started: false, network_requested: false });
  throw error;
}
