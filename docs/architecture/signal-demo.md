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

The browser host crate can run the pair form across two independent browser
host instances in one page-model fixture:

```text
cargo test -p conduit-browser-host
cargo check -p conduit-browser-host --target wasm32-unknown-unknown
```

This proves separate browser host IDs and boot IDs, independent capability
advertisements, a bounded in-memory browser link, DOM-state receipts, and a WASM
compile boundary without introducing Playwright or visual timing as acceptance.

## Receipts

Each completed std manifestation emits a machine-readable receipt line:

```text
receipt signal placement=<placement-id> sequence=<n> level=<true-or-false>
```

The local std pair proof emits sixteen receipt lines. The local std triple
fixture emits forty-eight receipt lines: sixteen for each of the three show
sinks. The deterministic browser pair fixture retains sixteen DOM-state receipts
on the sink browser host instance.

## Current Stop Line

The repository now contains a deterministic browser host crate, but not a WASM
operator page, WebSocket link, or Pico W host crate. M1 remains open until the
same `examples/triple-signal.form` can be planned across std, browser, and Pico
W hosts, with matching ordered receipts from stdout, DOM, and LED
manifestations.
