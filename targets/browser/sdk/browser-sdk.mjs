const PACKAGE_SCHEMA = "conduit.browser/sdk-package@1";
const DISTRIBUTION_SCHEMA = "conduit.browser/reviewed-distribution@1";
const RELEASE_SCHEMA = "conduit.release/host-bundle@1";
const IMAGE_PATH = "conduit-browser-image.json";
const MAXIMUM_MANIFEST_BYTES = 128 * 1024;
const MAXIMUM_RUNTIME_BYTES = 16 * 1024 * 1024;
const MAXIMUM_MODULE_BYTES = 256 * 1024;
const MAXIMUM_FILES = 16;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const mounted = new WeakMap();
const BROWSER_HOST_KEY = Symbol("Conduit BrowserHost");
import { BrowserForm, birthBrowserBody, reviewBrowserForms, setBrowserSdkErrors } from "./browser-sdk-forms.mjs";
import { acquireBrowserBodyHost } from "../host/assets/browser-body-host.mjs";
export { BrowserForm, BrowserBody } from "./browser-sdk-forms.mjs";

export class ConduitSdkError extends Error {
  constructor({ code, category = "RuntimeRefusal", message, operation, evidence, identities = {}, cause }) {
    super(message, cause ? { cause } : undefined);
    this.name = "ConduitSdkError";
    this.code = code;
    this.category = category;
    this.operation = operation;
    this.evidence = evidence;
    this.identities = Object.freeze({ ...identities });
  }
}

export class PlanRefusalError extends ConduitSdkError {
  constructor(details) { super({ ...details, category: "PlanRefusal" }); this.name = "PlanRefusalError"; }
}
export class ResourceLossError extends ConduitSdkError {
  constructor(details) { super({ ...details, category: "ResourceLoss" }); this.name = "ResourceLossError"; }
}
export class InvalidLifecycleError extends ConduitSdkError {
  constructor(details) { super({ ...details, category: "InvalidLifecycle" }); this.name = "InvalidLifecycleError"; }
}
export class PermissionDeniedError extends ConduitSdkError {
  constructor(details) { super({ ...details, category: "PermissionDenied" }); this.name = "PermissionDeniedError"; }
}
export class IncompatibleRuntimeAbiError extends ConduitSdkError {
  constructor(details) { super({ ...details, category: "IncompatibleRuntimeAbi" }); this.name = "IncompatibleRuntimeAbiError"; }
}

setBrowserSdkErrors({ PlanRefusalError, ResourceLossError, InvalidLifecycleError, PermissionDeniedError, IncompatibleRuntimeAbiError });

/** Exact Host + Boot incarnation. Its mutable projection is refreshed from Boot evidence. */
export class BrowserHost {
  #state;

  constructor(key, state) {
    if (key !== BROWSER_HOST_KEY) throw new TypeError("BrowserHost values come from Conduit.browser()");
    this.#state = state;
    Object.freeze(this);
  }

