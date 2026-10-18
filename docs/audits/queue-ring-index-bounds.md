# Fixed queue ring index bounds

Development slice for #5231. This repairs queue correctness under large admitted
layouts; it does not establish the parent scheduling performance acceptance.

## Failure and correction

The scheduler previously added `head + len` (and the coalescing variant) in
`u16` before applying the ring modulus. A 60,000-slot ring with head 59,000
therefore overflowed while filling. It also added `slot_start + offset` in
`u16` before widening to an arena address, although Plan validation already
checks the arena end in `usize` and admits an arena crossing slot 65,535.
Both cases reproduced debug overflow in the former production operations.

A shared private `queue_slot` helper computes the bounded ring position by
comparing against the distance to wrap, then widens each operand before adding
the arena base. Peek, pop, push, coalescing admission, and Host-value ownership
use the same calculation. It adds no production state or allocation. Its
preconditions are the existing validated nonempty ring capacity and bounded
head/offset invariants; empty rings return before indexing where appropriate.

## Local proof

Two native regressions exercise production scheduler queue operations:

- A 60,000-slot ring starts at a reachable empty head of 59,000, fills using
  retained references to one empty value, coalesces its newest item, pops through
  wrap, and refuses to discard the still-owned replacement.
- An admitted arena at base 65,000 with capacity 600 pushes, peeks, and pops at
  physical slot 65,550, then permits discard of the unowned value.

The fixtures seed valid heads to isolate address arithmetic and use diagnostic
threads with 32 MiB stacks for the large fixed scheduler. They are internal
native tests, not complete Plan/Play workloads or constrained-device proof.
A separate helper test compares every head/offset for capacities 1 through 32
against widened modulo arithmetic.

Local validation: formatting; kernel alloc suite (188 passed); kernel alloc plus
linear-ready-reference-test suite (188 passed); both focused queue regressions
without alloc; strict kernel all-target Clippy with alloc; and diff whitespace.
The same two queue regressions also pass in the optimized release profile.
No timing speedup, physical-device result, CI acceptance, or parent-issue closure
is claimed.
