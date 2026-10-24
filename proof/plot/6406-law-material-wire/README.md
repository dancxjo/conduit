# Bounded untrusted law-material transport (#6406)

`NativeLawMaterial::canonical_bytes` and `from_canonical_bytes` preserve the
complete original Source/import/package-member closure, exact Type, contracts,
and compiled predicate bytes. The versioned transport uses length-delimited
fields with a conservative 4 MiB total wire cap. Existing custody bounds remain
unchanged. The decoder scans the complete framing, document counts and depth
before allocating its decoded graph, then uses existing strict Type/contract/
program decoders and requires exact canonical re-encoding.

A decoded material is an untrusted public claim. Only the existing Source replay
factory can mint `NativeLawWitness`, with an independently known expected Type
and independently trusted family anchors. Those anchors are absent from the
wire format. A well-framed omitted law and a foreign package anchor still refuse
verification; transporting a claim does not authorize its laws.

Formatting and strict owning Plot/xtask checks pass. All five owning tests pass, including real imported and generic-package closure
round trips, complete predicate correspondence, every truncated prefix, trailing
and unknown bytes, excessive length, raw depth and aggregate document pressure.
The supported native-law-custody entrance includes this target. The prior shared
registry supported run separately passed 506 tests with one existing ignored
case; it predates this codec and is not a full-suite claim for this source.

This is no verified expression envelope/Kind, executed-node guard or selected
Std Back. #6406 remains open for those implementations, all four automatic G2
kernel journeys and its relevant Candidate/Integration gates.
