# Small `conduit.std` catalog readiness

This is the first M4 slice for a small standard catalog.

The implementation lives in `conduit-std-catalog`. It is a contract catalog,
not a runtime special case. The crate exposes allocator-backed contracts under
`no_std`; the optional `form-catalog` feature converts those contracts into the
existing `conduit-form::ProfileCatalog` for parser/planner conformance.

## Initial socket set

The catalog starts with the M4 candidate set:

- `flow/pulse` — Pulse: emit a bounded alternating signal sequence.
- `presentation/show` — Show: present input values through a host-honest manifestation.
- `flow/map` — Map: transform one bounded input stream into one bounded output stream.
- `flow/filter` — Filter: forward only values accepted by a bounded predicate.
- `flow/tee` — Tee: copy each input value to two bounded output branches.
- `text/format` — Format text: render input values into bounded text values.
- `time/tick` — Tick: emit a bounded timer tick sequence.
- `state/latest` — Latest state: retain the latest input value and emit bounded updates.

Each contract declares:

- semantic kind ID;
- plain-language name and summary;
- inputs and outputs;
- configuration fields;
- capability limits;
- terminal behavior;
- whether hosted implementation is required;
- whether browser/Pico manifestation claims are honest for that kind;
- a minimal example line.

## Boundary

`conduit-std-catalog` does not modify `conduit-runtime`. Adding these kinds is
represented by installing profile contracts and, in later slices, hosted
operation implementations through the existing runtime implementation registry.

The current slice proves parser/planner conformance independent of UI. It does
not yet provide hosted implementations for every new kind. `flow/pulse` and
`presentation/show` remain the currently executable signal implementations.

The current form grammar supports shorthand connections for one-output to
one-input operations. `flow/tee` already declares both output ports, but forms
that select one branch explicitly need a later grammar or authoring slice before
they can use `left` and `right` independently.

## Checkpoint commands

```text
cargo test -p conduit-std-catalog
cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi
just check-std-catalog-readiness
```
