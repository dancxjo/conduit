# Source-authored principal square roots (private development)

Under #5392, `math/sqrt(value)` constructs an exact principal-root denotation
from a checked dimensionless `MathematicalValue`. It uses the existing
`PrincipalSquareRoot` node, canonical normalization, 32-node/depth-16 finite
profile and exact rational codec. It does not approximate a root, introduce
another literal grammar, or make Scalar into Angle or a physical Quantity.
The spelling follows the existing `math/*` semantic expression-call surface;
#4109 points to the checked-in Wiki as language authority. This private spelling
and implementation still require ordinary review and admission.

This profile supports rational radicands, including dynamically supplied ones.
The existing builder proves nonnegativity, preserves nontrivial roots as graphs,
and normalizes zero and one. Negative dynamic inputs retain
`NegativeRadicand`; unsupported symbolic radicands retain
`UnsupportedDomainProof`. Host pure-expression failures preserve distinct
reserved details 33 and 34, with work refusal 35 and other root encoding or
resource refusal 36. Existing details remain unchanged. A closed negative
literal is already refused by the existing unary expression checker; no general
unary-MathematicalValue language change is made here.

Reference evaluation may allocate. Prepared evaluation owns the builder's five
buffers and the result slot before Play. Its builder payload ceiling is 131072
bytes, its encoded ceiling is the existing mathematical-value maximum, and one
writer call has a finite 60-million limb-work ceiling. Input decoding, builder
construction and encoding use the same existing metered implementation.
Literal results are constructed during preparation. Dynamic evaluation resets
only admitted storage. These component ceilings do not claim a new complete
Host memory or timing accounting law.

## Evidence

The focused `square_root_denotation` tests author `math/sqrt(2)` through ordinary
parse/check/expand and compare the canonical program's reference and prepared
outputs with the existing Core builder. A dynamic source is exercised 32 times
each for 0, 1, 2, 3 and 81, with zero observed allocations/reallocations and
unchanged output capacity. Wrong dimensional/IEEE input Types and wrong arity
refuse. Dynamic negative and unsupported symbolic inputs refuse without guarded
allocation, followed by successful valid-input recovery.

The actual CLI corpus is extended with a Source-authored root fixture feeding
`math/enclose`, inspecting retained source/request identity, and showing the
result through `face/text`. It also requires zero-width symbolic refusal without
success output. Until that extended CLI test has executed, these are authored
proof cases, not execution evidence.

This is private implementation and component proof, not public Candidate,
combined Integration or stable acceptance. Installed browser execution, firmware,
symbolic Quantity transport, independent physical Unit/numeric projections,
qualified floating-point composites and the original parent requirements remain
separate open obligations. No parent checkbox is changed by this work.
