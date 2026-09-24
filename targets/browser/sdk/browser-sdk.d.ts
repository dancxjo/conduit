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

/** Read-only projection of one exact browser Host and Boot incarnation. */
export class BrowserHost {
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
  form(source: string): BrowserForm;
  /** Rust expands selected Forms against this exact Host without acquiring resources or making a Plan. */
  review(forms: readonly (BrowserForm | CheckedForm | CheckedSource)[]): Promise<FormWorkloadReview>;
  /** Birth a Body only from Forms checked by this Host's exact Browser runtime. */
  birth(options: { name: string; forms: readonly (BrowserForm | CheckedForm | CheckedSource)[] }): Promise<BrowserBody>;
}

export interface FormDiagnostic {
  readonly code: string;
  readonly message: string;
  readonly span?: Readonly<{ start: number; end: number; line: number; column: number; endLine: number; endColumn: number }>;
}

export interface FormRequirements {
  /** Canonical Kind identities found by Rust checking. */
  readonly kinds: readonly string[];
  /** Null until a Rust planner has derived exact requirements for a proposed realization. */
  readonly resources: null;
  /** Null until a Rust planner has derived exact requirements for a proposed realization. */
  readonly capabilities: null;
}

export interface CheckedForm {
  readonly schema: "conduit.browser/checked-form@1";
  readonly name: string;
  readonly title: string;
  readonly source: string;
  readonly documentSource: string;
  readonly sourceDocumentId: string;
  readonly checkedFormId: string;
  readonly requirements: FormRequirements;
  readonly diagnostics: readonly FormDiagnostic[];
  readonly identity: Readonly<{ sourceDocumentId: string; checkedFormId: string }>;
}

export interface CheckedSource {
  readonly schema: "conduit.browser/checked-source@1";
  readonly ok: boolean;
  readonly sourceDocumentId: string | null;
  readonly diagnostics: readonly FormDiagnostic[];
  readonly requirements: FormRequirements;
  readonly forms: readonly CheckedForm[];
  readonly source: string;
  readonly refusal?: Readonly<{ code: string; message: string }>;
}

export interface FormWorkloadReview {
  readonly schema: "conduit.browser/form-workload-review@1";
  readonly sourceDocumentId: string;
  readonly checkedForms: readonly Readonly<{ sourceDocumentId: string; checkedFormId: string }>[];
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

export class BrowserForm {
  private constructor(source: string);
  readonly schema: "conduit.browser/form-source@1";
  readonly source: string;
  check(): Promise<CheckedSource>;
}

export class BrowserBody {
  readonly schema: "conduit.browser/body@1";
  readonly id: string;
  readonly receipt: Readonly<Record<string, unknown>>;
  current(): Promise<Readonly<Record<string, unknown>>>;
  install(form: BrowserForm | CheckedForm | CheckedSource): Promise<Readonly<Record<string, unknown>>>;
  remove(form: BrowserForm | CheckedForm | CheckedSource): Promise<Readonly<Record<string, unknown>>>;
}

export interface BrowserOptions {
  /** An application-owned Element or ShadowRoot used only for presentation. */
  root: Element | ShadowRoot;
  /** Same-origin directory containing the exact package-bound BrowserBundle. */
  bundleRoot?: string | URL;
  /** Optional assertion that this package contains the expected checked PROFILE. */
  profileId?: string;
  /** Use a fresh Host identity for an intentionally ephemeral Host. */
  durable?: boolean;
}

export const Conduit: Readonly<{
  browser(options: BrowserOptions): Promise<BrowserHost>;
}>;
