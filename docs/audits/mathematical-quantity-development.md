# Mathematical Quantity coordinates (private Core prototype)

The existing exact Quantity port profile admits decimal v1 and rational v2
coordinates. The Source symbolic pipeline instead carries dimensionless
`MathematicalValue`; it cannot by itself prove preservation of physical meaning.
This private continuation under #5392 introduces a proposed optional v3 Quantity
coordinate form containing the existing checked Unit and role plus canonical
mathematical data. It creates no new Unit catalog, physical Type, literal syntax,
binding authority or numerical evaluator.

`MathematicalQuantityView` borrows its bytes and delegates the coordinate codec
to the existing 32-node/depth-16 mathematical-value validator. Its finite extent
is `2 + UNIT_ENCODED_LEN + MATHEMATICAL_VALUE_MAX_ENCODED_BYTES`. Unit definition
validation and role admission reuse the existing physical law. The declared
family/role/dimension guard is shared with v1/v2 without changing that contract.
Representation identity includes the complete Unit, role and coordinate bytes;
it is not physical equality across different coordinate conventions. A known
rational coordinate can be explicitly materialized as the existing
`RationalQuantity`. An unresolved symbolic coordinate refuses that operation;
it is never silently rounded or stripped of its Unit.

The encoder accepts an already validated mathematical view, checks role, extent,
output capacity and metered copy work before mutating output. Source spelling,
conversion receipts and definition/binding authority remain independent custody;
this form alone does not attest any of them. Neither the legacy
`QuantityValueView`, default primitive validator, nor decimal/exact port profile
accepts v3. An explicit optional `Mathematical` port profile now negotiates v1/v2/v3;
Decimal and Exact retain v1 and v1/v2 respectively. The existing canonical wire
property uses code 3 for the new grant and keeps omitted ports Decimal. Ordinary
`QuantityValueView::decode` still rejects v3; selected profile decoding alone
returns a borrowed mathematical-coordinate view. Unresolved coordinates refuse
rational or decimal materialization. No Source/Plan offer or installed Back
advertises this profile yet. An unfamiliar form remains a named refusal.

## Unit coordinates and numerical realization remain separate

The Core conformance case checks `0.25turn = 90°` with the existing rational
physical conversion law. Each input then uses the existing checked
`PreparedAngleReferenceConversion` and `physical/angle-turn-radian-2pi@1` law to
produce an exact Pi-bearing radian coordinate. Original spellings and original
Unit identities stay in those receipts. The two equal physical inputs produce
the same proposed radian-coordinate form without inventing rational Pi.

`PreparedMathematicalQuantityEnclosure` then uses the existing borrowed interval
engine to realize those coordinates. It retains the complete physical source
and returns rational endpoint Quantities in the same Unit and role. Numerical
realization has no target-Unit argument and cannot silently select another
coordinate convention. The width is explicitly a coordinate coefficient,
not an affine physical point. A separate point/difference test retains
Fahrenheit coordinates during numerical realization and applies the existing
affine offset only during a later point-to-Kelvin coordinate conversion.

All scratch is prepared before evaluation. Container/vector payload has a
checked pre-allocation lower bound and an actual-capacity check; allocator
bookkeeping, stack, caller-owned input and returned endpoints are excluded.
Frame decoding and coordinate evaluation share a ceiling that a larger caller
grant cannot loosen. Numeric limb-work accounting is not a CPU-time or WCET
claim; Unit decoding is an independently fixed-size operation. Refusals charge
the work already spent and return no substituted value.

## Physical projection evidence

The coordinate enclosure now adapts to the existing generic `ProjectionReport`
through the same physical-obligation pattern as Quantity integer enclosures.
It delegates numerical obligations and width loss to the existing borrowed
interval domain, adding preservation of the complete Unit and role. The target
contains coordinate bounds in that retained frame. A different Unit, point vs
difference role, or bound fails validation; numerical tolerance cannot authorize
those substitutions. Omitted physical obligations and false exactness also
refuse. Exact rational point intervals may be exact, while nonzero symbolic
width remains reported as precision loss under the selected policy.

The numerical policy fixes coordinate-width requirements in that Unit. It is
not a physical point tolerance, general Unit conversion, Source provenance
attestation or installed projection custody. Source bytes remain available on
the checked enclosure; binding them to Source/Plan/Sign remains separate work.

## Evidence and remaining gates

Guarded tests cover the two angle conversions followed by coordinate enclosure,
physical family/role preservation, strict legacy rejection, malformed/oversized
frames, invalid roles, short-output and exhausted-copy-work atomicity, prepared
storage/work limits, and recovery. Point and difference roles remain distinct,
including under later affine conversion. Tests compare against independent
radian brackets and retain zero observed execution allocations and stable
prepared capacity. Existing v1/v2 role, rational transport and wire-contract
cases remain in the selected regression suite. The checked-contract entrance is
also exercised with an authored physical range: v1 retains its in/out-of-range
outcomes, while both in-range rational v3 and symbolic v3 refuse under every
existing profile. Codec validity alone cannot bypass a range or grant a profile.

The first compile exposed a variable-shadowing mistake during extraction of the
shared role guard. It was corrected before the regression suite, which explicitly
checks wrong-role rejection in both legacy and proposed forms. That diagnostic
is retained with the local capture.

This is private Core conversion/computation proof. Required follow-up remains:
reviewing the form and its explicit wire-profile negotiation, checked Source/Plan and
installed Host consumption, refinement/terminal/control proof, exact symbolic
Angle consumption by trig, Source/Plan/Sign custody for the general projection report, fresh candidate
and combined integration/publication. Qualified whole-float algorithms and all
original parent requirements remain authoritative. No checkbox is closed here.

The selected mathematical profile validates the same physical Kind before
applying authored checked-contract predicates. A representation grant does not
implement symbolic ordered comparison: current physical range predicates still
refuse v3, including rational-shaped v3, until that comparison path is reviewed.
This limitation is tested separately from successful unconstrained admission.
The extended view retains borrowed bytes rather than an owning symbolic DAG;
its fixed size stays within the existing Unit plus 64-byte envelope.
