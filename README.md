# Conduit

**Wire operations together. Run the resulting graph wherever the work can actually happen.**

Conduit is an experimental programming system for building programs as graphs.

You connect **operations** with **cords**. **Signals** flow along those cords. A graph can run inside one process, cross into a browser, reach a microcontroller, or span several machines without changing the basic programming model.

You describe the wiring.

Conduit plans how to make it real.

```text
[source] ──cord──> [transform] ──cord──> [display]
```

That simple idea is the center of the project.

Conduit programs are **not command sequences**. They do not fundamentally say “call this function, then invoke that service, then send this command.”

They say:

> connect this operation to that operation.

The rest is planning, placement, transport, execution, and evidence.

---

## A tiny example

```conduit
form signal-demo {
    pulse: flow/pulse(count = 16, period-ms = 250, initial = false)
    show: presentation/show

    pulse > show
}
```

This describes two operations and one cord:

```text
┌─────────┐             ┌─────────┐
│  pulse  │ ──────────> │  show   │
└─────────┘    signal   └─────────┘
```

It does **not** say that `show` means stdout.

It does not say that both operations must run in the same process.

It does not say whether a connection is local memory, WebSocket, USB CDC, Wi-Fi, or something else.

Those are realization questions.

The graph describes the program.

---

## Operations, cords, and signals

Three ideas get you surprisingly far.

### Operations

An **operation** produces, consumes, or transforms signals.

Examples might include:

```text
text input
uppercase
timer
filter
JSON decode
browser display
GPIO output
```

An operation describes a piece of meaning, not necessarily a Rust function or a particular executable implementation.

### Cords

A **cord** connects one operation to another.

```text
[key state] ──> [interpret] ──> [LED state]
```

A cord says that the signal produced here is connected to the input there.

When two operations live on different hosts, Conduit may have to realize that cord using a physical or network carrier.

The program still sees a cord.

### Signals

**Signals are what flow through cords.**

A signal may carry text, numbers, events, state changes, samples, records, frames, booleans, or other typed payloads.

At lower layers a signal may be encoded into messages, frames, packets, or bytes. Those are transport representations.

At the programming level, the important fact is simpler:

> operations transform signals, and cords conduct them.

---

## Fronts and backs

Conduit separates an operation's **interface** from its **implementation**.

We call them its **front** and **back**.

```text
                 FRONT
          ┌────────────────┐
input ──> │   operation    │ ──> output
          └────────────────┘
                  │
                  │ implemented by
                  ▼
          ┌────────────────┐
          │      BACK      │
          │                │
          │  op ──> op     │
          │         │      │
          │         ▼      │
          │        op      │
          └────────────────┘
```

The **front** is the stable contract presented to the surrounding graph.

The **back** is the graph that implements it.

That means backs can be swapped without requiring everything connected to the front to change.

For example, the surrounding program might simply contain:

```text
[source] ──> [uppercase] ──> [sink]
```

while `uppercase` could have one back based on several portable operations and another based on a native facility.

To the surrounding graph, both expose the same front.

And because a back is itself a graph, its operations can themselves have fronts and backs.

Composition is recursive.

---

## Then what does a host do?

This is a different layer of substitution.

A **back** answers:

> How is this front implemented as a Conduit graph?

A **host** answers:

> Which operations can I actually realize here?

Suppose a back contains:

```text
[normalize] ──> [uppercase-map]
```

A Linux host might realize `normalize` using one native implementation.

A constrained microcontroller might offer a smaller bounded implementation.

Another host might not offer that operation at all.

So there are two distinct kinds of freedom:

```text
FRONT
  │
  │ choose an implementation
  ▼
BACK
  │
  │ place and realize its operations
  ▼
HOSTS
```

A useful shorthand is:

> **Fronts specify what.  
> Backs specify how in graph terms.  
> Hosts specify how in machine terms.**

The planner reconciles all three.

---

## Planning

Once you have described a graph, Conduit must determine whether and how it can actually exist.

Running hosts advertise what they can currently provide: operations, resources, links, authority, and concrete implementation identities.

The planner combines those facts with the authored graph and produces an exact **plan**.

That plan answers questions such as:

- Which back should implement a front?
- Which host should run each operation?
- How should each cord be realized?
- Are the required resources available?
- Is the host authorized to perform the requested effects?
- Are memory and queue bounds acceptable?
- What evidence must be retained?
- Can this exact graph actually run?

The authored program describes **meaning and wiring**.

