# Conduit

Conduit is an experimental portable execution substrate for finite, typed flows
of work.

> Forms describe meaning. Hosts offer implementations. Plans make realization exact.

A Conduit **form** describes what should happen: named cells, typed cords,
semantic configuration, explicit boundaries, and finite work limits. It does not
hard-code which operating system, browser, microcontroller, process, transport,
device, or service must realize that work.

Running **hosts** report the implementations, resources, authority, and links
they can currently provide. The planner binds those concrete facts to the form
and produces an immutable **plan**. Each host receives its exact plan fragment,
and the Conduit kernel executes it with bounded, deterministic scheduling and
explicit pressure, failure, cancellation, and evidence semantics.

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
KIND   semantic contract
FORM   authored composition of semantic work
CELL   one named occurrence in a form
CORD   typed flow between cells
FACE   explicit visible boundary of a form

IMPL   platform-specific realization of a kind
HOST   running environment that offers implementations
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
responsible for: operations, ports, cords, queue items, bytes, host operations,
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
- native std-host execution;
- an actual Rust/WASM browser host and thin DOM adapter;
- a bounded loopback WebSocket path between native and browser kernels;
- Pico W firmware and typed build/flash/verify tooling;
- deterministic browser-shaped and Pico-shaped conformance fixtures;
- early composite, catalog, body/realm, and Observatory contracts.

Conduit is still under active development. Compile checks, simulations, hosted
execution, browser execution, live transport, firmware, and physical hardware
proof are deliberately treated as different evidence classes. See
[STATUS.md](STATUS.md) for the exact claims established by current code and named
checks.

## Run the examples

Install a recent Rust toolchain and [`just`](https://github.com/casey/just), then
run:

```bash
just demo-std
just demo-triple-local
```

Run the accepted native-to-browser loopback demonstration with:

```bash
just prove-std-browser-s4
```

That command also requires the repository's Node and Playwright dependencies.
It is a deliberately bounded local proof, not a claim of a general network
stack.

Inspect host prerequisites with:

```bash
just doctor
```

Build the Pico W firmware with:

```bash
just pico-build
```

The flash and verification commands require a connected board and the host-side
tools reported by `just doctor pico`.

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
