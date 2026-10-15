# Typed Angle trigonometry development proof

Issue #5532 introduces the reviewed `math/sin`, `math/cos`, and `math/tan`
expression calls. Each accepts Angle and returns MathematicalValue. Bare
scalars, distances and frequencies fail checking; no implicit radians are
introduced. Existing Angle units provide the exact turn coordinate.

Quarter turns reduce to the exact rational identities 0, 1 and -1. Tangent
at odd quarter turns refuses with TrigonometricPole. Known constant domain
failures refuse during preparation; dynamic failures retain the same typed
reason through the installed host boundary.

Other admitted angles retain exact sine/cosine/tangent turn denotations in the
bounded mathematical graph. They do not receive an invented floating-point
approximation: the existing numerical enclosure entrance explicitly refuses
unsupported trigonometric enclosures. Exact turn-to-radian pi transport and
inverse functions remain separate work in #5735 and #5734. Parent #5392 remains
open.

The prepared evaluator owns finite output storage before Play. Conformance
checks reference/prepared byte parity, zero evaluation allocations and
reallocations, stable capacity, canonical encoding and hostile decoding.
The installed std-host test executes actual checked Source through ordinary
verified Plan/Play and Host Calls. Its observation carries the original Angle
alongside the mathematical result; existing unit-coordinate and certified
numeric projections are inspected separately. This proves native hosted
execution, not browser/WASM or physical-device execution.

Proof targets are Core mathematical_trigonometry, Plot angle_trigonometry,
and std-host angle_trigonometry. Final source identities and command outcomes
are recorded separately after validation. Earlier numerical transport and
quantity arithmetic regressions are also retained in the local evidence.

The generic semantic-kind conformance fixture uses `fixture/pure`, avoiding the
canonical `math/sin` name now owned by the Angle contract. Its exact-front and
effectful-kind refusal checks remain intact. All 12 generic-expression tests and
six Angle tests pass after that fixture correction, with strict focused Plot
Clippy and formatting. Runtime code and all original source-bound proof files
remain unchanged; the additional evidence retains the new raw log hashes.
