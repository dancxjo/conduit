# Continuous execution over finite plays

Continuous execution is a semantic/lifecycle property of a form. It means
that the form may remain active for an externally unbounded lifetime while its
graph, retained state, instantaneous queues, host operations, signs, and every
concrete plan/play remain finite and admitted before play start. It is not a
special infinite mode, a timer-owned scheduler, or a loop that silently starts
fresh plays.

## Vocabulary

The following dispositions are distinct machine-readable results:

| Disposition | Meaning |
| --- | --- |
| `Continued` | One finite transition was accepted and the form remains live. |
| `Quiescent` | The form remains live but is awaiting admitted input or work. |
| `SemanticCompletion` | The form has completed its meaning; continuation is not expected. |
| `lull` | The current wake ended while the body/form identity was retained. |
| `Cancelled` | Explicit cancellation ended current work. |
| `ValueOverflow` | A finite typed value could not represent the next state. |
| `CapacityExhausted` | An admitted queue, operation, or resource bound was exhausted. |
| `Failed` | Current work failed for a reason other than capacity. |
| `HostBootResourceOrLineLost` | Current realization truth was lost. |
| `PlanRetired` | The immutable realization is no longer current. |
| `Replanned` | The same form and retained state continued under a replacement plan. |

`Quiescent`, `lull`, and `SemanticCompletion` are not synonyms. A quiescent
form can accept later input in the same active lifetime; lull ends the current
wake but retains the body; semantic completion ends the form's work. Likewise,
`PlanRetired` and `Replanned` describe realization lifecycle, not semantic
completion or a new source program.

## Finite admission and continuation

Each active play admits a fixed resource envelope before it starts: retained
value bytes, instantaneous queue slots, host-operation slots, route and line
capacity, cancellation/terminal bookkeeping, and mandatory sign storage. A
continuous form may perform arbitrarily many transitions over time, but each
transition uses only that admitted finite workset. No transition counter is a
semantic limit, and no restart is used to renew a resource or timer budget.
An admitted queue, slot pool, or hardware ring is ordinarily a simultaneous
occupancy bound: after an occupant is consumed or retired, that storage is
reusable in the same Play. It becomes a lifetime-transition bound only when
that cardinality is explicit authored meaning rather than an implementation
counter that happens to reach the storage capacity.

The finite-state specimen in `conduit-body` retains one bounded integer and one
fixed resource envelope. Its caller may provide any number of transitions. A
value overflow is reported as `ValueOverflow` and does not wrap or fabricate a
new state. A replacement plan changes realization identity only; source and
checked-form identity plus retained state remain unchanged when continuity is
admitted. The specimen is a contract proof, not a second scheduler or runtime.

## Lifecycle and replan rules

1. A form is authored as continuing meaning; it does not enumerate its future
   interactions.
2. A wake may hold one immutable plan and at most one active play at a time.
3. A realization loss or plan retirement ends or invalidates only the affected
   realization. The form, checked identity, and retained state survive unless
   their own semantics say otherwise.
4. Replanning creates a new immutable plan and, when admitted, a new play. It
   does not mutate the old plan or masquerade as a semantic restart.
5. lull, cancellation, failure, overflow, capacity exhaustion, and semantic
   completion remain distinct results and retain bounded evidence.

The contract therefore supports thermostats, servers, sensor pipelines,
compositors, audio graphs, robot controllers, and UIs without claiming an
infinite value domain, infinite reservation, background-realtime behavior, or
physical continuity.

## Stop line

No infinite resource reservation; no `universal` or `unsafe` escape hatch; no
semantically unbounded allocation; no hidden restart loop; no timer-owned
scheduler; and no conflation of lull, suspend, replan, or quiescence with
HALT.
