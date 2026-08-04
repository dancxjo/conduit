# Conduit

Conduit runs semantic forms across software hosts.

A realm contains host instances. Hosts advertise the capabilities they can currently provide. A form describes typed work without naming machines, operating systems, transports, or devices. A planner combines the form with current capability offers and produces an exact plan. Activating that plan runs the same semantic work wherever the selected hosts can faithfully realize it.

The first foundation proof uses one unchanged signal form across three host platforms:

- a portable Rust `std` host that shows the signal on stdout;
- multiple independent browser hosts that show it in the DOM;
- a Pico W host that shows it on the onboard LED.

The architecture, invariants, host profiles, planning model, bounded cord protocol, and implementation rules are documented in:

- [Portable Host Architecture](docs/architecture/portable-hosts.md)
- [Foundation implementation issue #347](https://github.com/dancxjo/conduit/issues/347)

## Reboot principle

> Forms describe meaning. Hosts offer implementations of that meaning. Plans make the mapping exact.

The reboot intentionally begins with one finite, inspectable, cross-host source-to-sink flow before restoring broader libraries, robotics, durable body identity, or `.soul` recovery.

## Current implementation slice

This repository currently implements the Rust `std` host vertical slice from issue #347:

- parses portable `form 0` files for `flow/pulse` and `display/show`;
- builds an exact local plan from host capability advertisements;
- validates bounded queue limits and host boot identity;
- executes finite pulse signals;
- manifests `display/show<Signal>` on stdout;
- records machine-readable show receipts and prints a concise completion summary.

Try the included forms:

```bash
just demo-std
just demo-triple-local
```