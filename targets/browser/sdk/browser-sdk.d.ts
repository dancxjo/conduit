export interface BrowserOffer {
  readonly implementation_id: string;
  readonly implementation_revision: number;
  readonly offer_id: string;
}

/** Typed refusal retaining the source evidence and operation identity. */
export class ConduitSdkError extends Error {
  protected constructor(details: {
    code: string; category?: string; message: string; operation: string;
    evidence: Readonly<Record<string, unknown>>;
    identities?: Readonly<Record<string, string>>;
  });
  readonly code: string;
  readonly category: string;
  readonly operation: string;
  readonly evidence: Readonly<Record<string, unknown>>;
  readonly identities: Readonly<Record<string, string>>;
}

export class PlanRefusalError extends ConduitSdkError { readonly category: "PlanRefusal"; }
export class ResourceLossError extends ConduitSdkError { readonly category: "ResourceLoss"; }
export class InvalidLifecycleError extends ConduitSdkError { readonly category: "InvalidLifecycle"; }
export class PermissionDeniedError extends ConduitSdkError { readonly category: "PermissionDenied"; }
export class IncompatibleRuntimeAbiError extends ConduitSdkError { readonly category: "IncompatibleRuntimeAbi"; }

/** Bounded same-origin client for one HTTP-carried Body Face. */
export class BrowserFaceError extends Error {
  readonly code: "FaceUnavailable" | "FaceHttpRefusal" | "FaceResponseBound" | "FaceResponseMalformed";
  readonly operation: string;
  readonly status: number | null;
}

export class BrowserFaceClient {
  constructor(options?: {
    base?: string | URL;
    fetch?: typeof globalThis.fetch;
    maximumResponseBytes?: number;
  });
  snapshot(): Promise<Readonly<Record<string, unknown>>>;
  interact(interaction: Readonly<Record<string, unknown>>): Promise<Readonly<Record<string, unknown>>>;
}

/** Read-only projection of one exact browser Host and Boot incarnation. */
export interface BrowserSyntaxProjection {
  readonly protocol: "conduit.syntax-highlight-projection@1";
  readonly source_bytes: number;
  readonly kinds: readonly string[];
  readonly spans: readonly (readonly [number, number, number])[];
}
export interface BrowserSyntaxEditor {
  render(): void;
  destroy(): void;
}

export class BrowserHost {
  /** Attach native syntax highlighting while preserving the accessible textarea. */
  attachEditor(textarea: HTMLTextAreaElement): BrowserSyntaxEditor;
  /** Return the bounded canonical Rust syntax projection, or throw on refusal. */
  syntax(source: string): BrowserSyntaxProjection;
  private constructor();
  readonly schema: "conduit.browser/host@1";
  readonly packageVersion: string;
  readonly runtimeAbi: "conduit.browser/runtime-abi@1";
  readonly id: string;
  readonly bootId: string;
  readonly profileId: string;
  readonly imageId: string;
  readonly offers: readonly BrowserOffer[];
  readonly state: Readonly<{ schema: "conduit.browser/boot-truth@1"; generation: number }>;
  current(): Readonly<{
    schema: "conduit.browser/host-snapshot@1";
    id: string;
    bootId: string;
    profileId: string;
    imageId: string;
    offers: readonly BrowserOffer[];
    state: Readonly<{ schema: "conduit.browser/boot-truth@1"; generation: number }>;
  }>;
  refresh(): Promise<ReturnType<BrowserHost["current"]>>;
  /** Preserve ordinary Conduitese source; the canonical Rust checker owns its meaning. */
  plot(source: string): BrowserPlot;
  /** Rust expands selected Plots against this exact Host without acquiring resources or making a Plan. */
  review(plots: readonly (BrowserPlot | CheckedPlot | CheckedSource)[]): Promise<PlotWorkloadReview>;
  /** Participate in one externally owned Body without creating a second Host or Boot. */
  participate(options: {
    invitation: string;
    expectedBodyId?: string | null;
    retainedCredential?: Readonly<Record<string, unknown>> | null;
    onCredential?: (credential: Readonly<Record<string, unknown>>) => void | Promise<void>;
    onState?: (state: string) => void;
    onBiographyEvidence?: (evidence: Readonly<Record<string, unknown>>) => void;
    onOfferEvidence?: (evidence: Readonly<Record<string, unknown>>) => void;
    renewPresence?: boolean;
    reconnectPresence?: boolean;
  }): Promise<BrowserBodyParticipation>;
  /** Birth a Body only from Plots checked by this Host's exact Browser runtime. */
  birth(options: { name: string; plots: readonly (BrowserPlot | CheckedPlot | CheckedSource)[] }): Promise<BrowserBody>;
  /** Recover the retained Body under this fresh Boot. Returns null when no Body is retained. */
  recover(): Promise<BrowserBody | null>;
  /** Terminate owned execution and release this application session. */
  close(): Promise<void>;
  /** Forget only this application state, preserving durable Host identity. */
  forget(): Promise<void>;
}

