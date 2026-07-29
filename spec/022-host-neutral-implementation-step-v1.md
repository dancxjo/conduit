# Host-neutral implementation and bounded step contract version 1

Status: C4 normative portable contract

Implementation execution-profile schema: 1

ExecutionPlan schema carrying profiles: 3

Foreign message-binding protocol: 1

## Boundary

This specification defines the abstract protocol between an executor and one
already selected node implementation. It is independent of Rust traits,
language ABI, async framework, process protocol, WASM component model, host
API, and scheduler implementation.

The contract begins after an exact plan has pinned the semantic node contract,
implementation, artifact, host observation, resources, authority, lifecycle
policy, and implementation execution profile. It performs no discovery,
provisioning, ambient configuration lookup, secret resolution, dynamic
loading, or grant acquisition.

The identities remain separate:

- `TypeContractRef` says what a value means;
- `ValueRepresentation` pins how one implementation boundary carries it;
- the implementation manifest identifies selected code;
- `ExecutionProfile` identifies exact execution limits and representation
  bindings;
- `ExecutionPlan` schema 3 pins that profile to one primitive instance;
- runtime attempt/run identity remains outside the immutable plan;
- executor observations become normative `ExecutionEvent` facts;
- implementation domain evidence and logs do not replace those observations.

No type, port, panel, or domain descriptor gains a Rust, WASM, process, or host
field.

## Exact execution profile

`conduit/implementation-execution-profile` schema 1 contains:

- profile ID, schema version, and canonical semantic hash;
- hard or observed boundedness claim;
- bounded, cooperative, or unbounded cancellation guarantee;
- whether the complete stack enforces the declared step-work ceiling;
- exact retained-value/byte and per-step scratch ceilings;
- simultaneous input-lease/output-reservation item and byte ceilings;
- transaction and incremental-fragment ceilings;
- pending-operation, timer, child-task, host-buffer, foreign-queue, and
  optional checkpoint ceilings;
- aggregate implementation-controlled memory;
- finite cancellation deadline ticks;
- per-port semantic type and distinct concrete representation pins;
- owned, borrowed, shared-handle, or exclusive-handle ownership;
- explicit disposal for every handle representation; and
- non-overlapping memory claims classified as executor-allocated,
  backend-bounded, externally bounded, or observed-only.

The sum of memory claims equals the aggregate implementation memory charge.
The sum of directly bounded byte categories cannot exceed it. A hard profile
rejects observed-only memory, cooperative/unbounded cancellation, or a stack
that cannot enforce its step ceiling. The profile must fit the node's exact
plan allocation, timers, and checkpoint count.

Plan schemas 1 and 2 require no execution profile and retain their frozen
identities. Plan schema 3 requires exactly one valid profile on every
primitive node and adds a domain-separated node/profile fact to plan identity.
A profile mutation without a matching canonical hash is rejected. Adding a
profile to a v1/v2 plan or omitting one from v3 is invalid.

## Lifecycle and prepare atomicity

The implementation instance protocol is:

```text
instantiate -> prepared -> started -> step*
                            |          |
                            +-> drain -+
                            +-> cancel -> terminal
                            +-> abort  -> terminal
```

`instantiate` requires the exact instance, implementation/artifact/profile
pins, a validated-configuration decision, exactly accounted caller memory,
the complete required/provided resource and grant sets, and a cancellation
scope. Missing or ambient bindings fail before an instance exists. It starts
no effect.

The executor collects a prepare result from every required instance before
changing any instance to `prepared`. If any prepare fails, all instances
remain instantiated and none starts. `start_all` likewise requires the entire
set to be prepared before any moves to started. This is the implementation
side of the existing lifecycle `created -> preparing -> ready -> running`
contract, not a second semantic lifecycle.

Prepare and start are themselves nonblocking. Executor-measured work and
scratch must fit the profile's step ceilings, and neither operation may leave
a pending host operation. A bound failure leaves every instance in its prior
phase.

Drain and cancellation use specification 008 causes, deadlines, and queue
disposition. Abort is terminal cancellation. A completed or failed step maps
to the existing succeeded or failed terminal states; terminal instances never
restart in place.

## One bounded nonblocking step

A step receives an exact work ceiling and returns exactly one outcome:

| Outcome | Required meaning |
|---|---|
| `progress` | at least one executor-observed operation committed |
| `pending(interests)` | zero observable commits; at least one unique named input, output, timer, host-operation, or cancellation interest |
| `yielded` | progress remains possible and the exact step-work ceiling was exhausted |
| `completed` | declared successful terminal condition |
| `failed(code)` | stable structured implementation failure |

The executor measures work, transactions, retained state, scratch, leases,
reservations, pending operations, timers, child tasks, host/foreign buffers,
and fragments. Binding replies cannot author or suppress these measurements.
Domain evidence requests are counted separately and are not proof of
progress.

`progress` with zero observable commits is `CND-IMP-006`. Empty or duplicate
pending interests are `CND-IMP-007`. Yield before exhausting the exact work
budget is false progress/monopolization. Any counter beyond the profile is
`CND-IMP-005`. A blocking call, hidden callback queue, anonymous poll-later,
or arbitrary spawned task cannot be represented by a valid outcome.

Sources, transforms, joins, sinks, and service nodes all use this same
protocol. Their differences are ports, wake interests, host operations, and
terminal contracts—not alternate callback models.

