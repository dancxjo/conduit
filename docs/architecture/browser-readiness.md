# Browser readiness drawbridge

The initial browser host crate is now allowed only as a deterministic protocol
fixture. Browser UI, WebSocket transport, visual smoke tests, and acceptance
claims remain gated until the deterministic checks below stay green.

- `conduit-runtime` has no dependency on `conduit-signal` and dispatches only through installed operation implementations.
- `conduit-form` has no dependency on `conduit-signal`; callers supply a `ProfileCatalog` containing kind IDs, ports, configuration defaults, and validation rules.
- `conduit-signal` defaults to its `no_std` semantic layer. A host that needs pulse/show runtime behavior explicitly enables `host-profile` and installs it.
- The std host explicitly enables and installs the signal host profile. Its remaining responsibilities are CLI assembly, timers, and stdout presentation.
- The `host_contract` test runs a one-byte `contract/source -> contract/sink` profile through the ordinary parser, planner, implementation registry, and runtime without importing `conduit-signal`.
- Controlled composite transport proves queue pressure, retained source values, byte release, malformed delivery, queued and empty disconnect, undeliverable counts, and terminal rejection.
- `conduit-wire` defines and tests the deterministic, bounded, `no_std`-compatible connection-envelope representation documented in `connection-envelope-wire.md`.
- The reusable in-memory provider belongs to `conduit-runtime::providers`, not the composite fixture.
- A fake browser-style adapter manually completes waits and presentations, delays a connection delivery, injects presentation failure and provider disconnect, and inspects structured observations.
- `conduit-browser-host` models multiple independent browser host instances in one page, advertises capabilities per instance, runs `flow/pulse -> display/show` over a bounded in-memory browser link without Playwright, and compiles for `wasm32-unknown-unknown`.
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
cargo check -p conduit-browser-host --target wasm32-unknown-unknown
just check-browser-readiness
```

Only after this drawbridge is green may browser work advance beyond the deterministic host fixture. Browser UI, Pico W, WebSocket, TCP, UDP, LED, DHCP, DNS, discovery, durable body identity, and `.soul` remain beyond this checkpoint.
