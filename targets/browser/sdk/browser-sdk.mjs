import { attachBrowserSyntaxEditor, projectBrowserSyntax } from "./browser-sdk-syntax.mjs";
import { applicationContinuity, createSdkContinuity } from "./browser-sdk-continuity.mjs";
import { checkedOwnerFaceArguments } from "./browser-sdk-face-actions.mjs";
import { checkedOwnerWardrobeReport, checkedOwnerWardrobeAction } from "./browser-sdk-wardrobe.mjs";
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
const BROWSER_HOST_KEY = Symbol("Conduit BrowserHost");
const BROWSER_PARTICIPATION_KEY = Symbol("Conduit BrowserBodyParticipation");
const BROWSER_PREPARATION_KEY = Symbol("Conduit BrowserBodyPreparation");
import { BrowserPlot, birthBrowserBody, recoverBrowserBody, reviewBrowserPlots, sdkRefusal, setBrowserSdkErrors } from "./browser-sdk-plots.mjs";
import { acquireBrowserBodyHost } from "../host/assets/browser-body-host.mjs";
import { openBrowserApplicationStorage } from "../host/assets/browser-application-storage.mjs";
import { joinBrowserBody } from "../host/assets/browser-membership.js";
export { BrowserPlot, BrowserBody } from "./browser-sdk-plots.mjs";
export { BrowserFaceClient, BrowserFaceError } from "./browser-sdk-face.mjs";

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

  /** Public proof identity for explicit first-admission authorization; never a secret key. */
  admissionIdentity() {
    return Object.freeze({ hostId: this.id, bootId: this.bootId,
      verifyingKey: Object.freeze([...this.#state.membership.verifyingKey]) });
  }
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

  attachEditor(textarea) { return attachBrowserSyntaxEditor(textarea, this.#state.api); }
  syntax(source) { return projectBrowserSyntax(source, this.#state.api); }

  plot(source) { return new BrowserPlot(source, this.#state.bridge); }

  async review(plots) {
    return reviewBrowserPlots({ bridge: this.#state.bridge, host: this.id, boot: this.bootId, plots });
  }

  /** Join this already-admitted Host and Boot to one external Body invitation. */
  async participate({ invitation, expectedBodyId = null, retainedCredential = null, onCredential,
    onState, onBiographyEvidence, onOfferEvidence, renewPresence = true, reconnectPresence = true } = {}) {
    if (typeof invitation !== "string" || invitation.length < 1 || invitation.length > 2048) {
      throw new TypeError("BrowserHost.participate requires one bounded Body invitation URL");
    }
    const membership = await joinBrowserBody({
      bodyUrl: invitation,
      wasmBytes: this.#state.runtimeBytes,
      admittedHost: {
        api: this.#state.api,
        membership: this.#state.membership,
        hostId: this.id,
        bootId: this.bootId,
      },
      expectedBodyId,
      retainedCredential,
      onCredential,
      onState: state => {
        if (state === "offline" || state.startsWith("refused:")) this.#state.bridge.ownerFaceClear?.();
        onState?.(state);
      },
      onBiographyEvidence,
      onOfferEvidence,
      renewPresence,
      reconnectPresence,
    });
    return new BrowserBodyParticipation(BROWSER_PARTICIPATION_KEY, {
      host: this.#state,
      membership,
    });
  }

  async birth({ name, plots }) {
    if (typeof name !== "string" || !Array.isArray(plots)) throw new TypeError("BrowserHost.birth requires a name and checked Plots");
    return this.#state.continuity.run("birth", () => birthBrowserBody({
      bridge: this.#state.bridge,
      host: this.id,
      boot: this.bootId,
      api: this.#state.api,
      root: this.#state.root,
      membership: this.#state.membership,
      createPlay: (options) => new BrowserPlay(BROWSER_HOST_KEY, options),
      acquireBodyHost: acquireSdkBodyHost,
      storage: this.#state.storage,
      name,
      plots,
      sequence: () => {
        if (this.#state.sequence >= Number.MAX_SAFE_INTEGER - 1) throw new RangeError("Body event sequence exhausted");
        return ++this.#state.sequence;
      },
    }));
  }

  /** Recover the exact retained Body into this fresh Boot without resurrecting an old Play. */
  async recover() {
    return this.#state.continuity.run("recover", () => recoverBrowserBody({
      bridge: this.#state.bridge,
      host: this.id,
      boot: this.bootId,
      api: this.#state.api,
      root: this.#state.root,
      membership: this.#state.membership,
      createPlay: (options) => new BrowserPlay(BROWSER_HOST_KEY, options),
      acquireBodyHost: acquireSdkBodyHost,
      storage: this.#state.storage,
    }));
  }

  close() { return this.#state.continuity.close(); }
  forget() { return this.#state.continuity.close(true); }
}

/** Current participation of this SDK Host incarnation in one external Body. */
export class BrowserBodyParticipation {
  #state;

  constructor(key, state) {
    if (key !== BROWSER_PARTICIPATION_KEY) {
      throw new TypeError("BrowserBodyParticipation values come from BrowserHost.participate()");
    }
    this.#state = state;
    Object.freeze(this);
  }

  get hostId() { return this.#state.membership.hostId; }
  get bootId() { return this.#state.membership.bootId; }
  get advertisement() { return this.#state.membership.advertisement; }
  membershipCredential() { return this.#state.membership.membershipCredential(); }
  biographyEvidence() { return this.#state.membership.biographyEvidence(); }
  offerEvidence() { return this.#state.membership.offerEvidence(); }
  state() { return this.#state.membership.state(); }
  presenceState() { return this.#state.membership.presenceState(); }
  pageLifecycle() { return this.#state.membership.pageLifecycle(); }
  freshnessProfile() { return this.#state.membership.freshnessProfile(); }
  requestOfferEvidence(options) { return this.#state.membership.requestOfferEvidence(options); }
  /** One bounded owner-produced Face on this exact admitted browser carrier. */
  ownerFaceSnapshot(options) { return this.#state.membership.requestFaceSnapshot(options); }
  /** Inspect current owner policy, or apply one explicit revision-bound change at rest. */
  async inspectOwnerWardrobe() {
    const credential = this.membershipCredential();
    if (this.presenceState() !== "available" || !credential) throw new Error("current browser presence is required for owner wardrobe");
    return checkedOwnerWardrobeReport(await this.#state.membership.ownerWardrobe(), credential.body_id);
  }
  async changeOwnerWardrobe(report, routeId, verb) {
    const credential = this.membershipCredential();
    if (this.presenceState() !== "available" || !credential) throw new Error("current browser presence is required for owner wardrobe");
    const action = checkedOwnerWardrobeAction(report, credential.body_id, routeId, verb);
    this.#state.lastOwnerShow = null;
    const checked = checkedOwnerWardrobeReport(report, credential.body_id);
    return checkedOwnerWardrobeReport(await this.#state.membership.ownerWardrobe({
      action, ownerPlanId: checked.ownerPlanId, basisRevision: checked.revision,
    }), credential.body_id);
  }
  /** Run the exact owner Face through this Host's checked browser Mask Plot. */
  async prepareOwnerFaceMask(options) {
    this.#state.lastOwnerShow = null;
    const credential = this.membershipCredential();
    if (this.presenceState() !== "available" || !credential) {
      throw new Error("current browser presence is required for the owner Face Mask");
    }
    const frame = await this.ownerFaceSnapshot(options);
    if (this.presenceState() !== "available") throw new Error("owner Face became stale before Mask preparation");
    const view = this.#state.host.bridge.ownerFacePrepare({
      body_id: credential.body_id, host_id: this.hostId, boot_id: this.bootId,
    }, frame);
    if (view?.schema === "conduit.browser/owner-face-refusal@1") {
      throw new Error(`owner Face refused: ${view.code}`);
    }
    if (view?.schema !== "conduit.browser/owner-face-mask@1" ||
        view.body_id !== credential.body_id || !["prepared", "available"].includes(view.show_state) ||
        typeof view.interactions_admitted !== "boolean") {
      throw new Error("owner Face Mask did not prepare one exact Show");
    }
    return view;
  }
  async acknowledgeOwnerFaceMask(view) {
    const credential = this.membershipCredential();
    if (this.presenceState() !== "available" || !credential ||
        view?.body_id !== credential.body_id || !["prepared", "available"].includes(view?.show_state)) {
      throw new Error("owner Face Show no longer has current browser presence");
    }
    const shown = view.show_state === "available" ? view
      : this.#state.host.bridge.ownerFaceAcknowledge({
        show_id: view.show_id, face_id: view.face_id, face_revision: view.face_revision,
      });
    if (shown?.show_id !== view.show_id || shown?.face_id !== view.face_id ||
        shown?.face_revision !== view.face_revision || shown?.show_state !== "available" ||
        shown?.interactions_admitted !== view.interactions_admitted) {
      throw new Error("browser Mask acknowledged another owner Face Show");
    }
    await this.#state.membership.acknowledgeFaceShow(this.#state.host.bridge.ownerFaceShowReceipt());
    this.#state.lastOwnerShow = shown;
    return shown;
  }
  /** Ask the installed owner to read its current acknowledged graphical Show aloud. */
  selectedDirectSpeechStart(view) {
    const shown = this.#state.lastOwnerShow;
    if (this.presenceState() !== "available" || !shown || view?.show_state !== "available"
        || view.show_id !== shown.show_id || view.face_id !== shown.face_id
        || view.face_revision !== shown.face_revision) {
      throw new Error("current acknowledged owner Show is required for selected speech");
    }
    return this.#state.membership.selectedSpeech("start", {
      showBytes: this.#state.host.bridge.ownerFaceShowReceipt(),
    });
  }
  selectedDirectSpeechStatus(operationId) {
    return this.#state.membership.selectedSpeech("status", { operationId });
  }
  selectedDirectSpeechStop(operationId) {
    return this.#state.membership.selectedSpeech("stop", { operationId });
  }
  /** Emit one typed Mask interaction on this still-live owner window. */
  submitOwnerFaceInteraction({ view, actionId, target, arguments: actionArguments, sequence = 1 }) {
    const credential = this.membershipCredential();
    if (this.presenceState() !== "available" || !credential ||
        view?.body_id !== credential.body_id || view?.show_state !== "available" ||
        view?.interactions_admitted !== true) {
      throw new Error("current browser Show and owner presence are required for interaction");
    }
    if (!Number.isSafeInteger(sequence) || sequence < 1) {
      throw new Error("choose one valid Face action sequence");
    }
    const checked = checkedOwnerFaceArguments(view, actionId, target, actionArguments);
    const submission = this.#state.host.bridge.ownerFaceInteract({
      show_id: view.show_id, face_id: view.face_id, face_revision: view.face_revision,
      action_id: actionId, target, arguments: checked, sequence,
    });
    this.#state.lastOwnerShow = null;
    return this.#state.membership.submitFaceInteraction(submission);
  }
  signalWebRtc(options) { return this.#state.membership.signalWebRtc(options); }
  requestWebRtcGrant(index, generation = 0) { return this.#state.membership.requestWebRtcGrant(index, generation); }
  webRtcSessions() { return this.#state.membership.webRtcSessions(); }
  offerWebRtcValue(identity, bytes) { return this.#state.membership.offerWebRtcValue(identity, bytes); }
  receiveWebRtcValue(identity) { return this.#state.membership.receiveWebRtcValue(identity); }
  pressureNextWebRtcValue(identity) { return this.#state.membership.pressureNextWebRtcValue(identity); }
  deliverWebRtcValue(identity, sequence) { return this.#state.membership.deliverWebRtcValue(identity, sequence); }
  waitWebRtcValueDelivered(identity, sequence) { return this.#state.membership.waitWebRtcValueDelivered(identity, sequence); }
  closeWebRtcLine(identity) { return this.#state.membership.closeWebRtcLine(identity); }
  replanWebRtc() { return this.#state.membership.replanWebRtc(); }
  publishMediaResource(evidence) { return this.#state.membership.publishMediaResource(evidence); }
  close() {
    this.#state.host.bridge.ownerFaceClear?.();
    this.#state.lastOwnerShow = null;
    return this.#state.membership.close();
  }

  /** Exact capability identities executable by this SDK Host runtime. */
  executionCapabilities() {
    const api = this.#state.host.api;
    if (api.conduit_browser_body_capabilities() < 0) {
      throw new Error("external Body execution capabilities unavailable");
    }
    const value = this.#state.host.bridge.browserPlotReadOutputJson();
    if (value?.schema !== "conduit.browser/body-capabilities@1"
        || !Array.isArray(value.capability_ids) || value.capability_ids.length > 112
        || value.capability_ids.some(identity => typeof identity !== "string"
          || identity.length < 1 || identity.length > 256)) {
      throw new Error("invalid external Body execution capabilities");
    }
    return Object.freeze([...value.capability_ids]);
  }

  /** Acquire this page's exact resources for an authority-owned external Body proposal. */
  prepare({ proposal, inputTarget, outputRoot, foregroundPlot, presentationRootFor,
    onApplicationEvent, onTutorialPresenterRequest, externallyManagedPlanIds = [] } = {}) {
    const credential = this.membershipCredential();
    if (this.presenceState() !== "available" || !credential
        || proposal?.plan?.body_id !== credential.body_id) {
      throw new Error("current external Body participation does not match the proposal");
    }
    const owner = acquireBrowserBodyHost({
      api: this.#state.host.api,
      hostId: this.hostId,
      bootId: this.bootId,
      proposal,
      storage: this.#state.host.storage,
      inputTarget,
      outputRoot,
      foregroundPlot,
      presentationRootFor,
      onApplicationEvent,
      onTutorialPresenterRequest,
      externallyManagedPlanIds,
    });
    return new BrowserBodyPreparation(BROWSER_PREPARATION_KEY, { owner, proposal });
  }
}

/** Page resources acquired for one exact external Body proposal, before Play admission. */
export class BrowserBodyPreparation {
  #state;

  constructor(key, state) {
    if (key !== BROWSER_PREPARATION_KEY) {
      throw new TypeError("BrowserBodyPreparation values come from BrowserBodyParticipation.prepare()");
    }
    this.#state = state;
    Object.freeze(this);
  }

  observations() { return Object.freeze(this.#state.owner.observations().map(item => Object.freeze({ ...item }))); }
  evidence() { return this.#state.owner.evidence(); }

  /** Immutable, read-only Plan truth suitable for a workbench Mask. */
  inspection() {
    const proposal = this.#state.proposal;
    return freezeSdkJson({
      schema: "conduit.browser/body-plan-inspection@1",
      bodyId: proposal.plan.body_id,
      wakeId: proposal.wake.wake_id,
      planId: proposal.plan.plan_id,
      plan: structuredClone(proposal.plan),
    });
  }

  start(playIdentity) {
    if (!playIdentity || !Number.isSafeInteger(playIdentity.play_sequence)
        || playIdentity.play_sequence < 1 || playIdentity.plan_id !== this.#state.proposal.plan.plan_id
        || playIdentity.wake_id !== this.#state.proposal.wake.wake_id
        || playIdentity.body_id !== this.#state.proposal.plan.body_id) {
      throw new Error("external Body Play identity does not match the prepared proposal");
    }
    const started = this.#state.owner.start(playIdentity.play_sequence);
    if (Object.keys(playIdentity).some(key => started.play[key] !== playIdentity[key])) {
      this.#state.owner.close();
      throw new Error("external Body runtime started a different Play identity");
    }
    return Object.freeze({
      wakeAtStart: Object.freeze(structuredClone(started.wake_at_start)),
      play: new BrowserPlay(BROWSER_HOST_KEY, { started, adapter: this.#state.owner }),
    });
  }

  close() { return this.#state.owner.close(); }
}

function freezeSdkJson(value) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return value;
  for (const child of Object.values(value)) freezeSdkJson(child);
  return Object.freeze(value);
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
  get state() { return this.#terminal ? "terminal" : this.#failure ? "failed" : "playing"; }
  get receipts() { return Object.freeze(this.#terminal ? [this.#terminal] : []); }
  dispatch() {
    if (!this.#completion) this.#completion = this.#adapter.run().then(receipt => {
      this.#terminal = receipt;
      return receipt;
    }, error => {
      this.#failure = sdkRefusal("Play.dispatch", {
        code: error?.refusal?.code ?? error?.code ?? "PlayExecutionFailed",
        message: error?.message ?? "Browser Host effect dispatch failed",
        ...(error?.refusal ?? {}),
      }, { bodyId: this.#started.play.body_id, planId: this.#started.play.plan_id,
        wakeId: this.#started.play.wake_id, playId: this.id });
      throw this.#failure;
    });
    return this.#completion;
  }
  async terminate() {
    // Retire the actual kernel even after dispatch refused. Keep the original
    // failure on this handle and its rejected completion promise; the terminal
    // receipt lets Body.lull/forget close execution through the ordinary runtime.
    const closed = this.#adapter.close();
    const receipt = closed.receipt ?? this.#terminal;
    if (!this.#failure) this.#terminal = receipt;
    return receipt;
  }
}

/**
 * Start one exact BrowserBundle as a BrowserHost in an ordinary static page.
 * The bundle must be produced and reviewed before this call; this loader never
 * compiles, discovers a CDN, or broadens its implementation selection.
 */
export const Conduit = Object.freeze({
  async browser({ root, bundleRoot = new URL("./bundle/", import.meta.url), profileId, durable = true, application } = {}) {
    try {
      if (root !== undefined && !(root instanceof Element || root instanceof ShadowRoot)) {
        refuse("InvalidRoot", "A supplied Browser Host root must be an application-owned DOM element or shadow root");
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
      const implementationRegistry = boot.offers.map(({ implementation_id }) => implementation_id);
      const continuityConfig = await applicationContinuity(application, pkg.bundle_sha256);
      const storage = durable && implementationRegistry.includes("browser/indexeddb@1") ? await openBrowserApplicationStorage(
        continuityConfig.identity,
        continuityConfig.version,
        continuityConfig.packageDigest,
        { implementationRegistry },
      ) : null;
      const state = {
        packageVersion: pkg.package_version,
        runtimeAbi: pkg.runtime_abi,
        hostId: initialized.hostId,
        api,
        runtimeBytes,
        root,
        bridge,
        membership: initialized.membership,
        storage,
        continuity: createSdkContinuity({ storage, identity: continuityConfig.identity,
          refusal: (code, message) => new InvalidLifecycleError({ code, message, operation: "BrowserHost.continuity" }) }),
        sequence: 0,
        boot,
        offers: Object.freeze(boot.offers.map((offer) => Object.freeze({ ...offer }))),
        async refresh() {
          this.boot = bootModule.refreshBrowserBootTruth(this.boot, await bootModule.observeBrowserHostEnvironment(globalThis));
          this.offers = Object.freeze(this.boot.offers.map((offer) => Object.freeze({ ...offer })));
        },
      };
      const host = new BrowserHost(BROWSER_HOST_KEY, state);
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
    || distribution.make_package_id !== "browser-wasm@1" || distribution.output !== "browser-bundle"
    || distribution.reviewed_distribution?.schema !== DISTRIBUTION_SCHEMA
    || distribution.reviewed_distribution.runtime_abi !== pkg.runtime_abi
    || distribution.bundle_sha256 !== image.reviewed_distribution?.distribution_digest) {
    refuse("IncompatibleDistribution", "reviewed BrowserBundle distribution does not match the SDK package ABI and identity");
  }
  if (bundle?.schema !== RELEASE_SCHEMA || bundle.target_id !== pkg.target_id
    || bundle.make_package_id !== "browser-wasm@1" || bundle.output !== "browser-bundle"
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

// Current Body adapters acquire connected application surfaces before starting Play.
// Host admission itself neither acquires these resources nor paints diagnostics.
function acquireSdkBodyHost(options) {
  if (!options.outputRoot?.isConnected || !options.inputTarget?.isConnected) {
    throw new ResourceLossError({ code: "ApplicationSurfaceUnavailable", operation: "Body.wake",
      message: "Browser Body execution requires a connected application-owned root supplied to Conduit.browser({ root })",
      identities: { hostId: options.hostId, bootId: options.bootId } });
  }
  return acquireBrowserBodyHost(options);
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
