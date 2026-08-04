# Conduit

Conduit runs semantic forms across software hosts.

A realm contains host instances. Hosts advertise the capabilities they can currently provide. A form describes typed work without naming machines, operating systems, transports, or devices. A planner combines the form with current capability offers and produces an exact plan. Activating that plan runs the same semantic work wherever the selected hosts can faithfully realize it.

The first foundation proof uses one unchanged signal form across three host platforms:

- a portable Rust `std` host that shows the signal on stdout;
- multiple independent browser hosts that show it in the DOM;
- a Pico W host that shows it on the onboard LED.

The architecture, invariants, host contract, planning model, bounded connection protocol, and implementation rules are documented in:

- [Portable Host Architecture](docs/architecture/portable-hosts.md)
- [Conduit Host Specification](docs/architecture/host-specification.md)
- [Foundation implementation issue #347](https://github.com/dancxjo/conduit/issues/347)

## Reboot principle

> Forms describe meaning. Hosts offer implementations of that meaning. Plans make the mapping exact.

The reboot intentionally begins with one finite, inspectable, cross-host source-to-sink flow before restoring broader libraries, robotics, durable body identity, or `.soul` recovery.

## Current implementation slice

This repository currently implements the Rust `std` host vertical slice from issue #347:

- parses portable `form 0` files for `flow/pulse` and `presentation/show`;
- compiles real `conduit-core`, `conduit-form`, `conduit-planner`, `conduit-runtime`, and `conduit-std-host` boundaries in the workspace;
- parses explicit operator placement files;
- builds an exact local plan from host capability advertisements and placement choices;
- validates bounded queue limits, boot identity, and offer generation at preparation time;
- executes finite pulse signals through a platform-neutral runtime and a real bounded local connection;
- manifests `presentation/show<Signal>` on stdout through the Rust `std` host adapter;
- records structured observations and prints an operator report plus a concise completion summary.

Try the included forms:

```bash
just demo-std
just demo-triple-local
```

The current local planner only accepts placements on the bundled `std-host-1` advertisement. The placement files in `examples/*.placements` make those selections explicit rather than relying on implicit local assignment.
