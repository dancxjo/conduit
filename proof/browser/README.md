# Browser conformance specimens

This directory owns browser conformance fixtures and proof-only launch support.
It is not a third browser Host implementation. The authoritative product owners
are `targets/browser/host` for the browser Host and assets and
`targets/browser/runtime` for browser composition and Bases.

The primary specimen parses and plans unchanged
`proof/fixtures/forms/signal-demo.conduit`, lowers its exact local fragment through
the shared plan-to-kernel contract, and executes it with `conduit-kernel`'s
port-aware fixed scheduler compiled to `wasm32-unknown-unknown`. It has no
alternate executor.
JavaScript owns only the browser platform effects: real timers and DOM presentation.

Each page host receives an independent WebAssembly instance, so its runtime state, host/boot
identity, exact plan fragment, active play, scheduler, presentation/sign identities, fixed-size
ABI buffers, and receipt count are not shared with the other page host. The runtime emits one host
operation request through a 4,096-byte output frame and accepts one completion through a separate
4,096-byte input frame. A completion advances execution only when its source, checked, expanded,
plan, fragment, host, boot, active-play, node/request/operation, placement, presentation/sign,
value-kind, and encoded-value fields are the exact bytes expected for the outstanding request.

Run the proof with:

```sh
rustup target add wasm32-unknown-unknown
cargo xtask check browser-host
```

All test pages, JavaScript adapters, Playwright configurations, and the static
server here exist to establish their named proof classes. Product browser
launch and runtime code must not be added here.

The Chromium test has one pinned project, one worker, no retries, and no forced interaction. It
runs two independent page hosts concurrently, waits on all fifteen 250 ms intervals per host,
retains sixteen nine-byte signal receipts per host, and verifies duplicate, malformed, item-bound,
byte-bound, cancellation, platform-failure, and mismatched-runtime-identity rejection. Rust seals
numeric routes, operation slots, values, sign, identities, and capture capacities before its
first scheduler step and checks that those capacities do not grow. This is a bounded-capacity proof,
not a claim that browser allocation can be measured reliably from JavaScript.

Independent browser proof processes use distinct bounded loopback ports and
result identities. For example, two one-worker shards may run concurrently as:

```sh
CONDUIT_BROWSER_HOST_PORT=4173 CONDUIT_BROWSER_PROOF_SHARD=tour npx playwright test --config proof/browser/playwright.config.mjs proof/browser/tour.spec.mjs --workers=1 --retries=0
CONDUIT_BROWSER_HOST_PORT=4174 CONDUIT_BROWSER_PROOF_SHARD=creche npx playwright test --config proof/browser/playwright.config.mjs proof/browser/creche-workload.spec.mjs --workers=1 --retries=0
```

Both values are validated before a server starts. The port is always bound to
`127.0.0.1`; the shard identity selects a child of `test-results/` and cannot
escape that directory. CI may retain the deterministic defaults because each
GitHub job already has an isolated runner.

## Explicit external-WebSocket webchat

`webchat.test.html` instantiates one independent WASM kernel per page. Rust
checks and expands `forms/webchat/main.conduit`, plans the exact browser fragment,
and surfaces correlated native socket and DOM host operations. JavaScript owns
the browser `WebSocket`, input event, and list mutation only; it does not own
chat history bounds, operation lifecycle, Plan identity, or terminal truth.

The focused Chromium proof opens two pages against `webchat-server`, sends
`hello from A` and `hello from B` through real controls, disconnects A, and
shows the remaining peer continues. History is limited to sixteen items,
messages to 256 bytes, and input events to eight. Disconnect, malformed input,
oversize input, and successful host completion stay distinct.

`net/websocket` is the authored external protocol operation. A Line using the
exact `conduit.base/websocket-rfc6455@1` Base realization remains the unrelated
carrier for Conduit sessions; the webchat does not use that Line or its session
runtime.