export class BrowserBodyParticipation {
  private constructor();
  readonly hostId: string;
  readonly bootId: string;
  readonly advertisement: Readonly<Record<string, unknown>>;
  membershipCredential(): Readonly<Record<string, unknown>> | null;
  biographyEvidence(): Readonly<Record<string, unknown>> | null;
  offerEvidence(): Readonly<Record<string, unknown>> | null;
  state(): string;
  presenceState(): string;
  pageLifecycle(): string;
  freshnessProfile(): Readonly<Record<string, unknown>>;
  requestOfferEvidence(options: { capabilityIds?: readonly string[]; resourcePoolIds?: readonly string[] }): void;
  signalWebRtc(options: Readonly<Record<string, unknown>>): void;
  requestWebRtcGrant(index: number, generation?: number): void;
  webRtcSessions(): Readonly<Record<string, unknown>>;
  offerWebRtcValue(identity: string, bytes: Uint8Array | readonly number[]): Promise<Readonly<Record<string, unknown>>>;
  receiveWebRtcValue(identity: string): Promise<unknown>;
  pressureNextWebRtcValue(identity: string): unknown;
  deliverWebRtcValue(identity: string, sequence: number): unknown;
  waitWebRtcValueDelivered(identity: string, sequence: number): Promise<unknown>;
  closeWebRtcLine(identity: string): unknown;
  replanWebRtc(): unknown;
  publishMediaResource(evidence: Readonly<Record<string, unknown>>): Promise<Readonly<Record<string, unknown>>>;
  executionCapabilities(): readonly string[];
  prepare(options: {
    proposal: Readonly<Record<string, unknown>>;
    inputTarget: Element;
    outputRoot: Element;
    externallyManagedPlanIds?: readonly string[];
  }): BrowserBodyPreparation;
  close(): number;
}

export class BrowserBodyPreparation {
  private constructor();
  observations(): readonly Readonly<Record<string, unknown>>[];
  evidence(): Readonly<Record<string, unknown>> | null;
  /** Immutable exact Plan truth for read-only workbench Masks. */
  inspection(): Readonly<{
    schema: "conduit.browser/body-plan-inspection@1";
    bodyId: string;
    wakeId: string;
    planId: string;
    plan: Readonly<Record<string, unknown>>;
  }>;
  start(playIdentity: Readonly<Record<string, unknown>>): {
    readonly wakeAtStart: Readonly<Record<string, unknown>>;
    readonly play: BrowserPlay;
  };
  close(): Readonly<Record<string, unknown>> | undefined;
}

export interface PlotDiagnostic {
  readonly code: string;
  readonly message: string;
  readonly span?: Readonly<{ start: number; end: number; line: number; column: number; endLine: number; endColumn: number }>;
}

export interface PlotRequirements {
  /** Canonical Kind identities found by Rust checking. */
  readonly kinds: readonly string[];
  /** Null until a Rust planner has derived exact requirements for a proposed realization. */
  readonly resources: null;
  /** Null until a Rust planner has derived exact requirements for a proposed realization. */
  readonly capabilities: null;
}

