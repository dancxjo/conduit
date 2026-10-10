Conduitese is the authored language of Conduit.

The simplest description is:

> **Conduitese describes semantic intent and construction truth without baking present realization machinery into portable meaning.**

It is intentionally unlike an imperative systems language. There is no ambient process model, hidden callback loop, arbitrary mutable heap, exception unwinding, or “just call this platform API” escape hatch in portable source.

The language is designed around a question: **what must remain true if the same authored meaning is realized on different machinery?** Whatever belongs to that answer should be visible in semantic source; whatever depends on the current world belongs to construction, planning, or execution.

## One language, several kinds of authored truth

Canonical `.conduit` source carries several different kinds of statements:

| authored thing | purpose |
|---|---|
| **plot** | portable semantic work and relationships |
| **type** | reusable semantic meaning of values |
| **form** | one exact portable representation of a type |
| **host** | intended finite host construction |
| **body** | intended durable body/part/host construction |
| **pack** | source shipment and dependency description |

These share one lexical and diagnostic world, but they do not mean the same thing.

A plot describes **what should happen**. A type describes **what information means**. A form describes **one portable representation of a type**. host and body source describe intended construction. A pack describes source/distribution identity.

None of those declarations creates live runtime truth such as the current `BootId`, active plan, present line, current authority, or observed device.

Current source uses `plot` for executable composition and `form` for portable
representation. See [[Current language surface|Current-language-surface]] for
the checked grammar and proof boundaries.

Read [[Conduitese by example|Conduitese-by-example]] for complete source,
[[units and quantities|Units-and-quantities]] for dimensional meaning and exact
conversion, and [[the feature example index|Conduitese-feature-coverage]] for
every reviewed surface and the explicitly proposed additions.

## plots are semantic graphs

```conduit
plot button_across_room {
    button: input/button
    state: input/button-indicator-state
    indicator: presentation/indicator-state

    button >> state >> indicator
}
```

A named **gear** is one configured occurrence of a semantic **kind**. Typed **ports** are connected by **cords**. `>>` is the authored direction token.

The plot says what should connect. A plan later decides where and how those connections are realized.

## kind, fore, back, gear

These four words are worth learning early:

```text
kind   reusable semantic meaning of work
fore   the checked callable boundary of that kind/plot
back   one concrete realization behind that fore
gear   one configured occurrence of a kind in a plot
```

A gear invokes a kind through its **fore**. A host offers a compatible back. A plan selects the exact back.

A fore is basically Conduit's function signature: the callable surface an author sees and writes against. For a plot, the parenthesized declaration is its fore; for a kind, the fore records the same callable shape in the catalog.

A checked fore includes the public calling facts that affect compatibility, such as startup parameters, runtime ports, their names and directions, value types, temporal shapes, and any declared shorthand input-to-output path.

Some older repository prose and internal Rust names still say **front**. That is historical/internal vocabulary for this callable signature. **face is unrelated**: face belongs to human interaction and presentation, not callability.

## plots are live unless completed

A plot without a trailing full stop is live:

```conduit
plot clock-demo {
    clock: time/every(1s)
    clock >> presentation/tick
}
```

![The clock demo plot as connected gears](assets/sample-diagrams/clock-demo.svg)

Structural drain means quiescence. Later admitted input may resume the same play.

A trailing full stop makes structural drain a completion witness:

```conduit
plot finite-example (
    >> input: Text
    output: Text >>
) {
    input >> text/upper >> output
}.
```

![The finite example plot as connected gears](assets/sample-diagrams/finite-example.svg)

The `.` is not an executable "stop now" statement. It changes the meaning of the plot boundary.

## Finite by default

Every actual executable value has a finite admitted carrier. A semantic numeric
domain may remain open-ended; that does not promise unlimited runtime storage.

```conduit
title: Text
memo: Text <= 4KiB
count: Count in 1..=100
choice: Text <= 8B in ["x", "y", "z"]
code: Text <= 64B ~ /[A-Z]{2}[0-9]{2}/
name: Text <= 32B not in ["root", "admin"]
```

Text without an explicit bound receives the canonical finite default during checking.

Refinements are checked relations, not runtime validation callbacks.

## Values, flows, and Current are different

Conduitese does not pretend all "things that contain T" are interchangeable.

```text
T       one ordinary value
T...    flow of values
T...|   flow with meaningful normal close
$T      current retained value
T?      optional value
$T?     optional current value
```

The distinctions matter for pressure, completion, sampling, state, cancellation, and replay.

## keep is retained current truth

```conduit
frequency: keep Frequency(440Hz) for this play
```

A `keep` is not an imperative variable. It is semantic current state with an exact lifetime.

Longer-lived plots may say:

```conduit
note: keep Text <= 4KiB for life
```

The persistence vertical is deliberately strict about the difference between "retain current truth" and "publish an immutable saved generation."