The plan describes **one exact realization of it**.

---

## Why this matters

Consider a program whose logical graph is:

```text
[browser key] ──> [interpret] ──> [Pico LED]
```

Perhaps the final cord initially crosses USB:

```text
browser
   │
   ▼
standard host
   │
   │ USB CDC
   ▼
Pico W
```

Now unplug USB.

If Wi-Fi is also available, the desired graph has not changed.

Only one realization of one cord has disappeared.

A planner can potentially produce another valid realization:

```text
browser
   │
   ▼
standard host
   │
   │ Wi-Fi
   ▼
Pico W
```

The important idea is not “send a reconnect command.”

It is:

> preserve the graph if another valid realization exists.

That is the direction Conduit is heading.

---

## Forms, plans, and plays

The current code uses a few more precise terms:

```text
OP       semantic operation
FORM     authored graph of semantic work
CELL     one named occurrence of an operation
CORD     typed signal connection between cells
FRONT    visible interface of a form or operation
BACK     graph implementing that interface

IMPL     concrete host realization of an operation
HOST     running environment offering realizations
PLAN     exact immutable realization of a form
PLAY     one active execution of a plan
```

A **form** is the authored thing.

A **plan** is one exact way of realizing it.

A **play** is one running instance of that plan.

Keeping those identities separate is important. Editing a program, planning it, deploying it, and running it are related acts, but they are not the same act.

---

## Portability without pretending machines are identical

Conduit aims to let the same authored graph be realized:

- in a native Rust process;
- in a real browser through Rust/WASM;
- on a constrained microcontroller;
- across multiple connected hosts;
- or inside a larger machine whose parts can take on different work.

That does **not** mean pretending every machine is interchangeable.

A browser is not a Pico W.

A Pico W is not Linux.

USB is not WebSocket.

Memory, clocks, devices, links, permissions, physical effects, failures, and resource limits remain visible.

Conduit tries to make programs portable by planning around those differences rather than hiding them.

---

## Bounded execution

Conduit is designed with constrained systems in mind.

Before activation, a host admits the finite execution shape it is responsible for: cells, ports, cords, queue items, bytes, resources, host operations, and required evidence.

A hosted implementation may do richer preparation while a plan is being prepared.

Once admitted execution begins, however, the runtime should not quietly acquire unbounded needs.

Pressure, exhaustion, cancellation, disconnects, and failure remain visible runtime facts.

This is what allows the same fundamental execution model to make sense on both ordinary computers and small embedded systems.

---

## What works today

Conduit is under active development, but this is not only an architecture sketch.

The repository currently contains:

- a `no_std`, port-aware bounded execution kernel;
- a lossless form parser and checker;
- exact planning, resource, identity, authority, link, and evidence contracts;
- native std-host execution;
- a real Rust/WASM browser host;
- browser-local planning and DOM adaptation;
- bounded live WebSocket sessions between native and browser kernels;
- Pico W firmware with bounded dual-CDC USB transport;
- physical evidence from the Pico;
- a three-host Signal path spanning stdout, browser DOM, and a physical Pico W LED;
- deterministic browser-shaped and Pico-shaped conformance fixtures;
- a read-only Observatory path over runtime reports;
- standard bounded text, time, and state operations.

Compilation, simulation, browser execution, live transport, firmware execution, and physical hardware proof are deliberately treated as different levels of evidence.

See [STATUS.md](STATUS.md) for the exact claims established by the current code and checks.

---

# Try it

