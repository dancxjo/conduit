# Physical declarations

Physical quantity families and unit relationships are authored in Conduitese.
Rust implements bounded admission, exact arithmetic, and the portable carrier;
the authored declarations supply the physical catalogue.

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

unit °F : Temperature = {
    reference: °C,
    scale: 5/9,
    offset: -160/9,
    delta: { quantity: TemperatureDelta, reference: K, scale: 5/9 }
}
```

The transformation is `coordinate_in_reference = coordinate * scale + offset`.
Scales are positive. Offsets belong to points; differences always have zero
offset. The declared point family and its difference family share one physical
dimension, while preserving their distinct semantic Types. A `TemperatureDelta`
cannot satisfy a `Temperature` port or record field merely because the two
values use the same unit scale.

An exact scalar uses ordinary decimal expressions, division, or an explicit
rational record such as `{ numerator: 5, denominator: 9 }`. Quoted strings do
not define unit transformations. Admission bounds coefficients, exponents,
dimension terms, declaration counts, and reference depth, and refuses a zero
denominator or a transformation that cannot fit those bounds exactly.

```conduit
dimension length
dimension time

type Distance = quantity { dimension: length }
type Area = quantity { dimension: { length: 2 } }
type Speed = quantity { dimension: { length: 1, time: -1 } }
type Ratio = quantity { dimension: {} }

unit m : Distance = { reference: origin, scale: 1, prefixes: si }
unit m² : Area = { reference: origin, scale: 1, prefixes: si, power: 2 }
unit cm : Distance = { reference: m, scale: 0.01 }
```

Prefixes are disabled when `prefixes` is absent or `none`. A declaration may
explicitly select `si`, `binary`, or `[si, binary]`. Prefix scale is raised to
the declared `power`, which defaults to one and is bounded to integers 1..8. Thus an area prefix scales its
coordinate by the square of the prefix factor. Prefix admission never silently
enables prefixes for every unit with a compatible dimension.

Prefix groups themselves are source declarations. Their exponent tables and
spellings are not supplied by a Rust match table:

```conduit
prefix si k = { exponent: 3 }
prefix si µ = { exponent: -6 }
prefix si u = { exponent: -6, alias: µ }
prefix binary Ki = { exponent: 10 }
```

`si` uses powers of ten and `binary` uses powers of two. The admitted policy
contains only the exponents declared in the chosen group. An alias must refer
to a declared prefix with the same exponent; physical value capsules retain
the admitted spelling, including an alias such as `u` instead of `µ`. Duplicate spellings, alias cycles, and aliases
with a different exponent are refused.

The last example deliberately declares `cm` explicitly while `m` admits SI
prefixes. That spelling collides with the synthesized centi-metre spelling and
must be refused; remove the explicit declaration or disable that prefix policy.
An explicit declaration cannot silently override a synthesized unit.

Definitions are admitted into an immutable startup catalogue before expressions
are resolved. Forward references are permitted. Duplicate names, conflicting
unit symbols, missing references, cycles, multiple roots in one family, and
unanchored families are refused with the authored source span. Adding a new
family or unit through this admission path does not require a Rust catalogue
edit. Imported aliases preserve semantic identity, while exact source custody
retains the declaration document and span.

The default `origin` is unique within a family. A separate exact coordinate
anchor is explicit. `Angle` uses `turn` as its canonical, reference, and default
Unit; degrees are declared with an exact scale of `1/360` relative to `turn`.
Thus `90°` converts exactly to `0.25turn`. Radians currently use the independent
named anchor `origin(radian)`. Conversion between radians and turns returns the
typed `Inexact` refusal until the symbolic `2π` relationship is implemented in
#5392; the catalogue does not substitute an approximate decimal for π.

After admission, `Hz` is a Unit and `1kHz` is a Quantity. Startup parameters,
ordinary values, record fields, and runtime ports use those same typed values.
Neither multiplying a unit by one nor choosing a dimensionless unit changes the
Unit into a Quantity implicitly.

The point family's unit can also construct an explicit difference:
`TemperatureDelta(21, °C)`. Bare `21°C` remains a temperature point, independent
of the expected Type. Admission never silently reinterprets a point literal as
a difference because a port or record field requests `TemperatureDelta`.

Checked catalogue snapshots are immutable. Source edits admit a new set of
definitions for subsequent preparation; existing Unit and Quantity capsules and
Plan/Play artifacts keep their prior meanings. A later declaration with the same
symbol cannot reinterpret an earlier capsule. A reference anchor binds the
source root Unit's symbol and exact scale/offset law, together with its family
and any explicit origin name. Changing that root law creates a distinct anchor;
paired Point/Delta root declarations share the same checked anchor.

Quantity constructors admit exact decimal constants and concrete immutable local
values, including forward numeric and Unit aliases. Constructor operands must be
known at checking/preparation time. For reusable startup interfaces, construct
the Quantity first and forward its ordinary typed value; this declaration feature
does not introduce a runtime numeric-constructor Gear.

## Development acceptance boundary

`Dimension` is a standalone checked value with a versioned finite encoding,
independent of Unit coordinates and nominal quantity families. A basis name
denotes its named anchor; a composed declaration denotes normalized dimensional
algebra. Named Angle retains its own anchor instead of becoming dimensionless.

```conduit
dimension travel
dimension clock
dimension velocity = travel / clock
dimension velocity2 = { travel: 1, clock: -1 }

