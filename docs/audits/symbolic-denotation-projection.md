# Exact symbolic transport and certified numeric consumers

For #5733, conformance Source `0f41e7c4021b878f4227d7bbca319543c2966300` joins the accepted finite
mathematical-value graph and enclosure implementations. It prepares exact
`π/180` and principal `sqrt(2)` using the accepted Core constructors, sends
them as `MathematicalValue` data through a checked Source expression, restores
the canonical encoding and invokes the real certified enclosure consumer.

All original bytes and semantic identity survive transport. Sixteen repeated
prepared evaluations allocate and reallocate zero times. The graphs remain
non-rational denotations; finite decimal candidate fractions have separate
identities. Different digests alone are not used as proof of mathematical
inequality. Unsupported Source equality returns the named
`UnprovedMathematicalComparison` refusal rather than fabricating a Boolean.

Numeric preparation retains the exact original value and derives an exact
rational interval inside independent established decimal brackets. Its width
is at most `1/10^16`; leaf width is at most `1/10^17`. The admitted profile has
32 constant terms, 64 root refinements, 131072-byte scratch and retained payload
ceilings, and 100000000 limb-work units. Actual usage stays inside those bounds.
Zero-width requests refuse with a named exact-constant, tolerance or finite root
refinement refusal. They never relabel a rounded result as exact.

Eight related Plot tests passed, including the two joined conformance cases.
Strict Clippy for the new test target passed with warnings denied. Production
code is unchanged. Reused accepted code is owned by
`architecture/core/src/exact_numeric/symbolic.rs` and its codec/enclosure modules,
plus `architecture/plot/src/mathematical_value.rs` and the prepared evaluator.

This proves imported finite exact mathematical data through checked Source and
native prepared/reference expressions, followed by numeric preparation. It does
not add authored compound-symbol literal syntax, dynamic symbolic arithmetic,
a new runtime, general algebraic normalization or ordinary-kernel/device proof.
The eligible fixed-width scalar paths are untouched. Parent #5392 keeps all its
broader cross-projection and runtime requirements.

Refreshed development validation at
`1d5df127e0658c0a96e21e21112b93534e035fea`, based on accepted development
`ce03de610633c26a804d42bfc14e515db94fdc09`: nine tests pass across
`symbolic_denotation_projection`, `mathematical_value_transport` and
`mathematical_value_bindings`. Strict Clippy for the joined conformance test and
pinned workspace formatting pass. The two joined test bodies remain byte-identical
to their prior validated Source. This refresh adds no production code.

A fresh rebuild at `16401cb5d3f15199add2e154d592528e368c221c`, after accepted development
`ac97b99fd9016e5879f9822432fe945ee3c500f0`, passes the same nine tests, strict focused
Clippy and pinned workspace formatting. Tracked compiler/semantic input mtimes
were refreshed without changing bytes to exclude relocated-worktree Cargo
cache reuse. No production implementation was added by this refresh.
