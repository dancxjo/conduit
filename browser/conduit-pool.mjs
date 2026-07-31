// Browser reference implementation of the exact replicated-pool contract.
// All arrays are allocated to plan maxima at construction; no callback,
// Promise, worker, or transport queue is created by this controller.

export const POOL_CONTRACT_VERSION = 0;

export const PoolReason = Object.freeze({
  InvalidContract: "CND-POL-001",
  InvalidIdentity: "CND-POL-002",
  IllegalTransition: "CND-POL-003",
  DeadlineOverflow: "CND-POL-004",
  ReservationExceeded: "CND-POL-005",
  EvidenceExhausted: "CND-POL-006",
});

const terminalStates = new Set(["succeeded", "cancelled", "failed"]);
const liveStates = new Set([
  "reserved",
  "running",
  "checkpointing",
  "restart-backoff",
  "draining",
  "cleanup",
]);
const drainableStates = new Set([
  "reserved",
  "running",
  "checkpointing",
  "restart-backoff",
]);
const cleanupEligibleStates = new Set([...drainableStates, "queued", "draining"]);
const resourceKeys = Object.freeze([
  "memoryBytes",
  "storageBytes",
  "cpuUnits",
  "timers",
  "transports",
  "checkpoints",
  "evidenceBytes",
  "childNodes",
  "childCords",
  "stateBytes",
  "schedulerSlots",
  "hostOperations",
  "cancellationScopes",
]);

