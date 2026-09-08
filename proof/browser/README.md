# Browser conformance

This directory contains browser tests, fixtures, and proof-only launch support.
The browser Host lives in `targets/browser/host/`, its WASM runtime in
`targets/browser/runtime/`, and application code under `products/`. Start with
[the contributor guide](../../CONTRIBUTING.md) for environment setup.

## Run the suite

```sh
cargo xtask check browser-host
```

To run the proof with its evidence manifest:

```sh
cargo xtask prove browser-host
```

Both commands use the repository's browser tooling and build the required
product artifacts. Inspect prerequisites with `cargo xtask doctor browser`.
For reviewed canonical Forms specifically, use
`cargo xtask forms run --browser`. See the [visual evidence guide](../../docs/visual-evidence.md)
for capture and publication rules.

Browser acceptance uses pinned Chromium, one worker, zero retries, and ordinary
interaction. A test should perform an action once, then assert its correlated
semantic result. Screenshots document that result. Compatibility projects have
separate roles and do not replace the canonical capture environment.

## What the tests establish

The basic Signal specimen parses and plans unchanged
`proof/fixtures/forms/signal-demo.conduit`, lowers the exact fragment, and runs
`conduit-kernel` compiled to WASM. JavaScript supplies timers and DOM effects.
Each page has its own WASM instance, Host/Boot identity, Plan, active Play,
fixed ABI buffers, and receipts. Completion must match the outstanding
operation and its exact runtime identity before execution advances.

Other suites exercise Tour, Crèche, Patchbay, Body lifecycle, resource
operations, remote execution, and transport-specific contracts. Keep the claim
at the boundary actually exercised: a WASM build is not a browser test, and a
local browser fixture does not establish physical hardware behavior.

`webchat.test.html`, for example, runs one kernel per page over
`forms/webchat/main.conduit`. The external-WebSocket proof sends messages through
real controls and observes disconnect behavior with bounded history and input.
Its authored `net/websocket` operation is separate from a Conduit session Line
using the WebSocket Base.

## Adding or debugging a proof

Keep new product code with its product or Host owner; add only the test and its
necessary fixture here. Reuse existing semantic assertions and fixtures before
adding another server or runtime arrangement. The owning issue determines
whether new browser proof is needed.

Independent proof processes can use distinct bounded loopback ports and result
identities. Internal launch support validates `CONDUIT_BROWSER_HOST_PORT` and
`CONDUIT_BROWSER_PROOF_SHARD`, binds only `127.0.0.1`, and confines each shard to
its result directory. These are harness details; the supported repository
entrance remains `cargo xtask`.