type Motion = quantity { dimension: velocity }
unit stepper : Motion = { reference: origin, scale: 1 }
unit doublestep : Motion = { reference: stepper, scale: 2 }

plot transport (
    >> input: Dimension
    output: Dimension >>
) = (.)
```

`velocity` and `velocity2` have equal normalized values and content digests.
Declaration names and source spans remain separate evidence. Dimensions can be
startup arguments, structured fields, generic Type parameters and runtime port
values. The canonical leaf is `value/dimension@1`; its Rust binding is
`conduit_core::Dimension`. Unit and Quantity values cannot satisfy that Type.
Decoding checks the version, canonical sorted vector, exponent/term bounds and
content digest. The digest checks consistency; checked declaration custody and
consumer compatibility supply authority. It is not a grant to rebind an
existing definition.

Composition admits at most eight surviving basis terms and exponents in
−16..16. Declaration resolution admits depth 32 and 65,536 work steps; unknown
references, cycles, duplicate declarations and physical symbol collisions
refuse at their source spans. Every intermediate composition must fit its
admitted bounds. A new catalogue snapshot cannot reinterpret the checked
constants retained in an existing expression program.

`units/dimension(source: Unit)` and `units/quantity-dimension(source: Quantity)`
extract the pinned Dimension from an immutable typed startup value and emit it
on their `dimension: Dimension` runtime port. Preparation consumes the checked
capsule; it does not parse a unit string or consult a mutable catalogue.

The remaining #5390 acceptance includes checked Quantity multiplication/division,
Angle-consuming trigonometry, full integration and the required stable proof.
Nominal quantity families and point/difference roles remain separate from
dimension algebra. Exact symbolic
constants, including the turn/radian relationship, and planner-selected numeric
precision belong to the separate #5392 implementation.

## Full-turn spellings

`turn` remains the canonical Angle Unit. `tr` is the preferred compact spelling;
`rev` and `rot` are exact one-turn synonyms. `pla` is an uncommon historical
spelling for a full angle. All four are checked source-defined Units, without
prefixes: `1tr = 1rev = 1rot = 1pla = 1turn = 360° = 400gon`, and
`0.25tr = 90°` exactly. Bare names denote Units; `1tr` and `2rev` denote
Angle Quantities. Unknown spellings and attempts to redeclare these built-in
Units are refused by ordinary checking (`CND-FRM-055` for an undeclared
quantity suffix; `CND-FRM-058` for a conflicting declaration).

## Optional international full-turn vocabulary

`plots/units/international-turns.conduit` provides `tour`, `toer`,
`revolución`, `rivoluzione`, `revolução`, `Umdrehung`, `varv`, and `obrót`.
Each checked declaration references `turn` with scale 1. Bare names denote
Units; suffixed quantities such as `1revolución` equal one turn, and
`0.25obrót` equals a quarter turn exactly.

These names are optional. Include the declarations in authored Source, or
check the declaration document with `checked_physical_catalog_for_document`
and pass the returned immutable catalogue to the consumer's checker. The
default catalogue does not acquire these names globally. This uses existing
checked declarations and catalogue custody; it introduces no file-import syntax.

Spellings are case-sensitive and preserve their UTF-8 bytes. There is no
Unicode normalization, accent removal, transliteration, or locale-based lookup:
`revolucion` and decomposed `revolución` remain undeclared and receive
`CND-FRM-055`. An import cannot rebind `turn` or an already imported alias
(`CND-FRM-058`). The canonical reference and existing checked values remain fixed.
