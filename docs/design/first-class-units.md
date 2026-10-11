# First-class Units and source-authored Quantities

Development contract for [#5390](https://github.com/dancxjo/conduit/issues/5390).
Quantity families, unit definitions, point/delta relationships, and prefixes are
authoritative Conduitese. Rust supplies bounded checking and exact arithmetic.

| Expression | Type | Meaning |
| --- | --- | --- |
| `Hz` | `Unit` | Hertz |
| `kHz` | `Unit` | Kilohertz |
| `1kHz` | `Frequency` (a Quantity) | One kilohertz |
| `1000Hz` | `Frequency` (a Quantity) | One thousand hertz |
| `°C` | `Unit` | Celsius scale with its point/delta association |
| `21°C` | `Temperature` | A temperature point |
| `TemperatureDelta(21, °C)` | `TemperatureDelta` | A difference of 21 Celsius degrees |

A Unit never implicitly becomes a Quantity by multiplication by one. `Empty`
remains the separate empty product Type at `value/empty`.

## Authoritative declarations

The built-in catalogue is [physical.conduit](../../architecture/core/definitions/physical.conduit).
Generated Rust bindings must agree with full Plot parsing and admission of that
source. New families and units use the same checked path, extending an immutable
startup catalogue without editing a Rust physical catalogue.

```conduit
dimension temperature
type TemperatureDelta = quantity { dimension: temperature }
type Temperature = quantity { point: TemperatureDelta }
unit K : Temperature = {
    reference: origin,
    scale: 1,
    delta: { quantity: TemperatureDelta, reference: origin, scale: 1 },
    prefixes: si
}
unit °C : Temperature = {
    reference: K,
    scale: 1,
    offset: 273.15,
    delta: { quantity: TemperatureDelta, reference: K, scale: 1 }
}
```

These illustrate the built-in declarations; a scope cannot redeclare an imported
physical name. Exact scalar fields use ordinary decimal expressions, rational
division, or `{ numerator: 5, denominator: 9 }`. Text is not a unit language.
The law is `reference_coordinate = coordinate * scale + offset`. The delta
block must have the same exact derivative scale and reference anchor as the
point law, with zero offset. Bare `21°C` always means a point; an expected delta
Type cannot reinterpret it. Explicit constructors select the declared role.
A unit declared as `TemperatureDelta` makes its bare quantity literals deltas.

Prefix symbols and exponents are also source declarations:

```conduit
prefix si k = { exponent: 3 }
prefix si µ = { exponent: -6 }
prefix si u = { exponent: -6, alias: µ }
prefix binary Ki = { exponent: 10 }
```

Prefixes are disabled unless the unit selects `si`, `binary`, or both. Only
source-admitted exponents enter its policy. Area and volume explicitly select
`power: 2` or `power: 3`. Prefixes scale coordinates, never affine offsets.
Celsius/Fahrenheit do not acquire prefixes from their dimension or a target Type.
Admission refuses conflicting names, unknown references, cycles, unanchored
units, and repeated root origins. `origin(radian)` explicitly declares an
independent exact anchor; shared dimensions do not manufacture a rational law
involving π. See [declaration authoring](../physical-declarations.md).

## Self-contained canonical values

`Unit`, at `value/unit@1`, is a bounded 768-byte capsule containing family and
dimension facts, point/delta role names, exact scale and offset with separate
decimal exponents, prefix policy and selected prefix, declared literal role,
reference anchor, and content identities. Decode validates these facts without
looking up a mutable catalogue or interpreting a historical unit tag.

`Quantity`, at `value/quantity@1`, has one 788-byte codec: version, explicit
role, complete Unit capsule, signed decimal exponent, and signed 128-bit
coefficient. Coordinates admit 38 significant decimal digits and exponents
-128 through +128. Zero has normalized coefficient/exponent zero. `1kHz` keeps
coordinate one and Unit `kHz`; conversion to `Hz` is explicit.

A declared quantity Type uses an intrinsic leaf identity containing its family
digest and role. Runtime validation decodes the capsule and checks both.
Nominal aliases, records, and ports retain that validation; a compiler-only
annotation is insufficient. Consumers needing integers explicitly select a
Unit and use checked projections. Fractional precision, incompatible families,
unsupported exact relationships, and overflow refuse. There is no second
narrow carrier, implicit rounding, or floating-point fallback.

## Typed operations and retained decisions

```conduit
plot convert-pitch {
    operation: units/convert(source = 1kHz, to = Hz)
    exact: units/converted-equals(expected = 1000Hz)
    show: presentation/text
    operation.receipt >> exact.receipt
    exact.result >> (. ? "Exactly 1000 Hz" : "Conversion refused or differed") >> show.text
}.
```

`units/convert` takes `source: Quantity` and `to: Unit`. Its retained receipt
binds source spelling, checked capsules, exact result or typed refusal, and the
conversion law. Readmission verifies the outcome independently. The Boolean
projection requires a valid receipt; false can mean a valid refusal or a
different successful result. Malformed evidence fails admission.

`units/compare` returns `less`, `equal`, `greater`, or `refused(reason)`.
`1kHz` and `1000Hz` compare physically equal, as do `21°C` and `69.8°F`.
Physical equality, representation identity, and source spelling remain distinct.
This does not introduce a direct Boolean quantity `==` law.

```conduit
plot change {
    converted: units/convert-temperature-difference(source = TemperatureDelta(9, °F), to = K)
}.
```

The exact difference is five kelvin. Bare `9°F` supplies a point and refuses the
delta contract. Difference operations use the same Quantity codec and an
intrinsic `TemperatureDelta` profile, not a separate physical record carrier.
The scoped ordinary `=?` import abbreviates `units/converted-equals`; it adds
no private parser or delimiter family.

## Proof boundaries

Core/Plot tests, std installed execution, Chromium execution, WASM compilation,
and no_std compilation are separate proof classes. Generated artifact freshness
requires the actual language parser and admission checker. Syntax alone does
not prove physical output, public Handbook deployment, or stable acceptance.
