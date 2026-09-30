> **Canonical reference.** These pages were migrated from issue [#4109](https://github.com/dancxjo/conduit/issues/4109) on 2026-09-29. The wiki is now the readable language reference; implementation tickets remain evidence of conformance and provenance.

## Semantic domains may be open; execution may not be

Conduit distinguishes **the set of values a type means** from **the resources required to represent and execute one actual value**.

A semantic numeric domain may therefore be open-ended:

```conduit
type Temperature = Scalar in -273.15..
type Count = Integer in 0..
type AtMostOne = Scalar in ..=1
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

## Type versus form

A **type** is semantic meaning. A **form** is one concrete portable representation of that meaning.

```text
type  what values mean
form  how values are carried/stored in one compatibility contract
```

A variant case does not mean its byte tag; the tag belongs to a form. The same type may therefore have more than one form without changing semantic identity.

Exact forms can expose facts such as width, tags, byte order, maximum encoded extent, malformed-input behavior, and compatibility identity to checking and planning without contaminating the type itself.

---

---

## Checked type identity is not source spelling

Source aliases are authoring convenience only.

> **Two source spellings that resolve to the same exact semantic type must produce the same checked type/fore identity. Renaming an alias must not change plan compatibility.**

This applies to startup parameters as well as runtime ports.

Checked startup-parameter contracts therefore carry canonical semantic type/profile identity, not raw alias text.

If omitted arguments are semantically filled before realization and a changed default changes meaning, the canonicalized default semantics must be represented at the correct checked layer rather than hidden behind a mere `has_default` bit.

Implementation/artifact identity is never a substitute for semantic type identity.

Provenance: #3724.


---

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

---

## keep: explicit retained current value

Canonical retained-value grammar:

```conduit
count:    keep Count(0) for life
middle:   keep Integer for this play
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

Gear-name sugar resolves the one eligible input/output port by cord direction. Ambiguity must refuse and require explicit port spelling.

Provenance: #3956, #3977.

---

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

`&T` is **not** a pointer, borrow, path, residence, capability, handle or authority token. Initially reject accidental recursive `&&T` spelling unless a concrete semantic need later earns it.

`&T` in type grammar coexists with infix bitwise `&` in expression grammar. Grammar position, never whitespace, distinguishes them.

keep duration and explicit publication are different promises:

> **If a plot says a value must last, planning selects a Back that can truthfully make it last or refuses. save is publication, not survival.**

Therefore:

- Body-lived keep may survive Host/Boot replacement without authored save/load;
- save produces one immutable independently addressable generation after complete validation;
- load returns ordinary info and does not receive privileged access to a destination keep;
- disk names durable data residence without implying POSIX paths/VFS/files;
- generic non-content resource pools remain resources where data would be nonsense.

Exact catalog Kind paths for save/load remain owned by their semantic catalog; do not freeze an incidental implementation spelling here.

Living proof owner: #4116.

Provenance: #3954, #3958.

---

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

Exact authored sequence syntax beyond the accepted structural forms remains subject to parser/canon conformance; do not invent a second collection language.

Provenance: #3717, structured-info ancestry #1386/#1387.


---

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

---

## Finite variants

Semantic law is canonical: variants are finite typed structured info, with exact cases and optional bounded payloads.

Existing structured-info heritage such as:

```conduit
note_on({ velocity: 96, pitches: [60, 62, 64] })
```

is the preferred construction direction.

Graph patterns use the routing machinery. Preferred direction already recorded:

```conduit
event >> ? {
    [MusicEvent.note]: . >> play-note
    [MusicEvent.rest]: . >> keep-silence
}
```

Closed variants must be exhaustive.

**STATUS: FROZEN.** Qualified construction uses `Variant.case(payload)` / payloadless `Variant.case`; graph matching uses `[Variant.case]`. Closed variants remain exhaustive. See #4002.

Provenance: #4002.

---

---

## Resource/capability-valued ports

Low-level plots may pass admitted runtime resources through typed ports, but **descriptive info cannot forge authority**.

```text
{ address, length }        descriptive info
MmioRegion capability      admitted unforgeable resource/authority
```

Resource values are created only by admitted backs/kernel operations and carry exact lifecycle/provenance constraints. Impossible cross-host transfer must refuse before play.

No raw pointers, integer handles as authority, or serializable capability tokens by default.

Canonical source spelling is `resource T`. There are no resource literals and no parallel `capability T` wrapper.

Provenance: #4065.

---
