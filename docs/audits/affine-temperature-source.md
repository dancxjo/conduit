# Source-owned affine temperature proof

Issue #5528 uses the existing checked physical declaration and exact conversion
implementation. `architecture/plot/tests/fixtures/affine_temperature.conduit`
owns a temperature difference family, a separate point family, and Kelvin,
Celsius and Fahrenheit laws. Names end in `Point` to avoid redefining intrinsic
unit symbols or silently changing existing Source.

Kelvin is the point origin. Celsius adds 273.15 Kelvin; Fahrenheit uses scale
5/9 and offset 45967/180 Kelvin. Each point unit separately declares its delta
relationship, whose scale is the same but whose offset is absent. Both rational
laws remain exact rather than approximating 5/9 with a decimal.

The Source corpus checks 0 Celsius → 273.15 Kelvin, 32 Fahrenheit → 0 Celsius,
30 Celsius → 86 Fahrenheit, a nine-Fahrenheit difference → five Kelvin, and a
one-Celsius difference → 1.8 Fahrenheit. It checks exact coordinates and distinct
point/delta roles. Point-plus-point, point/delta substitution and same-dimension
nominal-family substitution refuse with Source location evidence. Zero scale
denominator, foreign difference family and invalid point/delta relationships
also refuse with owned Source spans.

`targets/std/tests/affine_temperature_play.rs` checks and expands the same Source,
selects the installed math Back and executes each conversion through a real
Plan/kernel Play and acknowledged receipt Fore. Complete receipt bytes must
match preparation, re-admit from their retained Quantity/Unit evidence, preserve
finite value-store capacity and complete without counterfeit Host Calls.

This is hosted native conformance. It adds no new unit parser, role coercion,
affine arithmetic implementation, browser claim or physical sensor evidence.
The parent #5390 retains its full acceptance audit; angle and prefix policy
siblings keep their separate ownership.
