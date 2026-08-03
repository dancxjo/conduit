// Current browser host adapter. This module deliberately has no DOM or Patchbay
// dependency: a browser can be an execution host and a presentation client at
// the same time, but those are separate identities.

export const BROWSER_HOST_SCHEMA_VERSION = 0;

export const PermissionState = Object.freeze({
  Granted: "granted",
  Prompt: "prompt",
  Denied: "denied",
  Unavailable: "unavailable",
});

export const TaskActionPermissionState = Object.freeze({
  Permitted: "permitted",
  Denied: "denied",
  Revoked: "revoked",
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
  ImplicitEnrollment: "CND-GEN-005",
});

export const BrowserMembershipSignal = Object.freeze({
  Navigation: "browser-navigation",
  PwaInstall: "pwa-install",
  Permission: "browser-permission",
  TransportHandshake: "transport-handshake",
  CapabilityReport: "capability-report",
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
 * Browser lifecycle and capability signals are observations only. They never
 * become a realm enrollment request, passport, role, or effect grant.
 */
export function assessBrowserMembershipSignal(signal) {
  if (!Object.values(BrowserMembershipSignal).includes(signal)) {
    return fail(BrowserHostReason.InvalidReport, "unknown browser membership signal");
  }
  return fail(BrowserHostReason.ImplicitEnrollment, signal);
}

function validStatusBinding(reporter, tick) {
  const status = reporter?.statusObservation;
  return Boolean(
    reporter?.realmId &&
    reporter?.entityId &&
    reporter?.passportIdentity &&
    status &&
    status.realmId === reporter.realmId &&
    status.entityId === reporter.entityId &&
    status.passportIdentity === reporter.passportIdentity &&
    status.reporterIdentity &&
    status.timeBasis &&
    Number.isSafeInteger(status.observedAtTick) &&
    Number.isSafeInteger(status.validUntilTick) &&
    status.observedAtTick <= tick &&
    tick < status.validUntilTick &&
    status.status === "active"
  );
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
    taskActionPolicies = [],
    activation = false,
    resources,
  } = input;
  if (!hostId || !observationId || !validStatusBinding(reporter, tick) ||
      !Number.isSafeInteger(tick) || !Number.isSafeInteger(validUntilTick) ||
      tick >= validUntilTick || !context || !resources ||
      !Array.isArray(placements) || !Array.isArray(permissions) ||
      !Array.isArray(taskActionPolicies)) {
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
  const frozenTaskActionPolicies = taskActionPolicies.map((policy) => cloneFact({
    schemaVersion: policy.schemaVersion,
    observationId: policy.observationId,
    generation: policy.generation,
    action: policy.action,
    activeControls: Object.freeze([...(policy.activeControls ?? [])]),
    state: policy.state,
    observedAtTick: policy.observedAtTick,
    validUntilTick: policy.validUntilTick,
    code: policy.code,
    explanation: policy.explanation,
  })).sort((left, right) => left.action.localeCompare(right.action));
  const boundedPolicyText = (value) =>
    typeof value === "string" && value.length > 0 && value.length <= 256;
  if (frozenPlacements.some((placement) =>
        !Object.values(Placement).includes(placement.id) ||
        !placement.lifetime || !placement.scheduling || !placement.transfer ||
        !Number.isSafeInteger(placement.limits?.queueBytes) ||
        placement.limits.queueBytes <= 0) ||
      new Set(frozenPlacements.map((placement) => placement.id)).size !== frozenPlacements.length ||
      frozenPermissions.some((permission) => !Object.values(PermissionState).includes(permission.state)) ||
      frozenTaskActionPolicies.some((policy) =>
        policy.schemaVersion !== BROWSER_HOST_SCHEMA_VERSION ||
        !boundedPolicyText(policy.observationId) ||
        !Number.isSafeInteger(policy.generation) || policy.generation <= 0 ||
        policy.action !== "run-exact-plan" ||
        policy.activeControls.length > 2 ||
        policy.activeControls.some((control) => !["cancel", "drain"].includes(control)) ||
        new Set(policy.activeControls).size !== policy.activeControls.length ||
        !Object.values(TaskActionPermissionState).includes(policy.state) ||
        !Number.isSafeInteger(policy.observedAtTick) ||
        !Number.isSafeInteger(policy.validUntilTick) ||
        policy.observedAtTick > tick || policy.observedAtTick >= policy.validUntilTick ||
        !boundedPolicyText(policy.code) || !boundedPolicyText(policy.explanation)) ||
      new Set(frozenTaskActionPolicies.map((policy) => policy.action)).size !==
        frozenTaskActionPolicies.length) {
    return fail(BrowserHostReason.InvalidReport, "unknown permission state");
  }
  return Object.freeze({
    schemaVersion: BROWSER_HOST_SCHEMA_VERSION,
    hostId,
    observationId,
    // The adapter only retains exact #88 identity/status references. It does
    // not validate credentials, enroll entities, rotate keys, or prompt.
    reporter: Object.freeze({
      ...reporter,
      statusObservation: Object.freeze({ ...reporter.statusObservation }),
    }),
    observedAtTick: tick,
    validUntilTick,
    context: Object.freeze({ ...context }),
    placements: Object.freeze(frozenPlacements),
    permissions: Object.freeze(frozenPermissions),
    taskActionPolicies: Object.freeze(frozenTaskActionPolicies),
    activation: activation === true,
    resources: Object.freeze({ ...resources }),
  });
}

/** Pure resolver predicate: no browser APIs are read or called here. */
export function resolveBrowserPlacement(report, request) {
  if (!report || report.schemaVersion !== BROWSER_HOST_SCHEMA_VERSION ||
      !Number.isSafeInteger(request?.tick) ||
      request.tick < report.observedAtTick ||
      request.tick >= report.validUntilTick) {
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

function requirePlacement(binding, expected) {
  if (binding?.placement !== expected) {
    throw new TypeError(`${BrowserHostReason.InvalidPlan}:${expected}`);
  }
}

function cloneByteLength(value) {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

function boundedProviderPromise(promise, milliseconds, label) {
  return Promise.race([
    promise,
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error(`${label}-timeout`)), milliseconds)),
  ]);
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

/** Executes one explicitly bounded synchronous step on the document thread. */
export class WindowExecutionAdapter extends BrowserExecutionAdapter {
  constructor(binding, emitEvidence) {
    requirePlacement(binding, Placement.Window);
    super(binding, emitEvidence);
  }
  step(handler, input) {
    if (this.state !== "running" || typeof handler !== "function") {
      return fail(BrowserHostReason.Terminal, this.state);
    }
    try {
      return { ok: true, value: handler(input) };
    } catch (error) {
      this.terminate("window-step-failed");
      return fail(BrowserHostReason.Terminal, String(error));
    }
  }
}

class BoundedMessageAdapter extends BrowserExecutionAdapter {
  #port;
  #pending = new Map();
  #nextId = 1;
  constructor(binding, emitEvidence) {
    super(binding, emitEvidence);
    if (!Number.isSafeInteger(binding.maximumPending) || binding.maximumPending <= 0 ||
        !Number.isSafeInteger(binding.maximumMessageBytes) || binding.maximumMessageBytes <= 0 ||
        !Number.isSafeInteger(binding.responseTimeoutMs) || binding.responseTimeoutMs <= 0) {
      throw new RangeError(BrowserHostReason.QueueCapacity);
    }
  }
  attachPort(port) {
    this.#port = port;
    port.onmessage = (event) => {
      const pending = this.#pending.get(event.data?.id);
      if (!pending) return;
      this.#pending.delete(event.data.id);
      clearTimeout(pending.timeout);
      if (cloneByteLength(event.data) > this.binding.maximumMessageBytes) {
        pending.resolve(fail(BrowserHostReason.QueueCapacity, "response-message-bytes"));
        this.terminate("response-message-bytes");
        return;
      }
      pending.resolve({
        ok: event.data.ok === true,
        value: event.data.value,
        code: event.data.code,
      });
    };
    port.onmessageerror = () => this.terminate("structured-clone-failed");
    port.start?.();
  }
  request(operation, value) {
    if (this.state !== "running" || !this.#port) {
      return Promise.resolve(fail(BrowserHostReason.Terminal, this.state));
    }
    if (this.#pending.size === this.binding.maximumPending) {
      return Promise.resolve(fail(BrowserHostReason.QueueCapacity, "pending"));
    }
    const id = this.#nextId++;
    const message = { id, operation, value };
    if (cloneByteLength(message) > this.binding.maximumMessageBytes) {
      return Promise.resolve(fail(BrowserHostReason.QueueCapacity, "message-bytes"));
    }
    return new Promise((resolve) => {
      const timeout = setTimeout(() => {
        this.#pending.delete(id);
        resolve(fail(BrowserHostReason.Terminal, "response-timeout"));
        this.terminate("response-timeout");
      }, this.binding.responseTimeoutMs);
      this.#pending.set(id, { resolve, timeout });
      this.#port.postMessage(message);
    });
  }
  terminate(cause) {
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timeout);
      pending.resolve(fail(BrowserHostReason.Terminal, cause));
    }
    this.#pending.clear();
    this.#port?.close?.();
    return super.terminate(cause);
  }
}

export class DedicatedWorkerExecutionAdapter extends BoundedMessageAdapter {
  #worker;
  constructor(binding, emitEvidence, workerConstructor = globalThis.Worker) {
    requirePlacement(binding, Placement.DedicatedWorker);
    super(binding, emitEvidence);
    this.workerConstructor = workerConstructor;
  }
  start() {
    if (typeof this.workerConstructor !== "function") {
      this.emitEvidence(Object.freeze({ kind: "unsupported", feature: Placement.DedicatedWorker }));
      return fail(BrowserHostReason.PlacementUnavailable, Placement.DedicatedWorker);
    }
    const started = super.start();
    if (!started.ok) return started;
    try {
      this.#worker = new this.workerConstructor(this.binding.artifactUrl, { type: "module" });
      this.#worker.addEventListener("error", () => this.terminate("dedicated-worker-died"));
      this.attachPort(this.#worker);
      return started;
    } catch (error) {
      this.terminate("dedicated-worker-start-failed");
      return fail(BrowserHostReason.Terminal, String(error));
    }
  }
  terminate(cause) {
    this.#worker?.terminate();
    return super.terminate(cause);
  }
}

export class SharedWorkerExecutionAdapter extends BoundedMessageAdapter {
  #worker;
  constructor(binding, emitEvidence, workerConstructor = globalThis.SharedWorker) {
    requirePlacement(binding, Placement.SharedWorker);
    super(binding, emitEvidence);
    this.workerConstructor = workerConstructor;
  }
  start() {
    if (typeof this.workerConstructor !== "function") {
      this.emitEvidence(Object.freeze({ kind: "unsupported", feature: Placement.SharedWorker }));
      return fail(BrowserHostReason.PlacementUnavailable, Placement.SharedWorker);
    }
    const started = super.start();
    if (!started.ok) return started;
    try {
      this.#worker = new this.workerConstructor(this.binding.artifactUrl, {
        type: "module",
        name: this.binding.planIdentity,
      });
      this.#worker.addEventListener("error", () => this.terminate("shared-worker-died"));
      this.attachPort(this.#worker.port);
      return started;
    } catch (error) {
      this.terminate("shared-worker-start-failed");
      return fail(BrowserHostReason.Terminal, String(error));
    }
  }
}

/**
 * Uses an already-installed and active service worker. Registration remains a
 * separate explicit effect and is never hidden in resolution or adapter start.
 */
export class ServiceWorkerExecutionAdapter extends BrowserExecutionAdapter {
  #pending = 0;
  constructor(binding, emitEvidence, activeWorker, messageChannel = globalThis.MessageChannel) {
    requirePlacement(binding, Placement.ServiceWorker);
    super(binding, emitEvidence);
    this.activeWorker = activeWorker;
    this.messageChannel = messageChannel;
    if (!Number.isSafeInteger(binding.maximumPending) || binding.maximumPending <= 0 ||
        !Number.isSafeInteger(binding.responseTimeoutMs) || binding.responseTimeoutMs <= 0) {
      throw new RangeError(BrowserHostReason.QueueCapacity);
    }
  }
  start() {
    if (!this.activeWorker || typeof this.messageChannel !== "function") {
      this.emitEvidence(Object.freeze({ kind: "unsupported", feature: Placement.ServiceWorker }));
      return fail(BrowserHostReason.PlacementUnavailable, Placement.ServiceWorker);
    }
    return super.start();
  }
  request(operation, value) {
    if (this.state !== "running") {
      return Promise.resolve(fail(BrowserHostReason.Terminal, this.state));
    }
    if (this.#pending === this.binding.maximumPending) {
      return Promise.resolve(fail(BrowserHostReason.QueueCapacity, "pending"));
    }
    this.#pending += 1;
    const channel = new this.messageChannel();
    return new Promise((resolve) => {
      const timeout = setTimeout(() => {
        this.#pending -= 1;
        channel.port1.close();
        resolve(fail(BrowserHostReason.Terminal, "response-timeout"));
        this.terminate("response-timeout");
      }, this.binding.responseTimeoutMs);
      channel.port1.onmessage = (event) => {
        clearTimeout(timeout);
        this.#pending -= 1;
        channel.port1.close();
        resolve({ ok: event.data?.ok === true, value: event.data?.value, code: event.data?.code });
      };
      this.activeWorker.postMessage({ operation, value }, [channel.port2]);
    });
  }
}

export class AudioWorkletExecutionAdapter extends BoundedMessageAdapter {
  #node;
  constructor(binding, emitEvidence, audioContext) {
    requirePlacement(binding, Placement.AudioWorklet);
    super(binding, emitEvidence);
    this.audioContext = audioContext;
  }
  async start() {
    if (!this.audioContext?.audioWorklet || typeof globalThis.AudioWorkletNode !== "function") {
      this.emitEvidence(Object.freeze({ kind: "unsupported", feature: Placement.AudioWorklet }));
      return fail(BrowserHostReason.PlacementUnavailable, Placement.AudioWorklet);
    }
    const started = super.start();
    if (!started.ok) return started;
    try {
      await boundedProviderPromise(
        this.audioContext.audioWorklet.addModule(this.binding.artifactUrl),
        this.binding.responseTimeoutMs,
        "audio-worklet-load",
      );
      this.#node = new AudioWorkletNode(this.audioContext, this.binding.processorName);
      this.#node.addEventListener("processorerror", () => this.terminate("audio-worklet-died"));
      this.attachPort(this.#node.port);
      return started;
    } catch (error) {
      this.terminate("audio-worklet-start-failed");
      return fail(BrowserHostReason.Terminal, String(error));
    }
  }
  terminate(cause) {
    this.#node?.disconnect();
    return super.terminate(cause);
  }
}

export class WasmExecutionAdapter extends BrowserExecutionAdapter {
  #instance;
  constructor(binding, emitEvidence, bytes, imports = {}) {
    requirePlacement(binding, Placement.Wasm);
    super(binding, emitEvidence);
    this.bytes = bytes;
    this.imports = imports;
  }
  async start() {
    if (!globalThis.WebAssembly) {
      this.emitEvidence(Object.freeze({ kind: "unsupported", feature: Placement.Wasm }));
      return fail(BrowserHostReason.PlacementUnavailable, Placement.Wasm);
    }
    const started = super.start();
    if (!started.ok) return started;
    try {
      const loaded = await WebAssembly.instantiate(this.bytes, this.imports);
      this.#instance = loaded.instance;
      return started;
    } catch (error) {
      this.terminate("wasm-instantiation-failed");
      return fail(BrowserHostReason.Terminal, String(error));
    }
  }
  call(exportName, ...arguments_) {
    const callable = this.#instance?.exports?.[exportName];
    if (this.state !== "running" || typeof callable !== "function") {
      return fail(BrowserHostReason.InvalidPlan, exportName);
    }
    return { ok: true, value: callable(...arguments_) };
  }
}

export class WebGpuExecutionAdapter extends BrowserExecutionAdapter {
  #device;
  constructor(binding, emitEvidence, gpu = globalThis.navigator?.gpu) {
    requirePlacement(binding, Placement.WebGpu);
    super(binding, emitEvidence);
    if (!Number.isSafeInteger(binding.providerTimeoutMs) || binding.providerTimeoutMs <= 0) {
      throw new RangeError(BrowserHostReason.QueueCapacity);
    }
    this.gpu = gpu;
  }
  async start() {
    if (!this.gpu?.requestAdapter) {
      this.emitEvidence(Object.freeze({ kind: "unsupported", feature: Placement.WebGpu }));
      return fail(BrowserHostReason.PlacementUnavailable, Placement.WebGpu);
    }
    const started = super.start();
    if (!started.ok) return started;
    try {
      const adapter = await boundedProviderPromise(
        this.gpu.requestAdapter(),
        this.binding.providerTimeoutMs,
        "webgpu-adapter",
      );
      if (!adapter) {
        this.terminate("webgpu-adapter-unavailable");
        return fail(BrowserHostReason.PlacementUnavailable, Placement.WebGpu);
      }
      this.#device = await boundedProviderPromise(
        adapter.requestDevice(),
        this.binding.providerTimeoutMs,
        "webgpu-device",
      );
      this.#device.lost.then((info) => this.terminate(`webgpu-device-lost:${info.reason}`));
      return started;
    } catch (error) {
      this.terminate("webgpu-device-request-failed");
      return fail(BrowserHostReason.Terminal, String(error));
    }
  }
  get device() { return this.#device; }
}
