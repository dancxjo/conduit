# Conduit

**Wire meaning together. Let Conduit work out how that meaning can exist here, now.**

Conduit is an experimental programming system for building programs as graphs and realizing those graphs across very different machines.

You choose reusable **Operations** and configure them as **Gears** in a **Form**. **Cords** connect those Gears. **Signals** flow through the cords. The same meaning can live inside one process, cross into a browser, reach a microcontroller, or span several machines without baking those machines into the authored program.

```text
Operation catalog
      │
      │ configured as
      ▼
   [Gear] ──cord──> [Gear] ──cord──> [Gear]
      │
      └──────────── Form = meaning
```

Conduit then asks a different question:

> Given what is actually true right now, how can this Form be realized?

That is where Hosts, Bases, Clues, Plans, and Plays enter.

---

## A tiny example

```conduit
form signal-demo {
    pulse: flow/pulse(count = 16, period-ms = 250, initial = false)
    show: presentation/show

    pulse > show
}
```

`flow/pulse` and `presentation/show` name reusable semantic Operations. `pulse` and `show` are configured occurrences of those Operations in this Form: **Gears**.

```text
┌────────────┐             ┌────────────┐
│ Gear pulse │ ──────────> │ Gear show  │
└────────────┘    cord     └────────────┘
      │                          │
      ▼                          ▼
 flow/pulse              presentation/show
  Operation                  Operation
```

The Form does **not** say that `show` means stdout. It does not say both Gears must run in the same process. It does not say whether a cross-machine cord is realized by WebSocket, USB CDC, shared memory, or some future carrier.

Those are realization questions.

The Form says what the program means.

---

## Operations, Gears, Cords, and Signals

These four ideas describe most of the semantic graph.

### Operations

An **Operation** is a reusable semantic behavior or transformation contract.

Examples might include:

```text
text input
uppercase
timer
filter
JSON decode
presentation
GPIO state
```

An Operation is not necessarily a Rust function, executable artifact, task, thread, process, or machine-specific implementation. It says what behavior is available to be used.

### Gears

A **Gear** is one configured use of an Operation inside a Form.

If the same Operation is used twice with different names or configuration, those are two Gears:

```text
Operation: text/uppercase@1

      ┌── Gear: title-case-input
      │
      └── Gear: normalize-command
```

Gear identity belongs to the meaning of the Form. The planner may later place a Gear on different Hosts, select different implementations for it, or fuse/lower it differently without turning it into a different semantic Gear.

The word is intentionally local. Conduit is not pretending the whole system is a literal gearbox. **Cords remain cords.** They connect semantic work; they are not shafts or belts.

### Cords

A **Cord** is a typed semantic connection between Gears.

```text
[key state] ──> [interpret] ──> [LED state]
```

When two Gears are realized on different Hosts, Conduit may need a network or physical mechanism to carry that connection. The program still sees a Cord.

A carrier is not a Cord.

### Signals

**Signals are what flow through Cords.**

A Signal may carry text, numbers, events, state changes, samples, records, frames, booleans, or other typed values. At lower layers it may be encoded into frames, packets, messages, or bytes. Those are realization details.

At the semantic level:

> Operations define behavior. Gears put that behavior to work. Cords connect Gears. Signals flow.

---

## Forms, Fronts, and Backs

A **Form** is meaning expressed as a graph of configured Gears and Cords.

Conduit also separates an Operation's visible contract from graph-level ways of implementing it. We call those its **Front** and **Back**.

```text
                 FRONT
          ┌────────────────┐
input ──> │   Operation    │ ──> output
          └────────────────┘
                  │
                  │ may be implemented by
                  ▼
          ┌────────────────┐
          │      BACK      │
          │                │
          │ Gear ──> Gear  │
          │          │     │
          │          ▼     │
          │         Gear   │
          └────────────────┘
```

The **Front** is the stable semantic contract presented to the surrounding graph.

A **Back** is a Form that implements that contract in Conduit terms.

Because a Back is itself a Form, its Gears can use Operations that themselves have alternative Backs. Composition is recursive.

This is different from machine realization. A Back answers:

> How can this meaning be expressed as more Conduit meaning?

A Host answers:

> What can actually be realized here?

---

# From meaning to a running world

Conduit's current lifecycle is:

```text
SEED → BODY → WAKE → PLAN → PLAY
              │
              └──────────→ LULL
```

The words name different things on purpose.

### Seed

A **Seed** is authored workspace/source material: Forms, Body definitions, assets, and policy source.

### Body

A **Body** is the durable intended world constituted from Seed material. It owns durable obligations.

A Body is not a Host, process, device, deployment, or current execution. It can persist while the machines and realizations beneath it change.

