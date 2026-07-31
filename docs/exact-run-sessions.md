# Hosted exact-run sessions

An exact plan is a checked blueprint. It does not start merely because someone
opens source, connects a cord, checks a panel, or opens Patchbay. One explicit,
authorized Start creates one hosted exact-run session.

That session owns the scheduler, implementation state, admitted resource
reservation, fixed host-I/O storage, and the identities selected at Start. Its
plan identity, source semantic hash, epoch, and run ID remain fixed until the
session becomes terminal and is finalized.

## Start, pump, and wait

A host validates the exact plan and bindings, admits the plan-declared session
memory, and starts every implementation atomically. Start does not execute a
node step. The host then calls `pump` with a finite scheduler quantum. A pump
returns control after that quantum, after a wait, or after terminal state; it
never restarts the run or resets its counters.

`Active` means ready work exists. `Waiting` means the run is still alive but
needs an admitted input, timer, output, host operation, or cancellation wake.
Waiting is not failure and is not completion. A finite graph can reach natural
success, while a service may remain Waiting between requests for its whole
authorized lifetime.

The host resumes the same epoch by advancing its admitted clock or notifying
the exact named host operation. A wrong wake does nothing to another subject.
Hosts must not use an unbounded loop or silently jump a real clock forward.

## Stop, finalization, and abrupt loss

Normal Stop acts on the active session. `Drain` first prevents further normal
work and settles admitted work; the session reports `Quiescing` while that is
in progress. `Abort` takes the same session through its abort policy. Both
produce exact cancellation and cleanup evidence. A terminal session may then
be finalized, which releases its scheduler and admission reservation.

Dropping or losing a nonterminal session is not a graceful Stop. It is an
abrupt placement failure: the host records that the live session was abandoned
and does not admit a replacement until deliberate host recovery. This keeps a
lost worker or process from being presented as orderly cancellation.

## Source and presentation stay separate

Editing source creates a candidate revision. It cannot mutate or reinterpret
an already started run, even if the candidate is invalid. Patchbay layout,
selection, and display state are presentation facts; they do not create a
session or change the active epoch. Later observation work may inspect an
admitted live run, but an observation never becomes the graph's executor or
data-plane authority.

## Finite convenience runs

The terminal `run_exact_report` helper is only a convenience for finite work.
It starts this same hosted session, pumps it cooperatively to terminal, then
returns its normalized report. It is not a second runtime and does not change
the meaning of a persistent run.

## Hosted provider adapter

Every linked hosted provider uses the same prepare, start, bounded-step,
interest, cancel, and cleanup adapter. A finite request/response provider is
the simple case: its first bounded step returns its exact outputs and declares
completion. A live provider may instead return outputs and remain active, or
register one named timer or host-operation interest and become Waiting.

`time/ticker` is the reference timer source: its current contract is an
open-ended `u64` stream, not a one-shot result. It reserves the first output
in the producing step, then waits for its one admitted timer. Advancing that
timer resumes the same plan, epoch, provider binding, and retained state; it
does not synthesize a new run or bypass the output transaction.

The scheduler, not a provider callback, owns the clock and wake registry. A
deterministic host can advance an admitted test clock. A real host registers
the provider's finite timer and wakes the same run later; it must not jump time
to make a callback look ready. Wrong or late named wakes do not resume another
provider or another epoch.

`fs/watch` follows the same rule for host I/O: it emits its required initial
event, then waits for the exact filesystem host-operation notification before
polling for one more bounded event. A changed file is not a new run and an
unrelated host wake does nothing. The ordinary finite `conduct run` helper
cannot own that lifetime, so a persistent watch is resolved and checked by its
inventory entry while its session, notification, and Abort disposition are
proved by the hosted lifecycle tests.

Cancellation invokes the provider's bounded stop disposition and cleanup on
the same scheduler path. Natural completion also runs cleanup before the node
is terminal. Provider-owned callbacks, queues, timers, tasks, and buffers must
be declared in the selected execution profile and admitted by the exact plan.

## Bounds

The host admits a finite concurrent-session count and the plan's runtime
memory before Start. The session retains only its fixed host-I/O capacity and
the scheduler's plan-accounted queues, timers, operations, and evidence. A
pump quantum bounds one host turn; it is not an implicit lifetime deadline.
Long-lived value reclamation, incremental evidence retention, and Patchbay
Watches build on this boundary and each have their own explicit bounds and
lifecycle rules.