function positive(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function nonnegative(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function fits(value, available) {
  return resourceKeys.every((key) =>
    nonnegative(value?.[key]) &&
    nonnegative(available?.[key]) &&
    value[key] <= available[key]);
}

function equal(left, right) {
  return resourceKeys.every((key) => left[key] === right[key]);
}

function multiply(value, count) {
  const result = {};
  for (const key of resourceKeys) {
    const product = value[key] * count;
    if (!Number.isSafeInteger(product)) throw new TypeError(PoolReason.ReservationExceeded);
    result[key] = product;
  }
  return Object.freeze(result);
}

function add(left, right) {
  const result = {};
  for (const key of resourceKeys) {
    const total = left[key] + right[key];
    if (!Number.isSafeInteger(total)) throw new TypeError(PoolReason.ReservationExceeded);
    result[key] = total;
  }
  return Object.freeze(result);
}

function validate(contract) {
  if (contract?.schemaVersion !== POOL_CONTRACT_VERSION ||
      !contract.pool || !contract.planIdentity || !contract.templateIdentity ||
      !contract.implementationSetIdentity ||
      !positive(contract.maximumLive) ||
      !nonnegative(contract.maximumQueued) ||
      !positive(contract.maximumEvidenceEvents) ||
      !positive(contract.deadlineTicks) ||
      !positive(contract.idleTimeoutTicks) ||
      !positive(contract.cleanupTicks) ||
      !["reject", "block", "queue-bounded", "fail"].includes(contract.admission) ||
      !["drain", "abort"].includes(contract.cleanup)) {
    throw new TypeError(PoolReason.InvalidContract);
  }
  if ((contract.admission === "queue-bounded") !== (contract.maximumQueued > 0)) {
    throw new TypeError(PoolReason.InvalidContract);
  }
  const supervision = contract.supervision;
  if (!supervision ||
      !["fail-together", "isolate", "restart-bounded", "fallback", "escalate"]
        .includes(supervision.kind) ||
      (supervision.kind === "restart-bounded" &&
        (!positive(supervision.maximumAttempts) || !positive(supervision.backoffTicks))) ||
      (supervision.kind === "fallback" && !supervision.target)) {
    throw new TypeError(PoolReason.InvalidContract);
  }
  if (!resourceKeys.every((key) => nonnegative(contract.perInstance?.[key])) ||
      !resourceKeys.every((key) => nonnegative(contract.queuedReservation?.[key])) ||
      !resourceKeys.every((key) => nonnegative(contract.totalReservation?.[key]))) {
    throw new TypeError(PoolReason.InvalidContract);
  }
  const overlap = contract.generationReservation;
  if (!overlap ||
      !nonnegative(overlap.candidateMaximumLive) ||
      !nonnegative(overlap.rollbackMaximumLive) ||
      !positive(overlap.reservedSlots)) {
    throw new TypeError(PoolReason.InvalidContract);
  }
  const queued = contract.queuedReservation;
  const queueProfileValid = contract.maximumQueued === 0
    ? equal(queued, multiply(queued, 0))
    : queued.childNodes === 0 &&
      queued.childCords === 0 &&
      queued.hostOperations === 0 &&
      queued.storageBytes === 0 &&
      queued.cpuUnits === 0 &&
      queued.timers === 0 &&
      queued.transports === 0 &&
      queued.checkpoints === 0 &&
      positive(queued.memoryBytes) &&
      positive(queued.evidenceBytes) &&
      positive(queued.stateBytes) &&
      positive(queued.schedulerSlots) &&
      positive(queued.cancellationScopes);
  if (!queueProfileValid) {
    throw new TypeError(PoolReason.InvalidContract);
  }
  const generationSlots = contract.maximumLive +
    overlap.candidateMaximumLive +
    overlap.rollbackMaximumLive;
  if (generationSlots !== overlap.reservedSlots ||
      !equal(multiply(contract.perInstance, generationSlots), overlap.reservedResources) ||
      !equal(
        add(
          overlap.reservedResources,
          multiply(contract.queuedReservation, contract.maximumQueued),
        ),
        contract.totalReservation,
      )) {
    throw new TypeError(PoolReason.ReservationExceeded);
  }
}

function failed(code, reason = null) {
  return Object.freeze({ ok: false, code, reason });
}

export class BoundedBrowserPool {
  #slots;
  #evidence = [];
  #cursor = 0;
  #accepting = true;

  constructor(contract, epoch, generation) {
    validate(contract);
    if (!positive(epoch) || !positive(generation)) {
      throw new TypeError(PoolReason.InvalidIdentity);
    }
    this.contract = Object.freeze({
      ...contract,
      supervision: Object.freeze({ ...contract.supervision }),
      perInstance: Object.freeze({ ...contract.perInstance }),
      queuedReservation: Object.freeze({ ...contract.queuedReservation }),
      totalReservation: Object.freeze({ ...contract.totalReservation }),
      generationReservation: Object.freeze({
        ...contract.generationReservation,
        reservedResources: Object.freeze({
          ...contract.generationReservation.reservedResources,
        }),
      }),
    });
    this.generationIdentity =
      `${contract.planIdentity}\0${contract.pool}\0${epoch}\0${generation}` +
      `\0${contract.templateIdentity}`;
    this.#slots = Array.from(
      { length: contract.maximumLive + contract.maximumQueued },
      () => null,
    );
  }

  get slots() {
    return Object.freeze(this.#slots.map((slot) => slot && Object.freeze({ ...slot })));
  }

  get evidence() {
    return Object.freeze([...this.#evidence]);
  }

  get population() {
    const result = {
      queued: 0,
      live: 0,
      restarting: 0,
      retiring: 0,
      cleanup: 0,
      terminal: 0,
    };
    for (const slot of this.#slots) {
      if (!slot) continue;
      if (slot.state === "queued") result.queued += 1;
      if (liveStates.has(slot.state)) result.live += 1;
      if (slot.state === "restart-backoff") result.restarting += 1;
      if (slot.state === "draining") result.retiring += 1;
      if (slot.state === "cleanup") result.cleanup += 1;
      if (terminalStates.has(slot.state)) result.terminal += 1;
    }
    return Object.freeze(result);
  }

  #reserveEvidence(count) {
    return this.#evidence.length + count <= this.contract.maximumEvidenceEvents;
  }

  #emit(slot, from, to, reason, tick, cause = null) {
    if (!this.#reserveEvidence(1)) return false;
    this.#evidence.push(Object.freeze({
      sequence: this.#evidence.length + 1,
      tick,
      instance: slot.instance,
      workUnit: slot.workUnit,
      attempt: slot.attempt,
      correlation: slot.correlation,
      from,
      to,
      reason,
      cause,
    }));
    return true;
  }

  #identity(work, attempt) {
    const callerCorrelation = work?.callerCorrelation ?? work?.correlation;
    if (!work?.request || !work.workUnit || !callerCorrelation || !positive(attempt)) {
      throw new TypeError(PoolReason.InvalidIdentity);
    }
    const instance = `${this.generationIdentity}\0instance\0${work.request}`;
    return {
      request: work.request,
      workUnit: work.workUnit,
      callerCorrelation,
      instance,
      attempt,
      correlation:
        `${this.generationIdentity}\0${work.request}\0${work.workUnit}\0${attempt}\0${callerCorrelation}`,
    };
  }

  #emptyIndex() {
    return this.#slots.findIndex((slot) => !slot);
  }

  #recordDenial(work, tick, reason) {
    const identity = this.#identity(work, 1);
    if (!this.#emit(identity, "empty", "empty", reason, tick)) {
      return failed(PoolReason.EvidenceExhausted);
    }
    return null;
  }

  #start(work, tick, state, reason) {
    const index = this.#emptyIndex();
    if (index < 0) return failed(PoolReason.ReservationExceeded, "reservation-drift");
    const identity = this.#identity(work, 1);
    const deadlineTick = tick + this.contract.deadlineTicks;
    if (!Number.isSafeInteger(deadlineTick)) return failed(PoolReason.DeadlineOverflow);
    const slot = {
      ...identity,
      state,
      admittedAtTick: state === "queued" ? 0 : tick,
      lastProgressTick: state === "queued" ? 0 : tick,
      deadlineTick: state === "queued" ? 0 : deadlineTick,
      wakeTick: 0,
      cleanupDeadlineTick: 0,
      cause: null,
    };
    if (!this.#emit(slot, "empty", state, reason, tick)) {
      return failed(PoolReason.EvidenceExhausted);
    }
    this.#slots[index] = slot;
    return Object.freeze({ ok: true, state, slot: index, identity: slot.instance });
  }

  offer(work, facts, tick) {
    if (!nonnegative(tick) || this.#slots.some((slot) =>
      slot && slot.request === work?.request)) {
      return failed(PoolReason.InvalidIdentity);
    }
    if (!this.#accepting) {
      const evidenceFailure = this.#recordDenial(work, tick, "generation-draining");
      return evidenceFailure ?? failed(PoolReason.IllegalTransition, "generation-draining");
    }
    let denial = null;
    if (facts?.templateIdentity !== this.contract.templateIdentity ||
        facts?.implementationSetIdentity !== this.contract.implementationSetIdentity) {
      denial = "implementation-mismatch";
    }
    else if (!facts.authorityGranted) denial = "authority-denied";
    else if (!facts.sensitivityAllowed) denial = "sensitivity-denied";
    else if (!fits(this.contract.perInstance, facts.available)) {
      denial = "reservation-unavailable";
    }
    if (denial) {
      const evidenceFailure = this.#recordDenial(work, tick, denial);
      if (evidenceFailure) return evidenceFailure;
      return failed(
        this.contract.admission === "fail"
          ? PoolReason.IllegalTransition
          : PoolReason.ReservationExceeded,
        denial,
      );
    }
    if (this.population.live < this.contract.maximumLive) {
      return this.#start(work, tick, "reserved", "admitted");
    }
    if (this.contract.admission === "reject") {
      const evidenceFailure = this.#recordDenial(work, tick, "capacity");
      if (evidenceFailure) return evidenceFailure;
      return failed(PoolReason.ReservationExceeded, "capacity");
    }
    if (this.contract.admission === "block") {
      const evidenceFailure = this.#recordDenial(work, tick, "caller-blocked");
      if (evidenceFailure) return evidenceFailure;
      return Object.freeze({ ok: false, blocked: true, reason: "caller-blocked" });
    }
    if (this.contract.admission === "fail") {
      const evidenceFailure = this.#recordDenial(work, tick, "admission-failed");
      if (evidenceFailure) return evidenceFailure;
      return failed(PoolReason.IllegalTransition, "admission-failed");
    }
    if (this.population.queued >= this.contract.maximumQueued) {
      const evidenceFailure = this.#recordDenial(work, tick, "queue-full");
      if (evidenceFailure) return evidenceFailure;
      return failed(PoolReason.ReservationExceeded, "queue-full");
    }
    return this.#start(work, tick, "queued", "queued");
  }

  start(slotIndex, tick) {
    return this.#transition(slotIndex, "running", "started", tick);
  }

  progress(slotIndex, tick) {
    const slot = this.#slots[slotIndex];
    if (!slot || slot.state !== "running") return failed(PoolReason.IllegalTransition);
    if (!this.#emit(slot, "running", "running", "progress", tick, slot.cause)) {
      return failed(PoolReason.EvidenceExhausted);
    }
    slot.lastProgressTick = tick;
    return Object.freeze({ ok: true });
  }

  observePressure(slotIndex, loss, cause, tick) {
    const slot = this.#slots[slotIndex];
    if (!slot || slot.state !== "running") {
      return failed(PoolReason.IllegalTransition);
    }
    if (!this.#emit(
      slot,
      "running",
      "running",
      loss ? "loss" : "pressure",
      tick,
      cause,
    )) {
      return failed(PoolReason.EvidenceExhausted);
    }
    return Object.freeze({ ok: true });
  }

  observeUsage(slotIndex, usage, tick) {
    const slot = this.#slots[slotIndex];
    if (!slot || !cleanupEligibleStates.has(slot.state) || slot.state === "queued") {
      return failed(PoolReason.IllegalTransition);
    }
    if (fits(usage, this.contract.perInstance)) return Object.freeze({ ok: true });
    return this.#cleanup(slotIndex, "foreign-profile-exceeded", tick);
  }

  checkpoint(slotIndex, templateIdentity, tick) {
    const slot = this.#slots[slotIndex];
    if (!slot || slot.state !== "running") {
      return failed(PoolReason.IllegalTransition);
    }
    if (templateIdentity !== this.contract.templateIdentity) {
      if (!this.#emit(
        slot,
        "running",
        "running",
        "checkpoint-incompatible",
        tick,
        slot.cause,
      )) {
        return failed(PoolReason.EvidenceExhausted);
      }
      return Object.freeze({ ok: true, accepted: false, state: "running" });
    }
    const result = this.#transition(
      slotIndex,
      "checkpointing",
      "checkpoint-compatible",
      tick,
    );
    return Object.freeze({ ...result, accepted: result.ok });
  }

  resume(slotIndex, tick) {
    return this.#transition(slotIndex, "running", "progress", tick);
  }

  complete(slotIndex, tick) {
    return this.#cleanup(slotIndex, "completed", tick);
  }

  cancel(slotIndex, cause, tick) {
    const slot = this.#slots[slotIndex];
    if (!slot) return failed(PoolReason.IllegalTransition);
    if (slot.state === "queued") {
      return this.#transition(slotIndex, "cancelled", "cancelled", tick, cause);
    }
    return this.#cleanup(slotIndex, "cancelled", tick, cause);
  }

  fail(slotIndex, cause, tick) {
    const slot = this.#slots[slotIndex];
    if (!slot || !liveStates.has(slot.state)) return failed(PoolReason.IllegalTransition);
    const policy = this.contract.supervision;
    if (policy.kind === "restart-bounded") {
      if (slot.attempt >= policy.maximumAttempts) {
        return this.#cleanup(slotIndex, "restart-exhausted", tick, cause);
      }
      const wakeTick = tick + policy.backoffTicks;
      if (!Number.isSafeInteger(wakeTick)) return failed(PoolReason.DeadlineOverflow);
      const result = this.#transition(
        slotIndex,
        "restart-backoff",
        "restart-scheduled",
        tick,
        cause,
      );
      if (result.ok) slot.wakeTick = wakeTick;
      return Object.freeze({ ...result, wakeTick, nextAttempt: slot.attempt + 1 });
    }
    if (policy.kind === "fail-together") {
      const affected = this.#slots.filter((candidate) =>
        candidate && cleanupEligibleStates.has(candidate.state));
      const needed = affected.reduce(
        (count, candidate) => count + (candidate.state === "queued" ? 1 : 2),
        0,
      );
      if (!this.#reserveEvidence(needed)) return failed(PoolReason.EvidenceExhausted);
      for (let index = 0; index < this.#slots.length; index += 1) {
        const candidate = this.#slots[index];
        if (candidate && cleanupEligibleStates.has(candidate.state)) {
          this.#cleanup(index, "fail-together", tick, cause);
        }
      }
      return Object.freeze({ ok: true, disposition: "fail-pool" });
    }
    const reason = policy.kind === "fallback" ? "fallback" :
      policy.kind === "escalate" ? "escalated" : "isolated";
    const result = this.#cleanup(slotIndex, reason, tick, cause);
    return Object.freeze({
      ...result,
      disposition: policy.kind,
      target: policy.target ?? null,
    });
  }

  tick(tick) {
    for (let index = 0; index < this.#slots.length; index += 1) {
      const slot = this.#slots[index];
      if (!slot) continue;
      if (slot.state === "running" &&
          (tick >= slot.deadlineTick ||
            tick - slot.lastProgressTick >= this.contract.idleTimeoutTicks)) {
        const result = this.#cleanup(
          index,
          tick >= slot.deadlineTick ? "deadline-expired" : "idle-expired",
          tick,
          slot.cause,
        );
        if (!result.ok) return result;
      } else if (slot.state === "restart-backoff" && tick >= slot.wakeTick) {
        const deadlineTick = tick + this.contract.deadlineTicks;
        if (!Number.isSafeInteger(deadlineTick)) {
          return failed(PoolReason.DeadlineOverflow);
        }
        const next = this.#identity(slot, slot.attempt + 1);
        if (!this.#emit(slot, "restart-backoff", "reserved", "restarted", tick, slot.cause)) {
          return failed(PoolReason.EvidenceExhausted);
        }
        Object.assign(slot, next, {
          state: "reserved",
          admittedAtTick: tick,
          lastProgressTick: tick,
          deadlineTick,
        });
      } else if (slot.state === "cleanup" && tick >= slot.cleanupDeadlineTick) {
        const terminal = slot.terminalReason === "completed"
          ? "succeeded"
          : slot.terminalReason === "cancelled" ? "cancelled" : "failed";
        const result = this.#transition(index, terminal, "cleanup-expired", tick, slot.cause);
        if (!result.ok) return result;
      }
    }
    if (this.population.live >= this.contract.maximumLive) {
      return Object.freeze({ ok: true, started: null });
    }
    for (let offset = 0; offset < this.#slots.length; offset += 1) {
      const index = (this.#cursor + offset) % this.#slots.length;
      const slot = this.#slots[index];
      if (slot?.state === "queued") {
        const deadlineTick = tick + this.contract.deadlineTicks;
        if (!Number.isSafeInteger(deadlineTick)) {
          return failed(PoolReason.DeadlineOverflow);
        }
        const result = this.#transition(index, "reserved", "admitted", tick);
        if (result.ok) {
          slot.admittedAtTick = tick;
          slot.lastProgressTick = tick;
          slot.deadlineTick = deadlineTick;
          this.#cursor = (index + 1) % this.#slots.length;
        }
        return Object.freeze({ ...result, started: result.ok ? index : null });
      }
    }
    return Object.freeze({ ok: true, started: null });
  }

  drain(cause, tick) {
    const affected = this.#slots.filter((slot) =>
      slot && (slot.state === "queued" ||
        drainableStates.has(slot.state)));
    if (!this.#reserveEvidence(affected.length)) return failed(PoolReason.EvidenceExhausted);
    this.#accepting = false;
    for (let index = 0; index < this.#slots.length; index += 1) {
      const slot = this.#slots[index];
      if (slot?.state === "queued") {
        this.#transition(index, "cancelled", "generation-draining", tick, cause);
      } else if (slot && drainableStates.has(slot.state)) {
        this.#transition(index, "draining", "generation-draining", tick, cause);
      }
    }
    return Object.freeze({ ok: true, affected: affected.length });
  }

  retireDrained(tick) {
    const affected = this.#slots.filter((slot) => slot?.state === "draining");
    if (!this.#reserveEvidence(affected.length * 2)) {
      return failed(PoolReason.EvidenceExhausted);
    }
    for (let index = 0; index < this.#slots.length; index += 1) {
      if (this.#slots[index]?.state === "draining") {
        this.#cleanup(
          index,
          "generation-retired",
          tick,
          this.#slots[index].cause,
        );
      }
    }
    return Object.freeze({ ok: true, affected: affected.length });
  }

  rollback(cause, tick) {
    const affected = this.#slots.filter((slot) =>
      slot && cleanupEligibleStates.has(slot.state));
    const needed = affected.reduce(
      (count, slot) => count + (slot.state === "queued" ? 1 : 2),
      0,
    );
    if (!this.#reserveEvidence(needed)) {
      return failed(PoolReason.EvidenceExhausted);
    }
    this.#accepting = false;
    for (let index = 0; index < this.#slots.length; index += 1) {
      const slot = this.#slots[index];
      if (slot && cleanupEligibleStates.has(slot.state)) {
        this.#cleanup(index, "generation-rollback", tick, cause);
      }
    }
    return Object.freeze({ ok: true, affected: affected.length });
  }

  reclaim(slotIndex) {
    const slot = this.#slots[slotIndex];
    if (!slot || !terminalStates.has(slot.state)) {
      return failed(PoolReason.IllegalTransition);
    }
    this.#slots[slotIndex] = null;
    return Object.freeze({ ok: true });
  }

  #cleanup(slotIndex, reason, tick, cause = null) {
    const slot = this.#slots[slotIndex];
    if (!slot || (!liveStates.has(slot.state) && slot.state !== "queued")) {
      return failed(PoolReason.IllegalTransition);
    }
    if (slot.state === "queued") {
      return this.#transition(slotIndex, "cancelled", reason, tick, cause);
    }
    const deadline = tick + this.contract.cleanupTicks;
    if (!Number.isSafeInteger(deadline)) return failed(PoolReason.DeadlineOverflow);
    if (!this.#reserveEvidence(2)) return failed(PoolReason.EvidenceExhausted);
    if (!this.#emit(slot, slot.state, "cleanup", reason, tick, cause) ||
        !this.#emit(
          slot,
          "cleanup",
          "cleanup",
          this.contract.cleanup === "drain" ? "cleanup-drain" : "cleanup-abort",
          tick,
          cause,
        )) {
      return failed(PoolReason.EvidenceExhausted);
    }
    Object.assign(slot, {
      state: "cleanup",
      cleanupDeadlineTick: deadline,
      terminalReason: reason,
      cause,
    });
    return Object.freeze({ ok: true, disposition: "cleanup" });
  }

  #transition(slotIndex, to, reason, tick, cause = undefined) {
    const slot = this.#slots[slotIndex];
    if (!slot) return failed(PoolReason.IllegalTransition);
    const transitions = new Set([
      "reserved:running",
      "queued:reserved",
      "running:checkpointing",
      "checkpointing:running",
      "running:restart-backoff",
      "reserved:restart-backoff",
      "restart-backoff:reserved",
      "running:draining",
      "reserved:draining",
      "checkpointing:draining",
      "restart-backoff:draining",
      "draining:cleanup",
      "queued:cancelled",
      "cleanup:succeeded",
      "cleanup:cancelled",
      "cleanup:failed",
    ]);
    if (!transitions.has(`${slot.state}:${to}`)) {
      return failed(PoolReason.IllegalTransition);
    }
    const exactCause = cause === undefined ? slot.cause : cause;
    if (!this.#emit(slot, slot.state, to, reason, tick, exactCause)) {
      return failed(PoolReason.EvidenceExhausted);
    }
    slot.state = to;
    slot.cause = exactCause;
    if (to === "running") slot.lastProgressTick = tick;
    return Object.freeze({ ok: true, state: to });
  }
}
