# Workload admission and deadline guarantees current form

Status: C5 semantic and host-evidence contract

This contract keeps reservation authority, enforcement capability, host
observations, measurements, benchmark reports, runtime evidence, and
presentation separate. A benchmark or a successful run never becomes an
admission promise.

## Plan-visible declaration

Execution-current plan schema may pin a `PlanWorkload` to an exact service identity
and real node. Its contract classifies the claim as hard, measured,
host-observed best-effort, or unsupported. CPU/work, task, process, descriptor,
connection, storage, device, network, callback, foreign-queue, and
transition-overlap categories are each either a positive finite maximum or
explicitly unsupported. Zero and omitted implicit ceilings are invalid.

A deadline names one exact time basis, a positive relative deadline, and a
finite jitter ceiling. Checked arithmetic derives its absolute tick.

## Admission boundary

Hard admission requires fresh `exact-enforcement` capability evidence from the
host observation pinned by the plan. The capability must use the same clock,
cover every required finite category, and cover the deadline and jitter
contract. Measurement, benchmark, generic host observation, browser
availability, priority hints, and prior successful runs cannot satisfy this
boundary.

Task ceilings include threads. current form exposes no implicit priority or
thread-escalation mechanism: work requiring more tasks fails capacity
admission, while any future priority-specific hard claim must add an exact
versioned capability rather than infer one from a host hint.

Measured and host-observed profiles remain useful evidence, but their labels
do not imply deadline enforcement. An unsupported declaration fails closed.

## Use-time outcomes

The allocation-free workload state accounts every reported use, including
callbacks, foreign queues, and old/candidate/rollback overlap. Overload, wrong
clock, missed deadline, excessive jitter, and evidence exhaustion are explicit
terminal outcomes. Best-effort downgrade is never reported as hard success.

Linux witnesses record process/descriptor/timing observations as
`measurement`. Browser witnesses are host-observed best effort. A constrained
profile may truthfully report unsupported. RP2040 hard timing requires an
exact firmware/build profile; physical timing remains HIL evidence.

## Stable diagnostics

- `CND-WRK-001`: unsupported workload contract version
- `CND-WRK-002`: malformed declaration or capability
- `CND-WRK-003`: workload explicitly unsupported
- `CND-WRK-004`: benchmark used as admission authority
- `CND-WRK-005`: exact enforcement evidence required
- `CND-WRK-006`: stale observation
- `CND-WRK-007`: wrong clock basis
- `CND-WRK-008`: finite capacity exceeded
- `CND-WRK-009`: deadline or jitter unsupported
- `CND-WRK-010`: deadline arithmetic overflow
- `CND-WRK-011`: required evidence capacity exhausted
- `CND-WRK-012`: illegal lifecycle transition
- `CND-WRK-013`: admitted workload overloaded
- `CND-WRK-014`: deadline missed
- `CND-WRK-015`: jitter ceiling exceeded

## Requirements

| ID | Obligation |
|---|---|
| WRK-001 | Keep admission authority distinct from telemetry, measurements, and benchmarks |
| WRK-002 | Classify every workload guarantee honestly |
| WRK-003 | Make every resource category finite or explicitly unsupported |
| WRK-004 | Admit hard deadlines only from fresh exact enforcement evidence |
| WRK-005 | Bind deadline arithmetic and outcomes to one named clock |
| WRK-006 | Account callbacks, foreign queues, and transition overlap |
| WRK-007 | Produce bounded deterministic terminal and evidence outcomes |
| WRK-008 | State deterministic, Linux, browser, and constrained guarantees non-equivalently |
