# Fixed value-store first-free search bound

Development slice of #5231 value/Host dispatch overhead, 17 October 2026.

FixedValueStore previously searched its entire occupied prefix on every store.
A 4,096-value dense fill inspected 8,390,656 slots. It now retains a lower search
bound: every slot before the bound is occupied. A successful store advances past
the selected first-free slot; final reference release rewinds to the freed slot
if earlier. Retain and partial release leave the bound unchanged. Clear resets
it to zero while preserving existing generation advancement rules.

Slot selection remains exactly first-free, including holes released out of
order. Existing size/byte admission checks precede search, with their refusal
order unchanged. A full store reaches the empty suffix without scanning occupied
slots. Dense fill now inspects 4,096 slots; sparse refill may still traverse a
long occupied interval. The optimization does not claim constant worst-case
search or change generation-wrap semantics.

The implementation is extracted from lib.rs into fixed_value_store.rs, retaining
the public FixedValueStore path and crate-internal storage conformance access.
One usize field is added to production storage. The slot-probe counter exists
only in tests. No dynamic allocation is introduced; layout and tiny-host elapsed
time must be measured separately before broader performance claims.

Tests verify exact slot identities through dense fill, full refusal, retained
aliases and partial release, out-of-order holes, stale handles, clear and reuse.
Size/byte/stale-release refusals preserve the bound and subsequent first-free
selection. Temporarily restoring search from slot zero reproduces 8,390,656
probes versus 4,096 expected; the implementation is restored afterward.

Validation: both complete kernel suites with packed and linear ready selectors,
the complete plan-lowering suite, strict all-target Clippy for both packages,
formatting and diff checks. Existing production many-Host and 10,000-target
allocation/ownership suites pass. Whole-workspace tests were not run for this
slice. No CI dispatched or awaited; stable/physical/browser and parent acceptance
remain open.

Follow-up: `value-generation-retirement.md` supersedes the earlier wrapping
behavior. The bound now excludes occupied **or retired** slots; ordinary dense
fill and reusable-hole identity/probe observations above remain valid.

## Hosted full-store refusal fast path

HostedValueStore now uses its existing used-item count to refuse an entirely
occupied store before scanning for a smallest-fitting slot. Size and live-byte
checks still run first, retaining ValueTooLarge/ByteCapacityExceeded precedence.
Generation retirement still uses the existing fitting-slot scan whenever the
store has a non-live slot; this guard cannot hide a free retired identity.
No production storage or allocation is added.

A 4,096-slot test fills the store with empty values and performs 32 repeated
full-store attempts (empty and one-byte payloads). It now inspects zero slots
instead of 131,072. Retain/partial release preserves the full refusal without
scanning; final release reopens the exact slot, reuses it with a new generation,
and leaves the old handle stale. Clear reopens slot zero. A separate full-byte
store case checks exact size/byte/item refusal order and unchanged held value.
Removing the guard reproduces 131,072 probes versus zero expected.

Both complete local kernel suites, strict all-target kernel Clippy and
formatting/diff checks pass. The probe counter exists only in tests. This is a
work-bound improvement, with no elapsed-time or whole-host speedup claim. No CI
dispatched or awaited.
