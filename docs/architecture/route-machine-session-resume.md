# Deterministic routes and bounded session resume

Issue #501 adds runtime policy over the exact route candidates sealed by the
active Plan. `RouteMachine` copies that finite ordered candidate set once at
construction. Later `LinkObservation` values may change only the availability
of a candidate already in that set. The first Ready candidate wins. An unknown
binding ID is rejected; an exhausted set yields `RouteDisposition::Unsatisfied`
with `replan_may_be_requested`, but the machine never invokes a planner or
changes the Plan.

`RouteUpdate` is the machine-readable transition record. It retains the exact
observation evidence ID, the previous selected binding when present, and either
the newly selected exact `BoundLink` with `same_plan_continues` or the explicit
unsatisfied result. Thus observation change, selection change, and no-route
state are distinguishable without carrier-owned policy.

## Finite reconciliation

`SessionCheckpoint` contains only:

- the next sequence number;
- no transfer, one Offered sequence, or one Accepted sequence;
- whether input has closed.

No message history or replay log is retained. The operation driving the
one-transfer-at-a-time session must continue retaining its one admitted payload
until delivery, as it already must for pressure and acceptance handling.

`SessionMachine::checkpoint_offer` binds a checkpoint to the logical session
identity. `resume_with_attachment` accepts an already-admitted exact
`SessionBinding` for the replacement route and one peer offer. It first
requires both the replacement binding and checkpoint offer to carry the
unchanged logical identity. It then produces a machine-readable
`SessionCheckpointAcceptance`, including both finite checkpoints, an explicit
same-Plan fact, and one `SessionResumeAction`:

- `Continue` for matching clean or matching in-flight state;
- `ReplayOffered(n)` when the source offered `n` but the sink did not observe it;
- `AwaitReplay(n)` for the corresponding sink state;
- `AdvanceDelivered(n)` when the sink advanced after delivery but the source
  retained Accepted `n`.

Contradictory, stale, closed-state, failed-state, terminal-state, or different
logical-session combinations fail closed. A successful reconciliation clears
Hello/Ready admission for both directions, so the peers must establish the same
logical identity and exact new attachment before application traffic resumes.
This proves only the enumerated finite transitions; it does not claim general
exactly-once delivery across physical failure.
