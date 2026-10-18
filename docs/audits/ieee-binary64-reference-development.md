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

This is a bounded correctness-reference component. It alone does not establish
Public Source syntax, installed Backs, sealed Plan resource/operation identities,
native fast realizations or product execution; subsequent integration is
described separately below.
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
pass. These initial contract captures cover checking and preparation codecs; they do
not establish initialized Backs, native availability or actual product execution.

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
claim. That capture includes the helper through its component test only and does not
establish production availability. Later initialized Back integration requires
separate admission and actual Source product proof.

## Prepared result encoding

The private result encoder prepares exact canonical Types, Boolean values and
three bounded composition buffers before execution. It composes the arithmetic
or comparison result with five independent Boolean exception fields. Prepared
heap capacity includes the composition schema and child buffers; it excludes
inline object size and allocator bookkeeping. Output buffers reserve the exact
fixed encoded extent of their reviewed Types.

The component test validates exact outer Type identity, field names, all five
flags and raw IEEE bits over 352 encodings: all 32 flag combinations across nine
carrier specimens and both comparison results. Signed zeros, subnormals,
infinities and NaN payloads survive encoding. Mixing arithmetic/comparison output
forms refuses. All calls observe zero allocations/reallocations and unchanged
retained heap capacity. Strict test lint passes. That capture covers test-included
preparation work; it does not establish an advertised or installed Back.

## Initialized Back development

Private development now wires the two Source Kinds through the ordinary std
factory and existing kernel Step dispatch. Source-specific initialization offers
the arithmetic Kind only for the compiled x86_64 SSE2 profile and offers quiet
comparison through constant-work integer classification. Unsupported native
arithmetic refuses before Play; no software-reference fallback is installed.
The arithmetic execution profile identifies its CPU instruction contract.
Operation, rounding and predicate remain authored semantic configuration sealed
in the ordinary Plan.

An inspectable `std/ieee-binary64-bounds@1` property records one operation,
16 input payload bytes, the exact output extent and retained encoder heap
capacity. These are component bounds, excluding allocator bookkeeping, inline
objects and whole-Host memory. Installation reconstructs the expected offer and
bounds and checks the selected Kind, Fore, implementation, artifact, profile,
limits, effect/authority absence and exact typed configuration before Play.

One kernel Step checks both inputs and output readiness before consuming them.
It charges one computation fuel unit for its fixed instruction/classification
and fixed-shape encoding; ordinary input consumption and output staging each
charge their existing unit, giving four total units. This is a bounded atomic
operation, not an instruction-count or wall-time claim. It performs no
source-sized iteration or exact-rational arithmetic. Normal commit completes
the one-value operation; cancellation clears staged output. Typed result bytes
remain owned by prepared storage until the normal scheduler commits them.

The isolated Step conformance passes all sixteen arithmetic configurations,
checks all result bits and flags against the Core reference, and observes no
warm allocation. Additional cases distinguish pressure, cancellation, malformed
input, fuel exhaustion, insufficient output capacity, signed-zero numerical
equality and signaling-NaN invalid. Two installed-factory/Source-batch tests also pass: selected identity and bounds
are recomputed, tampering refuses, preparation is idempotent, invalid Source
leaves the advertisement intact, and a replacement Source clears owned IEEE
offers. This section is not stable acceptance.

The complete actual CLI numerical-expression corpus passes all ten tests. The
IEEE tests execute 22 arithmetic cases and eight quiet-comparison cases from
the independent SoftFloat fixture through authored Source, ordinary planning,
selected initialized Backs and the shared kernel. All sixteen arithmetic
operation/rounding combinations are covered, plus divide-by-zero, overflow,
underflow, signaling/quiet NaNs and signed zero. All four quiet predicates are
covered over signed zeros and signaling NaNs. Source conditions check result
bits and each flag. The test verifies sealed Plan identity, exact selected
configuration/profile/bounds, terminal Completed and Plan/Play correlation. Raw
logs retain the oracle rows and full selected IEEE binding alongside semantic,
Plan and Play identities. Existing exact/symbolic/uncertainty/finite-float/tape
product cases remain passing. These are development and local std CPU proofs;
they do not establish stable-main acceptance, a throughput guarantee or
installed browser/tiny-device arithmetic profiles.
