# Bounded binary32 projection (development)

This #5392 component adds integer-only, single-round binary32 conversion of a
bounded exact rational. Binary32 and binary64 share one parameterized finite
IEEE converter; the existing binary64 entrance and operational IEEE semantics
remain separate public contracts. No host floating-point instruction computes
the projected value or its error certificate.

The prepared result retains the exact source, target bits, exact represented
rational, rounding policy, inexactness, an exact-distance or conservative verified
bound, and admitted/spent limb work. Negative tolerance, unmet tolerance, work
exhaustion and unsupported finite range have distinct refusals. Nonfinite IEEE
inputs have no mathematical rational denotation. Signed zero remains a bit-carrier
fact; both zeros denote rational zero. This is not a new IEEE binary32 operational
mode and does not reinterpret existing F32 Types.

Independent SoftFloat source `a0c6494cdc11865811dec815d5c0049fba9d82a8`
produces 4296 exact binary128-to-binary32 oracle cases, including all four rounding
modes, signs, subnormal/normal boundaries, finite overflow, and an exact midpoint
plus/minus 2^-100. The midpoint case detects the wrong answer produced by a
binary64 detour. Existing binary64 conversion, operation-order and explicit IEEE
oracle tests remain regression requirements. A tracked window repeatedly performs
successful third projections and tolerance/work refusals without allocations or
reallocations. This does not measure an installed kernel or whole process.

`PreparedQuantityBinary32Projection` retains the existing Quantity Unit capsule,
family, dimension and role. Its tolerance is a difference in the exact same Unit;
an affine point is never an error distance. A coordinate conversion must be
explicitly performed and retained separately. The component proof converts 90cm
to exact 9/10m, then projects that coordinate to binary32 while preserving the
conversion's source evidence and target Unit. No Unit catalog, literal syntax,
Unit-binding authority or native Quantity codec is changed.

Both numerical and physical wrappers use the existing #4952 projection domain,
policy, facts and report API. A lossy result refuses exact consumption. A weaker
compatible bound can consume a stronger certificate; an insufficient or
physically incompatible policy yields an inspectable `Insufficient` report whose
consumption refuses. Physical validation remains an independent obligation,
including for zero numerical loss. Every consuming contract must check its actual
bound and physical facts; a static policy-class name alone does not identify all
tolerance parameters. Preservation and composition of incoming error remain
requirements for the runtime boundary.

The [source-pinned receipt](evidence/binary32-projection-development-20261018.json)
records 123 passing selected Core tests, strict all-target Clippy, compilation
without default features, and workspace formatting. Raw oracle C source, the
4296-case fixture, final logs, the corrected insufficient-report test attempt,
and compiler/library/executable hashes are retained. The conformance source is
`33d7b2959b0df9bcf0c0fc9804200468ce07313d`.

This is private development work, not #5392 completion. It does not yet install a
runtime input-consuming `math/near` Kind, add the proposed Fore qualification,
transport/revalidate its certificates between consumers, or prove composed float,
DSP, ML, browser or device behavior. The numerical law here is one explicit
conversion of an exact input, not a certificate for a larger floating-point
computation. Full workspace integration and exact-main publication remain due.