See [[State, time and data|State-time-and-data]].

## Explicit Current sampling

There is no silent `$T -> T` coercion.

The standard glyph `@` names ordinary `current/sample`:

```conduit
note @ save-request >> save.value
```

This means: when the semantic trigger is consumed, sample the exact causally visible current note and emit one ordinary value.

## Glyphs are gear names, not operators

The default glyph prelude includes:

```text
><   flow/merge
&>   flow/zip
?>   flow/race
<>   state/combine-latest
@    current/sample
```

A glyph is merely a lexical name for an ordinary gear.

```conduit
with text/upper as ^^

input ^^ output
```

has the same checked meaning as:

```conduit
input >> text/upper >> output
```

There is no custom precedence, type-directed overloading, or hidden glyph runtime.

Authors can opt out:

```conduit
sans glyphs
```

## Imports and packs

Imports use `with`:

```conduit
with audio/plots/tone
with math/geometry/{vector2, matrix2}
with house/sensors/temperature as room-temperature
with text/upper as ^^
```

The ecosystem noun is **pack**:

```conduit
pack house/sensors (
    version = 1.4.0
) {
    ship temperature
    need math/geometry = ^2.1
}
```

Pack identity, source identity, module paths, and semantic kind/type identity remain distinct.

## Conduitese can now author semantic types

Conduitese v1 is actively moving portable semantic type ownership out of handwritten Rust.

A merged current-tree example is:

```conduit
type LinguisticOffsetBasis =
    unicode_scalar
    | utf8_byte
```

Rust consumes a generated binding from that authoritative Conduitese type rather than independently defining its meaning.

The larger v1 direction is summarized as:

> **type : info :: kind : gear**

A type is reusable semantic meaning for finite information. An info value is one value of that type. A kind is reusable semantic meaning for work. A gear is one configured occurrence of that kind.

The migration is ongoing. Do not assume every existing Rust semantic type has moved yet.

## types have forms

A semantic type and its portable encoding are separate. This declaration comes
from [the artificial-life types](https://github.com/dancxjo/conduit/blob/dev/semantics/alife/types.conduit):

```conduit
type LeniaRegionChunkKind =
    work
    | result

form alife/lenia-region-chunk-kind = LeniaRegionChunkKind as u8 from 1
```

The type owns the alternatives. The form assigns consecutive `u8` tags starting
at 1. `work` does not mean `1`; it is carried by that tag in this form.
Generated bindings consume the checked mapping rather than restating it in Rust.

## host source is still Conduitese

Canonical host authoring keeps construction facts separate from live runtime truth:

```conduit
host conduitos-native (
    target = conduitos/x86_64/pc
    build = release
    loader = limine
) {
    surface: resource presentation/surface (
        slots = 4
        bytes = 8MiB
    )

    mmio: base machine/mmio

    framebuffer: back display/linear-framebuffer (
        memory = mmio
    )

    graphics: back presentation/graphics (
        surface = surface
        display = framebuffer
    )

    bounds = {
        heap: 16MiB,
        calls: 64,
        signs: 1024
    }
}
```

This describes machinery to build and bounds to admit. It does not claim a device was discovered, a host booted, an offer exists, or a plan selected anything.

## body wardrobe is authored, too

masks remain ordinary plots. A body may state encounter policy:

```conduit
with masks/native-graphical as graphical
with masks/spoken as spoken

body roseau {
    wear graphical, spoken
    want graphical over spoken
}
```

The comma list is unordered eligibility. The optional `want` line alone carries
preference. Same-plan recovery can select only an eligible route already sealed
by that immutable plan; a newly available route requires ordinary replacement planning.

## What Conduitese deliberately does not have

Current v1 direction explicitly rejects:

- general mutable variables;
- arbitrary imperative loops;
- exceptions and hidden unwinding;
- `async` / `await` as an execution ontology;
- classes/object identity;
- ambient services or authority;
- unbounded collections;
- author-asserted purity or replay safety;
- hidden host-language callbacks.

Instead, bounds, effects, state, terminal behavior, and temporal relationships are meant to remain inspectable.

## Current v1 growth

The completed v1 self-authoring work is recorded by [#4375](https://github.com/dancxjo/conduit/issues/4375). Authored payload-rich types, checked behavioral parameters, generated bindings, concise expansion, and bounded activation are present. Bounded each/select/fold/scan and collection are implemented with the scoped proof recorded by completed [#4378](https://github.com/dancxjo/conduit/issues/4378). Completed work includes the audited migration of portable semantic families and bounded record-law proof propagation in [#4639](https://github.com/dancxjo/conduit/issues/4639). See [[the current reference|Current-language-surface]] for exact syntax and target limits.

Proposed syntax in open issues is **not automatically canon**. The exact current language surface lives in [[Current language surface|Current-language-surface]].
