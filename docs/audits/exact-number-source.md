# Exact finite numbers through checked Source

Issue #5530 uses the existing `MathematicalValue` carrier and exact rational
implementation. A rational has finite 4096-bit numerator and denominator
storage. Each checked `+`, `-`, `*`, or `/` operation spends at most
60,000,000 limb-work units; exceeding work or extent refuses evaluation.
Division by zero and symbolic operands also refuse. Named/algebraic symbolic
arithmetic remains owned by #5733.

Prepared evaluation allocates a fixed result buffer for each rational arithmetic
node during preparation. Each buffer is `EXACT_RATIONAL_MAX_ENCODED_BYTES + 3`
bytes; expression depth and node count retain the existing program bounds.
Fixed-width arithmetic uses its existing inline result path. Evaluation grows
neither path.

`architecture/plot/tests/mathematical_value_transport.rs` checks authored
`10^1000 + 1` (written as its decimal literal plus one), exact thirds, all four
rational operators, and canonical transport through typed pack/relay/unpack
expressions. Both evaluators agree byte-for-byte. Repeated prepared evaluation
and refusal cases count allocations and reallocations. The thousand-digit
result refuses `OutwardI64` realization and admits `ExactRational`; zero division,
4096-bit extent overflow, and symbolic operands refuse without output.

These are checked expression and canonical transport proofs. They do not claim
physical target execution, unbounded integers, symbolic arithmetic, or numerical
performance improvements. The full numerical parent #5392 remains separate.
