# First-class Units and exact Quantities

Development contract for [#5390](https://github.com/dancxjo/conduit/issues/5390),
built on the reviewed suffix catalogue and exact arithmetic from
[#5328](https://github.com/dancxjo/conduit/issues/5328).

A physical unit and a physical quantity are ordinary checked values. Native
spellings need neither imports nor quotation marks:

| Expression | Type | Meaning |
| --- | --- | --- |
| `Hz` | `Unit` | Hertz |
| `kHz` | `Unit` | Kilohertz |
| `1kHz` | `Quantity` | One kilohertz |
| `1000Hz` | `Quantity` | One thousand hertz |
| `°C` | `Unit` | Celsius temperature scale |
| `21°C` | `Quantity` | Absolute temperature point at 21 degrees Celsius |

A Unit is a catalogue descriptor, not a quantity of magnitude one. It never
implicitly becomes a dimensionless Quantity. The empty product value
uses the authored Type `Empty`; its `value/empty` identity has a zero-byte codec.

## Representations and bounded admission

The authored `Unit` Type is `value/unit@1`, with the Core Rust carrier
`Unit`. Its three-byte codec contains version 1, a reviewed base-unit
tag, and a signed decimal-prefix exponent. Version 1 pins the reviewed unit table
and `quantity/decimal-prefix-catalog@1`. Decoding rejects unknown versions,
noncanonical already-prefixed base indices, unreviewed prefixes, and prefixes
on bases without an admitted composition position. A descriptor resolves the
physical dimension and exact reference scale, denominator and affine offset.
The decimal exponent stays symbolic: cubic quetta metres require exponent 90,
not an unbounded integer allocation.

`Quantity` is the sole physical quantity Type, checked as `value/quantity@1`
with the Core Rust carrier `Quantity`. Its 22-byte codec stores version 1, the
full three-byte Unit descriptor, a signed 16-bit decimal exponent and a signed
128-bit coefficient. Bounds are 38 significant decimal digits, coordinate
exponents from -128 through +128, 128 authored literal bytes and 96 numeric
bytes. Prefix descriptors remain part of the value: `1kHz` has coordinate one
and Unit `kHz`; conversion to `Hz` selects that target descriptor explicitly.
A zero coordinate has normalized coefficient/exponent zero. Decoding refuses
noncanonical numeric and Unit encodings.

There is no second narrow Quantity family or separately authored base-unit
codec. The reviewed `CatalogUnit` enumeration identifies internal catalogue
entries; it is not an authored Type or primitive Info profile. Dimension-specific
consumer Types such as `Distance`, `Frequency`, `Duration` and `Temperature`
use the same canonical Quantity codec with checked dimensions. A domain that
needs an integer frequency, duration, pixel count or device coordinate performs
an explicit checked projection in its selected Unit. `Quantity::to_i64(unit)`
and `Quantity::convert_to_u64(unit)` refuse fractional precision, incompatible
dimensions and integer overflow. No floating-point fallback or silent narrowing
is permitted.

Both public Types work through startup parameters, parameter forwarding,
structured fields and exact runtime ports. Their ordinary value contracts admit
finite bytes before Play. Native Rust bindings use the same checked codecs.

## Resolution, collisions and source custody

Resolution uses the existing whole-suffix catalogue. It never guesses a unit
from an expected Type, applies Unicode normalization, stacks prefixes or adds
arbitrary unit algebra. Symbols are case-sensitive. Canonical micro is `µ`;
ASCII `u` and reviewed ASCII powers remain explicit source aliases. Greek `μ`
and `21C` refuse.

A bound local, reusable parameter or runtime name takes precedence over an
otherwise valid bare unit spelling. Thus a local called `Hz` keeps its declared
meaning; a Quantity expected downstream cannot silently reinterpret it as a
Unit. An unbound reviewed symbol resolves as Unit. Unknown or ambiguous symbols
remain source diagnostics rather than acquiring meaning from the destination.

Checked startup wrappers retain canonical bytes and bounded authored spelling
separately. The checked source retains the original document and UTF-8 spans.
Preparation computes from the checked Quantity and Unit; spelling is correlated
evidence. Alias normalization, source custody and semantic identity are distinct.
Receipt readmission verifies evidence against typed facts, then verifies all
retained descriptors, exact transforms, result coordinates and refusal reasons.
Changing only a receipt result or descriptor cannot produce a valid receipt.

## Ordinary conversion and comparison Fores

```conduit
plot convert-pitch (
    receipt: ExactQuantityConversionReceipt <= 8192B >>
) {
    converted: units/convert(source = 1kHz, to = Hz)
    converted.receipt >> receipt
}.
```

`units/convert` takes `source: Quantity` and `to: Unit`. Its bounded receipt
retains the original spelling, checked source, resolved units, dimensions,
exact reference transforms, catalogue/profile and either an exact selected-target
coordinate or a typed refusal. A target coordinate stays paired with its Unit;
it cannot masquerade as a quantity in the unprefixed base unit.

`units/compare(left: Quantity, right: Quantity)` provides physical comparison
without unpacking coefficient and exponent fields. `1kHz` and `1000Hz` compare
physically equal; so do `21°C` and `69.8°F`. Its retained outcome is `less`,
`equal`, `greater` or `refused(reason)`. Incompatible dimensions, unsupported
exact relationships and bounded-profile failures are refusals, not inequality.
This contract does not introduce a new direct `==` Boolean law for quantities.

Physical equality differs from representation identity. Two normalized values
can use different reviewed units and still name the same physical point.
Canonical codec bytes and semantic digests identify their representations;
authored spelling identifies source evidence. Use the checked comparison Fore
when the question is physical equality.

## Temperature points and differences

`TemperatureDifference` is a distinct structured Type containing an exact
coordinate. It uses the existing difference law and cannot connect to an
absolute Quantity port. Its checked configuration identity is the record's
shape-derived profile, not its schema name or the raw coordinate leaf identity.

```conduit
plot change (
    receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>
) {
    converted: units/convert-temperature-difference(source = 9°F, to = K)
    converted.receipt >> receipt
}.
```

Here the source Fore explicitly admits a difference of nine Fahrenheit degrees,
which converts to `5K`. `units/convert(source = 9°F, to = K)` means an absolute
point and applies the temperature origin instead. The corresponding
`units/compare-temperature-differences` Fore takes two checked differences.
A previously bound absolute Quantity cannot become a difference through an
expected output Type. Prefixes scale coordinates; they never scale an affine
offset. Generalized SI prefixes on Celsius/Fahrenheit remain unavailable;
reviewed historical milli spellings retain their existing law.

## Retained receipt and explicit Boolean projection

The ordinary comparator takes configured `expected: Quantity`, runtime
`receipt: ExactQuantityConversionReceipt`, and emits `result: Boolean`:

```conduit
plot convert-pitch-demo {
    operation: units/convert(source = 1kHz, to = Hz)
    exact: units/converted-equals(expected = 1000Hz)
    show: presentation/text

    operation.receipt >> exact.receipt
    exact.result
        >> (. ? "Exactly 1000 Hz"
               : "Conversion did not yield exactly 1000 Hz")
        >> show.text
}.
```

True means a validated successful conversion physically equals expected. A
successful different magnitude, an incompatible expected dimension, or a valid
refused receipt yields false. A malformed or forged receipt fails admission;
other bounded comparison failures remain typed errors. Boolean is an explicit
projection for this decision, not a replacement for the original receipt and
its refusal evidence.

After the full-name Fore, an optional plot-scoped ordinary alias abbreviates the
same operation:

```conduit
with units/converted-equals as =?
plot convert-pitch-demo {
    operation: units/convert(source = 1kHz, to = Hz)
    exact: =?(expected = 1000Hz)
    show: presentation/text
    operation.receipt >> exact.receipt
    exact.result >> (. ? "Exactly 1000 Hz" : "Conversion did not yield exactly 1000 Hz") >> show.text
}.
```

The import is required in that scope. `=?` uses ordinary Gear alias checking and
the same Fore, configuration and ports. Assignment, equality and ternary syntax
retain their own tokenization. Existing Gear aliases remain available.

## Design rules and implementation ownership

Use an existing checked Type when input has structured meaning. Text is suitable
for text, not a substitute for a resolved Unit or exact Quantity. Add reusable
operations through ordinary Fores first; glyphs abbreviate checked operations
rather than adding a private parser or a new delimiter family.

| Module | Responsibility |
| --- | --- |
| Core `unit.rs` | Catalogue-pinned Unit representation and validation |
| Core `quantity_configuration.rs` | Checked startup values and correlated source evidence |
| Plot `authored_quantity.rs` | Ordinary physical value authoring and selected-Type admission |
| Plot `quantity_conversion/typed_arguments.rs` | Typed Fore/configuration contract and receipt source readmission |
| Plot `quantity_conversion/converted_equals.rs` and `converted_equals/validation.rs` | Reusable Boolean projection with borrowed semantic receipt validation |

Validation of this development change must report Core/Plot component tests,
std installed-kernel execution, browser/WASM execution or compilation, and
no_std compilation separately. Hosted checks, stable acceptance, public Handbook
deployment and physical execution require their own evidence; documented source
syntax alone establishes none of those proof classes.
