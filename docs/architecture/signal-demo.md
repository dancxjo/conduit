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

The Pico host crate can run the pair form locally as a deterministic hosted
fixture while preserving a constrained no-`std` build boundary:

```text
cargo test -p conduit-pico-host
cargo check -p conduit-pico-host --no-default-features --target thumbv6m-none-eabi
```

This proves the Pico W advertisement exposes only bounded `flow/pulse` and
`display/show` capabilities, the hosted protocol fixture manifests show values
as retained onboard-LED receipts, and the non-hosted crate surface still checks
for the Cortex-M0+ target without Rust `std`.

The std host can also run the pulse source while a Pico host runs the show sink
through a deterministic bounded UDP relay fixture:

```text
cargo test -p conduit-pico-host std_host_sends_signal_to_pico_over_bounded_udp_relay
```

That fixture plans the pair form across std and Pico advertisements with the
`Udp` connection provider, serializes every connection envelope through
`conduit-wire`, enforces datagram bounds, and compares retained onboard-LED
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
instance. The deterministic Pico fixture retains sixteen onboard-LED receipts
with the same sequence and level values; the std-to-Pico UDP relay fixture
retains the same sixteen onboard-LED receipts after bounded wire transit.

## Current Stop Line

The repository now contains deterministic browser and Pico host crates, a
canonical `WebSocket` connection provider, and a bounded std-to-browser relay
fixture. It also contains a canonical `Udp` connection provider and bounded
std-to-Pico relay fixture. It does not yet contain a WASM operator page, browser
socket client/server runtime, physical Pico LED acceptance, live std-to-Pico UDP
sockets, or final std/browser/Pico fan-out proof. M1 remains open until the same
`examples/triple-signal.form` can be planned across std, browser, and Pico W
hosts, with matching ordered receipts from stdout, DOM, and LED manifestations.
