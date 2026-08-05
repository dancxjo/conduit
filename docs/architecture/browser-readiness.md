# Simulated-host conformance drawbridge

The browser-shaped and Pico-shaped crates are deterministic protocol fixtures.
They are not platform hosts. S4 has crossed the browser drawbridge: an actual
Chromium document runs the Rust/WASM planner/runtime and bounded DOM adapter,
both locally and as the sink of the live std-to-browser WebSocket described in
`browser-host-s4.md`. Firmware, UDP, non-loopback deployment, and physical
acceptance claims remain gated.

- `conduit-runtime` has no dependency on `conduit-signal` and dispatches only through installed operation implementations.
- `conduit-form` has no dependency on `conduit-signal`; callers supply a `ProfileCatalog` containing kind IDs, ports, configuration defaults, and validation rules.
- `conduit-signal` defaults to its `no_std` semantic layer. A host that needs pulse/show runtime behavior explicitly enables `host-profile` and installs it.
- The std host explicitly enables and installs the signal host profile. Its remaining responsibilities are CLI assembly, timers, and stdout presentation.
- The `host_contract` test runs a one-byte `contract/source -> contract/sink` profile through the ordinary parser, planner, implementation registry, and runtime without importing `conduit-signal`.
- Controlled composite transport proves queue pressure, retained source values, byte release, malformed delivery, queued and empty disconnect, undeliverable counts, and terminal rejection.
- `conduit-wire` defines and tests the deterministic, bounded, `no_std`-compatible connection-envelope representation documented in `connection-envelope-wire.md`.
- The reusable in-memory provider belongs to `conduit-runtime::providers`, not the composite fixture.
- A fake browser-style adapter manually completes waits and presentations, delays a connection delivery, injects presentation failure and provider disconnect, and inspects structured observations.
- `conduit-browser-sim` models multiple independent simulated browser instances, advertises fixture capabilities, and runs `flow/pulse -> presentation/show` through memory plus a bounded frame relay fixture using `conduit-wire`. It compiles for `wasm32-unknown-unknown`, but is not the DOM adapter and has no socket.
- `hosts/browser-runtime` compiles the real planner, runtime, and shared signal implementation to WASM. Chromium runs two independent local instances, plus a remote sink connected to a native std source by one actual bounded WebSocket.
- `conduit-pico-sim` exposes a Pico-shaped contract fixture, compiles for `thumbv6m-none-eabi` without default features, and retains simulated onboard-LED receipts through memory plus a bounded datagram relay fixture using `conduit-wire`. It is not firmware and has no device driver.
- `examples/triple-signal.form` is planned unchanged across std and the two simulations. The compared stdout, DOM-state, and onboard-LED-shaped receipts are conformance data, not three-host acceptance.
- A composite definition owns a set of child bindings and exact plan fragments. The current fixture permits one exposed in-memory boundary, while runtime dispatch and terminal tracking are keyed by child host identity rather than source/sink fields.
- Parent-facing events and observations use only the composite identity. Child host IDs and child details are available only through explicit internal diagnostic methods.
- Host-contract tests pass without browser automation.

The required checkpoint commands are:

```text
cargo check -p conduit-signal --no-default-features
cargo check -p conduit-wire --no-default-features
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p conduit-browser-sim std_host_sends_signal_to_browser_through_bounded_frame_fixture
cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts
cargo check -p conduit-browser-sim --target wasm32-unknown-unknown
cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture
cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi
just check-sim-readiness
npm run test:browser-host
```

The deterministic fixture drawbridge, browser-side form execution, DOM adapter,
and loopback std-to-browser WebSocket are green. Physical Pico LED acceptance,
live UDP, non-loopback deployment, TCP services, DHCP, DNS, discovery, durable
body identity, and `.soul` remain beyond this checkpoint.
