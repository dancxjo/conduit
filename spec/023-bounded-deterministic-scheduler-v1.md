# Bounded deterministic scheduler contract version 1

Status: C4 normative portable contract

Scheduler contract version: 1

Depends on: specifications 007, 008, 011, 012, and 022

## Boundary

This specification defines how an executor turns an exact plan's `FlowPolicy`
cords and implementation profiles into bounded live execution. It does not add
a scheduler, task, callback, or async-framework field to semantic node, port,
type, panel, or plan descriptors. The exact plan already supplies sufficient
inputs: primitive topology, finite queue policies, lifecycle policy pins,
implementation profiles, resource allocations, pool maxima, and evidence
budget.

The scheduler is an implementation of those inputs, not another semantic
graph. A hosted Tokio reactor, a single-threaded loop, a WASM host, and an
embedded interrupt dispatcher may supply wake observations, but they MUST
produce the same contract-visible decisions. No executor exposes its channel
or task handle directly to a node.

Actual replicated-pool admission, supervision, and generation transitions
belong to issue #60. This specification freezes the ready/wake substrate and
population reconciliation that work consumes; it does not reinterpret the
frozen pool source or lowering fields.

## Atomic allocation

Before calling any node's `prepare`, an executor MUST:

1. validate the complete exact plan at the current named run-start time;
2. match every scheduled implementation machine to the corresponding
   plan-pinned execution profile;
3. sum every fixed node allocation, exact cord byte reservation, and pool
   worst-case reservation with checked arithmetic;
4. compute bounded executor overhead for queue slots, ready slots, wait
   interests, transaction staging, scheduler events, node/cord metadata, and
   startup scratch;
5. prove that executor overhead fits both the host-declared overhead ceiling
   and the uncommitted portion of the plan memory budget;
6. prove that plan allocations plus overhead fit the caller's complete runtime
   memory reservation; and
7. allocate every mandatory fixed structure.

Failure at any step starts and prepares nothing. After preallocation, the
executor obtains all prepare results before `prepare_all`, then all start
results before `start_all`. Prepare and start remain effect-free,
nonblocking implementation phases from specification 022.

Cord byte storage is exactly the plan's `queue_memory_bytes`. Fixed slot
metadata is executor overhead. A queued `RuntimeValue` is an
executor-mediated representation handle plus its accounted byte charge; a
second payload queue cannot exist behind it. Adapter, transport, callback,
evidence, and foreign-runtime buffers remain charged to the plan/profile even
when an operating-system allocator has different internal bookkeeping.

## Ready scheduling

Version 1 has one scheduling discipline: bounded round robin.

- The ready queue contains at most one entry per primitive node.
- Initial nodes enter in exact plan order.
- One decision invokes at most one bounded nonblocking node step.
- A valid `progress` or full-budget `yielded` result appends the node at the
  tail.
- A pending node leaves the ready queue until one of its exact interests
  changes.
- Waking several nodes visits them in plan order and appends them to the
  existing tail.
- Completed, failed, or cancelled terminal nodes never re-enter.

The decision log records the node and an exact reason: initial, continued
progress, fair yield, input ready, output ready, timer ready, host operation
ready, cancellation, or terminal propagation. Registry order, hash-map order,
wall time, host discovery, and async-reactor callback order are not scheduling
identities.

Each implementation's `max_step_work` is the per-turn budget. A yield is legal
only when that budget is exhausted. One node may yield only the scheduler
policy's finite consecutive-yield count; exhaustion fails the run with
`CND-SCH-011`. This prevents a node that repeatedly reports readiness without
progress from monopolizing the executor while permitting bounded compute
slices.

The scheduler also has finite maximum decisions and simulated-clock tick.
Exhausting either is terminal and explainable; it is never an invitation to
allocate a continuation queue.

## Staged port transactions

A node sees only `StepIo`, never a cord queue:

