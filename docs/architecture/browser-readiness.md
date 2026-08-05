# Browser readiness drawbridge

The initial browser host crate is now allowed only as a deterministic protocol
fixture. Browser UI, live WebSocket transport, visual smoke tests, and
acceptance claims remain gated until the deterministic checks below stay green.

- `conduit-runtime` has no dependency on `conduit-signal` and dispatches only through installed operation implementations.
- `conduit-form` has no dependency on `conduit-signal`; callers supply a `ProfileCatalog` containing kind IDs, ports, configuration defaults, and validation rules.
- `conduit-signal` defaults to its `no_std` semantic layer. A host that needs pulse/show runtime behavior explicitly enables `host-profile` and installs it.
- The std host explicitly enables and installs the signal host profile. Its remaining responsibilities are CLI assembly, timers, and stdout presentation.
- The `host_contract` test runs a one-byte `contract/source -> contract/sink` profile through the ordinary parser, planner, implementation registry, and runtime without importing `conduit-signal`.
- Controlled composite transport proves queue pressure, retained source values, byte release, malformed delivery, queued and empty disconnect, undeliverable counts, and terminal rejection.
- `conduit-wire` defines and tests the deterministic, bounded, `no_std`-compatible connection-envelope representation documented in `connection-envelope-wire.md`.
- The reusable in-memory provider belongs to `conduit-runtime::providers`, not the composite fixture.
- A fake browser-style adapter manually completes waits and presentations, delays a connection delivery, injects presentation failure and provider disconnect, and inspects structured observations.
- `conduit-browser-host` models multiple independent browser host instances in one page, advertises capabilities per instance, runs `flow/pulse -> presentation/show` over a bounded in-memory browser link without Playwright, plans std-to-browser delivery over a bounded `WebSocket` relay using `conduit-wire`, and compiles for `wasm32-unknown-unknown`.
- `conduit-pico-host` exposes a constrained Pico W advertisement without `std`, compiles for `thumbv6m-none-eabi` with default features disabled, runs `flow/pulse -> presentation/show` locally to retained onboard-LED receipts, and plans std-to-Pico delivery over a bounded `Udp` relay using `conduit-wire`.
- `examples/triple-signal.form` is planned unchanged across std, browser, and Pico host advertisements in a deterministic proof that compares matching stdout, DOM-state, and onboard-LED receipts.
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
cargo test -p conduit-browser-host std_host_sends_signal_to_browser_over_bounded_websocket_relay
cargo test -p conduit-browser-host triple_signal_form_fans_out_to_std_browser_and_pico_receipts
cargo check -p conduit-browser-host --target wasm32-unknown-unknown
cargo test -p conduit-pico-host std_host_sends_signal_to_pico_over_bounded_udp_relay
cargo check -p conduit-pico-host --no-default-features --target thumbv6m-none-eabi
just check-browser-readiness
```

Only after this drawbridge is green may browser and Pico work advance beyond deterministic host fixtures. Browser UI, physical Pico LED acceptance, live WebSocket sockets, live UDP sockets, TCP, DHCP, DNS, discovery, durable body identity, and `.soul` remain beyond this checkpoint.
