# Conduit

**Wire meaning together. Let Conduit work out how that meaning can exist here, now.**

Conduit is an experimental programming system for building programs as graphs and realizing those graphs across very different machines.

You choose reusable **Kinds** and configure them as **Gears** in a **Form**. Gears expose typed **Ports**, **Cords** connect compatible Ports, and **Info** flows through those Cords. Info is shaped, typed data—not an untyped bucket. The same meaning can live inside one process, cross into a browser, reach a microcontroller, or span several machines without baking those machines into the authored program.

```text
Kind catalog
    │
    │ configured as
    ▼
 [Gear] ──Cord──> [Gear] ──Cord──> [Gear]
    │
    └──────────── Form = meaning
```

Conduit then asks a different question:

> Given what is actually true right now, how can this Form be realized?

That is where Hosts, Bases, Signs, Plans, and Plays enter.

## Current accepted Patchbay

Open the example Form in the native Patchbay from a checkout with the canonical
repository-development entrance:

```sh
cargo xtask demo patchbay
```

The direct package invocation is useful while developing the native adapter:

```sh
cargo run -p patchbay-native -- --form examples/hello.conduit
```

The Patchbay view below is generated only after the exact accepted `main`
commit passes the semantic browser proof. It shows one manifestation of the
current Form and its structural subjects; the Form, Plan, Play, and Signs—not
the pixels—remain authoritative. Follow the image to inspect its exact commit,
browser, viewport, digest, and semantic provenance.

