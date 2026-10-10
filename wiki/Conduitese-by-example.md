# Conduitese by example

These examples cover current development source reviewed on **9 October 2026**.
The standalone language examples can be saved as `.conduit` source and checked
with `conduit check example.conduit`. Repository domain examples require their
own reviewed catalog; not every domain Kind is installed in the standard CLI.
Their source links identify that boundary. A source fragment is labeled as
such. Checking, expansion and execution prove different things.

For a deeper worked tutorial, start with [[units and quantities|Units-and-quantities]].
The [[feature example index|Conduitese-feature-coverage]] maps the remaining
language surfaces and separates current source from proposed extensions.

## Hello: a finite pipeline

Current tree: [plots/hello/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/hello/main.conduit)

```conduit
plot hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." >> upper >> show
}.
```

![The hello plot as connected gears](assets/sample-diagrams/hello.svg)

There is no explicit main function, process, display handle, or stdout. The plot composes semantic work. The full stop says drain completes the plot.

## Clock: a standing live plot

Current tree: [plots/clock/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/clock/main.conduit)

```conduit
plot clock-demo {
    clock: time/every(1s)
    clock >> presentation/tick
}
```

![The clock demo plot as connected gears](assets/sample-diagrams/clock-demo.svg)

No trailing full stop. The plot remains alive between ticks.

## Memory Lantern: input, editing, and Current

Current tree: [plots/memory-lantern/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/memory-lantern/main.conduit)

```conduit
plot memory_lantern {
    keyboard: input/keyboard
    keymap: input/keymap
    message: text/edit(maximum-bytes = 256)
    show: presentation/text

    keyboard.key >> keymap.key
    keymap.text >> message.fragment
    message.current >> show.text
}
```

The editor exposes current text. The plot does not own a window, DOM node, terminal, or framebuffer.

## Pocket Theremin: quantities and retained state

Current tree: [plots/pocket-theremin/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/pocket-theremin/main.conduit)

```conduit
plot pocket-theremin (
    >> distance: Distance
    audio: audio/pcm-frames@1...| >>
) {
    map: math/map-distance-frequency(source-minimum = 0cm, source-maximum = 30cm, target-minimum = 220Hz, target-maximum = 880Hz)
    frequency: keep Frequency(440Hz) for this play
    tone: audio/tone

    distance >> map.distance
    map.frequency >> frequency
    frequency >> tone.frequency
    tone.audio >> audio
}.
```

The units are semantic. The `keep` is current frequency state. Browser audio, hosted audio, or a future native realization can sit behind the same meaning.
This excerpt normalizes the invocation layout; its mapping/audio Kinds require
the domain's catalog. [[The quantity tutorial|Units-and-quantities]] starts with
standalone examples checkable in the installed CLI.

The `@1` catalog spelling in some current-tree examples is implementation/migration history, not a reason to invent new version-suffix syntax in authored canon.

## Button across the room: meaning without transport

Current tree: [plots/button-across-room/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/button-across-room/main.conduit)

```conduit
plot button_across_room {
    button: input/button
    state: input/button-indicator-state
    indicator: presentation/indicator-state

    button >> state >> indicator
}
```

Nothing here says whether button and indicator are on the same host. If planning places them apart, a line may realize the crossing cord.

## Desk Telegraph: reusable plots as gears

Current tree: [plots/desk-telegraph/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/desk-telegraph/main.conduit)

```conduit
plot text-record (
    text: Text >> record: TypedRecord
) {
    wrap: record/text-to-typed
    text >> wrap.text
    wrap.record >> record
}

plot bounded-record-send (
    maximum-items: Count = 4
    maximum-frame-bytes: Count = 4096
    frame: FramedTypedRecord >> queued: FramedTypedRecord
) {
    queue: record/ordered-send-queue(maximum-items, maximum-frame-bytes)
    frame >> queue >> queued
}
```

