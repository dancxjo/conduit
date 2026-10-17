# Bounded revision

Revision is explicit later truth, not mutation. Stability is a policy assertion,
not commitment. Commitment protects a semantic prefix, not finality or external
delivery. Correction preserves the old committed assertion (#4951).

## Lifecycle and identities

[`conduit_core::revision`](../../architecture/core/src/revision.rs) is a small
preparation/semantic library. Ordinary `T`, `T...`, `T...|` and `$T` do not change.
It introduces no scheduler, runtime instruction, universal wire envelope,
automatic Sign, authored kind, or mandatory event store. Native domain Types
remain ordinary Conduitese declarations; a Rust generic law checks their
lifecycle without importing text, syntax or speech ontology into core.

Each immutable event names an exact stream, subject, history epoch, producer,
policy, contract, sequence and event label. References include that whole scope,
not just a label. Revision names the exact current proposal; stability,
commitment and withdrawal name that proposal; correction names an exact retained
commit event. Every event retains one to eight exact source-generation evidence
references. Such references describe provenance; admission must separately
establish authenticity and availability. They grant no authority.

A proposal becomes the active interpretation. A revision explicitly supersedes
it. Stability declares a cursor under the context's exact policy; revising that
portion clears stability. Commitment may advance without first declaring
stability, but must strictly advance within the observed region. Withdrawal
removes the still-revisable proposal using a domain-owned delta and reason.
Closure stops ordinary revisions; later explicit correction remains possible
and does not reopen the stream. No lifecycle edge is inferred from value equality,
string differences, a timeout, or an animation.

## Finite law

`RevisionDomain` owns the finite delta representation, cursor type/unit, affected
region, distance and deterministic validation. The contract identity must bind
those meanings and configuration. The journal revalidates events under its own
domain before accepting them. Validators/reducers remain reviewed domain code,
not hostile-code confinement. No field specifies transport or resource binding.

The committed cursor advances monotonically within one epoch. Ordinary deltas
start at or after it. The observed high-water cursor retains the largest admitted
extent; the distance from committed to observed cannot exceed the admitted
revisable-unit limit. Domain reducers separately check whether an affected range
exists and what a replacement means; a high-water mark alone is not current text
length or evidence of a value in every slot. A correction must affect only the
committed prefix covered by its exact target commitment. It cannot move the
committed/observed cursors or erase the original event. If it touches an
asserted stable prefix, it clears that stability assertion; corrected truth
cannot inherit policy evidence established for the older interpretation.

IDs and reasons are borrowed, nonempty, at most 192 UTF-8 bytes. The journal uses
a fixed 64-slot array, admits a caller-selected capacity of one to 64 events,
and allocates nothing. Deltas/evidence are borrowed frozen data with domain
bounds; their backing storage must also be finite and admitted by the consumer.
Appending is atomic: refusal changes neither history nor lifecycle/frontiers.
Closed, stale reference, scope/sequence mismatch, domain invalidity, committed
rewrite, region pressure and history pressure have distinct typed refusals.

Explicit prefix truncation reports a cumulative count. It cannot remove the
current proposal. Once any portion commits, the entire retained epoch is pinned:
earlier deltas can contribute to committed truth even when the commitment names
only the latest proposal. Corrections remain pinned too. Capacity pressure
refuses; there is no automatic eviction, infinite archive, or hidden rollover.
A fresh epoch requires an explicit domain handoff outside this contract.

## Current truth and inspection

A consumer may apply a deterministic typed reducer to produce its current
retained view. That view is a projection of history, not its replacement, and
does not make other keeps event-sourced. Separate journals give text, syntax,
synthesis and other stages independent frontiers; no Body-wide done bit exists.
Semantic commitment cannot establish displayed text, heard audio or device
motion. Existing execution Signs and effect/output receipts establish delivery.

Inspection can borrow `history()`, `context()`, each event's immutable change,
contract/evidence, `frontiers()`, `current_proposal()`, closure and truncation
count, plus the domain reducer's optional view. A focused Watch/Patchbay consumer
can render those facts without a second truth store. This issue supplies the
read-only seam, not a new universal Patchbay panel.

Replay starts from the exact initial cursor/context/limits and complete bounded
event sequence with its explicit truncation count. It reruns the same admission law and reconstructs the same
frontiers, active proposal and closure. Domain reduction reconstructs the same
current view. A nonzero truncation count refuses `TruncatedHistory`, including when the
retained slice is empty; a noninitial first sequence also refuses. It cannot pretend
to reconstruct omitted truth without a separately admitted domain snapshot.

## Development proof

`cargo xtask check workspace-test-foundation` covers the core, language and
speech semantic fixtures through the repository's supported entrance.

- Core tracking uses frame cursors and finite class deltas. It exercises every
  lifecycle edge, late correction after closure, immutable commit history,
  region/history pressure, explicit truncation, independent stages and replay.
- The language adapter consumes native revisioned dependency arcs and exact
  source-token identities. A supplied garden-path analysis of “The old man the
  boats” revises an early `amod` arc to `nsubj`, stabilizes and commits it, then
  requires explicit later reanalysis and clears the superseded stability
  assertion while retaining commitment. This is #4907's reusable consumer seam,
  not a running parser or proof of linguistic accuracy.
- The ASR adapter consumes native partial, scalar replacement, cancellation and
  committed-segment Types. A Unicode fixture demonstrates scalar rather than
  byte ranges, explicit correction, unchanged native committed text and exact
  replay. Native commitment validates already-derived text; it cannot secretly
  replace it. This scalar-epoch profile admits only cardinality-preserving
  committed corrections; length-changing reanalysis requires an explicit new
  epoch so retained cursor positions do not silently change meaning. Native
  listening records remain domain evidence, not replaced by this lifecycle.
  Audio recognition quality and live delivery remain separate proof.

The limited lifecycle/frontier and delivery-separation ideas come from Tongues'
[pinned streaming contract](https://github.com/dancxjo/tongues/blob/b03702798db5a5e7d174278ac2fc1233f171b02e/docs/streaming-event-contract.md)
and [duplex implementation](https://github.com/dancxjo/tongues/blob/b03702798db5a5e7d174278ac2fc1233f171b02e/crates/speaking/src/duplex.rs).
The [reuse ledger](../reuse-ledger.md) records accepted and rejected reuse.
