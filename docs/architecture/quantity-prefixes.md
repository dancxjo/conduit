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

The bounded codec described below now separates the extended numeric profile
from legacy storage. Explicit `ExactQuantity` startup parameters and defaults
now admit that profile through ordinary checked Plot authoring. The remaining
migration must admit arithmetic execution and complete authored conversion receipts.
The legacy authored target now returns a representation-eligibility refusal
when it cannot realize a recognized value. Explicit checked conversion and compatible comparison must
retain source/target dimensions, exact ratio/offset, selected profile and result
or refusal. Extended runtime admission, public conversion entrance and Audio/non-Audio
integrations remain open. Startup eligibility diagnostics retain the original
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
physical target execution. Conversion receipts, temperature differences and
Audio/non-Audio integrations still need their own implementation and evidence.
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
the ordinary authored conversion or target-admission requirements.

`convert_to_target` additionally accepts a whole-suffix catalogue descriptor,
including all reviewed generalized prefixes. Its result retains that descriptor
and the exact numeric coefficient/exponent in the selected target coordinate.
It cannot be mistaken for a physical quantity tagged only with the base unit.
Target prefix scaling is applied before bounded decimal projection, allowing
small target coordinates even when an intermediate base coordinate exceeds the
output profile. All 456 inverse scales are checked against the independent
reference corpus. This remains a Core operation pending the ordinary authored
conversion entrance and receipts.