These are declarations excerpted from the larger Desk Telegraph source. A plot
can itself define reusable semantic work and then be invoked like another kind.
The standard CLI currently refuses these record Port profiles as exceeding
canonical bounds; this source excerpt is not a standalone execution recipe.
Use [[text identity|Conduitese-by-example#specialize-a-reusable-plot]] for a
standalone checked reuse example.

## Firefly Choir: omission is semantic composition

Current tree: [plots/firefly-choir/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/firefly-choir/main.conduit)

```conduit
plot pulse-manifestation (
    >> tick: value/tick@1...
) {
    observe: time/pulse-observe(period-ms = 240)
    light: presentation/pulse

    tick >> observe.tick
    observe.observation >> light.pulse
}
```

![The light-only pulse manifestation as connected gears](assets/sample-diagrams/pulse-manifestation.svg)

This is light-only because there is no tone gear and no tone cord. Sound is not a runtime flag that a host may quietly toggle.

## Explicit enrichment

```conduit
plot pulse-light-tone-manifestation (
    >> tick: value/tick@1...
) {
    observe: time/pulse-observe(period-ms = 240)
    light: presentation/pulse
    tone: sound/pulse-tone

    tick >> observe.tick
    observe.observation >> light.pulse
    observe.observation >> tone.pulse
}
```

![The light-and-tone pulse manifestation with explicit fan-out](assets/sample-diagrams/pulse-light-tone-manifestation.svg)

The fan-out is visible.

## Bounded navigation: portable intent down to motion requests

Current tree: [plots/bounded-navigation/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/bounded-navigation/main.conduit)

```conduit
plot bounded-navigation (
    >> pose: NavigationPose
    >> goal: NavigationGoal
    >> traversability: NavigationTraversability4x4
    >> time: NavigationTime
    decision: NavigationRouteDecision >>
    control: NavigationControl >>
    request: RoboticsMotionRequest >>
) {
    route: navigation/route-grid4
    timing: navigation/time-parameterize
    controller: navigation/local-control

    pose >> route.pose
    goal >> route.goal
    traversability >> route.traversability
    time >> route.time
    route.decision >> decision
    route.decision >> select(NavigationRouteDecision.route, unmatched=drop) >> timing.route
    time >> timing.time
    timing.trajectory >> controller.trajectory
    pose >> controller.pose
    time >> controller.time
    controller.control >> control
    controller.control >> select(NavigationControl.motion, unmatched=drop) >> request
}
```

The plot ends at semantic body-motion intent. Robot authority and actuator lowering belong to the plan and host realization.

## body Chat: application meaning, model choice still realization

Current tree: [plots/body-chat/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/body-chat/main.conduit)

```conduit
plot body-chat (
    >> interaction: FaceInteraction...|
    face: Presentation...| >>
) {
    ui: chat/state(title = "Body Chat", history-label = "Conversation", input-label = "Message", submit-label = "Send", status-label = "Body", maximum-history-items = 16, maximum-message-bytes = 256)
    submit: chat/submit(action = "chat/send", maximum-message-bytes = 256)
    available: boolean/literal(value = true)
    availability: chat/current-connection
    context: body/conversation-context
    prompt: body/chat-prompt
    model: llm/generate-flow(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = 2560, maximum-work-units = 4096, maximum-history-items = 0)
    text: llm/result-flow-to-text

    available.value >> availability.source
    availability.value >> ui.live
    ui.presentation >> face
    interaction >> submit.interaction
    submit.message >> prompt.message
    context.context >> prompt.context
    prompt.human-message >> ui.message
    prompt.request >> model.request
    model.result >> text.result
    text.text >> prompt.response
    prompt.body-message >> ui.message
}
```

The semantic model operation is named. Provider protocol, URL, credentials, or Ollama identity do not belong in the plot.

## A mask is an ordinary plot

Current tree: [plots/native-graphical-mask/main.conduit](https://github.com/dancxjo/conduit/blob/dev/plots/native-graphical-mask/main.conduit)

```conduit
plot native-graphical (
    >> face: Presentation
    interaction: FaceInteraction...| >>
    show: Show >>
) {
    spread: presentation/tee
    surface: presentation/show-resource-source
    render: presentation/resource-renderer
    human: face/interaction

    face >> spread.presentation
    spread.presentation >> render.presentation
    surface.show-resource >> render.show-resource
    spread.presentation >> human.presentation
    render.show >> human.show
    render.show >> show
    human.interaction >> interaction
}
```

mask is a role, not a special declaration or second UI language.

## Checked refinements

Canonical:

```conduit
plot guarded (
    >> name: Text <= 32B not in ["root", "admin"]
    >> count: Count in 1..=100
    >> code: Text <= 64B ~ /[A-Z]{2}[0-9]{2}/
) {
}
```

These relations become part of checked semantic type identity.
This declaration demonstrates the checked Fore only; its empty body supplies
no consuming behavior. The refinements belong to the Types, rather than to
runtime validation callbacks.

## Glyphs remain inspectable gears

Canonical:

```conduit
with text/upper as ^^

input ^^ output
a >< b >< c >> merged
note @ save-request >> snapshot
```

The checker can always expose `text/upper`, `flow/merge`, and `current/sample` behind the concise spelling.

## A native semantic type

Merged current tree:

```conduit
type LinguisticOffsetBasis =
    unicode_scalar
    | utf8_byte
```

The Rust binding is generated from this authored type.

## One type family, exact concrete values

The current [data types](https://github.com/dancxjo/conduit/blob/dev/semantics/data/types.conduit)
include this generic record and its bounded-text specialization:

```conduit
type DataGenerationValue<T> = {
    namespace: DataGenerationNamespace
    value: T
}

type DataGenerationTextValue = DataGenerationValue<Text <= 4096B>
```

`DataGenerationNamespace` is defined in that same source. `T` ranges over
checked semantic types, and the application fixes one exact finite value shape
before play. Generated bindings consume that checked shape. The generic family
does not imply unbounded values or a runtime type-erasure layer.

## Separate origin from certainty

These declarations are taken from
[the human-experience types](https://github.com/dancxjo/conduit/blob/dev/semantics/human/types.conduit):

```conduit
type ExperienceOrigin =
    observation
    | human_reported
    | remembered
    | model_derived
    | imagined

type ExperienceTemporalRole =
    current
    | recent
    | stale
    | historical

type ExperienceCertainty =
    certain
    | uncertain
```

Origin, age, and certainty are different facts. An uncertain observation and a
certain recollection should not collapse into one flag. These finite variants
make the distinctions available to checking and generated bindings; the
vocabulary alone does not prove an observation happened or make every consumer
report it honestly. That still needs the source's evidence and the consuming
contract.

## A pack

Canonical:

```conduit
pack house/sensors (
    version = 1.4.0
) {
    ship temperature
    need math/geometry = ^2.1
}
```

`conduit.lock` carries exact resolved truth. Imports grant no runtime authority.

## body wardrobe

Canonical:

```conduit
with masks/native-graphical as graphical
with masks/spoken as spoken

body roseau {
    wear graphical, spoken
    want graphical over spoken
}
```

The body permits either mask. The optional `want` line expresses its preference;
the comma list does not.

## host construction

Canonical:

```conduit
host tiny-screen (
    target = conduitos/x86_64/pc
    build = release
) {
    surface: resource presentation/surface (
        slots = 2
        bytes = 2MiB
    )

    display: base machine/display

    graphics: back presentation/graphics (
        surface = surface
        display = display
    )

    bounds = {
        heap: 8MiB,
        calls: 32,
        signs: 256
    }
}
```

This is construction source. A successful parse does not claim the machine booted or the display exists.

## Bounded collection behavior

A reusable plot can take exact checked behavior as a parameter. This is the
`flow/each` wrapper from the
[activation tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/activation_tests.rs):

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

`transform` is checked at specialization time. `activate` bounds its work and
keeps each selected invocation inspectable. The wrapper's name is defined by
this source; it is not a promise that bare `flow/each(...)` resolves everywhere.

For selection, folding, scanning, and collection proof, continue with
[[bounded activation in the reference|Current-language-surface#bounded-each-select-fold-and-scan]].
That reference gives the exact body fragments, required fores, and std/browser
proof boundaries. [#4378](https://github.com/dancxjo/conduit/issues/4378) is
completed work, not a remaining syntax gap.

## A record owns its law

```conduit
type Interval = {
    start: U32
    end: U32
    where .start <= .end
}
```

The law belongs to the type and is enforced when generated bindings construct
or decode a value. See the real
[TextSpan declaration](https://github.com/dancxjo/conduit/blob/dev/semantics/language/types.conduit).
Completed [#4639](https://github.com/dancxjo/conduit/issues/4639) carries this
relation into consuming-plot arithmetic proofs: `.end - .start` can be proven
safe, while `.end + 1` remains checked unless an upper bound justifies it.
Only operations established safe by the proof lose their runtime check.

## A pure plot with a default parameter

```conduit
plot increment (
    factor: U8 = 2
    >> value: U8
    result: U8 >>
) = (. * factor)
```

An expression plot has one current runtime input, `.`. It can capture an
immutable startup parameter. Calling `increment(factor = 3)` specializes that
parameter before play; `increment(3)` is the positional form. Omitting it uses
2. Exact integer overflow remains checked; a default does not license wrapping.

## Give an arithmetic invariant to the type

```conduit
type AlmostU32 = U32 where . < 4_294_967_295

plot next (
    >> value: AlmostU32
    result: U32 >>
) = (. + 1)
```

The input law establishes that adding one fits `U32`. This is a scalar law;
the earlier `Interval` example is a record law. Both are checked facts.
For explicit widening, the portable expression below returns 65313 for 255:

```conduit
plot widen (
    value: U8 >> result: U64
) = (value/u64(value/u16(.)) * 256 + 33)
```

See the [widening tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/integer_widening.rs).

## Construct a nested record

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
```

For input 7, the result is `{ inner: { value: 7 } }`. A consuming expression
can project `.inner.value`; the named record identity remains checked.

## Choose a payload-bearing variant

```conduit
type Choice =
    known I64
    | unknown

plot choose (
    >> value: I64
    result: Choice >>
) = (. >= 0 ? Choice.known(.) : Choice.unknown)
```

Input 7 yields `Choice.known(7)`; input −1 yields `Choice.unknown`.
Only the selected pure ternary branch evaluates. Payloadless construction
requires no empty call. A routing companion to append to the same source is:

```conduit
plot route-choice (
    >> value: Choice <= 4096B
    known: I64 >>
    unknown: Unit >>
) {
    value >> ? {
        [Choice.known] >> known
        [Choice.unknown] >> unknown
    }
}
```

Both alternatives have a route. Matching carries the selected payload directly
to its destination. Current arm syntax uses `>>`, not a colon.

## Finite collections and variable-length sequences

```conduit
type Samples = collection U16 = 2

plot make-samples (
    >> value: U16
    result: Samples >>
) = ([1, 2])
```

`Samples` has exactly two elements. The input activates this constant-producing
expression; it does not change the two samples.

```conduit
type Labels = sequence Text <= 2

plot labels (
    >> value: Boolean
    result: Labels >>
) = (["alpha", "β"])
```

`Labels` admits zero, one or two elements. Replacing the result with `[]`
produces a real empty sequence. Three labels or a Boolean element refuse.
The sequence bound counts items; `Text <= 2B` bounds bytes instead.

## Check an index against the actual length

This complete checked-expression example comes from the
[prepared selection tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/prepared_sequence_selection.rs):

```conduit
type Request = {
    bytes: sequence U8 <= 4
    index: U64
}

type Result =
    octet U8
    | short

plot guarded-index (
    value: Request >> result: Result
) = (.index < sequence/length(.bytes) ? octet(sequence/at(.bytes, .index)) : short(unit))
```

The selected branch uses a finite checked semantic call. An index inside the
capacity but outside the actual count yields `short`; capacity is not length.
The unqualified constructors here are resolved from the exact expected `Result`
Type, as in the test. Qualified constructors are useful when ambiguity exists.
The [byte observation tests](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/tests/prepared_byte_observation.rs)
apply the same law with `Bytes <= 2048B`, `bytes/length` and `bytes/at`.

## Specialize a reusable plot

```conduit
plot identity (
    item: type
    >> value: item
    result: item >>
) {
    value >> result
}

plot text-identity (
    >> value: Text
    result: Text >>
) {
    value >> identity(item = Text) >> result
}
```

The named argument binds a compile-time Type. When the Fore supplies enough
information the checker can infer it; unresolved parameters never reach play.
This differs from native `Pair<T>` family application.

## A private nested plot

```conduit
plot outer (
    item: type
    >> value: item
    mapped: item >>
) {
    plot helper (
        >> inner: item
        mapped: item >>
    ) {
        inner >> mapped
    }

    helper: helper
    value >> helper >> mapped
}
```

The helper captures the exact compile-time Type parameter. It is private to
`outer`; it cannot capture an outer runtime input or startup value as a hidden
closure. Pass such values through its own Fore explicitly.

## One shared pool, explicit consumers

The complete [pool webchat source](https://github.com/dancxjo/conduit/blob/dev/plots/pool-webchat/main.conduit)
declares a finite pool and passes its identity to ordinary consumers:

```conduit
plot chat/peer (
    recv: ChatMessage...| >> send: ChatMessage...|
) {
}

plot pool-webchat {
    pool peers: chat/peer(size = 32)
    merge: flow/merge(peers)
    room: chat/room(peers)
    fan: flow/fan(peers)

    merge.message >> room.recv
    room.send >> fan.message
}
```

The excerpt supplies the intended member Fore; its realization is separately
selected. The standard CLI currently refuses this `ChatMessage` Port profile
as exceeding canonical bounds. The finite pool language feature itself has
[checking and expansion fixtures](https://github.com/dancxjo/conduit/blob/dev/architecture/plot/src/canonical_expansion_tests.rs)
with their own exact catalogs; this excerpt is not an installed chat recipe. The three consumers refer to the same exact pool. Pool size is finite
admission truth, not an unbounded spawn operation or a data-port value.

## More exact surfaces

Continue with the examples for [[pure expressions, filtering and selectors|Plots-and-flow]],
[[optional values, retained state, references and Forms|Types-and-state]],
and [[close, failure, quiescence, cancellation and temporal joins|Terminals-and-concurrency]].
Each has a separate checked law; none is hidden inside an ordinary assignment.
