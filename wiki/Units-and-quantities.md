# Units and quantities by example

Write the quantity you mean: `250ms`, `440Hz`, `3.2m`, `21°C`.
The suffix carries a checked dimension. A duration, frequency and distance
remain different even when their representations happen to have the same size.

`Unit` is a physical catalogue descriptor, distinct from a quantity of magnitude
one and from `Empty`, the historical empty product value. `Quantity` uses the
bounded exact decimal profile carrying its full Unit. Dimension-specific
consumers such as `Distance` and `Frequency` use that same codec with checked
dimensions. A domain needing an integer performs an explicit checked projection
in its selected Unit, refusing fractional precision or overflow.

These examples describe current development source for [first-class values #5390](https://github.com/dancxjo/conduit/issues/5390),
built on completed [SI work #5328](https://github.com/dancxjo/conduit/issues/5328).
Published binaries can lag development. The complete conversion plots below
use the installed product catalog; save one as `example.conduit` and check it:

```sh
conduit check example.conduit
conduit expand example.conduit
```

Checking proves authoring and admission; it does not execute the plot.
The [quantity conformance sources](https://github.com/dancxjo/conduit/tree/dev/architecture/plot/tests)
and [product tests](https://github.com/dancxjo/conduit/blob/dev/products/conduit/tests/quantity_conversion.rs)
exercise the examples and their refusal cases.

## A quantity has a dimension

```conduit
plot beyond-reach (
    limit: Distance = 30cm
    >> distance: Distance
    beyond: Boolean >>
) = (. > limit)
```

This complete Plot compares distance with a typed distance threshold. Replacing
the default `30cm` with `440Hz` refuses checking of the `Distance` parameter. Changing it to `0.3m`
preserves the physical threshold. Units give the comparison its meaning.

## A distance becomes a musical pitch

The [Pocket Theremin](https://github.com/dancxjo/conduit/blob/dev/plots/pocket-theremin/main.conduit)
illustrates mapping one dimension into another explicitly. This domain example
requires its reviewed catalog; it is not a standalone installed-CLI example.
The invocation below uses the current single-line argument layout:

```conduit
plot pocket-theremin (
    >> distance: Distance
    audio: audio/pcm-frames@1...| >>
) {
    map: math/map-distance-frequency(source-minimum = 0cm, source-maximum = 30cm, target-minimum = 220Hz, target-maximum = 880Hz)
    frequency: keep Frequency(440Hz) for this play
    tone: audio/tone

    distance >> map.distance
    map.frequency >> frequency
    frequency >> tone.frequency
    tone.audio >> audio
}.
```

Moving within a 30-centimetre range controls a 220–880 Hz tone. `keep` retains
the current pitch, beginning at 440 Hz. The map's checked Fore explicitly
relates distance and frequency; unit conversion alone cannot turn metres into
hertz. Audio-device selection belongs to realization.

## Convert explicitly and retain the result

```conduit
plot convert-pitch (
    receipt: ExactQuantityConversionReceipt <= 8192B >>
) {
    converted: units/convert(source = 1kHz, to = Hz)
    converted.receipt >> receipt
}.
```

The result is exactly **1000 Hz**. `source: Quantity` and `to: Unit` are
ordinary checked startup values. Bare `Hz`, `kHz` and `°C` name Units; `1kHz`
and `21°C` name Quantities. Neither needs notation imports or quotation marks. The receipt retains original
spelling, resolved units and dimensions, prefix, conversion law, selected
profile, and either the exact coordinate or a typed refusal.

Try these replacements in the same plot:

| Source | Target | Exact result |
|---|---|---|
| `1µs` | `ns` | 1000 ns |
| `1cm²` | `mm²` | 100 mm² |
| `0°C` | `K` | 273.15 K |
| `30°C` | `°F` | 86 °F |
| `1MB` | `B` | 1,000,000 B |
| `1MiB` | `B` | 1,048,576 B |
| `1mg` | `g` | 0.001 g |
| `1kg` | `g` | 1000 g |
| `1km/h` | `m/s` | `inexact` refusal; no silent rounding |

Powered units apply the prefix's power too: a centimetre squared is
10⁻⁴ square metres. The reviewed compound suffix `m/s²` is supported;
this does not introduce arbitrary source algebra such as `kg*m/s^2`.

## Compare compatible dimensions exactly

```conduit
plot compare-distances (
    receipt: ExactQuantityComparisonReceipt <= 8192B >>
) {
    compared: units/compare(left = 1000mm, right = 0.001km)
    compared.receipt >> receipt
}.
```

The comparison is `equal`: both mean one metre. No rounded display coordinate
is needed to establish equality. The receipt retains both operands and their
exact reference transforms. The same Gear compares `1°F` and `0°C`
exactly even though converting 1 °F to a finite decimal °C coordinate is inexact.

## Turn a retained conversion receipt into a decision

```conduit
plot convert-pitch-demo {
    operation: units/convert(source = 1kHz, to = Hz)
    exact: units/converted-equals(expected = 1000Hz)
    show: presentation/text
    operation.receipt >> exact.receipt
    exact.result >> (. ? "Exactly 1000 Hz" : "Conversion did not yield exactly 1000 Hz") >> show.text
}.
```

This full-name invocation validates the receipt and returns true for the exact
conversion above. A different magnitude, incompatible expected dimension, or
valid refusal returns false. Forged receipts fail admission. The Boolean is a
chosen projection; retain the receipt when the exact outcome or refusal matters.
Physical comparison uses `units/compare`, not representation-byte equality.
For example, `21°C` and `69.8°F` compare as equal absolute points.

To abbreviate the same checked Gear in this plot, import
`with units/converted-equals as =?` before its declaration and replace the
configured invocation with `exact: =?(expected = 1000Hz)`. The alias is scoped
to its import and retains the full-name operation's Type checking and ports.

## A temperature point differs from a temperature change

`0°C` as a temperature point equals 273.15 K. A change of 0 °C has no
273.15 K offset. Choose the ordinary Gear whose source Fore states which meaning you want.
`TemperatureDifference` is distinct from absolute `Quantity`; forwarding an
already checked point into a difference parameter refuses:

```conduit
plot temperature-change (
    receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>
) {
    converted: units/convert-temperature-difference(source = 9°F, to = K)
    converted.receipt >> receipt
}.
```

This change is exactly **5 K**. For `1m°C` to `K`, the difference is
0.001 K. Difference conversion scales the change without applying an absolute
temperature offset.

```conduit
plot compare-temperature-changes (
    receipt: ExactTemperatureDifferenceComparisonReceipt <= 8192B >>
) {
    compared: units/compare-temperature-differences(left = 9°F, right = 5K)
    compared.receipt >> receipt
}.
```

Its result is `equal`. These contracts prevent a temperature change from being
silently treated as an absolute temperature point.

## All 24 decimal SI prefixes

Each row can be used as `source` in `units/convert`, with `to = m`.
The exact conversion profile preserves a finite coefficient and exponent;
these examples do not promise that every integer realization can store it.

| Prefix | Example | Metres |
|---|---|---|
| quetta `Q` | `1Qm` | 10³⁰ |
| ronna `R` | `1Rm` | 10²⁷ |
| yotta `Y` | `1Ym` | 10²⁴ |
| zetta `Z` | `1Zm` | 10²¹ |
| exa `E` | `1Em` | 10¹⁸ |
| peta `P` | `1Pm` | 10¹⁵ |
| tera `T` | `1Tm` | 10¹² |
| giga `G` | `1Gm` | 10⁹ |
| mega `M` | `1Mm` | 10⁶ |
| kilo `k` | `1km` | 10³ |
| hecto `h` | `1hm` | 10² |
| deca `da` | `1dam` | 10¹ |
| deci `d` | `1dm` | 10⁻¹ |
| centi `c` | `1cm` | 10⁻² |
| milli `m` | `1mm` | 10⁻³ |
| micro `µ` | `1µm` | 10⁻⁶ |
| nano `n` | `1nm` | 10⁻⁹ |
| pico `p` | `1pm` | 10⁻¹² |
| femto `f` | `1fm` | 10⁻¹⁵ |
| atto `a` | `1am` | 10⁻¹⁸ |
| zepto `z` | `1zm` | 10⁻²¹ |
| yocto `y` | `1ym` | 10⁻²⁴ |
| ronto `r` | `1rm` | 10⁻²⁷ |
| quecto `q` | `1qm` | 10⁻³⁰ |

The [authored prefix policies](../architecture/core/definitions/physical.conduit)
includes seconds, hertz, volts, amperes, kelvin, grams, metres, square/cubic
metres, litres, radians, bytes, newtons, joules, watts, pascals, metres per
second, metres per second squared, and ampere-hours. Prefixes are case-sensitive:
`1mW` and `1MW` differ by a factor of a billion. Mass prefixes attach to `g`;
`1mkg` refuses. Prefixes cannot be stacked or attached to arbitrary symbols.

Canonical micro is `µ` (U+00B5). ASCII `u` is an explicit source alias, so
`1us` can replace `1µs`. Greek `μ` (U+03BC) refuses. Powers use declared spellings such as `m²`, `m³`, and `m/s²`. Source bytes and provenance survive;
there is no implicit Unicode normalization. Binary `KiB`/`MiB`/`GiB` remain
distinct from decimal `kB`/`MB`/`GB`.

Quantity families, dimensions, Unit reference laws, affine Point/Delta associations,
and prefix policies are authored in Conduitese. The checked
[`physical.conduit`](../architecture/core/definitions/physical.conduit) library is
the builtin authority; a document can add `unit smoot : Distance = { reference: m,
scale: 1.7018 }` without changing Rust. Prefixes are disabled unless the declaration
explicitly opts into source-defined groups. Unit capsules occupy 768 bytes and
retain their checked laws independently of later catalogue changes.

`Angle` uses `turn` as its canonical, reference, and default Unit. Degrees have
an exact scale of `1/360turn`, so `90°` is exactly `0.25turn`. Radians currently
retain an independent named origin; radian/turn conversion returns typed
`Inexact` until #5392 supplies the symbolic `2π` relation. No decimal approximation
stands in for that relation.

Reference anchors bind the family, source root symbol, and exact root law.
Changing the root law creates a distinct anchor, preserving the meaning of old
Unit/Quantity capsules and Plan/Play snapshots. A paired Point/Delta root shares
one checked anchor.

## Extreme scale needs an explicit representation

```conduit
plot extreme-distance (
    receipt: ExactQuantityConversionReceipt <= 8192B >>
) {
    converted: units/convert(source = 1Qm, to = qm)
    converted.receipt >> receipt
}.
```

The exact coordinate has coefficient 1 and decimal exponent 60 in the target
unit. The public `Quantity` Type carries an exact decimal coordinate and the full
resolved Unit in one versioned 788-byte codec. There is no second narrow quantity
family. Recognized extreme scales remain ordinary Quantities; an integer domain
projection has its own finite precision and range. Recognizing a suffix does not make its value fit
every consumer. A fractional or overflowing integer projection refuses admission.

An ordinary startup parameter selects the source Type `Quantity`:

```conduit
plot exact-extent (
    extent: Quantity = 1Qm
) {
}
```

This Fore/default example checks the wider profile without performing an
effect. Changing `Quantity` to `Distance` retains the same codec and requires the
length dimension. A consumer requiring integer metres must explicitly project
and refuse overflow for this extreme value. The [exact-profile tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/quantity_exact_profile.rs)
also cover direct literals and the canonical 788-byte carrier.

## Refusal is an inspectable result

Replace the conversion arguments above with:

| Source | Target | Outcome |
|---|---|---|
| `1Hz` | `m` | typed `incompatible-dimensions` result |
| `1°F` | `°C` | typed `inexact` result |
| `1Qm³` | `qm³` | typed `overflow` result |
| `21C` | `K` | invalid request; write `21°C` |
| `1m` | `mkg` | invalid target suffix |
| `1μs` | `s` | invalid source alias |

The first three are well-formed conversion requests whose receipts say
`refused`; the last three are source diagnostics. A successful
`conduit check` therefore does not imply that a conversion result is `converted`.
The consumer must inspect the result. No case quietly rounds, guesses a unit,
or turns an incompatible dimension into a scalar.

## Acoustic quantities: current building blocks and planned integration

Native `440Hz` and `250ms` already work. The open
[acoustic contracts #5216](https://github.com/dancxjo/conduit/issues/5216) and
[Speech quantities #5262](https://github.com/dancxjo/conduit/issues/5262) ask for
more than literal parsing. Their worked acceptance scenario is a **0.1-second,
200-Hz** intent projected at **8, 16 and 48 kHz**: 800, 1600 and 4800 samples
describe the same source duration. The nominal cycle duration is 0.005 seconds;
an executed checked reciprocal operation must establish that projection.

For repeated one-third-second durations, retain the exact fraction and
cumulative remainder instead of rounding every append. Amplitude, power and
referenced dB need distinct Types and explicit conversions. Overlapping pitch
and energy trajectories retain their timebase, scope and provenance; intended
targets remain distinct from measured observations and uncertainty.

Those are planned end-to-end acceptance examples, not claims that every
projection or Speech bridge is complete. They build on the same quantity laws.
Optional typed delimiter glyphs do not become a prerequisite for native units.

Continue with [[the complete feature examples|Conduitese-feature-coverage]].

The [first-class value design](https://github.com/dancxjo/conduit/blob/dev/docs/design/first-class-units.md)
records bounded codecs, name shadowing, source evidence and proof boundaries.
