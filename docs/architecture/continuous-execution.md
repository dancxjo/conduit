# Continuous execution over finite Plays

Continuous execution is a semantic/lifecycle property of a Form. It means
that the Form may remain active for an externally unbounded lifetime while its
graph, retained state, instantaneous queues, host operations, Signs, and every
concrete Plan/Play remain finite and admitted before Play start. It is not a
special infinite mode, a timer-owned scheduler, or a loop that silently starts
fresh Plays.

## Vocabulary

The following dispositions are distinct machine-readable results:

| Disposition | Meaning |
| --- | --- |
| `Continued` | One finite transition was accepted and the Form remains live. |
| `Quiescent` | The Form remains live but is awaiting admitted input or work. |
| `SemanticCompletion` | The Form has completed its meaning; continuation is not expected. |
| `Lull` | The current Wake ended while the Body/Form identity was retained. |
| `Cancelled` | Explicit cancellation ended current work. |
| `ValueOverflow` | A finite typed value could not represent the next state. |
| `CapacityExhausted` | An admitted queue, operation, or resource bound was exhausted. |
| `Failed` | Current work failed for a reason other than capacity. |
| `HostBootResourceOrLineLost` | Current realization truth was lost. |
| `PlanRetired` | The immutable realization is no longer current. |
| `Replanned` | The same Form and retained state continued under a replacement Plan. |

`Quiescent`, `Lull`, and `SemanticCompletion` are not synonyms. A quiescent
Form can accept later input in the same active lifetime; Lull ends the current
Wake but retains the Body; semantic completion ends the Form's work. Likewise,
`PlanRetired` and `Replanned` describe realization lifecycle, not semantic
completion or a new source program.

## Finite admission and continuation

Each active Play admits a fixed resource envelope before it starts: retained
value bytes, instantaneous queue slots, host-operation slots, route and Line
capacity, cancellation/terminal bookkeeping, and mandatory Sign storage. A
continuous Form may perform arbitrarily many transitions over time, but each
transition uses only that admitted finite workset. No transition counter is a
semantic limit, and no restart is used to renew a resource or timer budget.

The finite-state specimen in `conduit-body` retains one bounded integer and one
fixed resource envelope. Its caller may provide any number of transitions. A
value overflow is reported as `ValueOverflow` and does not wrap or fabricate a
new state. A replacement Plan changes realization identity only; source and
checked-Form identity plus retained state remain unchanged when continuity is
admitted. The specimen is a contract proof, not a second scheduler or runtime.

## Lifecycle and replan rules

1. A Form is authored as continuing meaning; it does not enumerate its future
   interactions.
2. A Wake may hold one immutable Plan and at most one active Play at a time.
3. A realization loss or Plan retirement ends or invalidates only the affected
   realization. The Form, checked identity, and retained state survive unless
   their own semantics say otherwise.
4. Replanning creates a new immutable Plan and, when admitted, a new Play. It
   does not mutate the old Plan or masquerade as a semantic restart.
5. Lull, cancellation, failure, overflow, capacity exhaustion, and semantic
   completion remain distinct results and retain bounded evidence.

The contract therefore supports thermostats, servers, sensor pipelines,
compositors, audio graphs, robot controllers, and UIs without claiming an
infinite value domain, infinite reservation, background-realtime behavior, or
physical continuity.

## Stop line

No infinite resource reservation; no `universal` or `unsafe` escape hatch; no
semantically unbounded allocation; no hidden restart loop; no timer-owned
scheduler; and no conflation of Lull, suspend, replan, or quiescence with
HALT.
