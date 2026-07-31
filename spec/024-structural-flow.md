# Structural flow contract current form

Status: C4 normative portable contract

ExecutionPlan schema marker: `0`

Depends on: specifications 007, 009, 011, 017, 022, and 023

## Boundary

This specification makes branching, convergence, and structural transforms
ordinary, bounded, typed execution facts. It does not create a second graph
model. Splitters, duplicators, mergers, buffers, throttles, adapters, zip,
combine-latest, mux/demux, keyed dispatch, select, gate, switch, fallback, and
finite feedback delay are node contracts implemented through specification
022.

The compiler MUST NOT infer duplication, merge order, conversion, cloning, or
queueing from syntax, cord iteration order, executor wake order, or host
behavior. It MAY diagnose and recommend an explicit node or policy.

## Plan-visible fan-out

ExecutionPlan current records each multi-edge output as one exact `PlanFanOut`:

- a stable fan-out ID and exact producer output endpoint;
- coupled or isolated mode;
- the complete, unique set of branch cord IDs;
- an explicit shared-handle representation or pinned domain/provider copy
  descriptor; and
- for isolated mode, the explicit duplicator node and its input cord.

The branch set MUST exactly equal the cords leaving that endpoint. A branch
MUST NOT belong to two fan-outs. Missing, partial, overlapping, dangling, or
implicit multi-edge topology fails with `CND-STR-003`. A missing or unsafe
copy/share rule fails with `CND-STR-004`.

### Coupled mode

One logical production commits only if every required branch admits its
corresponding representation in the same bounded step transaction. Any block
rolls all staged branches back and the producer waits on the affected exact
outputs. Reject, loss, disconnect, and failure remain the respective branch
cord's declared `FlowPolicy`; they are never rewritten as success.

With `SharedHandle`, every branch reservation MUST name the identical
representation handle and byte charge. A pinned copy rule authorizes distinct
representations but does not allocate an unplanned cloning queue.

### Isolated mode

Isolation is an explicit ordinary duplicator node. Its single input cord and
separate bounded output cords are visible in the plan. Each branch has its own
capacity and pressure/loss/terminal policy. A slow or cancelled branch
therefore affects another branch only through facts explicitly shared by that
duplicator's bounded node contract.

The duplicator profile declares maximum retained values/bytes, per-step input
leases and output reservations, copy/share behavior, cancellation scope, and
evidence requests. There is no executor-owned hidden per-branch queue.

## Deterministic merge

ExecutionPlan current pins each merge ID, node, ordered input cord list, ordering
policy, and terminal policy. Input ordinals are dense from zero and are the
portable final tie-breaker.

Named orderings are:

- `arrival`: minimum executor-assigned cord arrival sequence, then ordinal;
- `round-robin`: first available ordinal at or after the retained cursor,
  wrapping once;
- `priority`: lowest numeric priority, then ordinal, except an input reaching
  the finite starvation-turn bound is selected by longest wait then ordinal;
- `event-time`: minimum event timestamp, then ordinal, only at or behind the
  explicit watermark.

Event-time ordering pins its timestamp `TypeContractRef`, finite maximum
lateness, and late-value policy: reject, drop-disposable, or fail. Absence of a
watermark or a minimum timestamp ahead of it is a bounded wait, not permission
to choose another future. Values older than `watermark - maximum_lateness`
take the exact late policy.

Terminal policies are:

- `drain-all`: drain all accepted values and complete after every input;
- `complete-any`: complete on the first terminal and apply exact cancellation
  policy to remaining inputs;
- `fail-fast-drain-success`: fail on an input failure, otherwise drain all
  successful inputs.

Merge cancellation uses the normal bounded lifecycle scope. Retained cursor,
wait counters, input terminals, watermark, and at most one head candidate per
input are charged to the merge node profile. “Whichever future wakes first”
is never a valid merge policy.

## Other structural nodes

Every structural family has named typed input/output port contracts, explicit
ordering and terminal behavior, finite `StructuralNodeLimits`, and a pinned
implementation profile:

- identity retains no semantic state;
- zip/synchronized join commits one value from every required input
  atomically;
- combine-latest declares initialization and terminal behavior and retains at
  most one value per input;
- mux/demux and keyed dispatch declare key handling and a finite route set;
- select, gate, and switch declare control/value ordering and cancellation;
- buffer and throttle expose their finite storage/timer allocations;
- feedback crosses an explicit finite state or delay boundary; and
- adapter pins exact input/output types and a domain/provider-owned
  implementation contract.

An adapter is never an unnamed compatibility coercion. Transcript-to-text,
unit conversion, and other domain transforms remain domain-owned ordinary
nodes.

Structural fallback routes already compatible values inside one immutable
plan. It does not select artifacts, replace implementations, acquire host
resources, weaken guarantees, or create a plan epoch. Those operations belong
to the later plan-transition contract. Evidence MUST distinguish a fallback
branch from a plan epoch transition.

## Identity

Current semantic identity includes fan-out mode, exact branches, duplicator facts,
copy/share rule, merge inputs/ordinals/priorities, ordering parameters, and
terminal policy. Collection order remains canonicalized; declared input
ordinal does not.

## Stable requirements

- STR-001: all structural behavior is ordinary, typed, and plan-visible.
- STR-002: structural storage, work, waits, routes, and feedback are finite.
- STR-003: multi-edge outputs have one exact coupled or isolated fan-out.
- STR-004: duplication has an explicit safe share/copy rule.
- STR-005: coupled publication is atomic across every required branch.
- STR-006: isolated branches use explicit duplicator-owned bounded flows.
- STR-007: merge selection is portable and deterministically tie-broken.
- STR-008: priority starvation and event-time lateness are finitely handled.
- STR-009: terminal and cancellation behavior is explicit.
- STR-010: adapters and conversions are never inserted implicitly.
- STR-011: logical composite paths survive structural lowering and evidence.
- STR-012: in-plan fallback is distinct from a plan transition.
- STR-013: current plan identity covers every structural execution fact.
- STR-014: the current plan identity covers all structural facts.

The normative fixture is
`conformance/c4/structural-flow.json`.
