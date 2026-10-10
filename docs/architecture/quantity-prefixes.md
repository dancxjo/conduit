# Source-authored prefixes and exact Quantities

The authoritative physical catalogue is [physical.conduit](../../architecture/core/definitions/physical.conduit).
Its quantity, unit, and prefix definitions use ordinary Conduitese parsing and
bounded admission. Generated Rust capsules must agree with that checked source.

```conduit
prefix si n = { exponent: -9 }
prefix si k = { exponent: 3 }
prefix si µ = { exponent: -6 }
prefix si u = { exponent: -6, alias: µ }
prefix binary Ki = { exponent: 10 }
unit Hz : Frequency = { reference: origin, scale: 1, prefixes: si }
```

`si` uses powers of ten; `binary` uses powers of two. A unit must explicitly
select its groups; omission disables composition. Policy masks contain only
source-admitted exponents. Thus `nV` and `nA` use `10^-9`, not `10^-6`. Aliases
select the canonical prefix capsule while retaining their source spelling.
Unicode normalization, recursive prefixes, and expected-Type guesses confer
no implicit meaning.

The unit's explicit power raises the prefix factor: `cm²` is `10^-4 m²` and
`cm³` is `10^-6 m³`. Whole declared symbols such as `m/s` admit prefixes only
through their own policy. A slash-containing suffix is recognized only when its
entire contiguous span matches the immutable registry; ordinary division and
lexical bindings retain their meaning. Prefixes never scale affine offsets.
Named independent origins cannot be converted by approximating π.

## One canonical boundary

`Unit` is the self-contained 768-byte `value/unit@1` capsule. `Quantity` uses
one 788-byte `value/quantity@1` codec, retaining its explicit role, Unit, signed
128-bit coordinate coefficient, and signed decimal exponent. Coordinate bounds
are 38 significant decimal digits and exponents -128 through +128. Decode
checks embedded family, role, transforms, policy, anchor, and content identity.

`1kHz` keeps coordinate one and Unit `kHz`; it compares physically equal to
`1000Hz`, although their representations differ. Source spelling is separate
evidence. Named quantity Types use intrinsic family-and-role leaf identities,
so runtime records and ports check the capsule. Integer consumers explicitly
select a Unit and project with checked Core helpers. Inexact precision,
incompatible families, unsupported relationships, and overflow refuse.

## Ordinary operations and explicit differences

```conduit
plot conversion {
    converted: units/convert(source = 1cm², to = mm²)
}.
plot comparison {
    compared: units/compare(left = 1000mm, right = 0.001km)
}.
plot difference {
    converted: units/convert-temperature-difference(source = TemperatureDelta(9, °F), to = K)
}.
```

The conversion yields `100mm²`; the comparison is equal. The difference is five
kelvin. Bare `9°F` remains a point and refuses the delta contract; the target
cannot reinterpret it. All three operations retain checked typed receipts.

`units/converted-equals` projects a validated conversion receipt to Boolean.
A valid refusal or different successful magnitude gives false; malformed
receipts fail admission. Its scoped `=?` alias abbreviates the same operation.

See [first-class physical values](../design/first-class-units.md) and
[declaration authoring](../physical-declarations.md). Component tests, installed
execution, Chromium, WASM, no_std, hosted acceptance, and public deployment
remain distinct proof classes.
