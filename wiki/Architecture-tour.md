Conduit has one recurring move:

> **Separate semantic meaning from concrete realization, then join them explicitly in a plan.**

Nearly every important noun exists to protect one side of that boundary.

## Begin with the authored happening

The user authors a **form**: a portable description of the intended course of semantic work.

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." >> upper >> show
}.
```

![The hello form as connected gears](assets/sample-diagrams/hello.svg)

A form is not a deployment recipe. It does not have to say which process, operating system, transport, device, library, or machine performs each piece of work.

It says what work is required and how that work relates.

The declaration is executable `form` source. [[Current language surface|Current-language-surface]] records the supported grammar and separates proposals from implemented behavior.

## Read the semantic graph

Inside the form, each named **gear** is one occurrence of a semantic **kind**.

```text
literal → text/upper → presentation/text
```

A kind says what reusable work means. Its **fore** is the function-like signature presented to authors: startup parameters and typed runtime ports, including their direction, value type, and temporal shape. **cords** connect compatible ports named by those fores.

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

These backs are not interchangeable because machines are magically equivalent. They are candidates because each one truthfully claims compatibility with the same semantic obligation and exact fore signature.

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

The form supplies enduring intent; the world supplies current facts.

```text
form
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

## play is execution, not selection

A **play** is one active execution of one exact plan.

```text
form ≠ plan ≠ play
```

A plan can exist without being played. The same plan may be played more than once. A replacement plan may realize the same form after the world changes.

Execution advances through bounded **steps**. A step may move info, invoke a back, make a bounded host call, observe pressure, quiesce, complete, or terminate abnormally according to the checked meaning and sealed realization.

## Quiescence is a pause, not a conclusion

Reactive software often reaches moments when no admitted work can presently progress.

Conduit names that **quiescence** rather than pretending the computation has ended.

```text
active → quiescent → active
```

A live play may later continue when new admitted work arrives. Semantic completion is different: the form must explicitly establish that the relevant drain is sufficient evidence that the authored work is finished.

This is one reason Conduit can model always-on/reactive systems without turning “the event loop is currently idle” into “the program terminated.”

## cords are not lines

A **cord** is semantic composition. A **line** is concrete carriage.

Suppose one form contains:

```text
sensor → classify → display
```

Planning may put all three gears on one host, or place the sensor and classifier on different hosts:

```text
form:
  sensor >> classify >> display

plan:
  sensor   on Host A
  classify on Host B
  display  on Host B

realization:
  sensor→classify cord uses Line L
```

If line L disappears, the cord did not become semantically different. The realization became invalid. Planning is the place to reconcile that new truth.

## A body is the thing that continues

A **body** is the durable logical computer.

hosts come and go; hosts reboot; lines appear and vanish; plans are replaced; plays begin and end. The body can remain the same continuant across those changes.

```text
body
  ├─ durable parts
  ├─ current hosts
  │    └─ current boots
  ├─ resident forms
  ├─ current plan
  └─ active play
```

A **wake** is a period of active availability. A **lull** preserves the body's continuity while active play ceases.

This distinction lets the architecture answer “what stayed the same?” without using process lifetime as a proxy for identity.

## Information has meaning and code

A **type** says what a value means; **info** is one value of that type.
A **code** states an exact portable encoding without changing the meaning.

```text
type = meaning
info = typed value
code = encoding contract
```

For example, a finite variant can use a compact byte code. The assigned tag is
a compatibility fact, not the meaning of the variant itself. Current support
and examples belong in [[the language reference|Current-language-surface]].

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

## signs make adaptation explainable

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

type → info
kind → fore
form containing gears, ports, and cords
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
