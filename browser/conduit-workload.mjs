export const WorkloadReason = Object.freeze({
  Unsupported: "CND-WRK-003",
  Benchmark: "CND-WRK-004",
  ExactEnforcement: "CND-WRK-005",
  Stale: "CND-WRK-006",
  Clock: "CND-WRK-007",
  Capacity: "CND-WRK-008",
  Deadline: "CND-WRK-009",
  Overflow: "CND-WRK-010",
  Overload: "CND-WRK-013",
  DeadlineMissed: "CND-WRK-014",
});

const categories = Object.freeze([
  "workUnits",
  "tasks",
  "processes",
  "descriptors",
  "connections",
  "storageBytes",
  "deviceOperations",
  "networkBytes",
  "callbacks",
  "foreignQueueItems",
  "transitionOverlapWorkUnits",
]);

const finiteOrUnsupported = (value) =>
  value === null || (Number.isSafeInteger(value) && value > 0);

export function admitBrowserWorkload(contract, capability, now) {
  if (contract.guarantee === "unsupported") {
    return { ok: false, code: WorkloadReason.Unsupported };
  }
  if (capability.evidenceKind === "benchmark") {
    return { ok: false, code: WorkloadReason.Benchmark };
  }
  if (contract.timeBasis !== now.basis || capability.timeBasis !== now.basis) {
    return { ok: false, code: WorkloadReason.Clock };
  }
  if (now.tick < capability.observedAtTick ||
      now.tick >= capability.validUntilTick) {
    return { ok: false, code: WorkloadReason.Stale };
  }
  if (!categories.every((name) =>
    finiteOrUnsupported(contract.budget[name]) &&
    finiteOrUnsupported(capability.capacity[name]) &&
    (contract.budget[name] === null ||
      (capability.capacity[name] !== null &&
       contract.budget[name] <= capability.capacity[name])))) {
    return { ok: false, code: WorkloadReason.Capacity };
  }
  if (contract.guarantee === "hard" &&
      capability.evidenceKind !== "exact-enforcement") {
    return { ok: false, code: WorkloadReason.ExactEnforcement };
  }
  if (!Number.isSafeInteger(contract.deadlineTicks) ||
      contract.deadlineTicks <= 0 ||
      contract.deadlineTicks > capability.maximumDeadlineTicks ||
      capability.maximumJitterTicks > contract.maximumJitterTicks) {
    return { ok: false, code: WorkloadReason.Deadline };
  }
  const deadlineTick = now.tick + contract.deadlineTicks;
  if (!Number.isSafeInteger(deadlineTick)) {
    return { ok: false, code: WorkloadReason.Overflow };
  }
  return Object.freeze({
    ok: true,
    guarantee: contract.guarantee,
    deadlineTick,
  });
}

export class BrowserWorkloadState {
  constructor(contract, admission) {
    this.contract = contract;
    this.deadlineTick = admission.deadlineTick;
    this.used = Object.fromEntries(categories.map((name) => [name, 0]));
    this.terminal = null;
  }

  record(usage) {
    if (this.terminal !== null) return { ok: false, code: this.terminal };
    for (const name of categories) {
      const addition = usage[name] ?? 0;
      const limit = this.contract.budget[name];
      if (!Number.isSafeInteger(addition) || addition < 0 ||
          (addition > 0 && limit === null) ||
          this.used[name] + addition > (limit ?? 0)) {
        this.terminal = WorkloadReason.Overload;
        return { ok: false, code: this.terminal };
      }
      this.used[name] += addition;
    }
    return { ok: true };
  }

  observe(now) {
    if (now.basis !== this.contract.timeBasis) {
      this.terminal = WorkloadReason.Clock;
    } else if (now.tick >= this.deadlineTick) {
      this.terminal = WorkloadReason.DeadlineMissed;
    }
    return this.terminal === null
      ? { ok: true }
      : { ok: false, code: this.terminal };
  }
}
