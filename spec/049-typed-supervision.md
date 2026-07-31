# Typed supervision and terminal handling current form

Status: proposed portable control and evidence contract

Depends on: specifications 008, 010 through 012, 019, 022 through 024, 027,
029, 032, 034, 036, 041 through 047

## Boundary and classification

Supervision applies only after an admitted semantic subject reaches terminal
runtime state. Three mechanisms remain distinct:

1. An expected negative domain outcome is an ordinary domain-owned typed
   value. HTTP status, validation rejection, empty recognition, and similar
   values do not invoke supervision unless the owning node independently
   becomes terminal.
2. A runtime failure of an admitted node, composite, or run is a structured
   terminal observation on the control and evidence plane.
3. Parse, lower, resolve, authorize, reserve, and admission failures are
   diagnostics before the affected run starts. A handler inside an unadmitted
   plan cannot catch them.

There is no universal `error` or `stderr` port. A supervision relationship
does not create a data cord, hidden callback, exception stack, executor task,
or handler registry. Its handler is an ordinary planned node or composite
whose typed terminal-observation input and supervision-decision output are
delivered and consumed by the exact control-plane binding.

## Portable representation

`TerminalObservation` is allocator-free and contains only finite borrowed
fields:

- semantic and expanded subject paths;
- run, plan identity, plan epoch, generation, and attempt;
- terminal class, stable cause code, phase, and a bounded causal slice;
- an explicit retry declaration;
- optional resource, authority, host, implementation, artifact, and
  transition identities;
- a redacted evidence cursor; and
- remaining observation, decision, attempt, evidence, and deadline budgets.

It does not contain arbitrary error strings, domain values, provider
exceptions, stack traces, secrets, or an expandable property map.

`SupervisionDecision` selects one exact pair of action kind and optional
choice identity. current form has seven actions:

| Action | Exact consequence |
| --- | --- |
| `propagate` | retain the cause and terminate this boundary outward |
| `stop-scope` | stop the admitted scope under its exact cleanup policy |
| `restart-same` | create the next attempt of the same admitted subject |
| `retry-same` | replay only a declared idempotent effect when the admitted action permits it |
| `activate-declared-fallback` | select an exact compatible alternative already in the plan |
| `continue-declared-degraded-mode` | select an exact admitted mode that preserves every required guarantee |
| `request-operator-action` | emit the exact bounded request already admitted in the plan |

Every admitted action has a positive maximum use count. Retry additionally
requires both the observation's idempotency declaration and the action's
effect-replay permission. Restart and retry produce a new attempt identity.
Fallback and degraded targets must be exact semantic matches in plan version
15; future directional compatibility may be accepted only with the existing
complete satisfaction proof, never by coercion.

An action marked `requires_new_epoch` cannot run in place and returns
`CND-SUP-013`. A new implementation, binding, topology, artifact, host,
authority set, population, budget, or plan epoch is a candidate transition
owned by specification 057. Supervision cannot grant authority, install or
fetch artifacts, discover a host, enroll a member, reset a persistent budget,
or approve its own expansion.

## Contract, limits, and profiles

`SupervisionContract` binds one subject and one distinct handler at child,
named-group, composite-boundary, or replicated-child scope. It carries:

- an exact contract identity and optional explicit outer boundary;
- the finite admitted action set;
- positive maxima for observations, decisions, in-flight observations, cause
  depth, nesting depth, handler ticks, recovery ticks, evidence events,
  observation bytes, decision bytes, and scratch bytes;
- an exact cleanup policy; and
- whether behavior is required rather than optional.

Named-group bindings carry an exact, duplicate-free member list containing
the semantic subject but not the handler, plus an exact `fail-together` or
`isolated-optional` failure mode. Other scopes carry no group members and use
`fail-together`. A stop-scope decision terminates the bound group in
fail-together mode and only the observed member in isolated-optional mode.
Bindings are unique by subject within a boundary. A handler cannot supervise
itself at the same boundary. Outer references must exist and form an acyclic
chain within the declared maximum nesting depth.

Hosted, browser, and deterministic profiles implement the complete action
vocabulary subject to exact plan admission. The constrained profile supports
only propagate, stop-scope, and bounded restart-same. Selecting any richer
action returns `CND-SUP-015`; firmware must not approximate it with local
behavior.

The portable state machine uses caller-owned action counters and borrowed
slices. It performs all budget, profile, idempotency, guarantee, epoch, and
evidence-capacity checks before changing state. Hosted `Vec` and `VecDeque`
storage is an exact-capacity convenience above this contract, not normative
hidden capacity.

## Deterministic handling

Terminal races choose the highest stable cause precedence. Ties use lexical
semantic subject, newest generation, newest attempt, and latest phase in that
order. Input or registry iteration order is not semantic.

An explicit inner-to-outer boundary list chooses the nearest valid boundary.
If no boundary exists, the original terminal cause propagates. A handler
failure, timeout, cancellation, cleanup failure, or exhausted finite budget
makes the supervisor terminal, clears its in-flight observation, emits the
required evidence, and propagates outward. It is never recursively handled by
the same binding. A timeout cannot be reported before the lesser of the
observation recovery deadline and `observation.now_tick +
maximum_handler_ticks`.

Cancellation wins over a still-pending decision at that boundary: the
pending observation is removed, cancellation evidence is retained, and a
late decision is rejected by correlation or terminal state. Decisions
correlate the exact run, plan identity and epoch, generation, attempt,
semantic subject, and expanded subject. Old and new generation observations
therefore cannot consume each other's decisions.