You will need a recent Rust toolchain and [`just`](https://github.com/casey/just).

## Run a graph on the native host

```bash
just demo-std
```

This parses the Signal form, asks the actual std host what it can provide, plans the form onto that host, and executes the resulting plan through `conduit-kernel`.

The output includes the selected host, placements, connection, Signal values, receipts, and terminal result.

---

## See a real browser host

```bash
just doctor browser
just toggle
```

`just toggle` builds the Rust/WASM browser runtime, starts the native side of a bounded WebSocket session, and prints a URL.

Open that URL in an ordinary browser.

Press Enter in the terminal.

The resulting activation travels:

```text
terminal
   │
std kernel
   │
WebSocket
   │
browser kernel
   │
DOM
```

This is an actual browser-hosted Conduit graph, not a browser simulation.

---

## Inspect the plan

You can ask Conduit to retain a runtime report:

```bash
cargo run -p conduit -- \
  examples/signal-demo.form \
  --placements examples/std-local.placements \
  --report /tmp/conduit-run.json
```

Then inspect it:

```bash
cargo run -p conduit -- \
  observatory-report /tmp/conduit-run.json
```

The report exposes the hosts, offers, resources, plan, fragments, placements, connections, active play, evidence, and bounded retention involved in that realization.

---

## Talk to a real Pico W

With supported Pico W firmware attached:

```bash
cargo xtask prove std-pico-usb --interactive
```

The Pico exposes two CDC interfaces.

```text
CDC 0   Conduit link
CDC 1   physical evidence
```

The proof tooling checks the running Pico identity and its relationship to the expected plan before entering the interactive session.

---

## Span three hosts

The accepted hardware demonstration fans one source out to:

```text
                    ┌──> stdout
                    │
[source kernel] ────┼──> browser DOM
                    │
                    └──> Pico W LED
```

The browser path crosses WebSocket.

The Pico path crosses USB CDC.

All three are parts of one planned form.

Because this proof requires attached hardware, it is intentionally hardware-gated rather than pretending to be ordinary CI.

See **[Try Conduit](docs/try-conduit.md)** for the complete workflow.

Useful proof commands include:

```bash
just prove-std-browser-s4
just prove-std-browser-toggle
cargo xtask prove std-pico-usb
```

Check prerequisites with:

```bash
just doctor
```

---

# Form syntax

Canonical Form source uses:

```text
(...)     front
{...}     back
name: operation(arguments)
=         declarative immutable value relationship
>         runtime cord
```

Statement order is **not execution order**.

The graph determines connectivity.

See:

- [canonical examples](examples/README.md)
- [runnable form examples](docs/try-forms.md)

Older `.form` files marked `form 0` remain explicit compatibility fixtures. Their source identities are retained rather than silently interpreting them as current canonical syntax.

---

# Repository map

```text
crates/       portable contracts, parser, planner, kernel, runtime, catalog, CLI
hosts/        actual platform hosts and adapters
firmware/     constrained firmware targets
fixtures/     deterministic conformance fixtures
examples/     forms and placement examples
xtask/        typed repository workflows
docs/         architecture, design records, proofs, and guides
```

If you are new to the project, a good path is:

1. Run `just demo-std`.
2. Read [Try Conduit](docs/try-conduit.md).
3. Open [the Conduit canon](docs/conduit-canon.md).
4. Check [STATUS.md](STATUS.md) to see what is proven today.
5. Browse the [roadmap issue #361](https://github.com/dancxjo/conduit/issues/361).
6. Read [AGENTS.md](AGENTS.md) before making substantial changes.

---

# Design principles

A few rules keep the project pointed in the intended direction.

### Programs are graphs, not scripts

Statement order does not secretly become execution order.

We wire operations together.

### Meaning is not placement

A program should not need to name a machine merely because some realization eventually must.

### Interfaces are not implementations

Fronts should remain stable while backs can be replaced.

### Graph implementation is not host implementation

Backs realize fronts using Conduit operations.

Hosts realize the leaf operations they actually know how to perform.

### A carrier is not a cord

WebSocket, USB CDC, Wi-Fi, shared memory, and future transports are possible realizations of connectivity.

They should not become the meaning of the connection itself.

### Availability is not authority

A host possessing a capability does not automatically imply that a program may use it.

### Planning is not execution

Selecting a realization does not mean that realization is currently running.

### Simulation is not physical proof

Tests, fixtures, native simulation, browser execution, firmware execution, live transport, and observed physical effects are different claims.

Conduit tries to say exactly which one has been established.

---

# Contributing

Conduit is experimental and changing quickly.

Narrow changes are easier to reason about than giant ones. Keep architectural claims tied to executable evidence, and do not promote compilation or simulation into proof of a platform or physical effect.

The primary local gate is:

```bash
just check
```

Additional platform-specific checks are documented in the `justfile`, [STATUS.md](STATUS.md), and the relevant roadmap issues.

---

## The short version

If everything above is too much, keep this picture:

```text
          signals
             │
             ▼
[operation] ─────> [operation] ─────> [operation]
               cords
```

You describe that graph.

Operations may hide graphs of their own behind stable fronts.

Hosts offer ways to realize the graph's operations.

Carriers provide ways to realize cords that cross boundaries.

The planner fits those pieces together.

The kernel runs the admitted result.

**Conduit is a way to wire a program once, then reason explicitly about how that wiring becomes real.**
