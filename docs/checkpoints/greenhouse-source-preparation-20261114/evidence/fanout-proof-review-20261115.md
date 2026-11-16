# PreserveOrder fanout stock review — 2026-11-15

Verdict: **do not adopt the proposed reduction as a general kernel guarantee yet.** The successful-execution queue bound is sound, including unequal item/byte capacities, but the current public failure/retry contract does not establish its essential atomic-publication premise. There is also a concrete reference-ownership counterexample after partial commit failure, including capacity-one fanouts present in this Source.

Read-only source audit; no builds or runtime fault injection were performed. Baseline HEAD: `d863617e78b1e27ef290b53e6f67e9f11906b5fe`. Inspected `architecture/kernel/src/{scheduler.rs,lib.rs,scheduler/payload_admission.rs,scheduler/retirement.rs,scheduler/prepared_session.rs}` and `outputs/source-fanout-opportunity-20261114.{json,md}`. Kernel files inspected had no working-tree modifications during this audit. Other workers own production edits.

## What is proved on successful commits

For one producer output, assume every accepted publication appends the same value identity to every sibling, all siblings use PreserveOrder, and consumers remove only prefixes. Each sibling's pending values are then a suffix of the producer's published sequence. The union of suffixes is the longest suffix, hence its cardinality is at most the maximum sibling item capacity. Byte capacities can prevent publication earlier; they cannot invalidate this invariant. No equal-byte-capacity assumption is needed for this cardinality proof. Actual per-class packing must still charge the maximum extent/class of the live identities.

Separate staging, retained Back values, native pins and other nonqueue ownership remain additional stock. Logical queue cells and generations are not eliminated. CoalesceLatest is excluded: replacing newest entries does not preserve the suffix argument. Remote siblings, omitted siblings, alternate output ports and abnormal-terminal deliveries require independent accounting.

## Concrete source-level failure counterexample

Use three local PreserveOrder recipients A/B/C, each item capacity one, identical schema/extent, an ordinary copied prepared output, and no retained Back/native reference. This shape occurs in the candidate Source (for example source node 7, Cords 8/9/10).

The public safe `SignSink` trait allows `record(...) -> Result<..., SignError>`. Its default `ensure_capacity` checks capacity but does not make subsequent records infallible or reserve a transaction. Choose a sink that accepts preflight, then returns an error on the first `ValueRouted` Sign of a publication, and accepts the consumers' subsequent Signs.

1. Materialization creates value V with one reference.
2. `commit` computes three target references and retains V twice: refcount three.
3. `push(A,V)` installs the first queue reference and marks A ready.
4. The following `signs.record(ValueRouted)` returns an error before B/C receive V.
5. The caller invokes `rollback_generated_output`; for this copied prepared output it releases V once. V now has two references, although only A owns a queued reference.
6. A consumes V and releases its queue reference. V remains live with one reference, no queue cell, and no Back-owned reference.
7. Public `step` can be called again: there is no fault poison requiring cancellation. B/C are empty, and after A drains all three outputs are writable. Repeat with a fresh materialized identity. Each repetition can leave another unreachable live slot.

Therefore physical live stock is not bounded by max sibling queue capacity plus a fixed single-step staging allowance under the current public API. Two such failures/drains leave two unqueued live identities; another publication needs a third physical slot where the proposed group charge is one queue slot plus one staging slot. Other unrelated stock may mask exhaustion on a particular Source; it does not repair the proof.

Relevant implementation: `scheduler.rs::commit` retains target references before the fanout loop; that loop calls `push` before fallible `signs.record` and fallible debug-value lookup. `scheduler.rs` calls `rollback_generated_output` on commit error and does not call `step_committed`. `payload_admission.rs::rollback_generated_output` releases one copied staged-output reference, without undoing installed queue entries or uninstalled target retains. This is an existing failure-custody gap, not a claim that the reduction creates the gap.

The built-in bounded Sign log does override preflight to check sequence capacity as well as retained capacity. I did **not** find evidence that it produces this failure after a successful, correctly counted preflight in ordinary operation. The counterexample uses the public trait as currently specified. A restriction to a particular certified sink/storage combination could exclude it, but must be explicit and proved rather than inferred.

## Partial fanout also defeats the queue-only claim in the general case

Three siblings of capacity two suffice, even disregarding leaked references. Publish V1 to all and drain A/B. Publish V2, fail after A/B are pushed, drain A. Publish V3, fail after A is pushed. Queues are now A=[V3], B=[V2], C=[V1]: three queued identities exceed max capacity two. This shape is not one of the candidate's current groups: nine groups have capacity-one siblings; the remaining group has capacities two and one. Thus this is a counterexample to a reusable unrestricted reduction, not evidence that those exact current groups reach three distinct queued identities by this schedule.

## Cancellation and rearm

Successful `cancel` clears value storage, all queue cells, and per-Cord lengths/bytes. It can reclaim the above state, but it is an explicit operation, and cancellation itself has a fallible Sign step before clearing. Normal prepared-session rearm checks normal drain, no pending calls, and empty queues; it is not a rollback transaction for a failed publication. Neither makes public retry after an error atomic.

## Smallest acceptable next step

Keep the reduction disabled until one of these contracts is established and tested:

* A transaction-safe commit: every fallible operation is preflighted/guaranteed infallible after mutation, or complete undo restores references, queues, readiness and driver ownership.
* An explicit faulted kernel state on partial commit that forbids further execution until successful retirement/cancellation, with admission accounting covering all intermediate ownership and a proven cleanup path.
* A narrowly certified no-partial-failure execution path, with the stock reduction gated on that certification rather than merely Source topology.

Then test partial-commit injection at every route boundary, public retry behavior, mixed item/byte capacities, cancellation, and rearm. The mathematical FIFO bound is worth retaining; current successful-path evidence alone is insufficient to install it as resource admission authority.
