# Performance and resource-accounting program version 1

Status: C4 verification contract

This program measures implementations of existing semantic contracts. A
benchmark result is an observation, never a deadline, admission promise, or
portable guarantee. Workload/deadline claims belong to issue #83.

## Reproducible matrix

`cargo xtask performance-gate` builds release artifacts, records the exact
commit, Rust release and host target, OS, architecture, CPU, and conformance
fixture revision, then runs the reviewed workload commands in
`benchmarks/baseline-v1.json`. The matrix covers canonicalization,
compatibility, panel parsing, source lowering, exact-plan validation,
scheduler throughput/local latency, coupled fan-out pressure, and runtime
evidence recording. All release test binaries are built before timing begins,
so compilation is excluded. Linear graph parsing uses reviewed small, medium,
and maximum benchmark scales of 2, 32, and 256 nodes; future higher supported
limits extend this explicit matrix rather than silently changing it.

Wall elapsed time is report-only because shared runners are noisy. The
deterministic scheduler fixtures strictly gate decisions, queue/ready/event
high-water marks, capacity, completion, and evidence ordering. There are no
sleeps, networks, external services, or developer-machine timing limits.

Tongues, Netherwick, RP2040 executable flash/static-RAM/stack, and transition
overlap workloads remain explicitly assigned to #31, #33, #28, and #57.

## Runtime memory equation

Before prepare, `SchedulerAllocation` reconciles:

```text
planned memory =
    node allocations
  + cord payload reservations
  + pool worst-case allocations
  + Resonance event-stream allocations
  + durable-job allocations

complete startup reservation =
    planned memory
  + queue slot metadata
  + ready state
  + wake interests
  + transaction staging
  + scheduler evidence storage
  + node/cord metadata
  + startup scratch
```

Every term uses checked arithmetic. The complete reservation must fit both the
plan budget and caller/host ceiling before any node prepares. Runtime high
water reports total queued items/payload bytes, ready slots, scheduler event
slots, and decisions. These observations must not replace the reservation or
exclude allocator/library bookkeeping. Process RSS is not a hard-memory
profile.

Evidence records are additionally constrained by the exact runtime-evidence
policy and plan-owned Resonance allocation from specification 029.

## Artifact-size baseline

The reviewed baseline tracks the optimized `conduct` host executable, hosted
`conduit-core` library archive, and allocator-free `thumbv6m-none-eabi`
`conduit-core` archive. The embedded archive is not represented as RP2040
flash or static RAM; those require #28's linked executor image and linker map.

Each artifact has a percentage allowance and a meaningful absolute allowance.
Growth fails when it exceeds the larger allowance. Intentional changes use
the baseline's named update command, inspect the semantic and size cause, and
commit the reviewed metadata. Opaque local benchmark caches are not baselines.

## CI and ownership

`just perf` runs the report and strict artifact gate. Hosted CI invokes it
after semantic tests. The baseline names its owner, toolchain, fixture
revision, reviewed commit, thresholds, update procedure, current workloads,
and deferred owners.

## Requirements

| ID | Obligation |
|---|---|
| PRF-001 | Record exact commit, toolchain, target, host/CPU, and fixture revision with every report |
| PRF-002 | Cover canonicalization, compatibility, parse/lower, plan validation, scheduling, fan-out, and evidence paths |
| PRF-003 | Keep shared-runner wall timing report-only and deterministic counts/capacities strict |
| PRF-004 | Reconcile every plan-owned memory category plus executor overhead before prepare |
| PRF-005 | Report deterministic queue, ready, event, and decision high-water observations without promoting them to guarantees |
| PRF-006 | Track host CLI, hosted core, and embedded core artifacts without mislabeling archives as flash/RAM |
| PRF-007 | Fail unreviewed artifact growth beyond explicit percentage and absolute allowances |
| PRF-008 | Store owner, update procedure, workloads, thresholds, and deferred workload ownership in the reviewed baseline |
