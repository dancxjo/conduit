# Plan replacement and retained Fore outcome custody

**Partial implementation:** #6102 and #5571 remain open. The public run report
and local API lifecycle now retain stopped output custody, but the normal
foreground owner still replaces its durable execution receipt with the latest
receipt. That remaining boundary must be fixed before either issue closes.

For #6102 and the remaining outcome-custody criterion of #5571, the installed
std Body runner now retains an interrupted Fore output in its owning
`BodyRunReport`. Previously, an adapter error after receiving a value discarded
that attempted delivery from the report. The kernel remained unacknowledged,
but the external effect could already have happened.

The new `fore_pending_output` retains the exact prepared payload slot, front
port, value kind, track and sequence. Its closed stage records no adapter call,
unknown adapter outcome, adapter acceptance, or kernel acceptance. The route
stops after failure; custody is not permission to retry or to replay the value
in a replacement. A successful kernel delivery is retained before subsequent
source/map acknowledgement, so a later acknowledgement failure cannot erase
an already established delivery. Product Body/Todo/thermostat receipts retain
the stage, exact output identity, byte length and SHA-256 alongside their
existing Plan/Play/terminal Sign identities. These compact receipt fields do
not claim external success or retain another copy of payload bytes.

## Actual local hosted lifecycle cases

All three cases use the checked `plots/todo/checkpoint-once.conduit` Source,
ordinary planning, installed Todo Backs, real production kernel, admitted
checkpoint Host Calls and real selected temporary filesystem residences.
Host/Boot labels and resource content identities are declared fixture offers;
this is local hosted execution, not physical machine reboot or remote-network
failure proof. No alternate scheduler or runtime is introduced.

1. An adapter retains the old committed output and then returns an error,
   deliberately making its acknowledgement unknown. The old report retains
   the exact immutable buffer (including shared storage pointer), failure,
   terminal Sign, complete kernel evidence and unacknowledged output. The
   ordinary `BodyPlanningSession` retains the exact started/failed execution
   claim and old Body Plan. Replacement uses a fresh Boot, authority, Plan,
   Play and terminal Sign. A stale old Plan refuses before effects. An explicit
   bounded pre-Play copy transfers the two published checkpoint records to the
   new selected residence; the next write selects a new immutable resource
   version. A new command extends the reconciled state, with the original
   command/effect occurring once. No unknown external acknowledgement is
   inferred or imported into the new Play.
2. An adapter accepts an in-flight output and requests operator cancellation.
   The kernel's offered, delivered and cancelled event sequence proves delivery
   drained before cancellation. The cancelled report retains the acknowledged
   output and terminal custody. A separately admitted replacement extends the
   retained checkpoint with one new command, without replaying the old command.
3. A selected filesystem root disappears after Play admission and becomes a
   regular file before the real publish Host Call. The old Play fails with its
   exact Host Call evidence and no attempted external output; cleanup completes
   and its Host owner is retired. Throughout loss and replacement, an independent
   Body on another Host's real admitted Play remains live with resident initial state and its
   selected checkpoint reservation. Only after replacement finishes does that
   same independent Play receive one fresh command and complete under its
   original Play identity. Its scheduler/state is never restarted.

The first case reparses/checks/expands the actual Source inside both cold and
incremental evaluation against current Host advertisements. The actual executed
ordinary Plan IDs are the oracle's result IDs; a fresh Boot/resource binding
invalidates the retained candidate. Old complete Plans and reports remain
unchanged across the replacement. Earlier multi-candidate Host/Boot/Line,
resource, authority and cost cases remain credited in
[incremental-host-line-20261020.md](incremental-host-line-20261020.md).

## Finite ownership and cleanup

Each product Plan contains two installed Gears and the same existing one-item
Fore bounds: 75 command bytes and 1,635 bytes each for current/committed state.
The new pending-output field is one optional fixed record; it moves one already
prepared output slot, without cloning payload bytes or growing a runtime atlas,
queue or evidence history. The successful-delivery vector retains its existing
pre-Play bound. Post-run metadata hashing/formatting is separate from Play work.

