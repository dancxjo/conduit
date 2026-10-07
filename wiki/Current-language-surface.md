# Current language surface

This reference describes the checked development surface reviewed on
**1 October 2026**, with the Plot/Form vocabulary reviewed on **2 October 2026**.
Source tests linked below establish grammar and checking;
target tests establish the named execution paths. Published products can lag
`dev`. Detailed semantic laws live on the topic pages in the sidebar.

## Declarations and status

- `plot` declares executable composition; its parenthesized **fore** is the callable signature
- `type` declares semantic value meaning
- `form` declares a portable representation, separate from type identity
- `host`, `body`, and `pack` declare construction or shipment truth, not live runtime state

The paired executable `form` to `plot` and representation `code` to `form`
migration is implemented on `dev` by [#4800](https://github.com/dancxjo/conduit/pull/4800).
Both former declaration spellings are rejected rather than retained as aliases. See the
[parser](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/surface_parser.rs)
and [syntax tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/syntax_check_tests.rs).

## Callable composition

### Named type parameters

This signature sketch omits its implementation; `...` below is an editorial
placeholder, not runnable body syntax. The complete bounded `flow/each` example
later on this page shows a checked generic implementation.

```conduit
plot latest (
    item: type

    >> values: item...
    current: $item >>
) {
    ...
}
```

`type` is the canonical compile-time parameter declaration. When inference is insufficient, explicit application uses ordinary named arguments such as `latest(item = Text)`. This plot-parameter surface does not require runtime erasure, implicit `any`, or hidden closures. Owner: #4059.

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

### gear glyphs

A gear may have an ordinary word name or a lexical glyph name. A glyph names an already-defined kind or one configured gear occurrence; it does not define an operator, precedence, associativity, fixity, parser rule, overload set, effect, or runtime.

For a one-input / one-output fore:

```conduit
with text/upper as ^^

input ^^ output
```

has the exact checked meaning of `input >> text/upper >> output`.

For exact multi-input fores, glyph operands bind in canonical fore order. A reviewed variadic homogeneous fore may flatten repeated use into one gear occurrence, e.g. `a >< b >< c >> merged`.

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

Checked expansion, plans and signs expose the ordinary gear behind every glyph. There is no glyph runtime. Owner: #4335.

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

`in [a, b, c]` is finite membership. `in a..b` is lower-inclusive / upper-exclusive; `in a..=b` is inclusive at both ends. Missing range ends express semantic openness: `Count in 4..` has no authored upper endpoint, while `Scalar in ..=1.000000` has no authored lower endpoint. This does not reserve infinite storage; each actual value still needs a finite admitted carrier. `in ..` adds no refinement and is rejected. See the [range tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/refinement_tests.rs).

`~ /.../flags` is the canonical portable text-pattern relation. Slash literals use Conduit's bounded regular language, not a host-selected regex dialect. The admitted language includes ordinary regular constructs plus non-capturing groups, named groups, and bounded-compilable positive/negative lookahead; it excludes backreferences, recursion, embedded code and any construct whose work cannot be admitted finitely.

`~ /pattern/` succeeds when the pattern has a match within the bounded text. Authors use canonical anchors when whole-value matching is intended. Flags are a finite reviewed Conduit set and participate in checked identity; a Boolean refinement does not admit a meaningless global-iteration flag.

The older `where pattern(...)`, `where range(...)` and `where member(...)` spellings are migration targets, not compatibility aliases. Owner: #4199.

## Native types, Forms, and record laws

### Generic native types

Native type declarations can bind checked type parameters using angle brackets:

```conduit
type Pair<T> = {
    left: T
    right: T
}

type TextPair = Pair<Text <= 16B>
```

This differs from a plot's named `item: type` parameter above. Checking
substitutes each argument through the complete finite structure and its laws.
The concrete identity includes the exact generic declaration and arguments;
play receives no open parameter, runtime closure, or dynamic dispatcher.
Invalid arity, unused/duplicate parameters, unknown applications, and recursive
or unbounded instantiation refuse. See the
[generic type tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/syntax_check_tests.rs)
and [checker](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/native_type/generic.rs).

A real current family is
[`DataGenerationValue<T>`](https://github.com/dancxjo/conduit/blob/dev/semantics/data/types.conduit),
whose text specialization is bounded to 4096 bytes. This language support does
not mean all generic domain families have already migrated.

### Compact Forms

This [checked-in declaration](https://github.com/dancxjo/conduit/blob/dev/semantics/alife/types.conduit)
keeps semantic alternatives separate from byte tags:

```conduit
type LeniaRegionChunkKind =
    work
    | result

form alife/lenia-region-chunk-kind = LeniaRegionChunkKind as u8 from 1
```

The compact form assigns consecutive `u8` tags beginning at 1. The checked
mapping owns finite extent/work, compatibility identity, and invalid-tag
refusal; generated bindings do not independently restate the mapping. This
example does not claim every proposed record or wire-layout encoding exists.

### Record laws

Records can own pure Boolean laws over the complete value:

```conduit
type Interval = {
    start: U32
    end: U32
    where .start <= .end
}
```

`where` laws participate in type identity and are enforced at generated
construction and decode boundaries. They use the finite checked expression
language. Real declarations include
[linguistic spans](https://github.com/dancxjo/conduit/blob/dev/semantics/language/types.conduit)
and [audio frame laws](https://github.com/dancxjo/conduit/blob/dev/semantics/audio/types.conduit).
The [generated-binding tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/rust_binding/generate_tests.rs)
cover refusal and independent bindings.

`variant/tag(value)` returns ordinary `Text`. A comparison such as
`variant/tag(.direction) == "mono"` is checked as text equality; the compared
literal is not validated against the variant alternatives. A typo can therefore
remain well-typed while making a law false. See the
[semantic call checker](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/expression_semantic_call.rs)
and the record-law syntax tests.

`text/material(value)` explicitly observes the exact UTF-8 material of primitive
or nominal `Text` as ordinary `Text`. This permits material equality between
distinct checked text contracts without implicitly converting either value into
the other contract. It rejects non-text representations and does not admit a
refined nominal output; that output still requires its own construction laws.

Construction-time enforcement is implemented by
[#4638](https://github.com/dancxjo/conduit/pull/4638). Completed
[#4639](https://github.com/dancxjo/conduit/issues/4639) carries applicable record
and scalar invariant facts into consuming-plot arithmetic proofs. For example,
`.end - .start` is proven safe when the input type establishes `.start <= .end`;
`.end + 1` remains checked without a sufficient upper bound. The
[portable arithmetic tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/canonical_expansion_tests.rs)
cover both outcomes. This is bounded proof propagation, not permission to erase
arbitrary arithmetic checks.

## Bounded each, select, fold, and scan

These are checked activation coordinators around exact selected behavior,
not general loops or runtime closures. The following wrapper is copied from
[activation conformance source](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/activation_tests.rs):

```conduit
plot flow/each (
    item: type
    result: type
    transform: kind (
        >> value: item
        mapped: result >>
    )
    >> values: item...|
    mapped: result...| >>
) {
    each: activate(maximum-items = 4) transform()
    values >> each.value
    each.mapped >> mapped
}
```

Here `flow/each` is a **source-defined wrapper**, not an implicit installed
catalog kind. With an exact checked `text/normalize` plot in scope, the fixture
specializes it with:

```conduit
mapped: flow/each(item = Text, result = Text, transform = text/normalize)
```

The same test file defines these body fragments within full declared fores:

```conduit
selection: select(maximum-items = 4) predicate()
values >> selection.value
selection.selected >> selected

folder: fold(initial, maximum-items = 4) integer/add()
items >> folder.item
folder.combined >> result

scanner: scan(initial, maximum-items = 4) integer/add()
items >> scanner.item
scanner.combined >> accumulators
```

- `activate` requires exact value-input/value-output behavior; a flow behavior is refused
- `select` calls a Boolean predicate and emits the retained original item through `selected` when true
- `fold` and `scan` require two value inputs, `accumulator` and `item`, and one `combined` output; the fixtures use exact `U64 <= 28B` ports and an `initial: U64` parameter
- `fold` returns one value on normal close; `scan` exposes the closing-flow progression
- `maximum-items` is a positive exact `u16` admission bound, not an unbounded iterator

The predicate/combine declarations in the checking fixtures are contract
fixtures, not complete useful algorithms. Their execution is proved separately
through the selected target backs.

[#4378 is complete](https://github.com/dancxjo/conduit/issues/4378#issuecomment-5923888244)
for each/select/fold/scan and bounded collection from a closing flow. Evidence
covers [std activation](https://github.com/dancxjo/conduit/blob/dev/targets/std/src/flow_activation/tests.rs),
[browser runtime activation](https://github.com/dancxjo/conduit/blob/dev/targets/browser/runtime/src/flow_activation/tests.rs),
[installed browser behavior](https://github.com/dancxjo/conduit/blob/dev/targets/browser/runtime/src/flow_activation/tests/installed_inventory.rs),
and [shared collect laws](https://github.com/dancxjo/conduit/blob/dev/semantics/data/src/flow_collect_back_tests.rs).
Pressure, cancellation, abnormal termination, bounds, and receipt identity stay
explicit. Browser runtime/WASM proof is not browser interaction/E2E proof.
Embedded applicability was audited, but current admitted ConduitOS resident
plots do not exercise these combinators; this is not universal embedded
execution evidence.

## Resources and construction

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
with audio/plots/tone
with math/geometry/{vector2, matrix2}
with house/sensors/temperature as room-temperature
with text/upper as ^^
```

Aliases may be ordinary names or admitted gear glyphs.

The authored ecosystem noun is **pack**. Pack authoring uses `pack.conduit`:

```conduit
pack house/sensors (
    version = 1.4.0
) {
    ship temperature
    need math/geometry = ^2.1
}
```

`package` is not an authored compatibility keyword. Resolution produces generated exact lock truth in `conduit.lock`. Pack/version/module/source/content/distribution identities remain distinct from semantic kind identity. Imports grant no runtime authority and execute no code. Owner: #4055.

### body wardrobe

mask remains an ordinary plot role; there is no `mask` declaration.

```conduit
with masks/native-graphical as graphical
with masks/spoken as spoken

body roseau {
    wear graphical, spoken
    want graphical over spoken
}
```

`wear a, b` is an unordered bounded set of permitted masks; comma order carries no preference. `want` is optional ordered policy only among eligible alternatives. Same-plan selection may use only exact routes already sealed by that plan; otherwise loss requires ordinary replacement planning. The legacy `wear a else b` spelling is rejected because its ordered fallback meaning cannot be silently migrated into unordered eligibility. Runtime `wear` and `doff` request planning where required and never mutate an immutable plan in place. Owner: #4115.

Body construction schema 2 owns this wardrobe shape. Schema 1 remains a
distinct legacy contract and is not reinterpreted as unordered eligibility.

### host source

host source is limited to construction truth: construction parameters, finite declared `resource` pools, concrete `base` boundaries, reviewed `back` realizations, and explicit policy/bounds.

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

`driver` and `facility` are not separate source ontologies. host source does not author current HostId, BootId, offers, device instances, authority, observations, lines, plans or plays. A line remains current connectivity realization, not profile source. Owner: #4117.


---
