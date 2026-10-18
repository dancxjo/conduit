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

The [validation record](evidence/ieee-binary64-reference-development-20261017.json)
retains source/compiler/oracle identities, the fixture hash, 17 passing focused
Core tests, strict lint and a no-default-features check. Its raw archive retains
the independent oracle program, captured expected vectors and validation logs.

## Source contract preparation

Private development now checks `math/ieee-binary64` with required typed
`operation` and `rounding` startup values, and
`math/ieee-binary64-compare-quiet` with a required typed predicate. Both consume
two raw F64 Value Fores and return a typed value plus separate Boolean flags.
The declarations reuse existing native variant constructors, for example
`operation=add(empty),rounding=nearest_ties_even(empty)`.

Tests check all sixteen arithmetic configuration forms and four predicates,
explicit-rounding refusal, foreign/extra configuration rejection and the pure
semantic contract. Existing numerical catalog tests and strict catalog lint
pass. These are checked contracts and preparation codecs; initialized Backs,
native availability and actual product execution remain unfinished.

## Native instruction component preparation

A private x86_64 SSE2 component executes fixed compiled scalar instructions
for the four arithmetic operations. It shares the Core profile's constant-work
NaN, infinity and zero classification, then executes the selected native
instruction directly. There is no implicit software-reference fallback.
Unsupported targets report unavailable before execution.

One assembly block saves MXCSR, installs the requested rounding with traps
masked and FTZ/DAZ disabled, captures the five IEEE flags, and restores MXCSR
before Rust resumes. This follows Rust's requirement that nondefault control
state must not escape into Rust execution. A preservation probe installs its
synthetic prior state inside that same block and restores the actual ABI state
before returning; it never establishes nondefault control across Rust code.

The component matches all 6,400 arithmetic rows of the independent retained
SoftFloat fixture, including result bits and all five flags, with no observed
allocations or reallocations. The preservation probe checks all four rounding
modes with preexisting sticky flags and a different rounding/FTZ setting.
These tests do not establish exhaustive operand conformance or a throughput
claim. This helper is currently included by the component test only. Production
factory registration, typed output preparation, sealed resource admission,
normal kernel execution and actual Source product proof remain unfinished.
