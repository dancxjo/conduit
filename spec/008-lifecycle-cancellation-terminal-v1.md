# Lifecycle, cancellation, and terminal semantics version 1

Status: stable

Lifecycle algebra version: 1

## Purpose and boundaries

Lifecycle is a semantic contract, not task-library behavior. A runtime may use
threads, futures, interrupts, or polling, but it MUST expose the same legal
transitions, hierarchical cancellation deliveries, terminal classification,
queue disposition, and ordered evidence.

“Stopping” is an operation, not a state. A subject is either draining accepted
work or has reached an exact terminal state. Absence of a value right now does
not imply completion. The `conduit-core` types are borrowed or caller-backed;
they do not allocate, read a wall clock, spawn tasks, or choose implementations.

## Node, composite, and run state machine

Primitive nodes, exported composites, and runs use the same states so a
compatible composite remains substitutable for a primitive:

| From | Operation | To |
|---|---|---|
| `created` | prepare | `preparing` |
| `preparing` | prepared | `ready` |
| `ready` | start | `running` |
| `running` | stop with drain | `draining` |
| `draining` | drained | `succeeded` |
| any nonterminal state | cancel | `cancelled` |
| any nonterminal state | fail | `failed` |

`succeeded`, `cancelled`, and `failed` are terminal. Every other edge is
illegal. A transition to `cancelled` or `failed` requires a structured cause;
ordinary progress and successful completion forbid one.

Restart never mutates a terminal node instance back to `running`. A supervisor
creates a new attempt identity and begins that attempt at `created`.

Authored `.panel` source has no runtime lifecycle. Its resolved top-level node
uses the composite state machine, while each invocation uses the run state
machine. This preserves source, plan, and run as distinct identities without
inventing a second runtime Panel species.

## Cord state machine and queued values

| From | Operation | To |
|---|---|---|
| `created` | prepare | `prepared` |
| `prepared` | open | `open` |
| `open` | source completion or draining cancel | `draining` |
| `draining` | queue becomes empty | `completed` |
| any nonterminal state | abort cancellation | `cancelled` |
| any nonterminal state | fail | `failed` |
| any nonterminal state | transport disconnect | `disconnected` |

`completed`, `cancelled`, `failed`, and `disconnected` are terminal. An empty
open cord remains open; only explicit source completion begins draining.

The allocator-free reference queue implements the value boundary:

- natural completion stops admission, retains accepted values, enters
  `draining`, and becomes `completed` after the last FIFO pop;
- draining cancellation does the same but becomes `cancelled`;
- aborting cancellation atomically transfers every queued value into
  caller-provided empty storage, emits aggregate loss evidence before its
  terminal cancellation event, and becomes `cancelled`; and
- insufficient or non-empty discard storage rejects abort before changing the
  queue.

A higher-precedence cause may upgrade a queue already draining toward a weaker
terminal result. Drain preserves the remaining FIFO values under the new
result; abort transfers all of them and emits loss before the upgraded
terminal event. A later weaker cause cannot downgrade the target.

A blocked value remains owned by its producer outside the queue. Cancellation
explicitly wakes blocked producers and empty-queue consumers.

## Composite derivation

A composite derives its visible state from children and boundary cords:

1. any child or boundary failure yields `failed`;
2. any child cancellation or boundary cancellation/disconnect yields
   `cancelled`;
3. all children succeeded and all boundaries completed yields `succeeded`;
4. any draining subject yields `draining`;
5. any running child or open boundary yields `running`;
6. all children ready and all boundaries prepared yields `ready`;
7. otherwise any preparing child yields `preparing`; and
8. otherwise it is `created`.

The function applies recursively. Scheduler tasks and presentation expansion
cannot affect the result.

## Cancellation scopes

Each registered owned resource has a stable resource identity, a stable scope
identity and optional parent, a positive finite relative deadline in
deterministic clock ticks, and exact `drain` or `abort` policy. A request adds a
structured reason and caller-supplied current tick; its delivery receives a
checked absolute deadline. The kernel never consults wall-clock time.

Cancellation reaches every not-yet-cancelled registration in the selected
scope and every descendant, in stable registration order. Child cancellation
is isolated from ancestors and siblings. Descendant deliveries retain
`parent-cancelled` and name the initiating scope. Repeated cancellation is
idempotent and emits no duplicate deliveries.

Unknown scopes or parents, cycles, conflicting definitions for one scope,
zero deadlines, deadline overflow, and insufficient caller evidence storage
fail before registrations change.

## Terminal causes and races