### Wake and Lull

A **Wake** says that a Body is currently maintaining its obligations.

A **Lull** ends that maintenance interval without deleting the Body.

One Body may therefore live through several realizations:

```text
Body B
  Wake W1
    Plan P1 → Play X
    Plan P2 → Play Y
  Lull
  Wake W2
    Plan P3 → Play Z
```

A changed machine or failed route does not automatically mean a new Body or a new Wake.

### Plan

A **Plan** is the exact immutable realization of meaning for an admitted basis of current truth.

It answers questions such as:

- Which Back satisfies a Front?
- Which implementation realizes each Gear?
- Which Host and exact Boot will perform that work?
- How is each Cord realized?
- Which resources and limits are admitted?
- Which authority is required?
- Which Bases and routes are involved?
- Is this realization actually possible now?

A Plan does not become mutable merely because the world changes. If its assumptions cease to hold, Conduit either continues using alternatives already sealed into that Plan or produces another Plan.

### Play

A **Play** is active execution of one exact Plan.

Planning is not execution. A Plan may exist without being played, and a new Plan/Play can appear beneath the same awake Body when circumstances require replanning.

---

## Clues, Hosts, and Bases

The authored Form is only half the story. Real machines are finite and inconvenient, which is useful information rather than something Conduit tries to erase.

### Clues

**Clues** are bounded machine-readable truth about what is currently true and what happened.

Examples include:

```text
Host H / Boot B is available
USB link L is ready
USB link L became unavailable
this implementation is installed
this resource has capacity N
this Play reached a terminal outcome
this physical effect was observed
```

Clues are not intent, authority, or Plans. They are the basis from which Conduit can reason honestly about realization and history.

### Hosts

A **Host** makes truthful, finite realization offers for one exact running environment.

A Host may offer implementations, resources, links, capabilities, and limits. A browser, Linux machine, Pico W, or bare-metal ConduitOS machine can all be Hosts without pretending they are interchangeable.

### Bases

A **Base** is a platform or machine mechanism beneath a Host offer.

Examples can include USB CDC machinery, WebSocket machinery, timers, framebuffer access, interrupt mechanisms, DOM or Wayland presentation machinery, and other platform-specific substrate.

```text
FORM
  Gear
    ↓
PLAN
  exact implementation
  exact Host / Boot
    ↓
HOST OFFER
    ↓
BASE
    ↓
machine / platform
```

A Base is not an Operation or Gear. Hardware existence does not automatically become a Host offer, and a Host offer does not automatically imply authority to use it.

---

## Why the distinctions matter

Consider one Body maintaining this meaning:

```text
[browser key] ──> [interpret] ──> [Pico LED]
```

The Pico might initially be reached over USB CDC while WebSocket is also available over Wi-Fi.

If USB is unplugged, several identities may remain perfectly intact:

```text
same Body
same Wake
same Form
same Gears
same Pico Host / Boot
```

What changed is a **Clue** about one possible realization.

If the current Plan already admitted WebSocket as an alternative route, the Play may continue under the same Plan by selecting that route.

If the current Plan admitted only USB, it remains immutable. The awake Body can require replanning, producing a new Plan and normally a new Play over WebSocket.

That distinction is central to Conduit:

> preserve meaning and durable intent while being exact about what changed in the realized world.

---

## The vocabulary at a glance

```text
SEED       authored workspace/source material
BODY       durable intended world and obligations
WAKE       one active maintenance interval for a Body
LULL       end that interval without deleting the Body

FORM       meaning expressed as a semantic graph
OPERATION  reusable semantic behavior/contract
GEAR       configured occurrence of an Operation in a Form
CORD       typed semantic connection between Gears
SIGNAL     typed value/state/event flowing through a Cord
FRONT      visible semantic contract
BACK       Form implementing a Front

CLUE       bounded truth about what is true or what happened
HOST       truthful finite realization offers for an exact running environment
BASE       platform/machine mechanism beneath a Host offer
PLAN       exact immutable realization for an admitted basis of Clues
PLAY       active execution of one exact Plan
```

A useful compression is:

> **Form is meaning. Body is durable intent. Clues describe the observed world. Hosts offer finite possibilities. Bases are machine mechanisms. Plan makes realization exact. Play makes that Plan active.**

---

## Portability without pretending machines are identical

Conduit aims to let the same authored meaning be realized:

- in a native Rust process;
- in a real browser through Rust/WASM;
- on a constrained microcontroller;
- across multiple connected Hosts;
- on a bare-metal machine running ConduitOS;
- or inside a larger machine whose parts can take on different work.

A browser is not a Pico W. A Pico W is not Linux. USB is not WebSocket. Memory, clocks, devices, links, permissions, physical effects, failures, and resource limits remain visible.

