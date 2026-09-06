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
input to commitment or scheduling. Snapshot/checkpoint behavior is not part of
this first contract and must be refused until separately admitted.

Externally continued State uses the same fixed current/candidate storage without
requiring a predetermined semantic transition count. Each admitted input/step
still crosses the explicit commitment boundary; awaiting another input neither
commits State nor renews a budget. The finite generation-identity representation
is a separate realization limit: exhaustion refuses before rollover, leaving
current State unchanged. It is not semantic completion or permission to reset.
Candidate evidence retains the identity assigned at offer through commit or
abort. A finite transition-budget realization continues to report its distinct
budget refusal. General checked-Form execution and cross-Play continuity remain
separately owned by #2688 and #2691.

The kernel `StateOperation` adapter exposes this cell through exact next/current
ports in the existing `OperationDriver` and fixed scheduler. Its profile admits
at most the existing canonical-emission byte envelope; larger cells refuse
construction. Input closure completes this adapter, while awaiting input remains
nonterminal. Output pressure retains the pending emission. The scheduler tests
use finite queue, value and Sign capacities, and distinguish an exhausted
transition allowance from successful processing under a larger allowance.
This adapter does not yet establish checked-Form Host installation or cross-Play
State continuity. Finite generation and Sign capacity are not claims of infinite
physical execution.

## Immutable State admission

`PlanFragment.states` carries the exact State identity, owning Gear, value Kind,
initial bytes, retained-byte capacity and continuation demand. Nonempty State
contracts participate in both fragment and Plan fingerprints. Mutating any of
those fields invalidates the sealed identity; a changed capacity requires a
fresh Plan. The ordinary planner's `seal_state_plan` validates checked State
graph admission and seals these contracts with their mandatory State evidence
reserve, preserving the original Plan and checked Form identity.

This is structural admission truth, not a claim that the authored Kind permits
this initialization or migration, or that execution is installed. Current lowering
profiles refuse `UnsupportedState` for a valid fragment carrying these contracts
until numeric State storage and Host installation implement them. A host must
not ignore a sealed State contract. Cross-Play typed migration and Boot authority
admission remain separate obligations under #2691.
