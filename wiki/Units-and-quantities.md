# Units and quantities by example

Write the quantity you mean: `250ms`, `440Hz`, `3.2m`, `21°C`.
The suffix carries a checked dimension. A duration, frequency and distance
remain different even when their representations happen to have the same size.

These examples describe current development source reviewed on **9 October
2026**, including completed [SI work #5328](https://github.com/dancxjo/conduit/issues/5328).
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
    converted: units/convert(source = "1kHz", to = "Hz")
    converted.receipt >> receipt
}.
```

The result is exactly **1000 Hz**. `source` and `to` are bounded textual
configuration for this explicit conversion Gear. Native quantity literals
elsewhere still need no imports or quotes. The receipt retains original
spelling, resolved units and dimensions, prefix, conversion law, selected
profile, and either the exact coordinate or a typed refusal.

Try these replacements in the same plot:

| Source | Target | Exact result |
|---|---|---|
| `"1µs"` | `"ns"` | 1000 ns |
| `"1cm²"` | `"mm²"` | 100 mm² |
| `"0°C"` | `"K"` | 273.15 K |
| `"30°C"` | `"°F"` | 86 °F |
| `"1MB"` | `"B"` | 1,000,000 B |
| `"1MiB"` | `"B"` | 1,048,576 B |
| `"1mg"` | `"g"` | 0.001 g |
| `"1kg"` | `"g"` | 1000 g |
| `"1km/h"` | `"m/s"` | `inexact` refusal; no silent rounding |

Powered units apply the prefix's power too: a centimetre squared is
10⁻⁴ square metres. The reviewed compound suffix `m/s²` is supported;
this does not introduce arbitrary source algebra such as `kg*m/s^2`.

## Compare compatible dimensions exactly

```conduit
plot compare-distances (
    receipt: ExactQuantityComparisonReceipt <= 8192B >>
) {
    compared: units/compare(left = "1000mm", right = "0.001km")
    compared.receipt >> receipt
}.
```

The comparison is `equal`: both mean one metre. No rounded display coordinate
is needed to establish equality. The receipt retains both operands and their
exact reference transforms. The same Gear compares `"1°F"` and `"0°C"`
exactly even though converting 1 °F to a finite decimal °C coordinate is inexact.

## A temperature point differs from a temperature change

`0°C` as a temperature point equals 273.15 K. A change of 0 °C has no
273.15 K offset. Choose the ordinary Gear that states which meaning you want:

```conduit
plot temperature-change (
    receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>
) {
    converted: units/convert-temperature-difference(source = "9°F", to = "K")
    converted.receipt >> receipt
}.
```

This change is exactly **5 K**. For `"1m°C"` to `"K"`, the difference is
0.001 K. Difference conversion scales the change without applying an absolute
temperature offset.

```conduit
plot compare-temperature-changes (
    receipt: ExactTemperatureDifferenceComparisonReceipt <= 8192B >>
) {
    compared: units/compare-temperature-differences(left = "9°F", right = "5K")
    compared.receipt >> receipt
}.
```

Its result is `equal`. These contracts prevent a temperature change from being
silently treated as an absolute temperature point.

## All 24 decimal SI prefixes

Each row can be used as `source` in `units/convert`, with `to = "m"`.
The exact conversion profile preserves a finite coefficient and exponent;
these examples do not promise that every value fits the legacy quantity carrier.

| Prefix | Example | Metres |
|---|---|---|
| quetta `Q` | `"1Qm"` | 10³⁰ |
| ronna `R` | `"1Rm"` | 10²⁷ |
| yotta `Y` | `"1Ym"` | 10²⁴ |
| zetta `Z` | `"1Zm"` | 10²¹ |
| exa `E` | `"1Em"` | 10¹⁸ |
| peta `P` | `"1Pm"` | 10¹⁵ |
| tera `T` | `"1Tm"` | 10¹² |
| giga `G` | `"1Gm"` | 10⁹ |
| mega `M` | `"1Mm"` | 10⁶ |
| kilo `k` | `"1km"` | 10³ |
| hecto `h` | `"1hm"` | 10² |
| deca `da` | `"1dam"` | 10¹ |
| deci `d` | `"1dm"` | 10⁻¹ |
| centi `c` | `"1cm"` | 10⁻² |
| milli `m` | `"1mm"` | 10⁻³ |
| micro `µ` | `"1µm"` | 10⁻⁶ |
| nano `n` | `"1nm"` | 10⁻⁹ |
| pico `p` | `"1pm"` | 10⁻¹² |
| femto `f` | `"1fm"` | 10⁻¹⁵ |
| atto `a` | `"1am"` | 10⁻¹⁸ |
| zepto `z` | `"1zm"` | 10⁻²¹ |
| yocto `y` | `"1ym"` | 10⁻²⁴ |
| ronto `r` | `"1rm"` | 10⁻²⁷ |
| quecto `q` | `"1qm"` | 10⁻³⁰ |

The [reviewed prefixable table](https://github.com/dancxjo/conduit/blob/dev/architecture/core/src/quantity_prefix.rs)
includes seconds, hertz, volts, amperes, kelvin, grams, metres, square/cubic
metres, litres, radians, bytes, newtons, joules, watts, pascals, metres per
second, metres per second squared, and ampere-hours. Prefixes are case-sensitive:
`1mW` and `1MW` differ by a factor of a billion. Mass prefixes attach to `g`;
`1mkg` refuses. Prefixes cannot be stacked or attached to arbitrary symbols.

Canonical micro is `µ` (U+00B5). ASCII `u` is an explicit source alias, so
`"1us"` can replace `"1µs"`. Greek `μ` (U+03BC) refuses. ASCII powers `m2`,
`m3`, `m/s2` are reviewed aliases. Source bytes and provenance survive;
there is no implicit Unicode normalization. Binary `KiB`/`MiB`/`GiB` remain
distinct from decimal `kB`/`MB`/`GB`.

## Extreme scale needs an explicit representation

```conduit
plot extreme-distance (
    receipt: ExactQuantityConversionReceipt <= 8192B >>
) {
    converted: units/convert(source = "1Qm", to = "qm")
    converted.receipt >> receipt
}.
```

The exact coordinate has coefficient 1 and decimal exponent 60 in the target
unit. The separately admitted `ExactDecimalQuantity` profile can carry extreme
scales; the existing `Quantity` profile still has its `i64` magnitude and
versioned 9-byte encoding. Recognizing a suffix does not make its value fit
every consumer. An ineligible legacy literal refuses representation admission.

An explicitly selected startup Type uses the source name `ExactQuantity`:

```conduit
plot exact-extent (
    extent: ExactQuantity = 1Qm
) {
}
```

This Fore/default example checks the wider profile without performing an
effect. Changing `ExactQuantity` to `Distance` refuses `RepresentationIneligible`
for this extreme value. The [exact-profile tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/quantity_exact_profile.rs)
also cover direct literals and the separately versioned 20-byte carrier.

## Refusal is an inspectable result

Replace the conversion arguments above with:

| Source | Target | Outcome |
|---|---|---|
| `"1Hz"` | `"m"` | typed `incompatible-dimensions` result |
| `"1°F"` | `"°C"` | typed `inexact` result |
| `"1Qm³"` | `"qm³"` | typed `overflow` result |
| `"21C"` | `"K"` | invalid request; write `21°C` |
| `"1m"` | `"mkg"` | invalid target suffix |
| `"1μs"` | `"s"` | invalid source alias |

The first three are well-formed conversion requests whose receipts say
`refused`; the last three are source diagnostics (`CND-QTY-001`). A successful
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