Each cause contains a stable code, semantic subject, optional structured
`caused_by` reference, and requested queue policy. A string may decorate but
never replace those fields.

Simultaneous causes sort independently of arrival order by descending
precedence, then stable code and subject. Every cause is copied to caller-owned
retained-cause storage; the winner does not erase the causal set.

| Precedence | Cause | Terminal class | Queue rule |
|---:|---|---|---|
| 5 | `node-failed` | `failed` | explicit cause policy |
| 4 | `authority-revoked` | `cancelled` | explicit cause policy |
| 3 | `deadline-expired` | `cancelled` | explicit cause policy |
| 2 | `cancellation-requested`, `parent-cancelled` | `cancelled` | explicit cause policy |
| 1 | `transport-disconnected` | `disconnected` | explicit cause policy |
| 0 | `natural-completion` | `succeeded` | always drain |

Thus natural completion with buffered values drains; downstream abort wakes a
blocked producer and evidences queued-value disposal; node failure during drain
wins while retaining completion; authority revocation wins a deadline;
deadline wins disconnect; and disconnect wins completion.

## Evidence

Every accepted state transition emits immutable evidence containing subject,
exact before/after state, local monotonic sequence, and structured terminal
cause when required. Rejected transitions do not advance sequence or state.
Specification 012 defines the common exact-run envelope that wraps this local
semantic observation without replacing its sequence or lifecycle facts.

Cancellation evidence contains its registration-order sequence, resource,
receiving and initiating scopes, local reason, initiating reason, absolute
deadline tick, and stop policy. Flow evidence records `drain-started`, `completed`,
`values-discarded-on-abort(items,bytes)`, `cancelled`, and required wakes.
Issue #12 owns the common execution-event envelope and provenance; it embeds
these payloads without reinterpretation.

## Replicated composite lifecycle and supervision

Issue #44 pools use:

```text
template -> queued-admission -> admitted-instance -> attempt
        -> draining -> cleanup -> succeeded
```

Cancellation and failure are legal from every nonterminal state. Restart is
only `cleanup -> attempt` and increments the attempt identity. A stable child
identity is `(template, instance, attempt)` with one-based attempts; attempts
are never reused and restart past `max_attempts` is rejected.

Pools have positive finite `max_queued` and `max_active`. Supervision is exactly
one of `fail-together`, `isolate`, bounded
`restart(max_attempts,backoff_ticks)`, `fallback(node)`, `drain`, `abort`, or
`escalate`. Each admitted child receives its instance identity, cancellation
scope, deterministic deadline/clock context, separately resolved resource and
authority slices, and immutable parent evidence lineage. Pool cancellation,
parent completion, cleanup, and escalation use the same rules above.

## Diagnostics and fixtures

| Code | Meaning |
|---|---|
| `CND-LIF-001` | illegal lifecycle transition |
| `CND-LIF-002` | missing or invalid structured terminal cause |
| `CND-LIF-003` | caller evidence/cause storage is too small |
| `CND-LIF-004` | replicated pool or restart policy is not finite |
| `CND-CAN-001` | cancellation scope, hierarchy, deadline, or reason is invalid |

`conformance/c2/lifecycle-v1.tsv` freezes positive and negative node,
composite, run, and cord transitions.
`conformance/c2/terminal-races-v1.tsv` freezes required race outcomes, queue
dispositions, and primary causes.

Reference tests exhaust every state pair, repeated and hierarchical
cancellation, isolated child cancellation, natural drain, abort with queued
values, permutation-independent cause resolution, nested-composite derivation,
and bounded replicated restart.

## Normative requirements

| ID | Obligation |
|---|---|
| LIF-001 | Reject every transition absent from the exact tables |
| LIF-002 | Treat stopping as an operation; draining is the only stop-progress state |
| LIF-003 | Require structured causes for cancellation and failure |
| LIF-004 | Derive composites solely from children and boundary cords |
| LIF-005 | Never infer completion from temporary queue emptiness |
| CAN-001 | Give every owned resource a hierarchical scope and finite deadline |
| CAN-002 | Propagate parent cancellation deterministically to every descendant |
| CAN-003 | Keep repeated cancellation idempotent |
| CAN-004 | Preserve explicit drain-versus-abort queue disposition |
| TRM-001 | Resolve races by the frozen precedence table |
| TRM-002 | Retain the complete structured cause set |
| TRM-003 | Emit loss evidence before abort becomes terminal |
| REP-001 | Bound queued and active replicated children |
| REP-002 | Give restarts new attempt identities and finite backoff/attempt limits |
