# Browser readiness drawbridge

Browser host work remains out of scope until every item below is true on the same revision.

- `conduit-runtime` has no dependency on `conduit-signal` and dispatches only through installed operation implementations.
- `conduit-form` has no dependency on `conduit-signal`; callers supply a `ProfileCatalog` containing kind IDs, ports, configuration defaults, and validation rules.
- `conduit-signal` defaults to its `no_std` semantic layer. A host that needs pulse/show runtime behavior explicitly enables `host-profile` and installs it.
- The std host explicitly enables and installs the signal host profile. Its remaining responsibilities are CLI assembly, timers, and stdout presentation.
- The `host_contract` test runs a one-byte `contract/source -> contract/sink` profile through the ordinary parser, planner, implementation registry, and runtime without importing `conduit-signal`.
- Controlled composite transport proves queue pressure, retained source values, byte release, malformed delivery, queued and empty disconnect, undeliverable counts, and terminal rejection.
- A composite definition owns a set of child bindings and exact plan fragments. The current fixture permits one exposed in-memory boundary, while runtime dispatch and terminal tracking are keyed by child host identity rather than source/sink fields.
- Parent-facing events and observations use only the composite identity. Child host IDs and child details are available only through explicit internal diagnostic methods.
- Host-contract tests pass without browser automation.

The required checkpoint commands are:

```text
cargo check -p conduit-signal --no-default-features
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Only after this drawbridge is green may a browser host be introduced. Browser, Pico W, WebSocket, TCP, UDP, DOM, LED, DHCP, DNS, discovery, durable body identity, and `.soul` remain beyond this checkpoint.
