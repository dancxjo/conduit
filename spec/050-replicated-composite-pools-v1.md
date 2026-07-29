# Bounded replicated composite pools version 1

Status: normative C4/C5 contract. Requirement identifiers `POL-001` through
`POL-015` are exercised by
`conformance/c4/replicated-pools-v1.json`.

## Boundary

A replicated pool is a finite plan-pinned population of ordinary expanded
composite instances. It is not dynamic graph mutation, an executor species,
an implementation callback array, or permission to address unexported child
ports. Every child remains an ordinary planned node, every internal cord
remains bounded, and composite exports remain the only external boundary.

The runtime consumes the frozen `.panel` pool grammar and lowering identity
from specifications 014, 015, and 017. It does not infer defaults or
reinterpret `maximum`, admission, deadline, idle timeout, supervision,
restart, fallback, or cleanup fields. Host resolution converts authored
milliseconds to one exact monotonic plan time basis and binds all resources.

## Plan schema 16

ExecutionPlan schema 16 adds one `PlanPoolRuntime` to every instance-pool
entry. Pre-schema-16 pool entries remain readable under their frozen identity
but are not executable as replicated runtimes.

The runtime entry retains:

- exact `reject`, caller-retained `block`, `queue-bounded`, or terminal `fail`
  admission;
- exact `fail-together`, `isolate`, bounded restart, resolved fallback, or
  escalation supervision;
- deadline, idle-timeout, restart-backoff, and cleanup ticks in the named plan
  time basis;
- `drain` or `abort` cleanup;
- the per-instance reservation and distinct queued-request reservation;
- the exact resolved implementation-set identity accepted by admission;
- the maximum normative evidence events;
- old, candidate, and rollback generation maxima and coexistence reserve; and
- the complete total reservation covering generation overlap plus every
  queued request.

The existing opaque admission and supervision descriptor pins remain exact
policy dependencies. The runtime fields are the finite resolved values those
policies authorize; neither substitutes for the other.

## Atomic reservation

Before an instance changes from queued or absent to live, the host proves that
the complete per-instance profile is available:

- child-node and child-cord slots;
- instance, scratch, and retained-state memory;
- scheduler and correlation slots;
- timers, deadline, retry, and backoff state;
- pending host-operation and transport slots;
- checkpoint and replay state;
- cancellation scopes; and
- normative evidence capacity.

Authority, sensitivity, exact template identity, and the plan-pinned
implementation-set identity are checked in the same admission decision; a
caller-supplied compatibility boolean is insufficient. Failure starts no child
and consumes no partial live reservation. An implementation that later
reports more memory, work, timers, host operations, or cancellation state than
reserved becomes terminally contained; the runtime does not grow the profile.

Queue entries reserve their identity, correlation, cancellation, and evidence
state but no child activation resources. `block` creates no queue entry: the
caller retains the offer and may retry it. No executor, Promise, worker,
transport, checkpoint, or evidence library may add a hidden queue.

## Identity

Generation identity is a canonical function of exact plan identity, exact pool
path, plan epoch, explicit generation number, and template identity. Instance
identity adds the caller's semantic request identity. Attempt correlation
additionally includes work-unit identity, one-based attempt, and caller
correlation.

Wall clock, arrival order, scheduler order, registry order, map iteration,
host discovery, and slot index are not identity inputs. A restart preserves
instance and work-unit identity while changing the one-based attempt and
attempt correlation. Replaying the same semantic request in a different
generation produces a different instance identity.

## Admission and lifecycle

Every fixed slot is in exactly one state:

```text
empty
queued
reserved
running
checkpointing
restart-backoff
draining
cleanup
succeeded | cancelled | failed
```

Live population consists of reserved, running, checkpointing,
restart-backoff, draining, and cleanup slots. Queued slots are counted
separately. Terminal slots retain evidence until explicitly reclaimed.

At capacity:

- `reject` returns a nonterminal rejection;
- `block` returns caller-retained backpressure without storing the request;
- `queue-bounded` stores at most `maximum_queued` requests; and
- `fail` produces the exact admission-failure outcome.

Queue-full behavior is explicit. Cancellation while queued records the exact
cause and never activates children. A deterministic round-robin cursor selects
queued slots and ready pools without consulting registry order. One controller
tick starts at most one queued instance, so fairness is observable and finite.

## Supervision, time, and cleanup

`fail-together` gives every still-live or queued member the same causal
failure. `isolate` changes only the observed instance. Bounded restart retains
the live reservation through an exact backoff tick and creates a new attempt
only below the maximum attempt count. Exhaustion enters cleanup. Fallback
selects one already resolved ordinary plan target without coercing values or
discovering an implementation. Escalation emits a decision; it acquires no
authority.

