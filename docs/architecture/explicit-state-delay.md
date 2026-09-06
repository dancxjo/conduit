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