- `receive` peeks and stages an input lease;
- `send` checks the exact endpoint, item bound, value-byte bound, aggregate
  byte bound, and pressure state before staging an output reservation;
- input and output access is rejected unless the node is the exact plan
  endpoint;
- several input leases support joins;
- several output reservations support bounded batches/multiport publication;
- representation fragments increment the profile's exact fragment counter
  without allocating spill storage; and
- terminal cord state is observable so a draining node can complete.

After the driver returns, the executor creates `StepUsage` itself and validates
the outcome through `ImplementationMachine`. A valid progress or completion
commits all staged input pops followed by output offers. Pending, yielded,
failed, or invalid results publish and consume nothing. This makes a partial
join or capacity wait a rollback, not data loss.

`RuntimeValue.handle` is opaque. Copying the handle into fixed queue metadata
does not copy, reinterpret, or weaken the represented value. Its semantic
type, concrete representation, ownership, disposal, sensitivity, and
authority remain specification-022/profile facts.

## Pressure and wake rules

Each runtime cord implements the exact specification-007 state machine and
retains occupancy after every transition.

- A full `block(fifo)` offer keeps the value with its producer, marks that
  producer waiting, and emits pressure entry if needed.
- A successful pop wakes the tracked producer.
- An offer into an empty cord wakes the tracked consumer.
- Reject, sample, coalesce, disposable drop, disconnect, and fail use the
  plan's exact policy and create the corresponding observation.
- No task-local channel exists between a node and its cord.
- Output readiness means finite capacity became available or the cord became
  terminal.
- Input readiness means a value arrived or the cord became terminal.

Wake interests are fixed-capacity per-node structures derived from the
implementation profile. Timer interests carry an exact simulated deadline.
Host-operation wakeups name the exact interest; a callback/library queue is
not implied and, if present in a binding, must fit its declared foreign/host
profile. Cancellation wakes every blocked state.

## Terminal and cancellation behavior

Natural node completion begins drain on every outgoing cord. Downstream nodes
continue consuming accepted values; an empty draining cord becomes completed.
The run succeeds only after every node and cord reaches compatible successful
terminal state.

Node or pressure failure aborts affected cord storage, records discarded
accepted values, and fails the run. Disconnect remains distinct from failure.

Abort cancellation:

- invokes the node cancellation hook;
- moves nonterminal implementation machines through cancelling;
- returns/disposes every queued accepted value under evidenced abort;
- wakes all input, output, timer, host, and cancellation waits; and
- becomes cancelled without further node work.

Drain cancellation preserves accepted values and rejects new output. Nodes may
take bounded cancelling steps to consume the drain and return completed; a
completed step while cancelling is terminal cancellation, not success. Every
machine's plan-pinned `cancellation_ticks` is enforced. Missing the deadline
fails closed with `CND-SCH-012`.

## Scheduler evidence and metrics

The executor preallocates exactly `max_events` scheduler observations. It
records allocation, prepare/start, every decision/reason, every node outcome,
flow pressure/occupancy transition, wake, cancellation, and terminal result.
Sequence and simulated tick are monotonic. Filling normative evidence storage
is terminal `CND-SCH-010`; observations never spill into an unplanned log.

These scheduler observations are executor facts. They can later be projected
into specification-012 `ExecutionEvent` records by issue #23, but are not
themselves a new plan or evidence identity. Domain logs and requested domain
evidence cannot replace them.

`DeterministicExecutor` exposes bounded metrics: decisions, maximum ready
depth, maximum cord occupancy, exact allocation, and event count. The
10,000-value reference stress fixture is both a capacity invariant and a
repeatable microbenchmark:

```sh
cargo test -p conduit-runtime --test scheduler \
  long_run_never_exceeds_plan_capacity_and_exposes_metrics -- --nocapture
```

Elapsed wall time is informative and host-specific. Exact decision count,
maximum ready depth, maximum occupancy, and completion are normative.

## Pool population substrate