Deadline and idle expiry use only the plan's monotonic tick basis. Progress
updates the idle observation and is itself evidence. Cleanup has an exact
deadline. `drain` may preserve a successful completion; `abort` produces a
failed terminal outcome unless cancellation already dominates. Evidence
capacity is reserved before each state mutation. Exhaustion rejects the whole
mutation before state or causality changes.

Checkpoint resume requires exact template identity. A mismatched checkpoint
is evidenced and leaves the running attempt unchanged; migration is a
separate explicitly planned operation.

## Generations

Before a transition, the plan reserves the checked sum of:

```text
old maximum live
+ candidate maximum live
+ rollback maximum live
```

and the matching resource profiles. Queue reservation is added separately.
The sum must fit the total pool reservation. A candidate generation cannot
borrow old-generation cleanup capacity, and rollback cannot manufacture new
slots.

Issue #57 owns transition orchestration. This specification supplies the
required finite primitives: stop admission, cancel queued requests, mark
active attempts draining, retire drained attempts, or clean up a candidate on
rollback. It does not claim seamless replacement when overlap, session state,
or foreign code cannot support it.

## Evidence

Every mutation records a monotonically sequenced event with tick, complete
generation/instance/work-unit/attempt/correlation identity, prior and next
state, stable reason, and optional exact cause. Required events include
admission, queueing, progress, checkpoint decisions, restart scheduling and
start, pressure/profile violation, completion, cancellation, cleanup,
generation drain/rollback/retirement, and terminal outcome.

Evidence is run identity, not `.panel` source, a resolved plan, or Patchbay
presentation. Hosted serializers may project pool events into the immutable
execution-event envelope, but cannot remove correlation or invent missing
events.

## Reference profiles

`conduit-core::PoolController` owns fixed arrays selected with const generics
and remains allocator-free. `conduit-runtime` instantiates that same contract
from schema-16 plan entries, rejects a host profile whose arrays are too
small, and atomically reconciles actual #56 `ImplementationMachine` step
outcomes with the owning pool slot. An invalid foreign step is contained
without advancing the implementation lifecycle copy. The browser reference
uses independently bounded arrays and the same admission/lifecycle decisions.
The RP2040 firmware package links the portable controller and runs the
long-population oracle with static storage.

The RP2040 package test is a firmware/HIL oracle, not evidence that a physical
board was attached. Physical transport and board execution retain the proof
boundary in specification 038.

## Stable reasons

- `CND-POL-001` invalid or unbounded contract / insufficient fixed profile
- `CND-POL-002` invalid generation, request, or derived identity
- `CND-POL-003` unknown slot or illegal lifecycle transition
- `CND-POL-004` deadline arithmetic overflow
- `CND-POL-005` reservation, population, or generation-overlap violation
- `CND-POL-006` evidence or sequence capacity exhausted
- `CND-POOL-HOST-001` selected plan pool is absent
- `CND-POOL-HOST-002` frozen pre-schema-16 pool is not executable

## Normative requirements

| ID | Obligation |
|---|---|
| POL-001 | Bound every live, queued, restarting, draining, cleanup, and terminal slot |
| POL-002 | Reserve all child, cord, memory, timer, host, transport, checkpoint, cancellation, and evidence state atomically |
| POL-003 | Derive generation, instance, work, attempt, and correlation identity without execution order |
| POL-004 | Keep reject, caller-retained block, bounded queue, and terminal fail distinct with no hidden queue |
| POL-005 | Enforce exact deadline, idle, restart-backoff, and cleanup ticks |
| POL-006 | Apply fail-together, isolate, bounded restart, resolved fallback, and escalation without semantic coercion |
| POL-007 | Select ready pools and queued instances with finite deterministic fairness |
| POL-008 | Check authority, sensitivity, template, and implementation profile before activation |
| POL-009 | Resume only an exact compatible checkpoint/template identity |
| POL-010 | Record complete bounded causal evidence before every mutation |
| POL-011 | Reserve simultaneous old, candidate, and rollback generations before transition |
| POL-012 | Contain foreign implementations that exceed any declared profile category |
| POL-013 | Preserve population maxima under long-running pressure and timer simulation |
| POL-014 | Execute equivalent decisions in hosted, browser, and constrained fixed-storage profiles |
| POL-015 | Consume frozen source/lowering semantics and reject legacy runtime execution rather than reinterpret them |