## Executor-mediated port transactions

Acquiring an input creates a lease; it does not consume the value. Reserving
an output proves bounded capacity before work commits. Counts, accounted
bytes, representation maximums, and fragment counts are checked before
transaction state changes.

One commit atomically:

1. consumes all of that transaction's input leases;
2. publishes its reserved outputs and produced bytes; and
3. creates executor-observed progress.

Rollback consumes and publishes nothing and releases every reservation.
Therefore output-full rejection after input lease cannot lose the input.
Several inputs implement joins. Several outputs in one atomic transaction
implement coupled publication. Separate independent transactions deliberately
permit partial multi-output progress without silently coupling branches.

Incremental producers reserve a finite total and emit no more than the
profile's fragment and byte maxima. The transaction is local to planned
storage; it is not a database transaction or an internal queue.

Zero-copy and host-handle values retain separate semantic and representation
identities. Borrow/own/share/exclusive rules are representation facts.
Shared/exclusive handles require explicit disposal; sensitivity remains the
owning port/type/authority contract and is never weakened by representation.

## Host services and checkpointing

A host operation names:

- operation;
- exact plan resource-binding ID;
- exact grant ID;
- named deadline basis and finite deadline;
- cancellation scope;
- finite buffer charge; and
- correlation ID.

Validation requires membership in the node's exact binding/grant slices, the
run's named time basis, a deadline no later than the profile cancellation
ceiling, a bounded buffer, and at least one planned pending-operation slot.
There is no ambient filesystem, network, process, device, clock, secret, or
authority path.

Checkpoint/state export is optional. A request must exactly match the
profile's pinned checkpoint contract and byte ceiling. Presence does not
claim portability across implementations, representations, hosts, or plan
epochs.

## Bindings

`conduit-core` contains only borrowed, allocator-free protocol types and
validators. `conduit-runtime` supplies two non-normative hosted examples:

- a direct native Rust call adapter; and
- an explicit version-1 request/reply adapter suitable for a process or WASM
  component transport.

Both adapters return outcomes/interests while executor-measured `StepUsage`
arrives separately. The same fixture proves equivalent semantic output and
executor observation. A protocol-version mismatch fails before step evidence.
These adapters do not define a universal ABI or executable format.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-IMP-001` | malformed, unsupported, or falsely hard execution profile/binding |
| `CND-IMP-002` | profile identity mismatch |
| `CND-IMP-003` | profile exceeds exact plan allocation |
| `CND-IMP-004` | illegal implementation lifecycle operation |
| `CND-IMP-005` | step-controlled resource ceiling exceeded |
| `CND-IMP-006` | false progress or premature yield |
| `CND-IMP-007` | pending without exact finite unique interests |
| `CND-IMP-008` | lease/reservation/fragment/commit transaction violation |
| `CND-IMP-009` | host operation lacks an exact bounded resource/authority/deadline context |
| `CND-IMP-010` | absent, mismatched, or oversized checkpoint contract |
| `CND-IMP-011` | prepare-all failed before any instance started |
| `CND-IMP-012` | instantiation differs from the exact validated plan binding |

## Conformance

`conformance/c4/implementation-step-v1.json` freezes 45 positive, negative,
boundary, migration, native/message-equivalence, and malicious-profile cases.
The allocator-free reference tests cover exact profile identity, plan-schema-3
pinning, atomic preparation, every step outcome, wake interests, resource
ceilings, transaction rollback/joins/fragments/publication modes, host
operations, handles, cancellation, checkpointing, and executor-owned evidence.

## Normative requirements

| ID | Obligation |
|---|---|
| IMP-001 | Keep the normative contract language/ABI/async/host neutral and allocator-free |
| IMP-002 | Preserve semantic type, concrete representation, implementation, profile, plan, run, evidence, and log identities |
| IMP-003 | Pin one exact canonical execution profile per primitive in plan schema 3 without rewriting v1/v2 |
| IMP-004 | Make every implementation-controlled allocation, queue, task, timer, operation, fragment, and work unit finite and plan-visible |
| IMP-005 | Prepare all required instances and runtime structures atomically before any start |
| IMP-006 | Accept only progress, exact pending interests, budget-exhausted yield, completion, or stable failure |
| IMP-007 | Reject false progress, anonymous polling, blocking, arbitrary task spawning, and zero-progress monopolization |
| IMP-008 | Lease inputs and reserve outputs before atomically committing declared consumption/publication |
| IMP-009 | Rollback without input consumption or output publication on capacity or step failure |
| IMP-010 | Support bounded joins, atomic and independent fan-out, incremental fragments, and explicit handle disposal |
| IMP-011 | Require exact resource, grant, deadline, cancellation, budget, and correlation facts for host operations |
| IMP-012 | Make checkpoint/state export optional, pinned, bounded, and non-portable by default |
| IMP-013 | Keep executor-observed lifecycle/flow/resource evidence authoritative over implementation logs/domain evidence |
| IMP-014 | Reject hard-profile claims when any dependency has only observed memory or unbounded step/cancellation behavior |
| IMP-015 | Prove materially different direct-native and message-based bindings against the same fixtures |
| IMP-016 | Instantiate only from validated configuration, exact pins, caller-accounted memory, complete resource/grant sets, and a scoped cancellation identity |