  get schema() { return "conduit.browser/host@1"; }
  get packageVersion() { return this.#state.packageVersion; }
  get runtimeAbi() { return this.#state.runtimeAbi; }
  get id() { return this.#state.hostId; }
  get bootId() { return this.#state.boot.boot_id; }
  get profileId() { return this.#state.boot.profile_id; }
  get imageId() { return this.#state.boot.image_id; }
  get offers() { return this.#state.offers; }
  get state() { return Object.freeze({ schema: "conduit.browser/boot-truth@1", generation: this.#state.boot.offer_generation }); }

  current() {
    return Object.freeze({
      schema: "conduit.browser/host-snapshot@1",
      id: this.id,
      bootId: this.bootId,
      profileId: this.profileId,
      imageId: this.imageId,
      offers: this.offers,
      state: this.state,
    });
  }

  async refresh() {
    await this.#state.refresh();
    return this.current();
  }

  form(source) { return new BrowserForm(source, this.#state.bridge); }

  async review(forms) {
    return reviewBrowserForms({ bridge: this.#state.bridge, host: this.id, boot: this.bootId, forms });
  }

  async birth({ name, forms }) {
    if (typeof name !== "string" || !Array.isArray(forms)) throw new TypeError("BrowserHost.birth requires a name and checked Forms");
    return birthBrowserBody({
      bridge: this.#state.bridge,
      host: this.id,
      boot: this.bootId,
      api: this.#state.api,
      root: this.#state.root,
      membership: this.#state.membership,
      createPlay: (options) => new BrowserPlay(BROWSER_HOST_KEY, options),
      name,
      forms,
      sequence: () => {
        if (this.#state.sequence >= Number.MAX_SAFE_INTEGER - 1) throw new RangeError("Body event sequence exhausted");
        return ++this.#state.sequence;
      },
    });
  }
}

export class BrowserPlay {
  #started;
  #adapter;
  #completion = null;
  #terminal = null;
  #failure = null;
  constructor(key, { started, adapter } = {}) {
    if (key !== BROWSER_HOST_KEY) throw new TypeError("BrowserPlay values come from an admitted runtime start");
    this.#started = started;
    this.#adapter = adapter;
    Object.freeze(this);
  }
  get schema() { return "conduit.browser/play@1"; }
  get id() { return this.#started.play.active_play_id; }
  get identity() { return Object.freeze({ ...this.#started.play }); }
  get plan() { return Object.freeze({ planId: this.#started.play.plan_id, wakeId: this.#started.play.wake_id, bodyId: this.#started.play.body_id }); }
  get state() { return this.#terminal ? "terminal" : this.#failure ? "refused" : "playing"; }
  get receipts() { return Object.freeze(this.#terminal ? [this.#terminal] : []); }
  dispatch() {
    if (!this.#completion) this.#completion = this.#adapter.run().then(receipt => { this.#terminal = receipt; return receipt; }, error => { this.#failure = error; return null; });
    return this.#completion;
  }
  async terminate() {
    if (this.#failure) throw this.#failure;
    const closed = this.#adapter.close();
    const receipt = closed.receipt ?? this.#terminal;
    this.#terminal = receipt;
    return receipt;
  }
}

/**
 * Start one exact BrowserBundle as a BrowserHost in an ordinary static page.
 * The bundle must be produced and reviewed before this call; this loader never
 * compiles, discovers a CDN, or broadens its implementation selection.
 */
export const Conduit = Object.freeze({
  async browser({ root, bundleRoot = new URL("./bundle/", import.meta.url), profileId, durable = true } = {}) {
    try {
      if (!root || !(root instanceof Element || root instanceof ShadowRoot)) {
        refuse("InvalidRoot", "Conduit.browser requires one application-owned DOM root");
      }
      const base = sameOriginDirectory(bundleRoot);
      const pkg = await readJson(new URL("browser-sdk-package.json", base), MAXIMUM_MANIFEST_BYTES, "SDK package manifest");
      requirePackage(pkg, profileId);

      const distributionBytes = await fetchAsset(base, "browser-page.json", MAXIMUM_MANIFEST_BYTES);
      const bundleBytes = await fetchAsset(base, "browser-bundle-release.json", MAXIMUM_MANIFEST_BYTES);
      const imageBytes = await fetchAsset(base, IMAGE_PATH, MAXIMUM_MANIFEST_BYTES);
      const distribution = parseJson(distributionBytes, "reviewed browser distribution");
      const bundle = parseJson(bundleBytes, "BrowserBundle release manifest");
      const image = parseJson(imageBytes, "BrowserBundle IMAGE");
      requireRelease(distribution, bundle, pkg, image);
      if (!Array.isArray(distribution.files) || distribution.files.length > MAXIMUM_FILES
        || JSON.stringify(distribution.files) !== JSON.stringify(image.files)
        || await digestFiles(distribution.files) !== distribution.bundle_sha256) {
        refuse("DistributionDigestMismatch", "reviewed BrowserBundle assets do not match the exact distribution manifest");
      }

      const payloads = await readBundlePayloads(base, bundle, image);
      const runtimeBytes = requiredAsset(payloads, "runtime.wasm", MAXIMUM_RUNTIME_BYTES);
      const bootBytes = requiredAsset(payloads, "browser-boot-profile.mjs", MAXIMUM_MODULE_BYTES);
      const bootDigest = image.boot_module?.sha256;
      const distributionDigest = distribution.bundle_sha256;
      const bootModule = await importVerifiedModule(bootBytes, bootDigest, "profile-gated Boot module");
      const bootId = `browser-boot/${crypto.randomUUID()}`;
      const boot = await bootModule.admitBrowserBoot({
        imageBytes,
        expectedImageId: pkg.image_id,
        expectedProfileId: pkg.profile_id,
        runtimeBytes,
        bootModuleDigest: bootDigest,
        artifactContentDigest: bundle.bundle_sha256,
        bootId,
        availableImplementations: distribution.reviewed_distribution.implementations.map(({ id, revision }) => ({ id, revision })),
        observations: await bootModule.observeBrowserHostEnvironment(globalThis),
        bundleVariant: "superset",
      });

      const bridgeBytes = requiredAsset(payloads, "browser-runtime-bridge.mjs", MAXIMUM_MODULE_BYTES);
      const bridgeModule = await importVerifiedModule(bridgeBytes, digestOf(bundle, "browser-runtime-bridge.mjs"), "runtime bridge");
      const instantiated = await WebAssembly.instantiate(runtimeBytes, {});
      const instance = instantiated.instance;
      const api = instance.exports;
      const bridge = bridgeModule.bindBrowserRuntimeBridge(api, { context: "public Browser SDK runtime" });
      const initialize = await materializeModule("browser-host-membership.mjs", distribution, payloads);
      const initialized = await initialize.initializeBrowserHostInstance(instance, { durable, bootId });
      if (initialized.bootId !== bootId || initialized.membership.bootId !== bootId
        || initialized.hostId !== initialized.membership.hostId) {
        refuse("HostIncarnationMismatch", "initialized membership does not match the admitted Host and Boot identity");
      }
      const state = {
        packageVersion: pkg.package_version,
        runtimeAbi: pkg.runtime_abi,
        hostId: initialized.hostId,
        api,
        root,
        bridge,
        membership: initialized.membership,
        sequence: 0,
        boot,
        offers: Object.freeze(boot.offers.map((offer) => Object.freeze({ ...offer }))),
        async refresh() {
          this.boot = bootModule.refreshBrowserBootTruth(this.boot, await bootModule.observeBrowserHostEnvironment(globalThis));
          this.offers = Object.freeze(this.boot.offers.map((offer) => Object.freeze({ ...offer })));
          renderHostRoot(root, this.hostId, this.boot.boot_id, this.boot.profile_id, this.offers);
        },
      };
      const host = new BrowserHost(BROWSER_HOST_KEY, state);
      renderHostRoot(root, host.id, host.bootId, host.profileId, host.offers);
      return host;
    } catch (error) {
      if (error?.evidence?.schema === "conduit.browser/sdk-load-failure@1") throw error;
      const message = error instanceof Error ? error.message : String(error);
      const cause = error instanceof Error && error.cause instanceof Error ? `; cause: ${error.cause.message}` : "";
      refuse(error?.code ?? "LoadFailed", `BrowserHost admission failed: ${message}${cause}`, error);
    }
  },
});

function requirePackage(pkg, requestedProfileId) {
  if (pkg?.schema !== PACKAGE_SCHEMA || pkg.runtime_abi !== "conduit.browser/runtime-abi@1"
    || pkg.package_id !== "@conduit/browser" || pkg.package_version !== "0.1.0"
    || pkg.target_id !== "browser/wasm32/page" || !identity(pkg.image_id) || !identity(pkg.profile_id)
    || !digest(pkg.bundle_sha256) || !Array.isArray(pkg.files) || pkg.files.length < 1 || pkg.files.length > MAXIMUM_FILES) {
    refuse("PackageManifestInvalid", "reviewed browser SDK package metadata is malformed or incompatible");
  }
  if (requestedProfileId !== undefined && requestedProfileId !== pkg.profile_id) {
    refuse("ProfileMismatch", "requested Browser PROFILE does not match this package's exact IMAGE");
  }
}

function requireRelease(distribution, bundle, pkg, image) {
  if (distribution?.schema !== RELEASE_SCHEMA || distribution.target_id !== pkg.target_id
    || distribution.fabrication_package_id !== "browser-wasm@1" || distribution.output !== "browser-bundle"
    || distribution.reviewed_distribution?.schema !== DISTRIBUTION_SCHEMA
    || distribution.reviewed_distribution.runtime_abi !== pkg.runtime_abi
    || distribution.bundle_sha256 !== image.reviewed_distribution?.distribution_digest) {
    refuse("IncompatibleDistribution", "reviewed BrowserBundle distribution does not match the SDK package ABI and identity");
  }
  if (bundle?.schema !== RELEASE_SCHEMA || bundle.target_id !== pkg.target_id
    || bundle.fabrication_package_id !== "browser-wasm@1" || bundle.output !== "browser-bundle"
    || bundle.distribution_id !== distribution.reviewed_distribution.distribution_id
    || bundle.distribution_sha256 !== distribution.bundle_sha256
    || bundle.browser_image_id !== pkg.image_id || bundle.browser_profile_id !== pkg.profile_id
    || bundle.bundle_sha256 !== pkg.bundle_sha256 || !Array.isArray(bundle.files)
    || bundle.files.length !== pkg.files.length || JSON.stringify(bundle.files) !== JSON.stringify(pkg.files)) {
    refuse("BundleManifestMismatch", "BrowserBundle release manifest is not the exact package-bound artifact set");
  }
  if (image?.schema !== "conduit.browser/bundle-image@1" || image.image_id !== pkg.image_id
    || image.profile_id !== pkg.profile_id || image.reviewed_distribution?.runtime_abi !== pkg.runtime_abi
    || !Array.isArray(image.files) || image.files.length > MAXIMUM_FILES) {
    refuse("ImageManifestMismatch", "BrowserBundle IMAGE does not match the package's exact Profile and identity");
  }
}

async function readBundlePayloads(base, bundle, image) {
  let total = 0;
  const declared = new Map(bundle.files.map((file) => [file.path, file]));
  if (declared.size !== bundle.files.length || !declared.has(IMAGE_PATH)) {
    refuse("BundleManifestInvalid", "BrowserBundle release manifest has duplicate paths or omits its IMAGE");
  }
  const paths = new Set([...image.files.map((file) => file.path), IMAGE_PATH]);
  if (paths.size !== declared.size || [...declared.keys()].some((path) => !paths.has(path))) {
    refuse("UnexpectedAsset", "BrowserBundle contains files outside its exact IMAGE closure");
  }
  const results = await Promise.all(bundle.files.map(async (file) => {
    if (!safeRelativePath(file.path) || !Number.isSafeInteger(file.bytes) || file.bytes < 1
      || file.bytes > (file.path === "runtime.wasm" ? MAXIMUM_RUNTIME_BYTES : MAXIMUM_MODULE_BYTES)
      || !digest(file.sha256)) refuse("ArtifactBound", `BrowserBundle asset ${file.path} exceeds its declared finite bound`);
    const bytes = await fetchAsset(base, file.path, file.bytes);
    if (bytes.byteLength !== file.bytes || await sha256(bytes) !== file.sha256) {
      refuse("ArtifactDigestMismatch", `BrowserBundle asset ${file.path} does not match its admitted identity`);
    }
    total += bytes.byteLength;
    return [file.path, bytes];
  }));
  if (total > 48 * 1024 * 1024 || await digestFiles(bundle.files) !== bundle.bundle_sha256) {
    refuse("BundleDigestMismatch", "BrowserBundle content digest or finite package bound is invalid");
  }
  return new Map(results);
}

async function materializeModule(path, distribution, payloads) {
  const cache = new Map();
  const urls = [];
  try { return (await materializeModuleTree(path, distribution, payloads, cache, urls)).module; }
  finally { for (const url of urls) URL.revokeObjectURL(url); }
}

async function materializeModuleTree(path, distribution, payloads, cache, urls) {
  if (cache.has(path)) return cache.get(path);
  const bytes = requiredAsset(payloads, path, MAXIMUM_MODULE_BYTES);
  const declaration = distribution.reviewed_distribution.modules.find((item) => item.path === path);
  if (!declaration || !Array.isArray(declaration.dependencies) || declaration.dependencies.length > 16) {
    refuse("ModuleDeclarationMissing", `reviewed distribution does not declare module ${path}`);
  }
  let source;
  try { source = decoder.decode(bytes); }
  catch (error) { refuse("ModuleEncodingInvalid", `reviewed module ${path} is not UTF-8`, error); }
  for (const dependency of declaration.dependencies) {
    if (!safeRelativePath(dependency) || !source.includes(JSON.stringify(`./${dependency}`))) {
      refuse("ModuleDependencyMismatch", `reviewed module ${path} does not use its exact declared dependency`);
    }
    const dependencyModule = await materializeModuleTree(dependency, distribution, payloads, cache, urls);
    source = source.split(JSON.stringify(`./${dependency}`)).join(JSON.stringify(dependencyModule.url));
  }
  if (/\bimport\s*\(/.test(source) || /(?:\bfrom\s*|\bimport\s*)["'](?!blob:)/.test(source)) {
    refuse("UndeclaredModule", `reviewed module ${path} imports an undeclared module`);
  }
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  urls.push(url);
  let module;
  try { module = await import(url); }
  catch (error) { throw new Error(`verified distribution module ${path} could not be loaded: ${error.message}`, { cause: error }); }
  cache.set(path, { module, url });
  return cache.get(path);
}

async function importVerifiedModule(bytes, expectedDigest, label) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_MODULE_BYTES
    || !digest(expectedDigest) || await sha256(bytes) !== expectedDigest) {
    refuse("ModuleDigestMismatch", `${label} failed exact byte admission`);
  }
  let source;
  try { source = decoder.decode(bytes); }
  catch (error) { refuse("ModuleEncodingInvalid", `${label} is not UTF-8`, error); }
  if (/\bimport\s*(?:\(|["'])|\bfrom\s*["']/.test(source)) {
    refuse("ModuleDependencyUnexpected", `${label} must be self-contained`);
  }
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  try {
    try { return await import(url); }
    catch (error) { throw new Error(`${label} could not be loaded: ${error.message}`, { cause: error }); }
  } finally { URL.revokeObjectURL(url); }
}

function renderHostRoot(root, hostId, bootId, profileId, offers) {
  mounted.get(root)?.remove();
  const surface = document.createElement("section");
  surface.dataset.conduitBrowserHost = "ready";
  const heading = document.createElement("h2");
  heading.textContent = "Conduit Browser Host";
  const identities = document.createElement("dl");
  for (const [label, value] of [["Host", hostId], ["Boot", bootId], ["Profile", profileId]]) {
    const term = document.createElement("dt"); term.textContent = label;
    const detail = document.createElement("dd"); detail.textContent = value;
    identities.append(term, detail);
  }
  const status = document.createElement("p");
  status.textContent = `${offers.length} exact Browser implementation offer(s) admitted.`;
  surface.append(heading, identities, status);
  root.append(surface);
  mounted.set(root, surface);
}

function sameOriginDirectory(value) {
  const base = new URL(value, document.baseURI);
  if (base.origin !== location.origin || !base.pathname.endsWith("/")) {
    refuse("ArtifactOriginRejected", "BrowserBundle assets must come from one same-origin static directory");
  }
  return base;
}

async function fetchAsset(base, path, maximum) {
  if (!safeRelativePath(path)) refuse("ArtifactPathRejected", `BrowserBundle path ${path} is outside its static package`);
  const url = new URL(path, base);
  if (url.origin !== base.origin || !url.pathname.startsWith(base.pathname)) {
    refuse("ArtifactOriginRejected", "BrowserBundle resource escaped its admitted static directory");
  }
  const response = await fetch(url, { cache: "no-store", credentials: "same-origin", redirect: "error" });
  if (!response.ok) refuse("ArtifactUnavailable", `reviewed BrowserBundle resource ${path} is unavailable`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength < 1 || bytes.byteLength > maximum) refuse("ArtifactBound", `BrowserBundle resource ${path} exceeds its finite bound`);
  return bytes;
}

async function readJson(url, maximum, label) {
  if (url.origin !== location.origin) refuse("ArtifactOriginRejected", `${label} must be same-origin`);
  const response = await fetch(url, { cache: "no-store", credentials: "same-origin", redirect: "error" });
  if (!response.ok) refuse("ArtifactUnavailable", `${label} is unavailable`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength < 1 || bytes.byteLength > maximum) refuse("ArtifactBound", `${label} exceeds its finite bound`);
  return parseJson(bytes, label);
}

function parseJson(bytes, label) {
  try { return JSON.parse(decoder.decode(bytes)); }
  catch (error) { refuse("ManifestMalformed", `${label} is not bounded UTF-8 JSON`, error); }
}

function requiredAsset(files, path, maximum) {
  const bytes = files.get(path);
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < 1 || bytes.byteLength > maximum) {
    refuse("ArtifactMissing", `BrowserBundle omits required asset ${path}`);
  }
  return bytes;
}

function digestOf(bundle, path) {
  const file = bundle.files.find((item) => item.path === path);
  if (!file) refuse("ArtifactMissing", `BrowserBundle release manifest omits ${path}`);
  return file.sha256;
}

async function digestFiles(files) {
  let source = "conduit.release/host-bundle-content@1\0";
  for (const file of files) source += `${file.path}\0${file.sha256}\n`;
  return sha256(encoder.encode(source));
}

async function sha256(bytes) {
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function digest(value) { return typeof value === "string" && /^sha256:[0-9a-f]{64}$/.test(value); }
function identity(value) { return typeof value === "string" && /^(?:sha256|image:sha256):[0-9a-f]{64}$/.test(value); }
function safeRelativePath(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 256
    && !value.startsWith("/") && !value.includes("\\") && !value.includes("%")
    && !value.split("/").some((segment) => !segment || segment === "." || segment === "..");
}
function refuse(code, message, cause) {
  const evidence = cause?.evidence ?? cause?.refusal ?? Object.freeze({ schema: "conduit.browser/sdk-load-failure@1", terminal: code, message });
  const details = { code, message, operation: cause?.operation ?? "BrowserHost.initialize", evidence, identities: cause?.identities ?? {}, cause };
  const ErrorType = code === "IncompatibleRuntimeAbi" ? IncompatibleRuntimeAbiError
    : code === "PermissionDenied" ? PermissionDeniedError
      : code === "ResourceLost" ? ResourceLossError
        : /Plan|OfferUnavailable|AdmissionRefused/.test(code) ? PlanRefusalError
          : /Lifecycle|Transition/.test(code) ? InvalidLifecycleError : ConduitSdkError;
  throw new ErrorType(details);
}