export interface CheckedPlot {
  readonly schema: "conduit.browser/checked-plot@1";
  readonly name: string;
  readonly title: string;
  readonly source: string;
  readonly documentSource: string;
  readonly sourceDocumentId: string;
  readonly checkedPlotId: string;
  readonly requirements: PlotRequirements;
  readonly diagnostics: readonly PlotDiagnostic[];
  readonly identity: Readonly<{ sourceDocumentId: string; checkedPlotId: string }>;
}

export interface CheckedSource {
  readonly schema: "conduit.browser/checked-source@1";
  readonly ok: boolean;
  readonly sourceDocumentId: string | null;
  readonly diagnostics: readonly PlotDiagnostic[];
  readonly requirements: PlotRequirements;
  readonly plots: readonly CheckedPlot[];
  readonly source: string;
  readonly refusal?: Readonly<{ code: string; message: string }>;
}

export interface PlotWorkloadReview {
  readonly schema: "conduit.browser/plot-workload-review@1";
  readonly sourceDocumentId: string;
  readonly checkedPlots: readonly Readonly<{ sourceDocumentId: string; checkedPlotId: string }>[];
  readonly requirements: Readonly<{
    kinds: readonly string[];
    resources: readonly Readonly<{ hostId: string; resourceClassId: string; units: number }>[];
    capabilities: readonly Readonly<{ hostId: string; capabilityId: string; activeInstances: number }>[];
  }>;
  readonly proposedHosts: readonly Readonly<{ hostId: string; bootId: string; profileId: string; offerGeneration: number }>[];
  readonly reviewedRealizationCount: number;
  readonly bodyPlanCreated: false;
  readonly playCreated: false;
  readonly authorityAcquired: false;
  readonly resourcesAcquired: false;
  readonly evidence: Readonly<Record<string, unknown>>;
}

export class BrowserPlot {
  private constructor(source: string);
  readonly schema: "conduit.browser/plot-source@1";
  readonly source: string;
  check(): Promise<CheckedSource>;
  /** Rust-checked authoring projection; creates no Body, Plan, Play, or authority. */
  patchbay(): BrowserPlotPatchbay;
}

export interface BrowserPlotPatchbay extends Readonly<Record<string, unknown>> {
  readonly schema: "conduit.patchbay/checked-plot-projection@1";
  readonly source_document_id: string;
  readonly checked_plot_id: string;
  readonly visible_expanded_plot_id: string;
  readonly plot_name: string;
  readonly front_inputs: readonly BrowserPatchbayPort[];
  readonly front_outputs: readonly BrowserPatchbayPort[];
  readonly gears: readonly Readonly<Record<string, unknown>>[];
  readonly cords: readonly Readonly<Record<string, unknown>>[];
}

export class BrowserBody {
  readonly schema: "conduit.browser/body@1";
  readonly id: string;
  readonly receipt: Readonly<Record<string, unknown>>;
  current(): Promise<Readonly<Record<string, unknown>>>;
  /** Refresh one immutable projection of current Rust Body and Host evidence. */
  snapshot(): Promise<BrowserBodySnapshot>;
  plots(): Promise<CheckedSource>;
  /** Project the selected resident Plot from a bundle (or the sole resident Plot). Refuses identity mismatch. */
  patchbay(): Promise<BrowserBodyPatchbay>;
  /** Stream retained runtime events; replay is opt-in and notifications are polled/coalesced from bounded evidence. */
  events(options?: { replay?: boolean; pollIntervalMillis?: number; signal?: AbortSignal }): AsyncIterable<BrowserBodyEvent>;
  /** Propose, admit, and start one exact runtime Play through the reviewed Browser Host adapters. */
  wake(options?: { root?: Element | ShadowRoot; presentationRootFor?: (placement: Readonly<{
    checkedPlotId: string; placementId: string; gearId: string;
  }>) => Element | ShadowRoot }): Promise<BrowserPlay>;
  /** Terminate or close the current Play and record the exact Body lull transition. */
  lull(): Promise<Readonly<Record<string, unknown>>>;
  close(): Promise<void>;
  select(plot: BrowserPlot | CheckedPlot | CheckedSource): Promise<Readonly<Record<string, unknown>>>;
  install(plot: BrowserPlot | CheckedPlot | CheckedSource): Promise<Readonly<Record<string, unknown>>>;
  replace(previous: BrowserPlot | CheckedPlot | CheckedSource, plot: BrowserPlot | CheckedPlot | CheckedSource): Promise<Readonly<Record<string, unknown>>>;
  remove(plot: BrowserPlot | CheckedPlot | CheckedSource): Promise<Readonly<Record<string, unknown>>>;
}

