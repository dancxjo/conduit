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
