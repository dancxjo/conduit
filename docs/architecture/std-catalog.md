# Small `conduit.std` catalog readiness

This is the M4 baseline for a small standard catalog.

The implementation lives in `conduit-std-catalog`. It is a contract catalog,
not a runtime special case. The crate exposes allocator-backed contracts under
`no_std`; the optional `form-catalog` feature converts those contracts into the
existing `conduit-form::ProfileCatalog` for parser/planner conformance. The
optional `host-profile` feature installs hosted implementations through the
existing `conduit-runtime::ImplementationRegistry`.

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
represented by installing profile contracts and hosted operation implementations
through the existing runtime implementation registry.

The default hosted profile provides bounded implementations for all eight
catalog kinds:

- `flow/pulse` and `time/tick` emit bounded numeric values and complete after
  the configured count.
- `flow/map` forwards one accepted value in the default hosted implementation.
- `flow/filter` forwards all values for predicate `0`; other predicates keep
  even numeric values.
- `flow/tee` relies on the host's existing multi-output connection delivery to
  copy a value to both branches.
- `text/format` emits bounded UTF-8 bytes such as `value:0`.
- `state/latest` stores and emits the most recent value, then clears retained
  state on release.
- `presentation/show` requests a host presentation and completes once the host
  reports that presentation as successful.

The executable std profile uses the generic `value/any` wire kind. The stricter
signal-specific browser/Pico manifestation profile remains in `conduit-signal`;
the std catalog does not claim browser or Pico implementations for the generic
flow/map, flow/filter, flow/tee, text/format, time/tick, or state/latest kinds.

The current form grammar supports shorthand connections for one-output to
one-input operations and explicit port selections such as
`split.left -> latest.in` and `split.right -> format.in`.

## Executable conformance receipts

The crate tests three UI-independent hosted forms through `HostRuntime`:

- `flow/pulse -> presentation/show`
- `time/tick -> text/format -> presentation/show`
- `time/tick -> flow/map -> flow/filter -> flow/tee`, with one branch to
  `state/latest -> presentation/show` and the other to
  `text/format -> presentation/show`

These receipts exercise the same host protocol used by other profiles:
advertisement, planning, preparation, activation, bounded local delivery,
presentation completion, placement terminals, connection terminals, and a
completed plan terminal observation.

## Checkpoint commands

```text
cargo test -p conduit-std-catalog
cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi
just check-std-catalog-readiness
```
