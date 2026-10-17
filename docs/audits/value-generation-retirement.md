# Finite value generations never revive stale handles

Development correctness fix for #5231 stale identity/value ownership, 17 October 2026.

Both fixed and hosted value stores advanced a u16 generation with wrapping_add,
skipping zero. After 65,535 allocations of the same slot, allocation 65,536 could
return generation one again. An old generation-one reference with the same byte
length then became indistinguishable from the new live value.

A slot at generation 65,535 now retires when no longer live. Stores select only
free slots whose generation can advance without wrapping. Other reusable slots
remain available in the existing first-free (fixed) or smallest-fitting (hosted)
order. If no reusable slot fits and retirement prevents allocation, the explicit
StorageError::GenerationExhausted refusal is returned. Existing size/byte budget
checks retain their earlier precedence. Clearing references never resets
generations or makes retired identities reusable.

The fixed search-bound invariant becomes: every earlier slot is occupied or
retired. Final release rewinds only for a reusable slot, and a monotonic boolean
records whether retirement occurred. Clear preserves that fact. Hosted selection
records fitting retired slots during its existing scan; no persistent hosted
field or allocation is added. A live final-generation reference remains valid
through retain/partial release until its final release or clear.

Public integration tests exhaust the full generation domain in both stores,
exercise final release and clear, verify explicit refusal/empty occupancy after
retirement, and prove the next fresh slot is still usable. They also verify old
handles stay stale after subsequent clear/reuse. Running these tests against the
actual prior store source reproduces both returning slot zero/generation one
instead of GenerationExhausted. The new implementation is restored afterward.

Validation: both full kernel selector suites, the complete plan-lowering suite,
strict all-target Clippy for both packages, formatting and diff checks. No
whole-workspace, browser, physical or stable acceptance claim is made. This adds
an observable storage refusal for genuinely exhausted identity space; long-lived
Hosts must propagate that refusal rather than retry forever. No CI dispatched
or awaited; parent acceptance remains open.

## Kernel remote-publication boundary

Two focused scheduler regressions now retire slots through the public
store_host_value/discard_host_value API rather than mutating internal generation
fields. After retiring both fixed-store slots and reaching Idle, single-target
and shared fan-out admission each refuse twice with GenerationExhausted. Exact
Sign entries, remote Sign count, queue usage/sequence/open flags, value occupancy
and unready state remain unchanged. Stale Host value inspection still refuses.
Metadata-only remote closure remains admissible, and subsequent input correctly
reports RemoteSequenceRejected before storage admission.

With only the first slot retired, shared admission selects slot one at generation
one, creates two references to a single one-byte value, and keeps the old
same-length slot-zero reference stale. Cancellation releases the shared value.
Both focused regressions pass under packed and linear ready selectors, alongside
strict all-target kernel Clippy and formatting/diff checks. No production code
changes in this proof slice. This establishes remote ingress behavior; broader
terminal-publication rollback is not inferred from these cases. No CI dispatched
or awaited.
