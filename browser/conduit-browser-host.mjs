// Browser host adapter v1. This module deliberately has no DOM or Patchbay
// dependency: a browser can be an execution host and a presentation client at
// the same time, but those are separate identities.

export const BROWSER_HOST_SCHEMA_VERSION = 1;

export const PermissionState = Object.freeze({
  Granted: "granted",
  Prompt: "prompt",
  Denied: "denied",
  Unavailable: "unavailable",
});

export const Placement = Object.freeze({
  Window: "window",
  DedicatedWorker: "dedicated-worker",
  SharedWorker: "shared-worker",
  ServiceWorker: "service-worker",
  AudioWorklet: "audio-worklet",
  Wasm: "wasm",
  WebGpu: "webgpu",
});

export const BrowserHostReason = Object.freeze({
  InvalidReport: "CND-BRW-001",
  PromptIsNotGrant: "CND-BRW-002",
  ActivationRequired: "CND-BRW-003",
  StaleObservation: "CND-BRW-004",
  InvalidPlan: "CND-BRW-005",
  ArtifactIntegrity: "CND-BRW-006",
  QueueCapacity: "CND-BRW-007",
  PlacementUnavailable: "CND-BRW-008",
  Terminal: "CND-BRW-009",
});

function fail(code, detail) {
  return { ok: false, code, detail };
}

function sortedUnique(values) {
  return [...new Set(values)].sort();
}

function cloneFact(fact) {
  return Object.freeze({ ...fact });
}

/**
 * Creates a fresh report from explicit observer output. Calling this function
 * is the observation boundary; resolution consumes its frozen result and
 * never performs feature detection, permission queries, or user prompts.
 */
export function observeBrowserHost(input) {
  const {
    hostId,
    observationId,
    reporter,
    tick,
    validUntilTick,
    context,
    placements,
    permissions = [],
    activation = false,
    resources,
  } = input;
  if (!hostId || !observationId || !reporter?.realmId || !reporter?.entityId ||
      !reporter?.passportIdentity || !reporter?.statusObservation ||
      !Number.isSafeInteger(tick) || !Number.isSafeInteger(validUntilTick) ||
      tick > validUntilTick || !context || !resources) {
    return fail(BrowserHostReason.InvalidReport, "missing or malformed observation facts");
  }
  const frozenPlacements = placements.map((placement) => cloneFact({
    id: placement.id,
    available: placement.available === true,
    lifetime: placement.lifetime,
    scheduling: placement.scheduling,
    transfer: placement.transfer,
    limits: Object.freeze({ ...placement.limits }),
    terminalRisks: Object.freeze(sortedUnique(placement.terminalRisks ?? [])),
  })).sort((left, right) => left.id.localeCompare(right.id));
  const frozenPermissions = permissions.map((permission) => cloneFact({
    capability: permission.capability,
    state: permission.state,
    scope: permission.scope,
  })).sort((left, right) => left.capability.localeCompare(right.capability));
  if (frozenPermissions.some((permission) => !Object.values(PermissionState).includes(permission.state))) {
    return fail(BrowserHostReason.InvalidReport, "unknown permission state");
  }
  return Object.freeze({
    schemaVersion: BROWSER_HOST_SCHEMA_VERSION,
    hostId,
    observationId,
    // The adapter only retains exact #88 identity/status references. It does
    // not validate credentials, enroll entities, rotate keys, or prompt.
    reporter: Object.freeze({ ...reporter }),
    observedAtTick: tick,
    validUntilTick,
    context: Object.freeze({ ...context }),
    placements: Object.freeze(frozenPlacements),
    permissions: Object.freeze(frozenPermissions),
    activation: activation === true,
    resources: Object.freeze({ ...resources }),
  });
}

/** Pure resolver predicate: no browser APIs are read or called here. */
export function resolveBrowserPlacement(report, request) {
  if (!report || report.schemaVersion !== BROWSER_HOST_SCHEMA_VERSION || request.tick > report.validUntilTick) {
    return fail(BrowserHostReason.StaleObservation, "fresh report required");
  }
  const placement = report.placements.find((candidate) => candidate.id === request.placement);
  if (!placement || !placement.available) {
    return fail(BrowserHostReason.PlacementUnavailable, request.placement);
  }
  for (const required of request.permissions ?? []) {
    const permission = report.permissions.find((candidate) => candidate.capability === required);
    if (!permission || permission.state !== PermissionState.Granted) {
      return fail(BrowserHostReason.PromptIsNotGrant, required);
    }
  }
  if (request.requiresActivation && !report.activation) {
    return fail(BrowserHostReason.ActivationRequired, request.placement);
  }
  for (const [resource, minimum] of Object.entries(request.minimumResources ?? {})) {
    if (!Number.isFinite(report.resources[resource]) || report.resources[resource] < minimum) {
      return fail(BrowserHostReason.QueueCapacity, resource);
    }
  }
  return Object.freeze({
    ok: true,
    hostId: report.hostId,
    observationId: report.observationId,
    placement: placement.id,
    allocation: Object.freeze({ ...request.minimumResources }),
  });
}

/** A bounded adapter-owned queue; external browser/provider memory is never counted as this queue. */
export class BoundedPortQueue {
  #values = [];
  constructor(capacity) {
    if (!Number.isSafeInteger(capacity) || capacity <= 0) throw new RangeError(BrowserHostReason.QueueCapacity);
    this.capacity = capacity;
  }
  offer(value) {
    if (this.#values.length === this.capacity) return fail(BrowserHostReason.QueueCapacity, "full");
    this.#values.push(value);
    return { ok: true, size: this.#values.length };
  }
  take() { return this.#values.shift(); }
  get size() { return this.#values.length; }
}

/**
 * Validates supplied bytes before an already-resolved adapter loads them. It
 * neither fetches an artifact nor changes a report/plan.
 */
export async function verifyExactArtifact(bytes, expectedSha256Hex, cryptoProvider = globalThis.crypto) {
  if (!cryptoProvider?.subtle || !/^[0-9a-f]{64}$/i.test(expectedSha256Hex)) {
    return fail(BrowserHostReason.ArtifactIntegrity, "invalid digest or crypto provider");
  }
  const digest = await cryptoProvider.subtle.digest("SHA-256", bytes);
  const actual = [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("");
  return actual === expectedSha256Hex.toLowerCase()
    ? { ok: true }
    : fail(BrowserHostReason.ArtifactIntegrity, "digest mismatch");
}

/**
 * A tiny lifecycle bridge for adapters. The supplied implementation is an
 * exact plan binding, not a DOM callback or an implicit placement fallback.
 */
export class BrowserExecutionAdapter {
  #state = "prepared";
  constructor(binding, emitEvidence) {
    if (!binding?.planIdentity || !binding?.placement || typeof emitEvidence !== "function") {
      throw new TypeError(BrowserHostReason.InvalidPlan);
    }
    this.binding = Object.freeze({ ...binding });
    this.emitEvidence = emitEvidence;
  }
  start() {
    if (this.#state !== "prepared") return fail(BrowserHostReason.Terminal, this.#state);
    this.#state = "running";
    this.emitEvidence(Object.freeze({ kind: "started", planIdentity: this.binding.planIdentity }));
    return { ok: true };
  }
  terminate(cause) {
    if (this.#state === "terminal") return { ok: true };
    this.#state = "terminal";
    this.emitEvidence(Object.freeze({ kind: "terminal", cause, planIdentity: this.binding.planIdentity }));
    return { ok: true };
  }
  get state() { return this.#state; }
}