export interface BrowserBodyPatchbay {
  readonly schema: "conduit.browser/body-patchbay@1";
  readonly bodyId: string;
  readonly hostId: string;
  readonly bootId: string;
  readonly planId: string | null;
  readonly playId: string | null;
  readonly topology: Readonly<{
    schema: "conduit.patchbay/checked-plot-projection@1";
    source_document_id: string;
    checked_plot_id: string;
    visible_expanded_plot_id: string;
    plot_name: string;
    front_inputs: readonly BrowserPatchbayPort[];
    front_outputs: readonly BrowserPatchbayPort[];
    gears: readonly Readonly<{
      gear_id: string;
      kind_id: string;
      inputs: readonly BrowserPatchbayPort[];
      outputs: readonly BrowserPatchbayPort[];
    }>[];
    cords: readonly Readonly<Record<string, unknown>>[];
  }>;
}

export interface BrowserPatchbayPort {
  readonly port_id: string;
  readonly info_kind: string;
  readonly temporal: string;
  /** Exact Rust-checked contract; null means no additional value refinement. */
  readonly value_contract: Readonly<{
    value_kind: string;
    maximum_bytes: number;
    constraints: readonly Readonly<Record<string, unknown>>[];
  }> | null;
}

export interface BrowserBodySnapshot extends Readonly<Record<string, unknown>> {
  readonly schema: "conduit.workspace/body@1";
  readonly evidence: Readonly<Record<string, unknown>> & {
    readonly body_id: string;
    readonly records: readonly Readonly<Record<string, unknown>>[];
    readonly membership: Readonly<{ revision: number }>;
    readonly body: Readonly<{ workload_revision: number }>;
  };
  readonly current_host_offers: readonly BrowserOffer[];
}

export interface BrowserBodyEvent {
  readonly schema: "conduit.browser/body-event@1";
  readonly id: string;
  readonly type: string;
  readonly identity: Readonly<Record<string, string | number | readonly string[]>>;
  readonly pressure?: Readonly<{ coalescedGenerationAdvances: number }>;
  readonly evidence: Readonly<Record<string, unknown>>;
}

export class BrowserPlay {
  private constructor();
  readonly schema: "conduit.browser/play@1";
  readonly id: string;
  readonly identity: Readonly<Record<string, string>>;
  readonly plan: Readonly<{ planId: string; wakeId: string; bodyId: string }>;
  readonly state: "playing" | "terminal" | "failed";
  readonly receipts: readonly Readonly<Record<string, unknown>>[];
  dispatch(): Promise<Readonly<Record<string, unknown>> | null>;
  terminate(): Promise<Readonly<Record<string, unknown>> | null>;
}

export interface BrowserOptions {
  /** An application-owned Element or ShadowRoot used only for presentation. */
  root?: Element | ShadowRoot;
  /** Same-origin directory containing the exact package-bound BrowserBundle. */
  bundleRoot?: string | URL;
  /** Optional assertion that this package contains the expected checked PROFILE. */
  profileId?: string;
  /** Use a fresh Host identity for an intentionally ephemeral Host. */
  durable?: boolean;
  application?: {
    identity: string;
    stateCompatibility: { identity: string; version: number };
    packageDigest: string;
  };
}

export const Conduit: Readonly<{
  browser(options?: BrowserOptions): Promise<BrowserHost>;
}>;
