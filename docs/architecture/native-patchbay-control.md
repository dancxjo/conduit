# Native Patchbay Plan and Play control

Issue #558 adds a control and inspection layer without making Patchbay a planner, scheduler, or
source of runtime truth. `F5` submits the currently checked and canonically expanded Form to the
ordinary local planner. The retained Plan is immutable and rendered with its independent source,
checked, expanded, and Plan identities; exact host, boot, generation, capability, implementation,
artifact, connection bounds, and sealed route candidates remain visible.

`F6` first admits that exact Plan against the current source identity and current advertisement.
Stale source, stale boot/generation, absent capability or implementation, missing authority, and an
invalid Plan are separate failures. Execution then runs asynchronously through
`StdHost::run_fragment_controlled_to` and the installed Conduit kernel. Patchbay never kills the
worker to implement Stop.

`Escape` submits one bounded, exact-identity Stop request. The std-host adapter admits at most one
request and invokes the existing scheduler cancellation operation. Its terminal report preserves
`OperatorRequested`, binds the accepted request to the resulting active Play identity, and exposes
the bounded kernel clue containing `CancellationRequested` and `RunCancelled`. Duplicate Stop
requests are rejected under their own identity.

The presentation keeps Form, Plan, and Play as separate rows. Plan route candidates are immutable
facts; Observatory link availability remains a separate live projection. Queue/byte pressure is
labelled unavailable when the current std-host report does not expose a snapshot, rather than being
invented. Terminal completion, cancellation, host failure, and clue gaps remain distinct.
