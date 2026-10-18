# Explicit IEEE binary64 reference operations (development)

The Core profile `numeric/ieee-binary64-reference@1` defines addition,
subtraction, multiplication and division with explicit nearest-even,
toward-zero, toward-positive and toward-negative rounding. Each call returns
the binary64 result and five independent exception flags: invalid,
divide-by-zero, overflow, underflow and inexact. Callers accumulate flags
explicitly; the functions do not change the Host floating-point environment.

Raw IEEE operands retain signed zero, infinities and NaN payloads. The first
NaN operand supplies the propagated sign/payload, with its quiet bit set;
signaling operands raise invalid. Invalid operations without a NaN operand
produce `0xfff8000000000000`. This is a declared profile convention. Quiet
equality, less, less-equal and unordered predicates have separate APIs from
the existing carrier's encoded-bit identity. Quiet predicates raise invalid
for signaling NaNs. Both zeros compare numerically equal.

Finite arithmetic reuses the existing bounded exact arithmetic and binary64
rounding implementation. Each operation rounds before another call consumes
its result. Underflow uses after-precision-rounding tininess, including the
minimum-normal boundary. Classification and exact arithmetic spend the caller's
finite work meter. Resource exhaustion returns `WorkLimit` rather than an IEEE
result or silently missing flags. Inputs, outputs and arithmetic storage have
fixed extents; the operation functions allocate nothing.

The independent oracle is
[SoftFloat at a0c6494](https://github.com/ucb-bar/berkeley-softfloat-3/tree/a0c6494cdc11865811dec815d5c0049fba9d82a8),
built with the 8086-SSE specialization and after-rounding tininess. Twenty
operands cover both zeros, signed finite values, subnormals, the normal boundary,
maximum finite values, infinities, distinct quiet/signaling NaNs and precision
neighbors. Their complete pair matrix produces 6,400 arithmetic cases across
four operations and four rounding directions, plus 1,600 quiet comparisons.
The retained fixture records independently generated expected result bits and
all flag bits. The test observes zero allocations/reallocations across all
8,000 calls. Separate tests distinguish operation order, explicit sticky flags
and resource refusal, including exceptional operands with exhausted work.

This is a bounded correctness-reference component. Public Source syntax,
installed Backs, sealed Plan resource/operation identities, native fast
realizations and their product proof remain integration work under #5392.
The checked finite expression operators retain their existing profile. The
reference component alone does not complete the parent IEEE escape-hatch or
performance acceptance. The captured matrix is edge conformance, not an
exhaustive proof over every binary64 operand pair.
