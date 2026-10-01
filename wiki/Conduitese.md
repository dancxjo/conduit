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
| **form** | one portable concrete representation of a type |
| **host** | intended finite host construction |
| **body** | intended durable body/part/host construction |
| **pack** | source shipment and dependency description |

These share one lexical and diagnostic world, but they do not mean the same thing.

A plot describes **what should happen**. A type describes **what information means**. A form describes **how values of that type may be carried or stored**. Host and body source describe intended construction. A pack describes source/distribution identity.

None of those declarations creates live runtime truth such as the current `BootId`, active plan, present line, current authority, or observed device.

The repository is currently migrating the historical executable keyword `form` to **plot**, while reusing **form** for type representation. Learning pages use the durable vocabulary; [[Current language surface|Current-language-surface]] records the exact parser surface during migration.

## Plots are semantic graphs

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

## Kind, fore, back, gear

These four words are worth learning early:

```text
kind   reusable semantic meaning of work
fore   the checked callable boundary of that kind/plot
back   one concrete realization behind that fore
gear   one configured occurrence of a kind in a form
```

A gear invokes a kind through its fore. A host offers a back. A plan selects the back.

Some older repository prose and internal Rust names still say **front**. The current language/architecture noun is **fore**. The human-facing noun is **face**.

## Plots are live unless completed

A plot without a trailing full stop is live:

```conduit
plot clock-demo {
    clock: time/every(1s)
    clock >> presentation/tick
}
```

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

The `.` is not an executable "stop now" statement. It changes the meaning of the plot boundary.

## Finite by default

Every checked value type has an exact finite bound.

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
with audio/forms/tone
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

## Types have forms

A semantic type and its concrete representation are deliberately separate.

Conceptually:

```conduit
type LeniaRegionChunkKind =
    work
    | result

form alife/lenia-region-chunk-kind =
    LeniaRegionChunkKind as u8 from 1
```

The type says that the semantic alternatives are `work` and `result`. The form says that one portable compatibility contract carries them as consecutive `u8` tags beginning at 1.

Thus `work` does not *mean* `1`; it has the code `1` **in this form**.

A type may have multiple forms without changing its semantic identity. Wire/storage/ABI details remain exact and checkable without becoming the meaning of the value.

## Host source is still Conduitese

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

## Body wardrobe is authored, too

Masks remain ordinary plots. A body may state encounter policy:

```conduit
with masks/native-graphical as graphical
with masks/spoken as spoken

body roseau {
    wear graphical else spoken
    want graphical over spoken
}
```

The fallback is admitted into the plan. Without authored `else`, loss requires ordinary replacement planning instead of a secret runtime fallback.

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

The live v1 self-authoring epic is [#4375](https://github.com/dancxjo/conduit/issues/4375). Current work includes bounded collection algebra and migration of portable semantic types into native Conduitese.

Proposed syntax in open issues is **not automatically canon**. The exact current language surface lives in [[Current language surface|Current-language-surface]].
