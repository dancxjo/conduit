# Signal conformance fixtures

This records the current implementation state for the portable signal proof
tracked by GitHub issue #350. Most cross-host paths below remain simulations;
the separately named DOM boundary is actual browser execution.

## Current Slice

The checked semantic forms use the portable M1 kind names:

- `flow/pulse`
- `presentation/show`

The std host can run the local pair form:

```text
cargo xtask demo std
```

It can also run the final platform-neutral fan-out form against a local std
fixture:

```text
cargo xtask demo triple
```

The local triple fixture intentionally places all three `presentation/show` sinks on
the std host. It proves the authored final form has no platform facts and that
the host protocol can produce independent bounded receipts for all three sinks.
It is not the final browser/Pico planning scope proof.

The browser simulation can run the pair form across two independent instances
in one page-model fixture:

```text
cargo test -p conduit-browser-sim
cargo check -p conduit-browser-sim --target wasm32-unknown-unknown
```

This proves fixture identity separation, bounded in-memory delivery, DOM-shaped
receipts, and a WASM compile boundary. It is not the actual DOM adapter or a
browser-side host runtime.

The first actual browser checkpoint runs a presentation effect/completion
adapter in Chromium:

```text
npm run test:browser-host
```

It creates two independent host/boot-bound instances in one page, appends
sixteen machine-readable DOM receipts to each, echoes exact play/presentation
identities, and rejects duplicate, malformed, and capacity-exhausted requests.
It does not yet plan or run the authored form in the browser.

The std host can also run the pulse source while the browser simulation runs
the show sink through a deterministic bounded frame relay fixture:

```text
cargo test -p conduit-browser-sim std_host_sends_signal_to_browser_through_bounded_frame_fixture
```

That fixture plans the pair form across std and simulated advertisements with
the `FixtureFrame` base, serializes every connection envelope through
`conduit-wire`, enforces frame bounds, and compares the browser DOM-state
receipts against the same sixteen ordered signal values.

The Pico-shaped simulation can run the pair form as a deterministic hosted
fixture while preserving a constrained no-`std` contract build boundary:

```text
cargo test -p conduit-pico-sim
cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi
```

This proves bounded advertised fixture capabilities, simulated LED receipts,
and a Cortex-M0+ compile boundary without Rust `std`. It is not firmware and
does not drive an LED.

The std host can also run the pulse source while the Pico simulation runs the
show sink through a deterministic bounded datagram relay fixture:

```text
cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture
```

That fixture plans the pair form across std and simulated advertisements with
the `FixtureDatagram` base, serializes every connection envelope through
`conduit-wire`, enforces datagram bounds, and compares retained onboard-LED
receipts against the same sixteen ordered signal values.

The same `fixtures/forms/triple-signal.conduit` can be planned across std and both
simulations in one deterministic proof:

```text
cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts
```

That proof keeps the authored form free of platform and transport facts while
the plan places `local` on std stdout and the other sinks on simulated
manifestations. Bounded frame/datagram fixtures carry the envelopes. The
comparison is conformance Sign, not browser, firmware, socket, or HIL proof.

## Receipts

Each completed std manifestation emits a machine-readable receipt line:

```text
receipt signal placement=<placement-id> sequence=<n> level=<true-or-false>
```

The local std pair proof emits sixteen receipt lines. The local std triple
fixture emits forty-eight receipt lines: sixteen for each of the three show
sinks. The deterministic browser pair and frame fixtures each retain sixteen
DOM-shaped receipts on the sink simulation. The Pico-shaped local and datagram
fixtures retain the same sixteen simulated LED receipts. The triple-simulation
proof compares all three streams from the unchanged form.

## Current Stop Line

The repository contains deterministic browser-shaped and Pico-shaped
simulations plus frame/datagram relay fixtures and one actual Chromium DOM
presentation adapter. It does not contain a browser-side planner/runtime, live
WebSocket or UDP base, Pico firmware, physical LED acceptance, or
three-host manifestation proof.
