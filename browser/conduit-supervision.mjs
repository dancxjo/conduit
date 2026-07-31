// Browser reference implementation of the portable supervision contract.
// It consumes an exact finite binding and never discovers actions, mutates a
// plan, acquires authority, or treats domain values as terminal observations.

export const SUPERVISION_CONTRACT_VERSION = 0;

export const SupervisionReason = Object.freeze({
  InvalidContract: "CND-SUP-001",
  UnboundedContract: "CND-SUP-002",
  ObservationInvalid: "CND-SUP-003",
  ObservationBudgetExhausted: "CND-SUP-004",
  DecisionBudgetExhausted: "CND-SUP-005",
  InFlightLimitReached: "CND-SUP-006",
  EvidenceBudgetExhausted: "CND-SUP-007",
  DeadlineExpired: "CND-SUP-008",
  ActionNotAdmitted: "CND-SUP-009",
  RetryNotDeclaredIdempotent: "CND-SUP-010",
  AttemptBudgetExhausted: "CND-SUP-011",
  RequiredGuaranteeWouldWeaken: "CND-SUP-012",
  CandidateEpochRequired: "CND-SUP-013",
  UnsupportedProfile: "CND-SUP-015",
  SupervisorTerminal: "CND-SUP-017",
});

const constrainedActions = new Set(["propagate", "stop-scope", "restart-same"]);
const targetActions = new Set([
  "activate-declared-fallback",
  "continue-declared-degraded-mode",
  "request-operator-action",
]);

function failure(code) {
  return Object.freeze({ ok: false, code });
}

function sameCorrelation(left, right) {
  return left.run === right.run &&
    left.planIdentity === right.planIdentity &&
    left.planEpoch === right.planEpoch &&
    left.generation === right.generation &&
    left.attempt === right.attempt &&
    left.subject === right.subject &&
    left.expandedSubject === right.expandedSubject;
}

function validPositiveInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function validNonnegativeInteger(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function validateBinding(binding) {
  const limits = binding?.limits;
  if (binding?.schemaVersion !== SUPERVISION_CONTRACT_VERSION ||
      !binding.id || !binding.subject || !binding.handler ||
      binding.subject === binding.handler || !Array.isArray(binding.actions) ||
      binding.actions.length === 0) {
    return SupervisionReason.InvalidContract;
  }
  if (!["child", "named-group", "composite-boundary", "replicated-child"]
      .includes(binding.scope) ||
      !["fail-together", "isolated-optional"].includes(binding.failureMode) ||
      !Array.isArray(binding.members)) {
    return SupervisionReason.InvalidContract;
  }
  const memberSet = new Set(binding.members);
  const groupValid = binding.scope === "named-group"
    ? binding.members.length >= 2 &&
      memberSet.size === binding.members.length &&
      memberSet.has(binding.subject) &&
      !memberSet.has(binding.handler)
    : binding.members.length === 0;
  if (!groupValid ||
      (binding.failureMode === "isolated-optional" && binding.scope !== "named-group")) {
    return SupervisionReason.InvalidContract;
  }
  if (!limits ||
      !validPositiveInteger(limits.maximumObservations) ||
      !validPositiveInteger(limits.maximumDecisions) ||
      !validPositiveInteger(limits.maximumInFlight) ||
      limits.maximumInFlight > limits.maximumObservations ||
      !validPositiveInteger(limits.maximumCauseDepth) ||
      !validPositiveInteger(limits.maximumNestedDepth) ||
      !validPositiveInteger(limits.maximumHandlerTicks) ||
      !validPositiveInteger(limits.maximumRecoveryTicks) ||
      !validPositiveInteger(limits.restartWindowTicks) ||
      !validPositiveInteger(limits.backoffTicks) ||
      !validPositiveInteger(limits.cooldownTicks) ||
      !validPositiveInteger(limits.operatorWaitTicks) ||
      limits.restartWindowTicks > limits.maximumRecoveryTicks ||
      limits.backoffTicks > limits.maximumRecoveryTicks ||
      limits.cooldownTicks > limits.maximumRecoveryTicks ||
      limits.operatorWaitTicks > limits.maximumRecoveryTicks ||
      !validPositiveInteger(limits.maximumEvidenceEvents)) {
    return SupervisionReason.UnboundedContract;
  }
  const identities = new Set();
  for (const action of binding.actions) {
    const identity = `${action.kind}\0${action.target ?? ""}`;
    if (!action.kind || !validPositiveInteger(action.maximumUses) ||
        targetActions.has(action.kind) !== Boolean(action.target) ||
        identities.has(identity)) {
      return SupervisionReason.InvalidContract;
    }
    identities.add(identity);
  }
  return null;
}

export class BoundedBrowserSupervisor {
  #pending = [];
  #evidence = [];
  #uses = new Map();
  #observations = 0;
  #decisions = 0;
  #terminal = false;
  #cancelled = false;

  constructor(binding, profile = "browser") {
    const reason = validateBinding(binding);
    if (reason) throw new TypeError(reason);
    if (!["browser", "hosted", "deterministic", "constrained"].includes(profile)) {
      throw new TypeError(SupervisionReason.UnsupportedProfile);
    }
    this.binding = Object.freeze({
      ...binding,
      limits: Object.freeze({ ...binding.limits }),
      members: Object.freeze([...binding.members]),
      actions: Object.freeze(binding.actions.map((action) => Object.freeze({ ...action }))),
    });
    this.profile = profile;
  }

  #reserveEvidence(count) {
    return this.#evidence.length + count <= this.binding.limits.maximumEvidenceEvents;
  }

  #emit(kind, actionIndex = null, reason = null) {
    const evidence = Object.freeze({
      sequence: this.#evidence.length,
      kind,
      actionIndex,
      reason,
    });
    this.#evidence.push(evidence);
    return evidence;
  }

  #reject(reason, actionIndex = null) {
    if (!this.#reserveEvidence(1)) return failure(SupervisionReason.EvidenceBudgetExhausted);
    this.#emit("decision-rejected", actionIndex, reason);
    return failure(reason);
  }

  admit(observation) {
    if (this.#terminal || this.#cancelled) return failure(SupervisionReason.SupervisorTerminal);
    if (!observation ||
        (observation.subject !== this.binding.subject &&
          !this.binding.members.includes(observation.subject)) ||
        !observation.run || !observation.expandedSubject ||
        !validPositiveInteger(observation.generation) ||
        !validPositiveInteger(observation.attempt) ||
        !Array.isArray(observation.causedBy) ||
        observation.causedBy.length > this.binding.limits.maximumCauseDepth) {
      return failure(SupervisionReason.ObservationInvalid);
    }
    const budget = observation.budget;
    if (!budget ||
        !validNonnegativeInteger(budget.remainingObservations) ||
        !validNonnegativeInteger(budget.remainingDecisions) ||
        !validNonnegativeInteger(budget.remainingAttempts) ||
        !validNonnegativeInteger(budget.remainingEvidenceEvents) ||
        !validNonnegativeInteger(budget.nowTick) ||
        !validPositiveInteger(budget.deadlineTick)) {
      return failure(SupervisionReason.ObservationInvalid);
    }
    if (budget.remainingObservations <= 0) {
      return failure(SupervisionReason.ObservationBudgetExhausted);
    }
    if (budget.remainingDecisions <= 0) {
      return failure(SupervisionReason.DecisionBudgetExhausted);
    }
    if (budget.remainingEvidenceEvents < 2) {
      return failure(SupervisionReason.EvidenceBudgetExhausted);
    }
    if (budget.deadlineTick <= budget.nowTick ||
        budget.deadlineTick > budget.nowTick + this.binding.limits.maximumRecoveryTicks) {
      return failure(SupervisionReason.DeadlineExpired);
    }
    if (this.#observations >= this.binding.limits.maximumObservations) {
      return failure(SupervisionReason.ObservationBudgetExhausted);
    }
    if (this.#pending.length >= this.binding.limits.maximumInFlight) {
      return failure(SupervisionReason.InFlightLimitReached);
    }
    if (!this.#reserveEvidence(2)) return failure(SupervisionReason.EvidenceBudgetExhausted);
    this.#observations += 1;
    this.#pending.push(Object.freeze({ ...observation, budget: Object.freeze({ ...budget }) }));
    this.#emit("terminal-observed");
    this.#emit("observation-admitted");
    return Object.freeze({ ok: true });
  }

  decide(observation, decision) {
    if (this.#terminal || this.#cancelled) return this.#reject(SupervisionReason.SupervisorTerminal);
    const index = this.#pending.findIndex((pending) => sameCorrelation(pending, observation));
    if (index < 0) return this.#reject(SupervisionReason.ObservationInvalid);
    const admittedObservation = this.#pending[index];
    if (this.#decisions >= this.binding.limits.maximumDecisions ||
        admittedObservation.budget.remainingDecisions <= 0) {
      return this.#reject(SupervisionReason.DecisionBudgetExhausted);
    }
    if (admittedObservation.budget.nowTick >= admittedObservation.budget.deadlineTick) {
      return this.#reject(SupervisionReason.DeadlineExpired);
    }
    if (admittedObservation.budget.remainingEvidenceEvents < 2) {
      return this.#reject(SupervisionReason.EvidenceBudgetExhausted);
    }
    if (this.profile === "constrained" && !constrainedActions.has(decision.kind)) {
      return this.#reject(SupervisionReason.UnsupportedProfile);
    }
    const actionIndex = this.binding.actions.findIndex((candidate) =>
      candidate.kind === decision.kind && (candidate.target ?? null) === (decision.target ?? null));
    if (actionIndex < 0) return this.#reject(SupervisionReason.ActionNotAdmitted);
    const action = this.binding.actions[actionIndex];
    if (action.requiresNewEpoch) {
      return this.#reject(SupervisionReason.CandidateEpochRequired, actionIndex);
    }
    if (decision.kind === "retry-same" &&
        (admittedObservation.retry !== "idempotent" || !action.permitsEffectReplay)) {
      return this.#reject(SupervisionReason.RetryNotDeclaredIdempotent, actionIndex);
    }
    if (["restart-same", "retry-same"].includes(decision.kind) &&
        admittedObservation.budget.remainingAttempts <= 0) {
      return this.#reject(SupervisionReason.AttemptBudgetExhausted, actionIndex);
    }
    if (decision.kind === "continue-declared-degraded-mode" &&
        this.binding.requiredBehavior && !action.preservesRequiredGuarantees) {
      return this.#reject(SupervisionReason.RequiredGuaranteeWouldWeaken, actionIndex);
    }
    const identity = `${action.kind}\0${action.target ?? ""}`;
    const uses = this.#uses.get(identity) ?? 0;
    if (uses >= action.maximumUses) {
      return this.#reject(SupervisionReason.ActionNotAdmitted, actionIndex);
    }
    if (!this.#reserveEvidence(2)) return this.#reject(SupervisionReason.EvidenceBudgetExhausted);
    const boundedDeadline = (delta) =>
      Math.min(
        admittedObservation.budget.deadlineTick,
        admittedObservation.budget.nowTick + delta,
      );
    const restarting = ["restart-same", "retry-same"].includes(decision.kind);
    const attemptNotBeforeTick = restarting
      ? admittedObservation.budget.nowTick + this.binding.limits.backoffTicks
      : null;
    if (attemptNotBeforeTick !== null &&
        attemptNotBeforeTick >= admittedObservation.budget.deadlineTick) {
      return this.#reject(SupervisionReason.DeadlineExpired);
    }

    this.#uses.set(identity, uses + 1);
    this.#decisions += 1;
    this.#pending.splice(index, 1);
    this.#emit("decision-accepted", actionIndex);
    const consequence = {
      "propagate": "propagated",
      "stop-scope": "final-outcome",
      "restart-same": "attempt-started",
      "retry-same": "attempt-started",
      "activate-declared-fallback": "fallback-selected",
      "continue-declared-degraded-mode": "degraded-selected",
      "request-operator-action": "operator-action-requested",
    }[decision.kind];
    this.#emit(consequence, actionIndex);
    if (decision.kind === "propagate" ||
        (decision.kind === "stop-scope" && this.binding.failureMode === "fail-together")) {
      this.#terminal = true;
    }
    return Object.freeze({
      ok: true,
      consequence,
      affectedScope: decision.kind === "propagate"
        ? "outward"
        : decision.kind === "stop-scope" &&
            this.binding.failureMode === "isolated-optional"
          ? "observed-subject"
          : restarting
            ? "observed-subject"
            : "bound-scope",
      nextAttempt: restarting ? admittedObservation.attempt + 1 : null,
      timing: Object.freeze({
        attemptNotBeforeTick,
        restartWindowDeadlineTick: restarting
          ? boundedDeadline(this.binding.limits.restartWindowTicks)
          : null,
        cooldownUntilTick: [
          "activate-declared-fallback",
          "continue-declared-degraded-mode",
        ].includes(decision.kind)
          ? boundedDeadline(this.binding.limits.cooldownTicks)
          : null,
        operatorDeadlineTick: decision.kind === "request-operator-action"
          ? boundedDeadline(this.binding.limits.operatorWaitTicks)
          : null,
      }),
    });
  }

  cancel() {
    if (this.#terminal) return failure(SupervisionReason.SupervisorTerminal);
    if (!this.#reserveEvidence(1)) return failure(SupervisionReason.EvidenceBudgetExhausted);
    this.#cancelled = true;
    this.#pending.length = 0;
    this.#emit("cancelled");
    return Object.freeze({ ok: true });
  }

  handlerTerminated(kind, nowTick = null) {
    if (this.#terminal || this.#cancelled) return failure(SupervisionReason.SupervisorTerminal);
    const observed = this.#pending[0];
    if (!observed) return failure(SupervisionReason.ObservationInvalid);
    if (!["failed", "timeout", "cleanup-failed"].includes(kind)) {
      return failure(SupervisionReason.ObservationInvalid);
    }
    const handlerDeadline = Math.min(
      observed.budget.deadlineTick,
      observed.budget.nowTick + this.binding.limits.maximumHandlerTicks,
    );
    if (kind === "timeout" &&
        (!validNonnegativeInteger(nowTick) || nowTick < handlerDeadline)) {
      return failure(SupervisionReason.ObservationInvalid);
    }
    if (!this.#reserveEvidence(1)) return failure(SupervisionReason.EvidenceBudgetExhausted);
    if (observed.causedBy.length + 1 > this.binding.limits.maximumCauseDepth) {
      return failure(SupervisionReason.ObservationInvalid);
    }
    const reason = kind === "timeout"
      ? SupervisionReason.DeadlineExpired
      : SupervisionReason.SupervisorTerminal;
    const evidenceKind = {
      failed: "handler-failed",
      timeout: "exhausted",
      "cleanup-failed": "cleanup-failed",
    }[kind];
    const outward = Object.freeze({
      ...observed,
      subject: this.binding.handler,
      expandedSubject: this.binding.handler,
      retry: "undeclared",
      causedBy: Object.freeze([
        ...observed.causedBy,
        Object.freeze({
          code: observed.code,
          subject: observed.subject,
          generation: observed.generation,
          attempt: observed.attempt,
        }),
      ]),
      code: kind === "timeout" ? "deadline-expired" : "node-failed",
      phase: kind === "cleanup-failed" ? "cleanup" : "step",
    });
    this.#pending.length = 0;
    this.#terminal = true;
    this.#emit(evidenceKind, null, reason);
    return Object.freeze({ ok: false, code: reason, outward });
  }

  get evidence() {
    return Object.freeze([...this.#evidence]);
  }

  get pending() {
    return this.#pending.length;
  }
}
