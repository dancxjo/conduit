Conduit has one recurring move:

> **Separate semantic meaning from concrete realization, then join them explicitly in a plan.**

Nearly every important noun exists to protect one side of that boundary.

## Begin with the authored happening

The user authors a **plot**: a portable description of the intended course of semantic work.

```conduit
plot hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." >> upper >> show
}.
```

A plot is not a deployment recipe. It does not have to say which process, operating system, transport, device, library, or machine performs each piece of work.

It says what work is required and how that work relates.

During the current terminology migration, repository source may still spell this declaration `form`; [[Current language surface|Current-language-surface]] records exact implemented syntax.

## Read the semantic graph

Inside the plot, each named **gear** is one occurrence of a semantic **kind**.

```text
literal → text/upper → presentation/text
```

A kind says what reusable work means. Its **fore** is its checked outward boundary. Typed **ports** appear on that boundary. **Cords** connect compatible ports.

At this altitude, the graph says nothing about where the gears will run.

That is deliberate: “uppercase text” should not become “call this Rust function on Linux” merely because the first implementation happened to be written that way.

## Realization lives behind the fore

A **back** is one concrete realization of a kind.

A host might offer several backs for the same semantic kind:

```text
presentation/text
  ↳ native graphical back
  ↳ browser back
  ↳ speech back
  ↳ deterministic linear-text back
```

These backs are not interchangeable because machines are magically equivalent. They are candidates because each one truthfully claims compatibility with the same semantic obligation and exact fore contract.

The planner may then consider:

- host and boot identity;
- available backs and implementations;
- resources and bases;
- explicit authority;
- line availability;
- queue and memory envelopes;
- policy and preferences;
- required signs/evidence;
- execution limits.

## Planning is the meeting point

The plot supplies enduring intent; the world supplies current facts.

```text
plot
  +
body / hosts / boots / backs / resources / authority / lines
  ↓
planner
  ↓
immutable plan
```

The **plan** is not a fuzzy strategy. It is the exact admitted realization selected for this moment.

A plan may say, in effect:

```text
gear A → Host 1 / Boot 7 / Back X
gear B → Host 2 / Boot 3 / Back Y
cord A→B → Line L
queues → these exact bounds
effects → this exact authority
```

Once sealed, the plan becomes history. A later change produces explicit fallback already admitted by that plan or a replacement plan; the old plan is not silently edited.

## Play is execution, not selection

A **play** is one active execution of one exact plan.

```text
plot ≠ plan ≠ play
```

A plan can exist without being played. The same plan may be played more than once. A replacement plan may realize the same plot after the world changes.

Execution advances through bounded **steps**. A step may move info, invoke a back, make a bounded host call, observe pressure, quiesce, complete, or terminate abnormally according to the checked meaning and sealed realization.

## Quiescence is a pause, not a conclusion

Reactive software often reaches moments when no admitted work can presently progress.

Conduit names that **quiescence** rather than pretending the computation has ended.

```text
active → quiescent → active
```

A live play may later continue when new admitted work arrives. Semantic completion is different: the plot must explicitly establish that the relevant drain is sufficient evidence that the authored work is finished.

This is one reason Conduit can model always-on/reactive systems without turning “the event loop is currently idle” into “the program terminated.”

## Cords are not lines

A **cord** is semantic composition. A **line** is concrete carriage.

Suppose one plot contains:

```text
sensor → classify → display
```

Planning may put all three gears on one host, or place the sensor and classifier on different hosts:

```text
plot:
  sensor >> classify >> display

plan:
  sensor   on Host A
  classify on Host B
  display  on Host B

realization:
  sensor→classify cord uses Line L
```

If Line L disappears, the cord did not become semantically different. The realization became invalid. Planning is the place to reconcile that new truth.

## A body is the thing that continues

A **body** is the durable logical computer.

Hosts come and go; hosts reboot; lines appear and vanish; plans are replaced; plays begin and end. The body can remain the same continuant across those changes.

```text
body
  ├─ durable parts
  ├─ current hosts
  │    └─ current boots
  ├─ resident plots
  ├─ current plan
  └─ active play
```

A **wake** is a period of active availability. A **lull** preserves the body's continuity while active play ceases.

This distinction lets the architecture answer “what stayed the same?” without using process lifetime as a proxy for identity.

## Information has meaning and form

Work is not the only place Conduit separates semantics from realization.

A **type** says what a value means. A **form** says one concrete portable representation of values of that type.

```text
type = meaning
form = carried/stored shape
```

A semantic variant may be encoded as a byte in one form and differently in another. The byte assignment is a compatibility fact, not the meaning of the variant itself.

This keeps wire/storage/ABI details available for exact checking without making them semantic identity.

## Time has several different promises

Conduit refuses to collapse every temporal relationship into “mutable state.”

- **info** is a value flowing now;
- a **keep** is current truth retained for an exact duration;
- **data** is an independently addressable immutable generation;
- **save** publishes one generation;
- **load** recovers typed info.

These distinctions preserve causality and durability promises that ordinary variable/storage terminology often smears together.

## Human meaning has its own realization boundary

The human-facing path has the same shape:

```text
authoritative body truth
        ↓
      face
        ↓
      mask
        ↓
      show
```

The **face** contains the humanly relevant meaning and agency. A **mask** realizes that face for a user agent or medium. A **show** is one concrete manifestation.

A browser DOM, a spoken interaction, a native surface, and a terminal transcript can therefore be different shows of the same face without any one rendering becoming the application truth.

## Signs make adaptation explainable

A **sign** is bounded evidence that something was selected, attempted, observed, completed, refused, lost, or replaced.

Good signs let the system answer causal questions:

- which plan selected this back?
- which host and boot supplied it?
- which line carried this cord?
- what observation invalidated the realization?
- was fallback already admitted?
- did planning create a replacement plan?
- was work replayed, moved, continued, or refused?

A sign is evidence, never authority.

## The whole movement

```text
AUTHORED MEANING

type → form
kind → fore
plot containing gears, ports, and cords
        │
        │ meets current truth
        ▼
PLANNING

backs + hosts + resources + authority + lines
        │
        ▼
exact immutable plan
        │
        ▼
EXECUTION

play → steps → signs

CONTINUITY

body persists across hosts, boots, plans, plays, wakes, and lulls

PRESENTATION

face → mask → show
```

The architecture is easiest to understand not by memorizing all the nouns, but by asking one recurring question:

> **Is this fact part of enduring meaning, or part of the present realization of that meaning?**

When that question is answered at the right altitude, many difficult distributed-systems questions become much less mysterious.
