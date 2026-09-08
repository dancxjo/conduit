# Explicit state/delay contract

Conduit preserves acyclic ordinary dataflow. Recurrence exists only through an
explicit `State` boundary whose semantic ports are:

```text
initial: T    next: T
--------------------
current: T
```

For generation `n`, `current` exposes the committed value. The ordinary graph
computes at most one candidate `next`. At the admitted transition point,
`current[n+1] = next[n]`; if no candidate exists, the current value is retained
and generation still advances. Multiple candidates refuse. Failure or
cancellation discards the candidate and cannot commit it. Reset discards any
candidate, restores the explicit initial value, and returns to generation zero.

The planner cuts only Cords entering declared State placements while checking
the one-step dependency graph. A cycle that remains is an ordinary zero-delay
cycle and refuses. Each State has exactly one typed writer, matching current
and next kinds, explicit initialization, a maximum value size, and either a
finite transition count or explicitly externally bounded continuation.

Admission reserves two value slots per State (current and candidate), twice the
maximum value bytes, and mandatory transition Sign storage. Fixed and hosted
profiles use the same allocation-independent transition machine; their only
difference is the storage ceiling chosen before Play start.

State evidence names the State, generation, current identity, candidate
identity when present, and initialized/candidate/commit/hold/reset/cancel/fail
transition. This evidence is a projection of the state machine and is never an
input to commitment or scheduling. General durable snapshot/checkpoint recovery is not implied by this cell.
The hosted owned-State handoff described below is a separately admitted path.

Externally continued State uses the same fixed current/candidate storage without
requiring a predetermined semantic transition count. Each admitted input/step
still crosses the explicit commitment boundary; awaiting another input neither
commits State nor renews a budget. The finite generation-identity representation
is a separate realization limit: exhaustion refuses before rollover, leaving
current State unchanged. It is not semantic completion or permission to reset.
Candidate evidence retains the identity assigned at offer through commit or
abort. A finite transition-budget realization continues to report its distinct
budget refusal. Checked-Form execution and cross-Play continuity use the installed hosted
State path described below; #2688 and #2691 retain the wider acceptance scope.
The kernel `StateOperation` adapter exposes this cell through exact next/current
ports in the existing `OperationDriver` and fixed scheduler. Its profile admits
at most the existing canonical-emission byte envelope; larger cells refuse
construction. Input closure completes this adapter, while awaiting input remains
nonterminal. Output pressure retains the pending emission. The scheduler tests
use finite queue, value and Sign capacities, and distinguish an exhausted
transition allowance from successful processing under a larger allowance.
The adapter is installed by the std typed-State preparation path. Its presence
in the kernel alone does not prove another Host implements that path. Finite
generation and Sign capacity are not claims of infinite physical execution.

## Immutable State admission

`PlanFragment.states` carries the exact State identity, owning Gear, value Kind,
initial bytes, retained-byte capacity and continuation demand. Nonempty State
contracts participate in both fragment and Plan fingerprints. Mutating any of
those fields invalidates the sealed identity; a changed capacity requires a
fresh Plan. The ordinary planner's `seal_state_plan` validates checked State
graph admission and seals these contracts with their mandatory State evidence
reserve, preserving the original Plan and checked Form identity.

The hosted implementation now lowers admitted State into numeric storage and
installs `TypedStateOperation` in the ordinary std operation registry. A
`KernelStorageProfile` must explicitly admit State instance and byte bounds;
profiles without State storage still return `UnsupportedState`, and oversized
contracts return `StateStorageExceeded`.

## Owned State continuity

The std continuation path consumes a terminal, execution-bound operation to
obtain `RetainedTypedState`. The cell is privately owned, not a cloneable
serialized checkpoint. Replacement planning seals the exact retained
provenance; replacement preparation validates source Form, State, generation,
value type, initial value, and distinct destination Play before moving the cell.
Generation and transition allowance are preserved. Refusal returns ownership
of the original cell; fresh initialization cannot silently reset a retained
State obligation.

These are concrete hosted implementation and conformance surfaces:

- [lowering admission](../../architecture/plan-lowering/src/lowering/admission.rs);
- [typed-State Host operation](../../targets/std/src/state_value.rs);
- [owned continuation](../../targets/std/src/state_value/continuity.rs);
- [installed-State conformance](../../targets/std/src/installed_std_tests/typed_state_conformance.rs);
- [ownership and forgery tests](../../targets/std/tests/owned_state_continuity.rs).

This does not claim arbitrary cross-process persistence, general schema
migration, another Host's installation, or physical continuity. Fresh Boot
resource and authority admission remains separate from transferring State.
