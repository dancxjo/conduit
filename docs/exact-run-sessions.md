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
register one or more plan-bounded timer and host-operation interests and
become Waiting.

`time/ticker` is the reference timer source: its current contract is an
open-ended public-text stream, not a one-shot result. It reserves the first output
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

`net/http/listen` is likewise a live source, not a one-request batch helper.
Binding, accepting one connection, reading one bounded request, and writing
one bounded response are distinct nonblocking steps. An accepted request waits
for both its exact host-operation and its source-declared deadline tick; the
same live epoch resumes from either, without reading or jumping a real clock.
At every hosted pump, timer advance, and named host wake, callers provide a
fresh bounded observation set. The executor rechecks the plan-pinned grant,
resource binding, lease identity, availability, and authority horizon before
resuming the provider. That observation also states whether the selected
provider is still available. Provider loss is distinct from a revoked or stale
grant/capability/lease and fails before the provider can poll or touch the host
resource; the persistent session never retains a borrowed observation or plan
arena between wakes.
`Drain` closes admission first, then lets an already accepted request reach its
declared response and cleanup; `Abort` disposes the same bounded remainder
immediately. Neither path creates a new listener, reuses a completed run, or
turns a host readiness callback into semantic authority.

Cancellation invokes the provider's bounded stop disposition and cleanup on
the same scheduler path. Natural completion also runs cleanup before the node
is terminal. Cleanup is itself one bounded nonblocking provider step: it may
complete immediately or wait on a named timer/host operation. During Abort the
session reports `Aborting`, not terminal cancellation, until that disposition
is known. A cleanup wake resumes the same epoch; advancing past the selected
execution profile's `cancellation_ticks` fails that epoch with `CND-RUN-013`
instead of hiding a stuck task or claiming graceful cancellation.
Provider-owned callbacks, queues, timers, tasks, and buffers must be declared
in the selected execution profile and admitted by the exact plan.

## Bounds

The host admits a finite concurrent-session count and the plan's runtime
memory before Start. The session retains only its fixed host-I/O capacity and
the scheduler's plan-accounted queues, timers, operations, and evidence. A
pump quantum bounds one host turn; it is not an implicit lifetime deadline.

Scheduler observations have a monotonic run-local cursor. A recorder reads a
caller-owned bounded batch, commits it through its configured evidence
provider, and then explicitly acknowledges the batch's exclusive end cursor.
Only that acknowledged prefix is released from the fixed resident log. A
stalled or failed recorder therefore consumes its declared event capacity and
fails closed instead of silently discarding observations. A reader that asks
for an already released cursor receives the first retained cursor as an
explicit gap; a reader ahead of the run receives the current end cursor. The
cursor never reuses sequence numbers when resident slots are reclaimed.

The session's `drain_exact_evidence` operation puts the commit before the
acknowledgement: it projects one bounded batch, passes it to the external
evidence sink, and releases the resident prefix only when that sink succeeds.
Sink failure leaves the same cursor and observations available for an explicit
retry. Patchbay consumes the provider's committed projection; it is never this
authoritative sink.

### Bounded value lifetime

The hosted executor admits one fixed value arena before Start. In plain terms,
a value enters an already reserved slot, waits while a cord, node, pending
output, host operation, or external output still owns it, and returns that slot
when the last such owner is gone. Queue pressure never grows the arena. A
coalescing replacement releases the displaced value; rejection and disposable
drop retain nothing; cancellation, failure, completion, and failed startup
cleanup release their bounded remainder.

After every scheduler turn the accounting order is:

1. clear the prior live marks;
2. mark handles reachable from queues and nonterminal implementation state;
3. release every unmarked slot and byte span;
4. report current and high-water resident slots and bytes.

Two tee branches may share one generation-safe opaque handle. Marking it twice
does not copy the payload or charge two resident slots. Once released, that
handle cannot resolve a later occupant of the reused slot. If the plan-admitted
slot or byte ceiling is exhausted, publication fails with the exact
`conduit/value-store-bound-exceeded` result rather than depending on allocator
behavior.

A public Watch copies only its admitted preview bytes into separate fixed Watch
storage after a cord publication commits. It does not keep the executor value
alive. Protected material remains redacted. Detaching a Watch stops future
copies without changing queue ownership or the run.

Hosted profiles use this preallocated byte arena and slot table. Constrained
profiles may use caller-owned static pools instead, but the ownership and
disposal rule is identical: accepted values have bounded storage, every live
owner is explicit, and terminal or discarded work returns its storage exactly
once. These storage-accounting transitions are session metrics, not fabricated
domain events.

## Browser worker control

The Tour's dedicated worker is a bounded host for the same exact session. It
first verifies the pinned WASM artifact, then opens one revisioned Patchbay
workspace and accepts only explicit Start, bounded pump, timer/host wake,
candidate-transaction, snapshot, and actual Drain/Abort requests for that
session. Pump quanta and ticks cross the JavaScript/WASM boundary as exact
non-negative integers; the worker never owns a JavaScript scheduler or runs
an unbounded loop. The old terminal-only worker `run` command and its
fresh-run `cancel` command are not part of the current protocol: Stop targets
the already started session and returns its own terminal cleanup evidence.

`patchbay-read-exact-evidence` reads one caller-selected bounded delta from
that same run. Its cursor is the scheduler's monotonic cursor, and its result
names `available`, `gap`, or `future` explicitly with the cursor to use next.
It is read-only: a Patchbay or browser renderer cannot acknowledge, compact,
or otherwise release executor evidence. A worker may request a fresh
authoritative snapshot after a gap, but it must not recreate omitted evidence
or resend the complete history as a substitute for cursor progress.

An invalid candidate remains visible in its next source revision without
removing the prior active plan epoch from the worker's authoritative
projection. Terminating a worker or closing its page outside that stop path is
an abrupt placement loss, not graceful cancellation.

Long-lived value reclamation and Patchbay Watches use this same boundary and
retain their own explicit bounds and lifecycle rules.