Replicated-child bindings remain subject to the population, correlation, and
attempt limits of specification 060. This contract defines the per-child
decision and identity rules but does not create or enlarge a pool.

## Source and lowering

current grammar adds:

```text
supervise request with request_policy
```

Both names must identify distinct nodes at the same authored boundary. The
form is valid at top level and inside a composite definition. Grammar version
1 remains current and rejects `supervise` with `CND-SRC-007`; self, missing,
duplicate-subject, or cross-boundary bindings return structured source
diagnostics.

current source-AST schema and current lowered-source schema retain each relationship as:

- an exact semantic path;
- the semantic and expanded subject path;
- the handler path;
- a semantic binding hash; and
- exact source provenance.

Lowering does not synthesize nodes, cords, actions, queues, timers, authority,
or fallbacks. The lowered source topology remains nested within the current
document.

## Exact plan current form

Every source binding has exactly one `PlanSupervision` entry. It pins:

- source-binding identity, subject, handler, scope, exact group members,
  failure mode, and outer boundary;
- policy, observation contract, and decision contract;
- every admitted action and compatible exact target;
- maximum observations, decisions, attempts, causal depth, nesting, handler
  and recovery time, and evidence;
- observation, decision, scratch, and in-flight storage;
- a positive CPU allocation;
- distinct deadline, backoff, and cooldown timers;
- cleanup policy and required-behavior flag; and
- plan identity participation for all of the above.

The minimum memory reservation is:

```text
maximum_in_flight * (observation_bytes + decision_bytes) + scratch_bytes
```

At least one CPU unit, three timers, and one evidence byte per maximum evidence
event are also reserved. The complete allocation is charged to the plan's
worst-case budget. Under-allocation, a missing subject or handler, an unknown
or incompatible target, a duplicate subject, a malformed outer chain, or an
implementation/host whose supported plan range excludes current form fails
before activation.

Changing any action, limit, target, timer, policy, or allocation changes exact
plan identity. Persisted exact-plan document `conduit.execution-plan`
round-trips all current-form supervision fields.

## Evidence and bounded reads

The required evidence vocabulary includes terminal observation, handler
admission, accepted and rejected decisions, attempt start, fallback or
degraded selection, operator request, exhaustion, propagation, cleanup,
cancellation, handler failure, and final outcome. The state machine reserves
evidence before consequential mutation.

Accepted decision evidence carries the canonical exact-plan action index;
rejected decision evidence carries that index when applicable and the stable
`CND-SUP-*` reason. Evidence is immutable and sequence-addressed. A bounded reader classifies an
older cursor as a gap and returns only the first retained sequence. It never
reconstructs evicted actions, causes, or outcomes. Patchbay may project the
original subject, owning boundary, allowed/rejected decisions, remaining
budgets, active epoch, and redacted cause identities from retained facts, but
that projection is rebuildable presentation and not normative evidence. The
Patchbay protocol's `project_supervision` consumes the core observation,
contract, evidence, and cursor status directly and retains source, plan, run,
binding, and evidence origins.

## Standard library, browser, and constrained witnesses

The standard `supervisor`, retry, fallback, terminal-projection,
operator-action, and deterministic fault-source families consume this
contract. They do not define parallel retry or error semantics. The
`supervision/supervisor` reference node exposes nominal
`std/terminal` and `supervision/decision` type
contracts; their source-visible ports are optional because the exact
supervision relationship, not a data cord, supplies and consumes them.
`validate_standard_supervisor` proves that the ordinary node's retained
values, bytes, pending operations, timers, and evidence reservation cover the
portable contract.

The hosted Rust implementation uses bounded exact-capacity queues. The browser
reference module independently executes the same finite admission,
correlation, restart, profile, and evidence rules and is exercised in
Chromium, Firefox, and WebKit CI. The constrained witness runs the
allocator-free core state machine with fixed arrays and reports richer actions
as unsupported. These witnesses prove normalized decisions where supported;
they do not prove arbitrary browser providers, physical firmware, or issue
060/057 integration.

## Stable requirements

- SUP-001: domain values, runtime terminal causes, and pre-run diagnostics are
  distinct mechanisms.
- SUP-002: supervision is an explicit typed relationship to an ordinary
  planned handler, never a universal error port or callback.
- SUP-003: observations and decisions have finite allocator-free forms.
- SUP-004: every action is exact-plan admitted and use-bounded.
- SUP-005: retry requires an explicitly idempotent replay contract.
- SUP-006: fallback and degraded targets are explicitly compatible and cannot
  weaken required guarantees.
- SUP-007: replacement and expansion route through a separate candidate epoch.
- SUP-008: observation, decision, attempt, time, storage, timer, CPU, and
  evidence bounds are reserved before activation.
- SUP-009: nesting, races, correlation, cancellation, exhaustion, handler
  failure, cleanup, and outward propagation are deterministic.
- SUP-010: supervision never broadens authority or resets containment policy.
- SUP-011: evidence is immutable, bounded, redacted, and gap-aware.
- SUP-012: current source AST, lowering, and plan preserve exact identity
  boundaries.
- SUP-013: browser, hosted, deterministic, and constrained profiles report the
  same normalized decisions where supported.
- SUP-014: unsupported constrained behavior fails explicitly before an
  implementation invents local semantics.
- SUP-015: standard nodes, Patchbay projections, and cookbook examples consume
  this contract rather than duplicating it.

The normative fixture is `conformance/c4/supervision.json`. Its 49 named
cases are independently dispatched by the hosted reference test. current-schema
compilation, actual browser execution, and the allocator-free constrained
profile have separate executable witnesses.
