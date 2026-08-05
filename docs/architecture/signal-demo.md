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

The std host can also run the pulse source while a browser host runs the show
sink through a deterministic bounded WebSocket relay fixture:

```text
cargo test -p conduit-browser-host std_host_sends_signal_to_browser_over_bounded_websocket_relay
```

That fixture plans the pair form across std and browser advertisements with the
`WebSocket` connection provider, serializes every connection envelope through
`conduit-wire`, enforces frame bounds, and compares the browser DOM-state
receipts against the same sixteen ordered signal values.

## Receipts

Each completed std manifestation emits a machine-readable receipt line:

```text
receipt signal placement=<placement-id> sequence=<n> level=<true-or-false>
```

The local std pair proof emits sixteen receipt lines. The local std triple
fixture emits forty-eight receipt lines: sixteen for each of the three show
sinks. The deterministic browser pair fixture and std-to-browser WebSocket relay
fixture each retain sixteen DOM-state receipts on the sink browser host
instance.

## Current Stop Line

The repository now contains a deterministic browser host crate, a canonical
`WebSocket` connection provider, and a bounded std-to-browser relay fixture. It
does not yet contain a WASM operator page, browser socket client/server runtime,
or Pico W host crate. M1 remains open until the same
`examples/triple-signal.form` can be planned across std, browser, and Pico W
hosts, with matching ordered receipts from stdout, DOM, and LED manifestations.
