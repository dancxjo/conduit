# Signal Demo M1 Progress

This records the current implementation state for the portable signal proof
tracked by GitHub issues #347 and #350.

## Current Slice

The checked semantic forms use the portable M1 kind names:

- `flow/pulse`
- `display/show`

The std host can run the local pair form:

```text
cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements
```

It can also run the final platform-neutral fan-out form against a local std
fixture:

```text
cargo run -p conduit -- examples/triple-signal.form --placements examples/triple-local.placements
```

The local triple fixture intentionally places all three `display/show` sinks on
the std host. It proves the authored final form has no platform facts and that
the host protocol can produce independent bounded receipts for all three sinks.
It is not the final browser/Pico realm proof.

## Receipts

Each completed std manifestation emits a machine-readable receipt line:

```text
receipt signal placement=<placement-id> sequence=<n> level=<true-or-false>
```

The local pair proof emits sixteen receipt lines. The local triple fixture emits
forty-eight receipt lines: sixteen for each of the three show sinks.

## Current Stop Line

The repository does not yet contain browser or Pico W host crates. M1 remains
open until the same `examples/triple-signal.form` can be planned across std,
browser, and Pico W hosts, with matching ordered receipts from stdout, DOM, and
LED manifestations.
