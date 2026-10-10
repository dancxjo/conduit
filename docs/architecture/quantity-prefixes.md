# Decimal prefixes and exact Quantity values

The immutable `quantity/decimal-prefix-catalog@1` records all 24 official SI
prefixes and reviewed composition positions. The whole-suffix resolver uses
that catalogue to establish exact semantic scale. The
[first-class Unit and Quantity design](../design/first-class-units.md) owns
ordinary authoring and the canonical value boundary for #5390, building on
#5328's reviewed suffix and arithmetic foundation.

## Reviewed composition law

The authoritative symbols and exponents come from the
[BIPM SI prefix table](https://www.bipm.org/en/measurement-units/si-prefixes).
Symbols match exact UTF-8 bytes. Canonical micro is U+00B5 `µ`. ASCII `u` and
powered `m2`, `m3`, `m/s2` are explicit source aliases. Greek U+03BC `μ`,
whitespace, zero-width characters and Unicode compatibility folding are not
admitted. Aliases retain their original bytes; spans belong to checked source.

Every official prefix is permitted at each position below. Prefix exponent `e`
is composed with the base's reviewed power:

| Reviewed suffix | Prefix position | Exponent relative to base |
| --- | --- | --- |
| `s`, `Hz`, `V`, `A`, `K`, `g`, `m`, `rad`, `N`, `J`, `W`, `Pa` | Before complete symbol | e |
| `L`, `B` | Before approved non-SI base | e |
| `m²` | Before powered meter | 2e |
| `m³` | Before powered meter | 3e |
| `m/s`, `m/s²` | Before numerator meter only | e |
| `Ah` | Before ampere only | e |

Gram is the mass base: `kg` is kilo + gram; kilogram cannot be prefixed again.
Byte prefixes are decimal; `MiB` has its separately reviewed binary meaning.
Minute, hour, year, degree, pixel, percent, `one` and other named units retain
their reviewed transforms without generalized prefix composition. Recursive
prefixes, arbitrary unit algebra and expected-Type guesses are unavailable.
The largest reviewed composed exponent is 90 for cubic quetta meters.

Celsius and Fahrenheit do not admit generalized SI prefixes. Reviewed `m°C`
has scale 1 and offset 273150 in millikelvin reference units; `°C` has scale 1000
and the same offset. Temperature differences have zero offset through their
explicitly distinct semantic role. A prefix never scales an affine offset.

## One canonical value boundary

Public `Unit` is `value/unit@1`, with a three-byte encoding: version, reviewed
base index and signed decimal-prefix exponent. The version pins the reviewed
unit/prefix catalogue. Dimension and exact scale, denominator and affine offset
are canonically resolved from those finite fields. Decode checks the base,
prefix position, version and canonical descriptor. The internal `CatalogUnit`
enumeration is catalogue metadata, not an authored Type or primitive value.
`Empty` is the distinct empty product Type at `value/empty`.

Public `Quantity` is `value/quantity@1`, with a 22-byte encoding: version, complete
three-byte Unit, signed 16-bit coordinate exponent and signed 128-bit
coefficient. It admits 38 significant decimal digits and coordinate exponents
from -128 through +128. Authored input is bounded to 128 bytes, including at
most 96 numeric bytes. Parsing and normalization are bounded; normalized zero
has coefficient/exponent zero. Decoding refuses noncanonical numeric fields.
Scientific exponent notation is refused rather than confused with a prefix.

The coordinate is `coefficient * 10^exponent` in the retained Unit. Thus `1kHz`
retains coordinate one and Unit `kHz`, while `1000Hz` retains Unit `Hz`. Their
physical equality does not imply representation-byte equality. Source spelling
is separately retained evidence; preparation uses checked canonical values.

Dimension-specific consumer Types use the same Quantity codec with checked
dimensions. Where a domain needs a finite integer, it explicitly selects the
Unit and projects with `to_i64` or `convert_to_u64`. Fractional coordinates,
incompatible dimensions and integer range overflow refuse. No second narrow
quantity family, implicit rounding or floating-point fallback exists.

## Exact conversion and comparison

```conduit
plot conversion (
    receipt: ExactQuantityConversionReceipt <= 8192B >>
) {
    converted: units/convert(source = 1cm², to = mm²)
    converted.receipt >> receipt
}.
```

The required startup Fores are `source: Quantity` and `to: Unit`. Both preserve
bounded authored evidence separately. The receipt retains checked source,
resolved descriptors, source/target dimensions, exact base reference
scale/offset/denominator, source and target prefix exponents, catalogue/profile,
and either an exact target coordinate or a typed refusal.

The physical equation is
`(coefficient * 10^(coordinate_exponent + unit_decimal_exponent) * base_scale + offset) / denominator`.
Target prefix scaling acts on the coordinate after the affine reference law.
A returned target coordinate stays paired with its complete Unit descriptor.
Readmission checks all retained facts and evidence, including modified results,
source descriptors, transforms, catalogue, profile and refusal reasons.

```conduit
plot compare (
    receipt: ExactQuantityComparisonReceipt <= 8192B >>
) {
    compared: units/compare(left = 1000mm, right = 0.001km)
    compared.receipt >> receipt
}.
```

The result is `equal`: both mean one metre. Comparison uses a common exact
rational reference, not a rounded coordinate selected from either operand.
Typed `less`, `equal`, `greater` and `refused` remain distinct. Incompatible
dimensions, unsupported exact relationships and bounded-profile failures do not
become ordinary inequality. Direct quantity `==` is not a newly introduced
Boolean physical-equality law.

Conversion arithmetic uses fixed-capacity intermediate storage. Decimal target
admission rejects a remaining denominator factor other than 2 or 5, and checks
finite coefficient/exponent bounds. The independent prefix, affine-temperature
and comparison fixtures remain distinct from the implementation's transforms.

## Explicit temperature differences

```conduit
plot difference (
    receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>
) {
    converted: units/convert-temperature-difference(source = 9°F, to = K)
    converted.receipt >> receipt
}.
```

This source Fore selects the distinct `TemperatureDifference` Type; the exact
result is `5K`. The absolute `units/convert` Fore with the same source spelling
selects a temperature point and applies its origin. A difference of `1m°C` is
`0.001K`; an absolute `1m°C` point is `273.151K`.

The difference Type is a checked record around the canonical Quantity coordinate,
with a shape-derived executable profile distinct from the point profile. Raw
coordinate bytes alone do not carry difference meaning. Its transforms have
zero offsets; its target coordinate, result and receipt Types remain distinct.
`units/compare-temperature-differences` takes two checked differences.
Already-bound point and difference values cannot be substituted for each other.

## Boolean receipt projection and glyphs

`units/converted-equals(expected = 1000Hz)` consumes a runtime exact conversion
receipt. It returns true for a validated successful conversion physically equal
to expected. A valid refusal, different magnitude or incompatible expected
dimension returns false. Malformed or forged receipts fail admission. This
Boolean decision does not replace the original retained receipt/refusal.

The full Kind name is the universal entrance. The scoped ordinary import
`with units/converted-equals as =?` abbreviates that same checked operation;
`exact: =?(expected = 1000Hz)` preserves its Fore, configuration and ports.
No new delimiter family or private unit parser is introduced.

## Proof boundaries

Core and Plot component conformance, installed std kernel execution, browser
execution, WASM compilation and no_std compilation prove different things.
The source model and deterministic fixtures do not alone prove an installed
Back, physical output, public Handbook deployment or stable acceptance.
The #5390 implementation report must identify its current local and hosted
results separately; #5328's foundation evidence is not proof of the changed
canonical codecs or typed Gear boundary.
