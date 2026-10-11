## Semantic domains may be open; execution may not be

Conduit distinguishes **the set of values a type means** from **the resources required to represent and execute one actual value**.

A semantic numeric domain may therefore be open-ended:

```conduit
type TemperatureRange = Temperature in -273.15°C..
type NonnegativeCount = Count in 0..
type AtLeastFour = Count in 4..
type AtMostOne = Scalar in ..=1.000000
```

The semantic meaning is not truncated merely because one host uses a finite machine representation.

The runtime promise is different:

> **Semantic domains may be unbounded; execution resources may not be.**

An actual value must still be representable by the selected form/back and admitted within finite resource envelopes. A semantically valid value that a particular host cannot represent is a representation/admission problem, not a reason to lie about the type's meaning.

Variable-sized values and collections still need finite execution bounds. For example:

```conduit
name: Text
memo: Text <= 4KiB
```

A bare `Text` receives the canonical finite value-size default during checking; explicit bounds use `<=`.

The distinction is:

```text
unbounded semantic value domain     allowed
unbounded collection growth         not implicit
unbounded stream backlog            not implicit
unbounded retained storage growth   not implicit
unbounded execution resource use    not admitted
```

Bounds constrain realizable values and resource use. They do not themselves imply retention, persistence, allocation strategy, or authority.

## type, info, and form

A **type** owns semantic meaning; **info** is one finite value of that type.
A **form** owns a portable representation contract. A variant case does not mean its
byte tag, and changing a selected form does not redefine the type.

The current compact-form declaration maps a finite variant to `u8` tags, with
checked mapping, finite extent/work, compatibility identity, and invalid-tag
refusal. This does not claim arbitrary record layouts, byte orders, or every
proposed wire/storage representation. See
[[Current language surface|Current-language-surface]] for the current example.

Executable declarations use `plot`; portable type representations use `form`.

## Checked type identity is not source spelling

Source aliases are authoring convenience only.

This complete source gives the existing `text/upper` Kind a local alias:

```conduit
with text/upper as shout

plot aliased-upper (
    input: Text >> output: Text
) {
    input >> shout >> output
}
```