Portability comes from **planning around those differences**, not denying them.

---

## Bounded execution

Conduit is designed with constrained systems in mind.

Before a Play begins, its exact realization admits finite execution needs: Gears, ports, Cords, queue items, bytes, resources, Host operations, Bases, and required Clue storage.

A hosted implementation may perform richer preparation while planning. Once admitted execution begins, however, the runtime should not quietly acquire unbounded needs.

Pressure, exhaustion, cancellation, disconnects, and failure remain explicit runtime truth.

This is what allows the same fundamental model to make sense on ordinary computers and small embedded systems.

---

## What works today

Conduit is under active development, but it is not only an architecture sketch. The repository already contains substantial native, browser/WASM, and physical Pico work, including a bounded execution kernel, checked Forms, exact planning machinery, live WebSocket and USB CDC paths, and physical proof tooling.

The architecture and vocabulary are moving quickly. **[STATUS.md](STATUS.md)** is the authority for exactly what the current code and checks have proven. In particular, compilation, simulation, browser execution, live transport, firmware execution, and observed physical effects are deliberately different proof classes.

---

# Try it

You will need a recent Rust toolchain and [`just`](https://github.com/casey/just).

## Run a graph on the native Host

```bash
just demo-std
```

This parses a Form, asks the actual std Host what it can truthfully offer, plans an exact realization, and executes it through `conduit-kernel`.

## See a real browser Host

```bash
just doctor browser
just toggle
```

`just toggle` builds the Rust/WASM browser runtime, starts the native side of a bounded WebSocket session, and prints a URL. Open it in an ordinary browser and use the terminal as instructed.

This is an actual browser-hosted Conduit realization, not a browser simulation.

## Inspect a realization

Runtime/Observatory tooling is evolving with the ontology. See **[Try Conduit](docs/try-conduit.md)** and **[STATUS.md](STATUS.md)** for the current commands and the exact fields they expose.

The intended inspection boundary keeps semantic identity and realization identity separate:

```text
MEANING
  Form
  Gear → Operation
  Cords

REALIZATION
  Plan
  Gear → implementation
  Host / Boot
  Bases / resources / routes

PLAY
  active runtime state

CLUES
  current truth + bounded history
```

## Talk to a real Pico W

With supported Pico W firmware attached, the physical proof tooling includes:

```bash
cargo xtask prove std-pico-usb --interactive
```

The Pico uses bounded USB CDC machinery for the Conduit path and separate physical proof/Clue reporting where the current proof profile requires it.

Because physical proofs require attached hardware, they are intentionally hardware-gated rather than being presented as ordinary CI.

Check prerequisites with:

```bash
just doctor
```

---

# Form syntax

Canonical Form source uses the graph itself, not statement order, to determine connectivity. Current examples use constructs such as:

```text
(...)     front
{...}     back
name: operation(arguments)
=         declarative immutable value relationship
>         runtime cord
```

The left-hand `name` is the configured occurrence, the **Gear**. The referenced `operation(arguments)` identifies and configures the reusable **Operation** it uses.

See:

- [canonical examples](examples/README.md)
- [runnable Form examples](docs/try-forms.md)
- [the Conduit canon](docs/conduit-canon.md)

---

# Repository map

```text
crates/       portable contracts, parser, planner, kernel, runtime, catalog, CLI
hosts/        actual platform Hosts and adapters
firmware/     constrained firmware targets
fixtures/     deterministic conformance fixtures
examples/     Forms and realization examples
xtask/        typed repository workflows
docs/         architecture, design records, proofs, and guides
```

If you are new to the project, a good path is:

1. Run `just demo-std`.
2. Read [Try Conduit](docs/try-conduit.md).
3. Open [the Conduit canon](docs/conduit-canon.md).
4. Check [STATUS.md](STATUS.md) to see what is proven today.
5. Browse [roadmap issue #361](https://github.com/dancxjo/conduit/issues/361).
6. Read [AGENTS.md](AGENTS.md) before making substantial changes.

---

# Design principles

### Programs are graphs, not scripts

Statement order does not secretly become execution order. Meaning lives in Forms, Gears, relationships, and Cords.

### Operations are not Gears

An Operation is reusable semantic behavior. A Gear is one configured use of it. Do not collapse catalog identity, graph identity, implementation identity, or runtime scheduling identity into one object.

### Meaning is not placement

A Form should not need to name a machine merely because some realization eventually must.

### Interfaces are not implementations

Fronts remain semantic contracts while Backs can express alternative graph-level implementations.

### Graph implementation is not Host implementation

Backs realize Fronts using Conduit meaning. Hosts offer concrete ways to realize the leaf Gears