# Empty armed-quiescence fast path

Development slice of #5231, 17 October 2026.

After the prepared-prefix optimization, emission still scanned that prefix on
every idle poll even when no quiescence edge was armed. The existing 128-node
fixture visited two cord entries on every post-emission poll. The runtime now
maintains an exact count of cord `quiescence_armed` flags. A zero count returns
without visiting any cords. Both fixed and boxed preparation initialize zero.

Arming increments only on false-to-true transitions. Successful emission clears
the flag and decrements the count after queue publication. Pressure or admission
refusal leaves the flag/count intact. Global cancellation clears both alongside
its existing cord cleanup. The count includes armed cords whose source has
completed; this is an occupancy count, not an eligibility claim, so such cases
may still scan and retain their existing semantics.

One usize field is added to the scheduler, with no per-cord index or runtime
allocation. Empty-set emission is constant work; nonempty cases retain the
existing prepared-prefix bound and ascending publication order. No elapsed-time
speedup or unchanged-layout claim is made.

The 128-node fixture now verifies zero post-emission probes rather than two.
It rearms twice while both queues are full, proves the count remains two and no
Sign is emitted, then frees one branch at a time. Each exact edge publishes and
the count falls by one; the final empty poll visits zero cords. Rearming before
global cancellation proves cancellation clears every flag/count and releases
all values. Existing refusal/retry, absent-track, boxed preparation and terminal
suites continue to pass.

Validation: both full local kernel suites under packed and linear ready
selectors, strict all-target kernel Clippy, formatting and diff checks. No CI
dispatched or awaited; no stable/physical/browser or parent-completion claim.