The portable `PoolPopulation` type gives #60 mutually exclusive counters for
queued, pending, blocked, ready, running, preempted, checkpointing, restarting,
retiring, and terminal-cleanup slots. An independently maintained
`reserved_total` MUST equal their checked sum. Live and queued sums MUST fit
the exact plan maxima on every transition.

Only ready plus running work is runnable. A task waiting on a GPU, device,
exclusive resource, timer, or host operation remains bounded `blocked` or
`pending`; it does not create indirect debt, shortfall, or new admission.

The portable restart assessment uses only explicit current state: attempt and
maximum attempt, measured progress, minimum useful progress, checkpoint cost,
remaining time, cooldown, and starvation deadline. Finite attempts and a
starvation deadline make zero-completion restart cycles terminal. #60 may
define concrete admission/supervision descriptors, but MUST preserve these
reconciliation and no-manufactured-demand invariants.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-SCH-001` | unsupported or malformed scheduler policy |
| `CND-SCH-002` | a scheduler-owned limit is zero or unbounded |
| `CND-SCH-003` | pool populations overflow or do not reconcile |
| `CND-SCH-004` | plan/node/profile set is invalid or mismatched |
| `CND-SCH-005` | checked allocation exceeds plan/host reservation or cannot be allocated |
| `CND-SCH-006` | atomic prepare/start failed |
| `CND-SCH-007` | step or endpoint access violates the execution contract |
| `CND-SCH-008` | fixed transaction/wait staging capacity was exceeded |
| `CND-SCH-009` | decision or simulated-clock bound was exhausted |
| `CND-SCH-010` | normative scheduler evidence storage is full |
| `CND-SCH-011` | bounded zero-progress yield budget was exhausted |
| `CND-SCH-012` | cancellation deadline was exhausted |
| `CND-SCH-013` | node or queue failure propagated to the run |

## Conformance

`conformance/c4/bounded-scheduler-v1.json` freezes 32 positive, negative,
boundary, stress, and anti-livelock cases. `conduit-core` owns portable policy,
population, and restart fixtures. `conduit-runtime` owns the deterministic
allocation, queue, transaction, wake, fairness, cancellation, terminal,
evidence-capacity, and long-run fixtures.

The deterministic executor is the normative event-order oracle. Hosted stress
tests may add concurrency or performance evidence but cannot redefine its
queue or lifecycle sequences.

## Normative requirements

| ID | Obligation |
|---|---|
| SCH-001 | Keep scheduling host-, language-, ABI-, and async-framework-neutral |
| SCH-002 | Validate and allocate all mandatory plan/runtime structures before prepare or start |
| SCH-003 | Bound runtime memory by exact plan allocations plus declared executor overhead |
| SCH-004 | Allocate every live cord from its exact finite `FlowPolicy` without hidden queues |
| SCH-005 | Schedule one bounded step per round-robin turn and retain an exact decision reason |
| SCH-006 | Wake only from exact input, output, timer, host-operation, cancellation, or terminal changes |
| SCH-007 | Prevent false-ready, zero-progress, or yielded nodes from monopolizing the executor |
| SCH-008 | Stage leases/reservations and atomically commit or completely roll back each step |
| SCH-009 | Support bounded joins, batches, fragments, and opaque zero-copy handles through the same interface |
| SCH-010 | Apply normative drain, abort, failure, disconnect, and cancellation-deadline state machines |
| SCH-011 | Observe exact pressure, occupancy, decisions, wakes, lifecycle, and terminal results in fixed evidence storage |
| SCH-012 | Fail closed when decisions, clock, transaction storage, allocation, or evidence capacity is exhausted |
| SCH-013 | Reconcile every pool population against plan maxima without deriving demand from blocked resource work |
| SCH-014 | Bound restart oscillation using current progress, cost, attempt, cooldown, and starvation facts |
| SCH-015 | Use deterministic simulation as the event-order oracle and supplement it with long-run capacity stress |
| SCH-016 | Keep actual replicated admission/generations in #60 and evidence projection in #23 |