Replacing `shout` with another local alias changes the authored spelling, not
the resolved Kind or its checked Fore. It does not create a new text Type.
By contrast, a native declaration such as `type AlmostU32 = U32 where
. < 4_294_967_295` introduces a checked scalar profile with its own law; see
[[the consuming arithmetic example|Conduitese-by-example#give-an-arithmetic-invariant-to-the-type]].

> **Two source spellings that resolve to the same exact semantic type must produce the same checked type/fore identity. Renaming an alias must not change plan compatibility.**

This applies to startup parameters as well as runtime ports.

Checked startup-parameter contracts therefore carry canonical semantic type/profile identity, not raw alias text.

If omitted arguments are semantically filled before realization and a changed default changes meaning, the canonicalized default semantics must be represented at the correct checked layer rather than hidden behind a mere `has_default` bit.

Implementation/artifact identity is never a substitute for semantic type identity.

Provenance: #3724.


---

## Temporal value modalities

Canonical meanings:

```text
T       one finite value
$T      immediately observable current T
T...    flow: more values may arrive; no promised normal close
T...|   closing flow: zero or more values plus meaningful normal close
T?      ordinary finite optional T
$T?     immediately observable current optional T
```

`T?` is semantic sugar for a finite two-case variant:

```text
none
some(T)
```

It is not nullable-everything, `null`, `nil`, `undefined`, or truthiness.

`$T` is not weakened to “maybe initialized later.” Use `$T?` when the current answer may explicitly be none.

Provenance: #3970, #4048 and established temporal ancestry.

---

## keep: explicit retained current value

Canonical retained-value grammar:

```conduit
count:    keep Count(0) for life
middle:   keep I64 for this play
name:     keep Text <= 128B for this body
draft:    keep Text <= 4KiB for this wake
cache:    keep Bytes <= 2MiB for this boot
scratch:  keep Text
```

Conceptually:

```text
name ":" "keep" retained-value [ "<=" finite-bound ] [ duration ]
```

Durations:

```text
for this step
for this play
for this wake
for this boot
for this body
for life
```

Rules:

- omitted duration canonicalizes to **for this step**;
- `for life` is exact source sugar for `for this body`;
- bounds are constraints, not constructor configuration;
- duration is semantic lifetime, not storage mechanism;
- keep owns one explicit current value and is the explicit temporal/cycle-breaking boundary;
- no arbitrary port acquires implicit recurrence.

Canonical sugar when the checked fore is unambiguous:

```conduit
latest: keep Temperature for this wake

readings >> latest >> current
```

gear-name sugar resolves the one eligible input/output port by cord direction. Ambiguity must refuse and require explicit port spelling.

Provenance: #3956, #3977.

---

## data, `&T`, save, load and disk

Preserve the natural-duration distinction recovered from #3954/#3958:

~~~text
info     typed value flowing through Ports/Cords
keep     one current retained info value with explicit duration
data     bounded independently addressable content with identity/generation/lifetime
&T       ordinary info naming one exact data generation containing T
save     publish one value as data
load     read exact data back into bounded typed info
disk     durable data residence/domain
sign     evidence of what occurred
~~~

Canonical data-reference type spelling:

~~~conduit
saved: &Text >>
image: &Image
memory: &Experience...
~~~

Law:

~~~text
T     the value itself
&T    ordinary info naming one exact data generation containing T
~~~

`&T` is **not** a pointer, borrow, path, residence, capability, handle or authority token. The current runtime-port grammar rejects accidental recursive `&&T` spelling.

`&T` in type grammar coexists with infix bitwise `&` in expression grammar. Grammar position, never whitespace, distinguishes them.

keep duration and explicit publication are different promises:

> **If a plot says a value must last, planning selects a back that can truthfully make it last or refuses. save is publication, not survival.**

Therefore:

- body-lived keep may survive host/boot replacement without authored save/load;
- save produces one immutable independently addressable generation after complete validation;
- load returns ordinary info and does not receive privileged access to a destination keep;
- disk names durable data residence without implying POSIX paths/VFS/files;
- generic non-content resource pools remain resources where data would be nonsense.

Exact catalog kind paths for save/load remain owned by their semantic catalog; do not freeze an incidental implementation spelling here.

Living proof owner: #4116.

Provenance: #3954, #3958.

---

## Anonymous finite structures

Anonymous records:

```conduit
{ a: expr, b: expr }
{ a, b }              # punning when exact and unambiguous
```

Tuples:

```conduit
(0, 1)
(.1, .0 + .1)
```

These are finite structural info with deterministic checked identity. They are suitable for local algorithmic state without polluting the global semantic catalog.

Public fores should still prefer named semantic types where the shape has reusable domain meaning.

Provenance: #3968.

---

## Bounded variable-length sequences

Variable cardinality remains finite without leaking storage capacity into semantic nouns.

Canonical semantic shape:

~~~text
sequence of Element
  minimum items
  maximum items
  actual finite item count in the value
~~~

The exact checked identity includes element type and bounds.

Law:

> **Capacity is contract, not noun, unless exact cardinality is itself semantic.**

Prefer semantic names such as:

~~~text
VisionDetections
LinguisticTokens
Path
Rows
~~~

whose checked profiles carry their finite bounds, rather than storage-shaped names such as `VisionDetectionsFour` or `Rows64`.

A suffix/count remains appropriate when exact cardinality is meaning, such as an RGB triplet, 3D vector, exact matrix dimension or protocol tuple.

An empty sequence is a real zero-item sequence, not a fixed storage vector padded with `unused` cases.

Encoding/validation must carry exact sequence type/bounds and actual item count, and remain allocation-free-capable for constrained targets.

Current declarations express both bounded variable cardinality and exact
cardinality:

```conduit
type Octets = sequence U8 <= 4
type NonemptyOctets = sequence U8 in 1..=4
type PairOfWords = collection U16 = 2

type Packet = {
    octets: Octets
}

plot packet (
    input: U8 >> packet: Packet
) = ({ octets: [., .] })

plot pair (
    input: U16 >> pair: PairOfWords
) = ([1, 2])
```

`sequence U8 <= 4` admits 0 through 4 actual items. `in 1..=4` supplies both
item-count bounds. `collection U16 = 2` admits exactly two items. The
[named record tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/named_record_expression.rs)
and [exact collection tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/nominal_collection_expression.rs)
cover contextual literals, cardinality refusal and prepared expression
execution.

Runtime selection uses an explicit finite index and actual count:

```conduit
type Request = {
    bytes: sequence U8 <= 4
    index: U64
}

type Result =
    octet U8
    | short

plot guarded (
    value: Request >> result: Result
) = (.index < sequence/length(.bytes) ? octet(sequence/at(.bytes, .index)) : short(empty))
```

Only the selected ternary branch evaluates, so a short sequence returns
`short` without indexing it. Unguarded out-of-range `sequence/at` refuses.
The [prepared selection tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/prepared_sequence_selection.rs)
establish these outcomes. Packed `Bytes` uses `bytes/length` and `bytes/at`
instead: byte extent and sequence item count are different contracts.

Provenance: #3717, structured-info ancestry #1386/#1387.


---

## Quantities

Units are semantic source, not decorative typography.

Canonical examples:

```conduit
target = 21°C
delay = 250ms
distance = 3.2m
angle = 90°
voltage = 12V
rate = 440Hz
width = 640px
```

The unit participates in the quantity type/dimension.

In particular:

```text
21°C   Celsius
21C    must not silently mean Celsius
```

Do not collapse all dimensioned values into one undifferentiated runtime `Quantity` contract merely because they share a representation. A distance-to-frequency transformation must be checked as such.

Affine units such as °C/K use reviewed conversion law.

Provenance: #3975.

---

## Finite variants

Current authored variants have exact finite alternatives and optional bounded
payloads. This complete conditional constructor comes from the
[native expression tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/native_expression_construction.rs):

```conduit
type Choice =
    known I64
    | unknown

plot choose (
    >> value: I64
    result: Choice >>
) = (. >= 0 ? Choice.known(.) : Choice.unknown)
```

A case may also own a record payload:

```conduit
type Note = U8 in 0..=127

type MusicEvent =
    note {
        velocity: U8 in 0..=127
        pitches: sequence Note <= 16
    }
    | rest
```

The checked record payload keeps both refinements and finite sequence bounds.
Qualified construction uses `MusicEvent.note({ velocity: 96, pitches: [60, 62, 64] })`
or payloadless `MusicEvent.rest` when the expected type is in scope.

Current graph matching uses `>>` arms, with the selected payload carried
directly into the named destination:

```conduit
event >> ? {
    [MusicEvent.note] >> play-note
    [MusicEvent.rest] >> keep-silence
}
```

These are scoped endpoint fragments, not declarations of the destination
fores. Closed variants must be exhaustive. `variant/is(value, "case")` and
`variant/tag(value)` provide checked expression observations; the latter
returns ordinary text, so spelling a tag in a text comparison does not itself
validate that spelling against the case set. See
[[current routing and placeholder details|Plots-and-flow#exactly-one-graph-routing]].

Provenance: #4002;
[surface tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/surface_tests.rs)
and [selector tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/structured_selectors.rs).

---

## Resource/capability-valued ports

Low-level plots may pass admitted runtime resources through typed ports, but **descriptive info cannot forge authority**.

```text
{ address, length }        descriptive info
resource machine/memory/mmio/region    admitted runtime resource
```

Resource values are created only by admitted backs/kernel operations and carry exact lifecycle/provenance constraints. Impossible cross-host transfer must refuse before play.

No raw pointers, integer handles as authority, or serializable capability tokens by default.

Canonical source spelling is `resource T`. There are no resource literals and no parallel `capability T` wrapper.

Provenance: #4065.

---

## Concrete temporal boundaries

Each temporal shape is visible in a fore. This declaration demonstrates the
current grammar; its empty body is a signature specimen, not a working
implementation:

```conduit
plot temporal-boundary (
    >> one: Text
    >> maybe: Text?
    >> flow: Text...
    >> closing: Text...|
    >> current: $Text
    current-maybe: $Text? >>
    snapshot: &Text >>
) {
}
```

There is no implicit Current-to-value coercion. `note @ save-request >> snapshot`
explicitly samples causally visible Current when a trigger is consumed.
`&Text` names immutable data; it does not sample Current or confer authority.
Port temporal parsing and reference refusal are covered by the
[surface tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/surface_tests.rs).

Optional retained state is explicit:

```conduit
plot retained {
    available: keep Boolean? for this play
    enabled: keep Boolean(true) for this wake
}
```

The optional cell can answer none. The initializer in the second declaration
provides ordinary Boolean truth. These checked declarations do not themselves
prove durable storage on every host; a selected back must satisfy the requested
lifetime. See the [retained expansion tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/canonical_expansion_tests.rs).

## Named records and finite floats

Named records can be constructed from the current expression input:

```conduit
type Inner = {
    value: I128
}

type Outer = {
    inner: Inner
}

plot nested (
    input: I128 >> output: Outer
) = ({ inner: { value: . } })

plot extract (
    input: Outer >> output: I128
) = (.inner.value)

type FiniteFloat = F32 finite
type Probability = F32 finite in 0.0..=1.0
```

Finite floating-point refinements exclude non-finite values; range refinement
adds a separate relation. A finite input does not guarantee every arithmetic
result stays finite. The
[nested record tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/named_record_expression.rs),
[finite float tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/finite_f32_arithmetic.rs)
and [generated ECMAScript tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/ecmascript_types.rs)
provide scoped construction, evaluation and binding evidence.
