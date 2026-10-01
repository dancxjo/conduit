> **Canonical reference.** These pages were migrated from issue [#4109](https://github.com/dancxjo/conduit/issues/4109) on 2026-09-29. The wiki is now the readable language reference; implementation tickets remain evidence of conformance and provenance.

This page collects the most recent frozen authored spellings. Detailed semantic laws live on the topic pages linked in the sidebar.

## 2026-09-28 authored-surface settlement

The remaining deliberately-unfrozen authored surfaces have earned canonical spelling. These decisions are normative; the linked tickets own implementation/conformance.

### Named type parameters

```conduit
form latest (
    item: type

    >> values: item...
    current: $item >>
) {
    ...
}
```

`type` is the canonical compile-time parameter declaration. When inference is insufficient, explicit application uses ordinary named arguments such as `latest(item = Text)`. No angle-bracket generic surface, wildcard zoo, implicit `any`, or runtime-erasure requirement is admitted. Owner: #4059.

### Explicit Current sampling

There is no implicit `$T -> T` coercion. `current/sample` samples one Current value for each accepted semantic trigger; `$T` yields `T`, and `$T?` yields `T?`. The sampled generation is the exact current generation causally visible when the trigger is consumed. It is distinct from cadence-driven `time/sample`.

Canonical concise spelling:

```conduit
note @ save-request >> save.value
```

Owner: #4064; first persistence proof: #4116.

### Finite variants

Construction is qualified by exact variant type:

```conduit
MusicEvent.note({ velocity: 96, pitches: [60, 62, 64] })
MusicEvent.rest
```

Graph matching is:

```conduit
event >> ? {
    [MusicEvent.note]: . >> play-note
    [MusicEvent.rest]: . >> keep-silence
}
```

Within a selected payload-bearing case, `.` is the case payload. Payloadless cases omit empty-call ceremony. Closed variants remain exhaustive. Owner: #4002.

### Gear glyphs

A Gear may have an ordinary word name or a lexical glyph name. A glyph names an already-defined Kind or one configured Gear occurrence; it does not define an operator, precedence, associativity, fixity, parser rule, overload set, effect, or runtime.

For a one-input / one-output Fore:

```conduit
with text/upper as ^^

input ^^ output
```

has the exact checked meaning of `input >> text/upper >> output`.

For exact multi-input Fores, glyph operands bind in canonical Fore order. A reviewed variadic homogeneous Fore may flatten repeated use into one Gear occurrence, e.g. `a >< b >< c >> merged`.

The standard glyph prelude is in lexical scope by default:

```text
><   flow/merge
&>   flow/zip
?>   flow/race
<>   state/combine-latest
@    current/sample
```

A source file may opt out at its header:

```conduit
sans glyphs
```

Explicit glyph imports remain legal afterward. One glyph has one lexical referent; there is no type-directed overloading. Mixed adjacent glyphs require explicit grouping. The prelude is versioned with the language surface and participates in checked source identity; it is not a mutable ambient pack dependency.

Checked expansion, Plans and Signs expose the ordinary Gear behind every glyph. There is no glyph runtime. Owner: #4335.

### Checked refinements and portable patterns

`when(...)` remains runtime graph filtering. Checked type refinements are written directly as relations on the type:

```conduit
choice: Text <= 8B in ["x", "y", "z"]
count: Count in 1..=100
code: Text <= 64B ~ /matches(?:lookahead)(?<name>[A-Z]+)/i
```

Adjacent refinement relations are conjunctive:

```conduit
code: Text <= 8B in ["AB12", "CD34"] ~ /[A-Z]{2}[0-9]{2}/
```

`in [a, b, c]` is finite membership. `in a..b` is lower-inclusive / upper-exclusive; `in a..=b` is inclusive at both ends. Missing range ends are legal only when the base type supplies the corresponding finite bound.

`~ /.../flags` is the canonical portable text-pattern relation. Slash literals use Conduit's bounded regular language, not a Host-selected regex dialect. The admitted language includes ordinary regular constructs plus non-capturing groups, named groups, and bounded-compilable positive/negative lookahead; it excludes backreferences, recursion, embedded code and any construct whose work cannot be admitted finitely.

`~ /pattern/` succeeds when the pattern has a match within the bounded text. Authors use canonical anchors when whole-value matching is intended. Flags are a finite reviewed Conduit set and participate in checked identity; a Boolean refinement does not admit a meaningless global-iteration flag.

The older `where pattern(...)`, `where range(...)` and `where member(...)` spellings are migration targets, not compatibility aliases. Owner: #4199.

### Runtime-bound resources

Canonical authored type spelling is `resource T`:

```conduit
region: resource machine/memory/mmio/region
surface: resource presentation/surface
```

Keep distinct:

```text
{ address, length }   forgeable descriptive info
&T                    ordinary info naming immutable data
resource T            admitted runtime possession
```

There are no resource literals and no parallel `capability T` wrapper. Owner: #4065.

### Imports and packs

Source imports use `with`:

```conduit
with audio/forms/tone
with math/geometry/{vector2, matrix2}
with house/sensors/temperature as room-temperature
with text/upper as ^^
```

Aliases may be ordinary names or admitted Gear glyphs.

The authored ecosystem noun is **pack**. Pack authoring uses `pack.conduit`:

```conduit
pack house/sensors (
    version = 1.4.0
) {
    ship temperature
    need math/geometry = ^2.1
}
```

`package` is not an authored compatibility keyword. Resolution produces generated exact lock truth in `conduit.lock`. Pack/version/module/source/content/distribution identities remain distinct from semantic Kind identity. Imports grant no runtime authority and execute no code. Owner: #4055.

### Body wardrobe

Mask remains an ordinary Form role; there is no `mask` declaration.

```conduit
with masks/native-graphical as graphical
with masks/spoken as spoken

body roseau {
    wear graphical else spoken
    want graphical over spoken
}
```

`wear a else b` admits fallback structure into the Plan. Without authored `else`, loss requires ordinary replacement planning. `want` is policy only among eligible alternatives. Runtime `wear` and `doff` are Body-control actions requesting wardrobe change and therefore new planning where required; they never mutate an immutable Plan in place. Owner: #4115.

### Host source

Host source is limited to construction truth: construction parameters, finite declared `resource` pools, concrete `base` boundaries, reviewed `back` realizations, and explicit policy/bounds.

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

    policy = {
        authority: explicit,
        ambient: false
    }

    bounds = {
        heap: 16MiB,
        calls: 64,
        signs: 1024
    }
}
```

`driver` and `facility` are not separate source ontologies. Host source does not author current HostId, BootId, offers, device instances, authority, observations, Lines, Plans or Plays. A Line remains current connectivity realization, not profile source. Owner: #4117.


---