The transfer proof admits two source records of at most
`CHECKPOINT_MAX_BYTES` each before new execution; after one new immutable
version, its destination has three final records. Publication can temporarily
retain two bounded immutable candidates plus the old and staged 36-byte selectors:
at most four files and `2 * CHECKPOINT_MAX_BYTES + 72` bytes. The copied source
records remain under their old owner and in the bounded pre-Play test transfer
buffer until evidence inspection. This explicit test transfer
is not a new automatic migration service or generic remote-resource capability.
The independent live Fore queue admits one command and a finite 30-second wait;
each case declares a 4 MiB proof worker stack and the independent case adds
one more. With the three test cases running concurrently, at most four such
worker stacks (16 MiB) exist, in addition to the test harness. These test
workers do not advertise a new runtime lane capability or ambient worker pool.
The existing kernel storage profile remains 32 nodes, 64 Cords and 64 queue
slots. No capacity constant is raised. Actual cleanup failure remains a separate
report field; all successful proof cleanup is checked. Old evidence is retained
through new execution and inspected before temporary fixture removal.

The existing large composition files receive only report-field wiring. New
custody behavior is in responsibility modules; shared checkpoint preparation is
extracted from the previous >600-line test into a common fixture. The old test
now remains below 500 lines. Neither Face meaning nor active Plans are mutated.

## Validation and limits

Executed source: `180b88c` (full identity in the retained validation receipt),
based on `22bff77`. Source and worktree were clean and unchanged before/after the
validation matrix. `cargo xtask check todo-durability` passes 44 tests; its one
ignored test requires a separate exact-source retained DOM exchange and does not
establish DOM reconstruction here. Four Parsec Fore tests and two exact
cancellation/release tests pass. Strict scoped Clippy and focused formatting pass.
The wrapper checks selected test counts, so a zero-test filter cannot pass as
coverage. The [validation receipt](evidence/plan-replacement-custody-20261020/validation.json)
retains exact commands and counts; the [summary](evidence/plan-replacement-custody-20261020/summary.json)
hashes every compressed/raw log. The full durability log retains complete actual
Body Plans and run reports for all three lifecycle cases.

Supported reproduction includes these cases in:

```text
cargo xtask check todo-durability
```

The independent Python verifier checks source/Plan/Play/Sign linkage, pending
payload fingerprints, oracle/execution binding, cancellation order, independent
Play identity and exact replacement item histories. Seven corrupted real records
must refuse: altered payload hash, erased uncertainty, wrong oracle result,
replayed old command, reordered cancellation, conflated independent Play and conflated Body birth.

The proof establishes this concrete local Todo replacement boundary and report
custody. It does not implement an automatic replan controller, make unknown
external delivery successful, authenticate remote producers, demonstrate arbitrary
resource migration, or establish performance savings. Cost profitability and
hysteresis remain #5568; #6101 remains the separate measured cost-fact contract.
#6078 owns automatic Integration/main/publication follow-through. Direct dev
publication follows the user's explicit task workflow override; no new CI or
accepted-release claim follows from these local results.

## Remaining normal-owner boundary

The [foreground owner transaction](../../products/conduit/src/body_owner/state.rs)
currently writes `body/owner-execution.json` from `last_execution`. Biography
archives preserve lifecycle history, but that write alone does not preserve each
old output's receipt/fingerprint and uncertain delivery stage after a later
execution replaces it. This audit therefore does not close #6102 or the original
old-outcome custody criterion of #5571.

Finish this through the existing recoverable owner transaction and bounded
retention contract: retain old Plan/Play/output/terminal Sign scope when a new
execution receipt is committed, preserve uncertainty, and verify actual recovery
and replacement through the normal owner route. Capacity exhaustion must refuse
before new effects rather than silently dropping old outcomes. Keep causal
history separate from current liveness and never infer permission to replay an
old external effect. This is remaining implementation work, not an external
blocker or a replacement definition of either issue's acceptance criteria.
