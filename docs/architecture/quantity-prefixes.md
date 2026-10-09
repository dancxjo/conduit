# Decimal prefixes and exact quantity profiles

Issue [#5328](https://github.com/dancxjo/conduit/issues/5328) extends the existing
Quantity contract in stages. The immutable `quantity/decimal-prefix-catalog@1`
now records all 24 official prefixes and reviewed prefix positions. The whole-suffix resolver uses this
catalogue to establish exact semantic scale. The authored literal parser has
not yet migrated, and resolution does not assert that every scale fits legacy
runtime storage.

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
unchanged offset 273150; `°C` has scale 1000 and the same offset. A future
point/difference contract must preserve these serialized absolute meanings and
admit a distinct difference explicitly. Scaling the Celsius offset is invalid.

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

The remaining migration must introduce a separately versioned, bounded exact
profile, adopt this resolver in the authored parser, preserve original source
facts, and return a
representation-eligibility refusal when a selected legacy target cannot realize
a recognized value. Explicit checked conversion and compatible comparison must
retain source/target dimensions, exact ratio/offset, selected profile and result
or refusal. Parser migration, extended value encoding, public conversion
entrance, source-span diagnostics and Audio/non-Audio integrations remain open.

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

This codec is not yet a checked Plot primitive, admitted target profile or public
conversion entrance. Target eligibility, exact rational physical-reference
comparison, conversion receipts, temperature differences and integrations still
need their own implementation and evidence. No silent projection to the legacy
9-byte encoding is provided. A decimal output profile cannot represent every
rational coordinate; exact conversions must report inexactness when appropriate.
