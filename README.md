# Conduit

Conduit is an experimental portable execution substrate for finite, typed flows
of work.

> Forms describe meaning. Hosts offer implementations. Plans make realization exact.

A Conduit **form** describes what should happen: named cells, typed cords,
semantic configuration, explicit boundaries, and finite work limits. It does not
hard-code which operating system, browser, microcontroller, process, transport,
device, or service must realize that work.

Running **hosts** advertise the exact operations they can currently realize,
together with exact realization identities, resources, authority, and links.
The planner binds those concrete facts to the form and produces an immutable
**plan**. Each host receives its exact plan fragment, and the Conduit kernel
executes it with bounded, deterministic scheduling and explicit pressure,
failure, cancellation, and evidence semantics.

The goal is for one authored form to be realizable:

- inside a native Rust process;
- in an actual browser through Rust/WASM;
- on a constrained microcontroller;
- across several connected hosts;
- or as part of a larger machine whose parts can take on different work.

Portability does not pretend these environments are identical. Memory, clocks,
devices, links, permissions, physical effects, and failure modes remain explicit
parts of planning and execution.

## The model

```text
OP     semantic operation such as text/upper
FORM   authored composition of semantic work
CELL   one named occurrence in a form
CORD   typed flow between cells
FACE   explicit visible boundary of a form

IMPL   platform-specific realization of an operation
HOST   running environment that offers exact operations
PLAN   exact immutable realization of a form
PLAY   one active execution of a plan
```

Several separations are fundamental:

- meaning is not deployment configuration;
- a capability being available is not authority to use it;
- a selected implementation is not an active execution;
- source, checked form, expanded form, plan, play, evidence, and presentation
  have distinct identities;
- platform adapters perform admitted effects, but do not become a second
  scheduler;
- pressure, disconnects, exhausted bounds, cancellation, and terminal failure
  remain visible runtime facts.

Before activation, a host must know and admit the finite execution shape it is
responsible for: cells, ports, cords, queue items, bytes, host operations,
resources, and mandatory evidence. Hosted implementations may allocate while
preparing a plan; admitted execution paths must not quietly grow without bound.

## A small form

```text
form 0

signal-demo {
    pulse: flow/pulse
    show: presentation/show

    pulse.count = 16
    pulse.period-ms = 250
    pulse.initial = false

    pulse > show
}
```

This form says that a finite pulse source feeds a presentation sink. It does not
say whether the sink is stdout, a browser DOM adapter, an LED, or another
implementation. Placement and realization belong to the plan, not the form.

## What exists today

The repository is a Rust workspace containing:

- a `no_std`, port-aware bounded execution kernel;
- a lossless form parser and checker;
- exact planning, identity, resource, authority, link, and evidence contracts;
- native std-host execution and boot-scoped portable planner offers;
- an actual Rust/WASM browser host, browser-local planning, and thin DOM adapter;
- bounded live WebSocket sessions between native and browser kernels;
- Pico W firmware with bounded dual-CDC USB transport and physical evidence;
- an accepted exact three-host Signal path spanning stdout, browser DOM, and a
  physical Pico W LED;
- a read-only Observatory path over neutral runtime-report artifacts;
- deterministic browser-shaped and Pico-shaped conformance fixtures; and
- the first installed `conduit.std` kind, `conduit.std/time-tick@2` over
  `value/tick@1`.

Conduit is still under active development. Compile checks, simulations, hosted
execution, browser execution, live transport, firmware, and physical hardware
proof are deliberately treated as different evidence classes. See
[STATUS.md](STATUS.md) for the exact claims established by current code and named
checks.

## Try Conduit now

Install a recent Rust toolchain and [`just`](https://github.com/casey/just), then
start with the native host:

```bash
just demo-std
```

That command parses the unchanged Signal form, plans it onto the actual std
host, executes it through `conduit-kernel`, and prints the selected host,
placements, connection, sixteen Signal values, receipts, and terminal result.

To **see an actual browser host**, run:

```bash
just doctor browser
just toggle
```

`just toggle` builds the Rust/WASM browser runtime, starts a bounded WebSocket
session from the std host, and prints an HTTP URL. Open that exact URL in a
normal browser, then press Enter in the terminal. Each admitted activation runs
through the std kernel, crosses the live session, enters the browser kernel, and
appears as a DOM receipt. This is an interactive hosted demonstration, not the
browser simulation.

To inspect what Conduit actually planned and executed, write a runtime report:

```bash
cargo run -p conduit -- \
  examples/signal-demo.form \
  --placements examples/std-local.placements \
  --report /tmp/conduit-run.json

cargo run -p conduit -- \
  observatory-report /tmp/conduit-run.json
```

The Observatory report shows hosts, capability offers, resources, plan,
fragments, placements, connections, the active Play, evidence, and bounded
retention. Current std-host reports also expose the installed
`conduit.std/time-tick@2` capability even though a polished standalone tick demo
is still separate usability work.

With a Pico W attached, try the real physical USB-CDC session:

```bash
cargo xtask prove std-pico-usb --interactive
```

CDC 0 carries the bounded Conduit session and CDC 1 carries physical evidence.
The operator tool verifies the running Pico identity and exact plan relationship
before entering the interactive session.

The accepted final S4 demonstration goes further: one unchanged three-host form
fans out through one source kernel to stdout, a real browser DOM over WebSocket,
and a physical Pico LED over USB CDC. Because that proof requires attached
hardware, it is deliberately hardware-gated rather than part of ordinary CI.

For exact commands, expected behavior, proof-class distinctions, and the final
three-host hardware workflow, see **[Try Conduit](docs/try-conduit.md)**.

Useful non-interactive proof commands include:

```bash
just prove-std-browser-s4
just prove-std-browser-toggle
cargo xtask prove std-pico-usb
```

Inspect host prerequisites at any time with:

```bash
just doctor
```

## Repository guide

- [`crates/`](crates/) contains the portable contracts, parser, planner, kernel,
  runtime, standard catalog, and command-line surfaces.
- [`hosts/`](hosts/) contains actual platform hosts and adapters.
- [`firmware/`](firmware/) contains constrained firmware targets.
- [`fixtures/`](fixtures/) contains deterministic conformance fixtures, not live
  transports.
- [`examples/`](examples/) contains forms and placement examples.
- [`xtask/`](xtask/) owns typed repository workflows.
- [`docs/`](docs/) contains the architectural direction and design records.

Start with:

- [Try Conduit](docs/try-conduit.md) for runnable programs and proofs;
- [The Conduit canon](docs/conduit-canon.md) for the durable project model and
  architectural invariants;
- [STATUS.md](STATUS.md) for the current executable claim boundary;
- [roadmap issue #361](https://github.com/dancxjo/conduit/issues/361) for the
  implementation sequence;
- [AGENTS.md](AGENTS.md) for contribution, review, proof, and module-size rules.

## Contributing

Keep changes narrow, keep runtime claims tied to executable evidence, and do not
promote compilation or simulation into proof of a platform or physical effect.
The repository's primary local gate is:

```bash
just check
```

Additional target-specific gates are documented in the `justfile`,
[STATUS.md](STATUS.md), and the relevant roadmap issues.