[![Current accepted Conduit Patchbay overview showing the Form graph and structural view](https://dancxjo.github.io/conduit/current/patchbay/overview.png)](https://dancxjo.github.io/conduit/current/patchbay/overview/)

## See ConduitOS in QEMU

Build the current x86-64 ConduitOS image and open a visible, interactive QEMU
window with its framebuffer and USB keyboard:

```sh
cargo xtask conduitos demo --arch x86-64
```

The command reports the exact image and emulator profile, leaves serial/debug
output in the invoking terminal, and runs until you close QEMU or interrupt it.
It requires `xorriso`, `qemu-system-x86_64` with GTK display support, and access
to a graphical display.

Use the separate machine-verification entrances when you want reproducible
evidence rather than an interactive window:

```sh
cargo xtask conduitos run --arch x86-64
cargo xtask conduitos prove --arch x86-64
```

`run` and `prove` inject and validate deterministic proof inputs and terminate
after collecting their exact evidence. The visible `demo` makes no proof claim.

---

## A tiny example

```conduit
form signal-demo {
    pulse: flow/pulse(count = 16, period-ms = 250, initial = false)
    show: presentation/show

    pulse > show
}
```

`flow/pulse` and `presentation/show` identify reusable semantic **Kinds**. `pulse` and `show` are configured occurrences of those Kinds in this Form: **Gears**.

```text
┌────────────┐             ┌────────────┐
│ Gear pulse │ ──────────> │ Gear show  │
└────────────┘    Cord     └────────────┘
      │                          │
      ▼                          ▼
 flow/pulse              presentation/show
    Kind                       Kind
```

The Form does **not** say that `show` means stdout. It does not say both Gears must run in the same process. It does not say whether a cross-machine Cord is realized by WebSocket, USB CDC, shared memory, or some future Line.

Those are realization questions.

The Form says what the program means.

---

## Kinds, Gears, Ports, Cords, and Info

These five ideas describe most of the semantic graph.

### Kinds

A **Kind** is reusable semantic behavior: a contract for what a Gear of that Kind means.

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

A Kind is not necessarily a Rust function, executable artifact, task, thread, process, or machine-specific implementation. It is the reusable semantic species from which Gears are configured.

### Gears

A **Gear** is one configured use of a Kind inside a Form.

If the same Kind is used twice with different names or configuration, those are two Gears:

```text
Kind: text/uppercase@1

      ┌── Gear: normalize-title
      │
      └── Gear: normalize-command
```

Gear identity belongs to the meaning of the Form. The planner may later place a Gear on different Hosts, select different implementations for it, or fuse/lower it differently without turning it into a different semantic Gear.

The word is intentionally local. Conduit is not pretending the whole system is a literal gearbox. **Cords remain Cords.** They connect semantic work; they are not shafts or belts.

### Ports

A **Port** is a typed directional point on a Gear or Face through which Info enters or leaves. Its exact semantic contract includes identity, direction, Info shape/type, and temporal behavior such as one Value, a Flow, or Current state.

A Port is not a drawn jack, queue slot, network socket, Line endpoint, or Base handle. Renderers and realizations may create those local objects, but they do not replace Port identity.

### Cords

A **Cord** is a typed semantic connection between compatible Ports on Gears.

```text
[key state] ──> [interpret] ──> [LED state]
```

When two Gears are realized on different Hosts, Conduit may need a network or physical mechanism to carry that connection. The program still sees a Cord.

A Line is not a Cord.

### Info

**Info is shaped, typed data that flows through Cords.**

Info may include typed text, numbers, events, state, samples, records, frames, booleans, or other shaped values. A **Signal** is one particular Info semantic or mechanism where a Kind explicitly defines it; Info in general is not automatically Signal. At lower layers Info may be encoded into frames, packets, messages, or bytes. Those are realization details.

At the semantic level:

> Kinds define behavior. Gears put that behavior to work. Ports expose typed entry and exit points. Cords connect them. Info flows.

---

## Forms, Faces, and Backs

A **Form** is meaning expressed as a graph of configured Gears and Cords.

Conduit can also separate a Kind's visible contract from graph-level ways of implementing it. We call those its **Face** and **Back**.

```text
                  FACE
          ┌────────────────┐
input ──> │      Kind      │ ──> output
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

The **Face** is the stable semantic contract presented to the surrounding graph, including its typed Ports.

A **Back** is a Form that implements that contract in Conduit terms.

Because a Back is itself a Form, its Gears can use Kinds that themselves admit alternative Backs. Composition is recursive.

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

A **Body** is the durable intended world born from Seed material. It owns durable obligations.

A Body is not a Host, process, device, realization, or current execution. It can persist while the machines and realizations beneath it change.

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

- Which Back satisfies a Face?
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

## Signs, Hosts, and Bases

The authored Form is only half the story. Real machines are finite and inconvenient, which is useful information rather than something Conduit tries to erase.

### Signs

**Signs** are bounded machine-readable truth about what is currently true and what happened.

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

Signs are not intent, authority, or Plans. They are the basis from which Conduit can reason honestly about realization and history.

### Hosts

A **Host** makes truthful, finite realization offers for one exact running environment.

A Host may offer implementations, resources, links, capabilities, and limits. A browser, Linux machine, Pico W, or bare-metal ConduitOS machine can all be Hosts without pretending they are interchangeable.

### Bases

A **Base** is a platform or machine mechanism beneath a Host offer.

Examples can include USB CDC machinery, WebSocket machinery, timers, framebuffer access, interrupt mechanisms, DOM or Wayland presentation machinery, and other platform-specific substrate.

```text
FORM
  Gear : Kind
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

A Base is not a Kind or Gear. Hardware existence does not automatically become a Host offer, and a Host offer does not automatically imply authority to use it.

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

What changed is a **Sign** about one possible realization.

If the current Plan already admitted WebSocket as an alternative route, the Play may continue under the same Plan by selecting that route.

If the current Plan admitted only USB, it remains immutable. The awake Body can require replanning, producing a new Plan and normally a new Play over WebSocket.

That distinction is central to Conduit:

> preserve meaning and durable intent while being exact about what changed in the realized world.

---

## The vocabulary at a glance

```text
SEED    authored workspace/source material
BODY    durable intended world and obligations
WAKE    one active maintenance interval for a Body
LULL    end that interval without deleting the Body

FORM    meaning expressed as a semantic graph
KIND    reusable semantic behavior/contract
GEAR    configured occurrence of a Kind in a Form
CORD    typed semantic connection between compatible Ports on Gears
PORT    typed directional point carrying an Info shape/type and temporal contract
INFO    shaped, typed data carried through a Cord
FACE    stable visible semantic contract, including Ports
BACK    Form implementing a Face

SIGN    bounded truth about what is true or what happened
HOST    truthful finite realization offers for an exact running environment
BASE    platform/machine mechanism beneath a Host offer
PLAN    exact immutable realization for an admitted basis of Signs
PLAY    active execution of one exact Plan
```

A useful compression is:

> **Form is meaning. Body is durable intent. Signs describe the observed world. Hosts offer finite possibilities. Bases are machine mechanisms. Plan makes realization exact. Play makes that Plan active.**

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

Before a Play begins, its exact realization admits finite execution needs: Gears, ports, Cords, queue items, bytes, resources, Host-side mechanisms, Bases, and required Sign storage.

A hosted implementation may perform richer preparation while planning. Once admitted execution begins, however, the runtime should not quietly acquire unbounded needs.

Pressure, exhaustion, cancellation, disconnects, and failure remain explicit runtime truth.

This is what allows the same fundamental model to make sense on ordinary computers and small embedded systems.

---

## What works today

Conduit is under active development, but it is not only an architecture sketch. The repository already contains substantial native, browser/WASM, and physical Pico work, including a bounded execution kernel, checked Forms, exact planning machinery, live WebSocket and USB CDC paths, and physical proof tooling.

The architecture and vocabulary are moving quickly. **[STATUS.md](STATUS.md)** is the authority for exactly what the current code and checks have proven. Compilation, simulation, browser execution, live transport, firmware execution, and observed physical effects are deliberately different proof classes.

---

# Try it

From a source checkout, you will need a recent Rust toolchain. Repository
development, demonstrations, and proofs enter through `cargo xtask`; installed
product workflows enter through `conduit`.

## Run a graph on the native Host

```bash
cargo xtask demo std
```

This parses a Form, asks the actual std Host what it can truthfully offer, plans an exact realization, and executes it through `conduit-kernel`.

## See a real browser Host

```bash
cargo xtask doctor browser
cargo xtask demo browser
```

`cargo xtask demo browser` builds the Rust/WASM browser runtime, starts the native side of a bounded WebSocket session, and prints a URL. Open it in an ordinary browser and use the terminal as instructed.

This is an actual browser-hosted Conduit realization, not a browser simulation.

## Inspect a realization

Runtime/Observatory tooling is evolving with the ontology. See **[Try Conduit](docs/try-conduit.md)** and **[STATUS.md](STATUS.md)** for the current commands and exact fields they expose.

The intended inspection boundary keeps semantic identity and realization identity separate:

```text
MEANING
  Form
  Gear → Kind
  Cords

REALIZATION
  Plan
  Gear → implementation
  Host / Boot
  Bases / resources / routes

PLAY
  active runtime state

SIGNS
  current truth + bounded history
```

## Talk to a real Pico W

With supported Pico W firmware attached, the physical proof tooling includes:

```bash
cargo xtask prove std-pico-usb --interactive
```

The Pico uses bounded USB CDC machinery for the Conduit path and separate physical proof/Sign reporting where the current proof profile requires it.

Because physical proofs require attached hardware, they are intentionally hardware-gated rather than being presented as ordinary CI.

Check prerequisites with:

```bash
cargo xtask doctor
```

---

# Form syntax

Canonical Form source uses the graph itself, not statement order, to determine connectivity. A declaration such as:

```conduit
pulse: flow/pulse(count = 16)
```

creates the Gear named `pulse` from the Kind `flow/pulse` with the supplied configuration.

Current examples also use:

```text
(...)     Face
{...}     Back
=         declarative immutable value relationship
>         runtime Cord
```

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

1. Run `cargo xtask demo std`.
2. Read [Try Conduit](docs/try-conduit.md).
3. Open [the Conduit canon](docs/conduit-canon.md).
4. Check [STATUS.md](STATUS.md) to see what is proven today.
5. Browse [roadmap issue #361](https://github.com/dancxjo/conduit/issues/361).
6. Read [AGENTS.md](AGENTS.md) before making substantial changes.

---

# Design principles

### Programs are graphs, not scripts

Statement order does not secretly become execution order. Meaning lives in Forms, Gears, relationships, and Cords.

### Kinds are not Gears

A Kind is reusable semantic behavior. A Gear is one configured use of it. Do not collapse Kind identity, Gear identity, implementation identity, or runtime scheduling identity into one object.

### Meaning is not placement

A Form should not need to name a machine merely because some realization eventually must.

### Interfaces are not implementations

Faces remain semantic contracts while Backs can express alternative graph-level implementations.

### Graph implementation is not Host implementation

Backs implement Faces using Conduit meaning. Hosts offer concrete ways to realize the Gears that remain after graph-level decomposition.

### A Line is not a Cord

WebSocket, USB CDC, shared memory, and future transports are possible realizations of connectivity. They should not become the meaning of the connection itself.

### Availability is not authority

A Host possessing a capability does not automatically imply that a Body or Play may use it.

### Planning is not execution

Selecting an exact realization does not mean that realization is currently active.

### Signs are not plans

A changed fact about the world may invalidate or pressure a realization, but it does not mutate an immutable Plan into a different Plan.

### Simulation is not physical proof

Tests, fixtures, native simulation, browser execution, firmware execution, live transport, and observed physical effects are different claims. Conduit tries to say exactly which one has been established.

---

# Contributing

Conduit is experimental and changing quickly.

Narrow changes are easier to reason about than giant ones. Keep architectural claims tied to executable proof, and do not promote compilation or simulation into proof of a platform or physical effect.

The primary local gate is:

```bash
cargo xtask check
```

Additional platform-specific checks are exposed by `cargo xtask --help` and documented in [STATUS.md](STATUS.md) and the relevant roadmap issues. The `justfile` is an optional shell façade over the same canonical entrances.

If you prefer `just`, the repository provides a thin optional façade over the
same two entrances:

```bash
just run examples/signal-demo.form --placements examples/std-local.placements
just form-check examples/signal-demo.form
just demo browser
just check
just prove std-browser-s4
```

These recipes contain no independent workflow logic; `just conduit ...` and
`just xtask ...` are also available as complete pass-throughs.

---

## The short version

```text
Kind catalog
    │
    ▼
 [Gear] ──Cord──> [Gear] ──Cord──> [Gear]
            Ports carry Info

        Form = meaning
              │
              ▼
Seed → Body → Wake
              │
              ▼
             Plan
              │
              ▼
             Play
```

**Kinds describe reusable semantic behavior. Gears are configured uses of those Kinds. Cords connect the Gears. Forms capture the meaning. Bodies make durable obligations from authored Seeds. Signs say what is true. Hosts offer finite possibilities through machine Bases. Plans choose an exact realization. Plays run it.**
