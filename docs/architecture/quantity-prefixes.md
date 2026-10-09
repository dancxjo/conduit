# Decimal prefixes and exact quantity profiles

Issue [#5328](https://github.com/dancxjo/conduit/issues/5328) extends the existing
Quantity contract in stages. The immutable `quantity/decimal-prefix-catalog@1`
now records all 24 official prefixes and reviewed prefix positions. The whole-suffix resolver uses this
catalogue to establish exact semantic scale. The authored literal parser now uses this resolver and preserves legacy
spellings and byte encodings. Newly composed spellings either fit an existing
exact legacy unit or return a representation-eligibility refusal; semantic
resolution does not assert that every scale fits legacy runtime storage.

The authoritative symbols and exponents come from the
[BIPM SI prefix table](https://www.bipm.org/en/measurement-units/si-prefixes).
Symbols match exact UTF-8 bytes. The canonical micro symbol is U+00B5 `µ`.
Greek U+03BC `μ`, ASCII `u`, whitespace, zero-width characters, and Unicode
compatibility folding are absent from the canonical prefix table. Existing
explicit ASCII aliases such as `us` remain accepted by the legacy suffix parser.
The resolver retains original spelling and admits ASCII `u` at each reviewed
micro prefix position, and `m2`, `m3`, `m/s2` at the corresponding powered
positions. These are explicitly reviewed aliases, not Unicode normalization.
Source spans remain the surrounding parser's responsibility.

## Reviewed compatibility matrix

Every official prefix is permitted at each position below. The descriptor's
power determines the exact decimal exponent relative to its unprefixed unit.
All 24 × 19 combinations are exercised by the catalogue conformance test.

| Reviewed suffix | Prefix position | Exponent relative to the base |
| --- | --- | --- |
| `s`, `Hz`, `V`, `A`, `K`, `g`, `m`, `rad`, `N`, `J`, `W`, `Pa` | Before the complete symbol | e |
| `L`, `B` | Before the approved non-SI base | e |
| `m²` | Before the powered meter | 2e |
| `m³` | Before the powered meter | 3e |
| `m/s`, `m/s²` | Before numerator meter only | e |
| `Ah` | Before ampere only | e |

The mass base is gram. `kg` is kilo + gram; kilogram is not another prefixable
base. Recursive or stacked prefixes are unavailable. Byte prefixes are decimal;
`MiB` retains its separate legacy binary meaning. Minute, hour, year, degree,
pixel, percent, `one`, historical units and already prefixed units do not occur
in this matrix. They retain their existing reviewed suffixes and transforms.
This policy does not add arbitrary unit-expression algebra or an expected-Type
choice among ambiguous suffixes.

Celsius and Fahrenheit are absent from generalized prefix composition. Existing
`m°C` is an absolute coordinate with scale 1 in millikelvin reference units and
unchanged offset 273150; `°C` has scale 1000 and the same offset. The explicit
`units/convert-temperature-difference` Kind admits a distinct difference Type
with zero offset. It does not reinterpret existing absolute literals or their
serialized values. Scaling the Celsius offset is invalid.

## Representation and migration

`Quantity` and `QuantityUnit` retain their 9-byte and 1-byte encodings and all
existing tags. The prefix catalogue allocates no new unit tag and introduces no
Type per prefix. It stores signed decimal exponents, never floating-point
factors or a materialized unbounded integer. The largest composed exponent in
this matrix is 90 for cubic quetta meters.

Semantic scale alone is not representation admission. `ResolvedQuantitySuffix` considers all reviewed decompositions and refuses
ambiguity rather than making an expected-Type or greedy choice. Existing
noncanonical `C`/`F` diagnostics take precedence, and unprefixable historical
suffixes retain their exact legacy lookup. Recognized extended suffixes expose
no legacy tag, keeping numeric representation admission separate.

The bounded codec described below now separates the extended numeric profile
from legacy storage. Explicit `ExactQuantity` startup parameters and defaults
now admit that profile through ordinary checked Plot authoring. The existing Audio and Robotics consumers now have explicit checked projections;
target realization eligibility is reported below.
The legacy authored target now returns a representation-eligibility refusal
when it cannot realize a recognized value. Explicit checked conversion and compatible comparison must
retain source/target dimensions, exact ratio/offset, selected profile and result
or refusal. Browser execution uses an installed bounded receipt profile. Bare no_std targets
retain explicit eligibility limits, as described below.
The checked authored conversion and receipt APIs are described below. Startup eligibility diagnostics retain the original
literal span, and highlighting recognizes extended literals without claiming
the legacy representation can realize them.

Catalogue conformance covers every official prefix against every reviewed base,
power composition, mass and stacking exclusions, case/confusables, and retained
legacy aliases. Existing Quantity tests still cover finite exact conversions,
legacy tags and structured transport. Those component results do not establish
browser execution or complete issue acceptance.

## Bounded exact decimal codec under development

`ExactDecimalQuantity` is a separate 20-byte versioned coordinate encoding:
version byte, existing reviewed unit tag, little-endian signed 16-bit decimal
exponent and little-endian signed 128-bit coefficient. It admits at most 38
significant decimal digits and exponents from -128 through +128. Authored input
is bounded to 128 bytes, with at most 96 numeric bytes. Parsing and normalization
use bounded iteration without allocation or floating point. Normalized zero has
coefficient/exponent zero; decoding refuses noncanonical encodings.

Prefix resolution selects the reviewed base and composes its exponent with the
literal's exact decimal scale. Historical unprefixable units retain their
existing unit and coordinate transform. `Qm`, `qm` and cubic extreme prefixes
can therefore have recognized exact coordinates without inventing legacy unit
tags. Scientific exponent notation is explicitly refused rather than being
confused with a unit prefix. The caller's authored source and resolved suffix
remain independent source facts; normalization of the semantic coordinate does
not authorize a formatter rewrite.

The codec is registered as the checked primitive `value/exact-decimal-quantity@1`.
The explicit `ExactQuantity` spelling selects it; unused catalogues retain their
existing identities. Ordinary startup arguments and defaults retain canonical
20-byte values, and Native Rust carriers validate the same primitive identity.
All 456 reviewed prefix/base pairs are checked through authored startup values
against the independent reference fixture. Reviewed compound suffixes such as
`dam/s` remain whole quantity literals while ordinary division retains its grammar.

Prepared identity transport admits exactly 20 input and output bytes, validates
the versioned codec, and performs no allocations during evaluation. This evidence
covers byte transport, not extended arithmetic execution, browser execution or
physical target execution. Conversion receipts and the distinct temperature
difference contract additionally use hosted preparation and the ordinary kernel;
The consumer projections below have independent native and component execution
evidence.
No silent projection to the legacy
9-byte encoding is provided. A decimal output profile cannot represent every
rational coordinate; exact conversions must report inexactness when appropriate.


## Shared exact conversion law under development

Both numeric profiles use one rational unit scale/offset law. Legacy conversion
and comparison retain checked `i128` intermediates and the original precision
and range refusals. Explicit extended conversion uses 64 fixed base-10^9 limbs
(576 decimal digits) for intermediates, with no allocation. The checked input
profile, reviewed transforms and pairwise common-reference products fit within
that capacity; arithmetic still checks every carry and refuses capacity overflow.
Binary long division has at most 1920 bit steps. Exact target decimal projection
reduces numerator and denominator, permits only denominator factors 2 and 5,
and checks the resulting 38-digit coefficient and ±128 exponent profile.
It never generates a repeating decimal or rounds to fit a selected profile.

`ExactDecimalQuantity::convert_to_legacy` explicitly projects to an existing
integer unit tag. `convert_to_decimal` explicitly selects the bounded decimal
coordinate in an existing reviewed unit. `compare` uses the same physical
reference without selecting a lossy target. Component conformance includes
`0°C` → `273.15K`, `30°C` → `86°F`, inch-to-meter rational conversion, squared
prefixes, decimal/binary bytes, signed limits and cubic extreme comparisons.
A non-terminating target coordinate (`1°F` → Celsius, or `1m` → inches) refuses
as inexact. `Qm` projected to legacy meters refuses overflow; `qm` refuses
inexactness. These Rust methods establish numeric behavior, not completion of
all authored conversion or target-admission requirements.

`convert_to_target` additionally accepts a whole-suffix catalogue descriptor,
including all reviewed generalized prefixes. Its result retains that descriptor
and the exact numeric coefficient/exponent in the selected target coordinate.
It cannot be mistaken for a physical quantity tagged only with the base unit.
Target prefix scaling is applied before bounded decimal projection, allowing
small target coordinates even when an intermediate base coordinate exceeds the
output profile. All 456 inverse scales are checked against the independent
reference corpus. The ordinary authored conversion Gear and prepared hosted Back use this same
operation. Existing consumer record projections are described below.

## Checked authored conversion Gear

The public `conduit_plot::quantity_conversion` module installs the reviewed
`units/convert` startup contract and `ExactQuantityConversionReceipt` output
Type into ordinary startup/profile catalogues. For example:

```conduit
plot conversion (
    receipt: ExactQuantityConversionReceipt <= 8192B >>
) {
    converted: units/convert(source = "1cm²", to = "mm²")
    converted.receipt >> receipt
}.
```

Quoted `source` preserves the exact original quantity spelling, including
reviewed aliases. `to` selects a whole suffix from the pinned catalogue. Both
are required Text startup values bounded to 128 decoded bytes. This constructor
selects the bounded exact decimal output profile explicitly through its contract;
it never retries a failed conversion in a different unit or legacy profile.

`install`, ordinary `check_syntax_document`, owner `validate_source`, canonical
expansion and `prepare_configuration` produce the checked receipt. The receipt
retains the source coordinate, original suffix, canonical prefix/base,
source/target dimensions, exact reference scale/offset/denominator triples,
composed target exponent, catalogue and profile identities, and a `converted`
coordinate or `refused` precision/dimension/range result. The reference equation
is `(coordinate * scale + offset) / denominator`; target decimal scaling acts on
the coordinate after the affine reference law. `validate_receipt` recomputes
these facts during admission, rejecting forged facts in otherwise valid records.

All 456 inverse prefix scales run through actual authored parsing, checking,
canonical expansion and constructor preparation against the independent corpus.
Invalid source/target requests retain original UTF-8 source locations, including
checked local aliases and escaped Text custody. Foreign checked Source identities
refuse correlation. Parameterized requests remain unresolved until concrete
preparation, rather than producing a premature result.

The standard product now installs this catalogue and maps preparation refusals
into both human and JSON diagnostics with identical source spans. The math
composition of the std Host advertises the prepared conversion Back; the minimal
composition omits it. Installed preparation checks the exact revision, artifact,
implementation, Front, semantic law and limits, and bounds startup configuration
before wide arithmetic. The artifact incorporates the compiled Back, codec,
resolver, reviewed transforms and receipt schema sources.

All numeric conversion and receipt encoding finish before Play. The installed
budget admits two value slots with twice the actual encoded receipt bytes, no
Host requests and eight Sign slots, within the 8192-byte receipt ceiling. Play
uses the existing structured Value Back to emit the immutable stored receipt
once. A 1000-step pressure test measures zero Back allocations and no output;
release emits once, and cancellation prevents delivery. Ten actual authored
Source → offer → plan → installed preparation → kernel scenarios preserve exact
receipt bytes and fixed value-storage capacity, including typed precision,
overflow and dimension refusals. These are std/Linux execution results, not
browser or firmware execution evidence.

Browser/no_std execution eligibility, Audio/non-Audio consumers and stable
acceptance of the quantity contracts remain open acceptance work
for #5328.

## Explicit temperature differences

The installed conversion catalogue also admits a separate ordinary Gear:

```conduit
plot difference (
    receipt: ExactTemperatureDifferenceConversionReceipt <= 8192B >>
) {
    converted: units/convert-temperature-difference(source = "9°F", to = "K")
    converted.receipt >> receipt
}.
```

This request means a temperature difference of nine Fahrenheit degrees. Its
exact result is `5K`. The original `units/convert` request with the same spelling
continues to mean an absolute Fahrenheit point. `1m°C` as a difference converts
to `0.001K`; as an existing absolute point it converts to `273.151K`.

`ExactTemperatureDifference` is a checked Core carrier, restricted to the
Temperature dimension. It shares the same bounded decimal coordinate and
rational arithmetic implementation. Its source/target transforms have zero
offsets, and its semantic digest is distinct from the absolute coordinate.
The Plot source is a distinct `quantity/exact-temperature-difference@1` record
containing the 20-byte coordinate; those coordinate bytes alone are not a
standalone difference encoding. Its target coordinate, result variant and
receipt also have distinct checked Type identities. There is no implicit
point-to-difference connection or absolute-point multiplication operation.

Both quoted arguments retain the 128-byte bound. Preparation checks dimension,
representation and exactness, emits the same 8192-byte-bounded receipt shape,
and preserves inexact/overflow/dimension results. Receipt readmission recomputes
the role-specific law and refuses forged offsets, source Types or results.
The optional std math Host offers a distinct implementation for this Kind;
both implementations prepare before Play and emit through the existing fixed
structured Value Back under pressure and cancellation. Browser/no_std arithmetic
checking remains separate from installed runtime realization eligibility.

The independent temperature fixture is generated by Python `Fraction` from
SI kelvin equations, including Celsius's origin and Fahrenheit's scale/origin;
it neither reads Rust results nor imports the implementation's transform table.
It covers 972 point/difference projections across signed, fractional, historical
milli and extreme Kelvin prefix coordinates. This development contract still
requires the full integration and stable-acceptance gates before #5328 closes.

## Checked exact comparison

The same installed quantity catalogue admits ordinary comparison authoring:

```conduit
plot compare (
    receipt: ExactQuantityComparisonReceipt <= 8192B >>
) {
    compared: units/compare(left = "1000mm", right = "0.001km")
    compared.receipt >> receipt
}.
```

Both required quoted operands are bounded to 128 decoded bytes and use the
reviewed suffix resolver and the bounded exact decimal source profile. The
result is a typed `less`, `equal`, `greater` or `refused` case. Ordering uses the
same exact rational common-reference law as Core comparison; it does not first
convert an operand into the other's bounded decimal profile. Thus `1°F` is
exactly less than `0°C` even though its Celsius projection is a repeating decimal.
Incompatible dimensions and unsupported exact radian/degree relationships remain
retained typed refusals.

Each operand retains its original spelling, coordinate, complete suffix,
resolved base/prefix, prefix/composed exponent, physical dimension and exact
scale/offset/denominator equation. The enclosing receipt retains the catalogue,
selected source representation and result. Readmission checks the exact Type and
8192-byte bound, bounds original operands before copying, then recomputes all
facts. Shape-valid modified results, coordinates or metadata refuse.

`units/compare-temperature-differences` instead produces
`ExactTemperatureDifferenceComparisonReceipt`, with distinct operand/result
Types and zero offsets. It accepts only explicitly admitted temperature
differences. A nine-degree Fahrenheit difference compares equal to `5K`;
those same authored magnitudes under `units/compare` remain absolute points.
No mixed-role comparison is inferred from unit spelling or expected output Type.

The optional std math composition advertises distinct exact implementations for
both comparison Kinds. Preparation checks their exact offered contracts and
finite configuration, computes and encodes the receipt, and admits actual
storage before Play. The existing fixed structured Value Back emits once under
pressure and cancellation; it performs no arithmetic or encoding during Play.
The independent comparison corpus contains 798 cases from Python `Fraction`,
the independent 456-prefix scale matrix, affine temperature equations, and
reviewed linear/binary factors. Ordinary source checking and canonical expansion
exercise that corpus without Rust output serving as its expected result.

## Existing consumer projections

The bounded exact carrier projects explicitly into existing Audio and Robotics
records. `MusicalPitch::from_exact_quantities` selects exact integer millihertz;
`ToneIntent::from_exact_time` and `MusicalNoteEvent::from_exact_time` select
microseconds while preserving correlation/occurrence, gate and ordering facts.
The common Core `convert_to_u64` projection uses the same rational conversion
law before checking integer precision and the full unsigned range. It permits
existing event times above `i64::MAX`; the sound record still refuses its
reserved `u64::MAX` endpoint. Sub-millihertz and sub-microsecond fractions refuse
without rounding. These constructors introduce no new audio quantity family.

`RangeObservation::from_exact_quantities` selects the existing millimetre and
millisecond profile. `BatteryObservation::from_exact_quantities` selects
permille and millivolts. Both retain their existing wire identities, numeric
bounds and precise conversion refusals. A constructed observation is a portable
value, not evidence of a physical measurement.

The consumer tests cover equivalent prefixed frequency, duration, length and
voltage inputs, native record readmission and unchanged encodings/digests,
precision/dimension/range refusals, and unsigned event-time boundaries. The
reference synth receives admitted note events built from exact prefixed pitch
and time; its PCM and state match the existing integer path across different
block sizes through note-on and release. This proves component execution, not
a physical speaker or human listening. Final accepted release evidence remains required before issue closure.


## Target realization eligibility

| Target | Available contract | Proof class |
| --- | --- | --- |
| std math Host | Four conversion/comparison Kinds; 8192-byte exact receipts prepared before Play | Actual hosted kernel execution, measured zero-allocation pressure/cancellation tests |
| browser | Same four Kinds and semantic receipt Types; installed 4096-byte value profile | Native browser runtime/kernel tests and pinned Chromium executing actual WebAssembly |
| no_std Core/Plot/Audio/Robotics/Synth | Fixed exact codec and arithmetic; explicit legacy integer consumer projections | Cross-target compilation and native deterministic contract tests; no firmware execution claim |
| bare no_std Host | Existing offered legacy Quantity realization; no installed exact receipt Back | An exact semantic coordinate does not imply an eligible bare Host realization |

The browser computes and encodes each receipt during installed preparation,
checks the exact offered artifact/contract/configuration, and stores it in the
existing finite value arena. Its existing source Back emits the stored bytes
once; no conversion or serialization occurs during Play. Receipts above the
browser's 4096-byte profile explicitly refuse representation eligibility even
though the portable Type permits 8192 bytes. Pressure tests retain full canonical
receipt bytes and unchanged arena capacities for all four roles. The browser
membership envelope admits its 129 installed capabilities while preserving its
independent 192 KiB advertisement bound.

The browser proof exercises all 24 prefix exponents plus ordinary frequency,
time, squared-length, affine-temperature, extreme-prefix and decimal/binary-byte
conversions. Separate cases execute temperature differences, exact comparison,
and retained inexact/overflow/dimension refusals. Ordinary typed expressions
inspect the actual receipt; an acknowledged Boolean presentation and completed
Play are checked in Chromium. This is browser execution evidence, not physical
measurement or firmware execution. Repository browser validation enters through
`cargo xtask prove browser-host` with the pinned Chromium project, one worker and
zero retries.

Bare target availability is explicit: the shared arithmetic is allocation-free,
but a compiled exact Type is not an offered Back. Unsupported extended runtime
placements must remain ineligible; ordinary legacy literals retain their offered
representation and recognized wider startup values retain the source-spanned
representation-eligibility diagnostic. No implicit narrowing or rounded fallback
is installed.
