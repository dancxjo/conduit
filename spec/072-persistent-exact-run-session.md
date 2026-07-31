# Persistent exact-run session current form

Status: hosted runtime contract

Depends on: specifications 011, 012, 022, 023, 029, and 051

## Boundary

One authorized Start admits one immutable exact plan epoch and creates one
owned hosted session. Checking source, resolving a candidate, connecting a
cord, inspecting a plan, or opening Patchbay never creates that session or
performs a node step.

The session retains only copied runtime facts required by the deterministic
scheduler: exact node/contract/implementation/artifact/host identities,
indexed topology, finite cord policies, value-envelope limits, feedback
reservations, plan budget, scheduler state, implementation state, and bounded
event storage. It does not retain a source parser arena or borrow an
`ExecutionPlan` after admission. Source, candidate revisions, presentation,
and the active plan epoch remain distinct identities.

## Lifecycle

`start_exact_session` receives a finite `ExactRunSessionRegistry`. Before it
prepares or starts any implementation, it reserves one session slot and the
caller's declared runtime-memory maximum in that registry. It then validates
the current exact plan, use-time grants and leases, source topology, installed
bindings, and all fixed allocations before returning a session. A full slot or
reserved-memory total fails Start with `CND-SCH-005` and starts nothing. It
prepares and starts every implementation atomically, but does not schedule a
node step itself.

The session pins plan identity, source semantic hash, plan epoch, and run ID
for its whole life. A source edit creates a candidate revision; it cannot
mutate, reinterpret, or replace an active session.

The host drives a session with a positive, caller-selected `pump` quantum.
Each call executes at most that many fair decisions and returns state, exact
decision/tick counters, bounded high water, and an event cursor. Reaching the
quantum yields host control; it is neither success nor failure and does not
reset queues, counters, timers, identities, or cancellation state.

The externally visible states are:

- **Active**: ready work exists and a later pump can continue it.
- **Waiting**: the run is alive with no ready work. It awaits an exact input,
  output, timer, host operation, or cancellation wake; it is not a failure.
- **Quiescing**: Drain was requested and admitted work is settling.
- **Terminal**: succeeded, cancelled, failed, or disconnected.

`advance_to` advances only the active session's exact clock. A host operation
wake names the exact previously retained interest. Wrong or unretained wake
subjects make no work ready. `cancel(Drain)` and `cancel(Abort)` use the same
session's scheduler cancellation state machine; process or worker death is
not a cancellation request.

`finalize` is allowed only after Terminal and releases all session-owned
runtime storage and its registry reservation. A failed Start releases its
reservation immediately. Dropping or replacing a nonterminal session is not
an implicit successful completion or a new epoch: it marks the registry
abandoned and fails subsequent Start attempts closed until its owning host is
replaced or deliberately recovered. Worker/process death remains a distinct
host observation.

## Bounds and evidence

All runtime-plan copies, queue payload reservations, feedback slots,
ready/wait entries, transaction staging, scheduler events, implementation
state, owned host-I/O storage, concurrent-session slots, and aggregate
reserved session memory are finite and admitted before Start. Exact host I/O
uses one fixed shared store for input, stdout, stderr, and display output; its
capacity is the aggregate plan profile host-buffer allowance and is charged as
executor overhead alongside scheduler metadata. A long-lived run may have no
lifetime decision deadline; that does not make a pump, queue, timer, value,
observer, or evidence store unbounded.

`SchedulerPolicy.max_decisions` is an optional lifetime decision deadline:
zero means no decision-count deadline. A positive value remains a terminal
deadline. It is distinct from the positive caller-selected pump quantum.
Simulated-clock, cancellation, queue, value, and evidence bounds remain exact
current policy and plan facts.

Exact evidence projects from the session-owned runtime identity snapshot, not
from a retained source arena. It therefore preserves the same plan, node,
cord, implementation, artifact, host, pressure, scheduler, and terminal facts
after the caller's planning allocation has been released.

## Headless conformance

The scheduler fixture suite proves finite completion, waiting across 100 pump
calls, one-decision quantum yield, named host wake, timer wake, Drain, Abort,
terminal-only finalization, capacity failure, and exact identity retention.
The existing finite `run_exact_report` helper is convenience only: it starts a
session, cooperatively pumps it to terminal, and projects its bounded evidence.
For the duration of that finite call it may borrow the caller's streams so a
blocking hosted implementation can publish and flush its process diagnostics.
It still uses the same exact session and executor; only `Start` produces a
persistent session, whose I/O boundary is owned. The helper must not become a
second executor or a compatibility path.

## Normative requirements

| ID | Obligation |
|---|---|
| SES-001 | Start only through an explicit authorized exact-plan admission. |
| SES-002 | Retain an owned bounded runtime snapshot, never a leaked plan arena. |
| SES-003 | Keep Waiting, Active, Quiescing, and Terminal distinct. |
| SES-004 | Bound every pump while keeping its quantum distinct from lifetime policy. |
| SES-005 | Pin run, source, plan, and epoch identities until terminal finalization. |
| SES-006 | Wake and cancel only the active session through exact retained interests. |
| SES-007 | Admit finite session count and aggregate reservation before Start; release the reservation at terminal finalization or failed Start, and fail a registry closed after nonterminal abandonment. |
| SES-008 | Project exact evidence from the owned runtime identity snapshot. |
