# Local Steps honor admitted coalescing pressure

Development fix for #5231 pressure/fan-out/ownership preservation, 17 October 2026.

The local output context and queue writer already supported CoalesceLatest, but
ordinary Step preflight unconditionally rejected a full queue. Thus the Back
could stage output within its advertised allowance and still receive
QueueCapacityExceeded instead of the admitted newest-item replacement.

Local output preflight now distinguishes a blocking full queue from an admitted
coalescing queue. Semantic value-envelope checks remain in place. Byte admission
uses the existing shared headroom calculation, which subtracts the superseded
newest item's bytes for full coalescing queues. Queue publication still performs
the replacement and releases the superseded ownership through the existing
commit path. No new storage, traversal, pressure policy or allocation is added;
the preflight responsibility moves into payload_admission.rs.

A regression executes actual local canonical-emission Steps to two two-item
cords. The first values are `old` and `new`; the next Step emits `last`. With both
branches coalescing, each queue retains `old` followed by `last`, holds seven
bytes, and maintains two references to each shared value. The superseded `new`
value is reclaimed. Exact routing Sign count is six. With one blocking branch,
both queues retain `old` followed by `new`, hold six bytes, and have only four
routing Signs: no branch partially replaces while its sibling is blocked.
Removing both oldest references leaves exactly one shared newest value with
two references; global cancellation releases the remaining storage.

Restoring the old unconditional full-queue refusal makes the regression fail
with QueueCapacityExceeded. The fix is restored before final validation. Both
complete local kernel suites pass under packed and linear ready selectors;
strict all-target kernel Clippy, formatting and diff checks pass. No timing,
physical/browser/stable acceptance or parent-completion claim is made. No CI
dispatched or awaited.
