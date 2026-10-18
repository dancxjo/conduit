# Optional exact Quantity coordinate transport (development)

The original #5392 gate remains open: the actual Source chain `1m -> /3 -> *3`
currently refuses at the first decimal-only expression output. Core already has
an optional `RationalQuantity` numerical representation with exact coordinate,
Unit, family, dimension and point/difference role. The new component reuses that
representation for a proposed optional wire form of the same physical Quantity.
It does not introduce a unit catalog, literal syntax or Unit-binding authority.

The existing decimal `Quantity` POD, 788-byte v1 encoding, literal elaboration
and native decoder are unchanged. Optional v2 transport uses the existing
rational codec with an explicit version discriminator, complete Unit capsule,
role and reduced rational coordinate. Its finite maximum is inherited from the
reviewed 4096-bit rational extent. Decode checks the complete capsule, role,
canonical rational, encoded extent and work meter. No float conversion occurs.

`QuantityValueView` borrows the encoded coordinate and retains checked physical
metadata. Decimal inspection does not construct a rational; numerical
materialization is an explicit metered call. Owning rational validation is in a
non-inlined helper entered only for the optional rational form, keeping its
bignum scratch out of the decimal caller's frame. Both forms compare through
the existing physical Quantity laws. Representation identity can differ for
physically equal quantities, including decimal and rational coordinates in the
same Unit. Exact projection to the old decimal profile either succeeds without
changing Unit/role or returns a distinct numerical refusal. Physical family or
role incompatibility remains a physical refusal.

Thirty focused Core tests pass across six targets. Four new cases cover exact
thirds through independent encoded values, restoration to the original value,
affine points/differences, angular and binary-prefix units, malformed/truncated
capsules, invalid roles, exhausted work and output capacity. A coordinate over
500 decimal digits round-trips beyond the old decimal carrier extent. Encoding,
validation and materialization observe zero allocations/reallocations in the
tracked window; native inspection spends one validation unit and does not
materialize a bignum. Existing decimal, structured and rational Quantity tests,
strict Core lint and the no-default-features check pass.

This is codec development, not Source or kernel acceptance. The generic
primitive validator, Source literal binder, expression storage bounds and
installed Backs do not yet admit v2. Thus the failing authored chain is not
repaired by this component alone. Before activation, every affected input and
output must have its finite representation/storage/work contract checked;
decimal-only consumers must not silently receive unsupported rational values.
Preserve the existing unit-resolution boundary and static family/role checks.
Runtime exact transformation followed by an admitted consuming numerical
projection, generic projection-loss evidence, full dev integration and automated
exact-main publication remain required under the original parent scope.

## Explicit profile and physical Type admission

A separate Core profile contract distinguishes decimal-only consumers from the
optional exact coordinate profile. Exact consumers accept decimal producers;
decimal consumers cannot accept an exact producer without an explicit admitted
projection. Each profile publishes its finite maximum encoded extent. Unknown
physical Kinds and unsupported wire versions refuse before numerical decode.
The exact decoder checks generic Quantity, dimension-specific leaves and the
existing intrinsic family/role identities against the same checked Unit capsule.

Ten focused tests pass across profile admission, transport and the existing
primitive registry; strict Core lint and a no-default-features check pass. New
cases cover the producer/consumer eligibility relation, exact family/role
checking, unrelated Type refusal with zero numerical work, and decimal-only
refusal before rational validation. These are reusable admission checks, not
installed planner or wire-policy enforcement. Primitive Source validation stays
on its existing decimal contract until runtime bounds and consumer fences are
integrated. The authored failure remains open.

## Public Source controls and remaining wire gates

The public CLI now installs the existing reviewed Quantity wrapper/face catalog.
A resolved physical literal can construct a generic Quantity value without
changing its capsule. Arithmetic retains each operand's resolved physical Type
before selecting its law; generic result context cannot erase dimensions or
affine roles. A nested ratio, addition and area product checks reference/prepared
parity. Fifteen focused Plot tests and strict Plot/product lint pass.

The actual CLI native control completes and selects the existing decimal wrapper
with its 788-byte input bound. Its sealed literal preserves the exact native
Quantity encoding. The paired product target deliberately records two failing
acceptance gates: an exact third still refuses late with HostCallFailed detail
45, and an unsupported exact-to-decimal wire edge has no before-Play profile
refusal yet. That target has one passing and two failing tests. No rational
Source transport or planner enforcement is claimed. These private development
changes are not published while the scalar specialization owns the implementation
slot.
