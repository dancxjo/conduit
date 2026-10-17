# Scalar tape scratch prepared to its admitted size

Local #5396 implementation slice coordinated with #5395, 17 October 2026.

The explicit fixed-integer/Boolean tape previously initialized a 128-slot stack
array on every evaluation, including tiny tapes. Preparation now reserves a
reusable scratch vector for the exact admitted instruction count. Runtime
clearing touches that tape's slots, while the 128-instruction admission ceiling
and all lowering/jump/arithmetic semantics stay the same. The reference evaluator
and explicit std profile selection are unchanged.

Scratch values own their at-most-16-byte scalar encodings. Input and literal
bytes are copied into those bounded inline values, so no borrowed input or
prepared-literal reference can survive in reusable storage. Each evaluation
clears results left by either conditional branch or an arithmetic refusal.
No unsafe code or allocation during warm evaluation is introduced.

This moves scratch ownership from an evaluation stack array to pre-Play heap
storage; it does not claim lower total resident memory. Preparation adds a
scratch allocation and the tape owns another vector header. Existing instruction,
literal and output allocations remain separate. `scalar_tape_limits` reports
actual reserved scratch payload bytes; whole-Host stack/heap/Plan footprint is
not established by that number. No installed browser/no_std/tiny-device tape
profile or physical performance claim follows from this change.

Tests compare the identity and 1/3/8/32-operation scratch bounds, retain the
127/129-instruction boundary, and alternate lazy branches, successful values and
overflow refusals repeatedly on one evaluator with no allocations/reallocations
and stable scratch/output capacities. Existing differential tests preserve exact
bytes and refusals against the reference.

The retained before/after diagnostic uses the existing ignored std release test
`scalar_tape_performance`, with an explicit clean git source pin. Each variant
retains eight rows per 1/3/8/32-operation shape and both reference/tape modes,
including 100,000-evaluation warm batches and real sealed-Plan preparation/Play
with validated output. These are descriptive same-process batches on an
uncontrolled host, not independent confidence intervals or performance gates.
