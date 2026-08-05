# Browser host

This host runs `examples/signal-demo.form` through the real Rust planner, runtime, and
`conduit-signal` implementation compiled to `wasm32-unknown-unknown`. JavaScript owns only the
browser platform effects: real timers and DOM presentation.

Each page host receives an independent WebAssembly instance, so its runtime state, host/boot
identity, active play, presentation identities, fixed-size ABI buffers, and receipt count are not
shared with the other page host. The runtime emits one effect through a 4,096-byte output frame and
accepts one completion through a separate 4,096-byte input frame. A completion advances execution
only when its plan, placement, active-play, presentation, value-kind, and encoded-value fields are
the exact bytes expected for the outstanding effect.

Run the proof with:

```sh
rustup target add wasm32-unknown-unknown
just check-browser-s4
```

The Chromium test has one pinned project, one worker, no retries, and no forced interaction. It
runs two independent page hosts concurrently, waits on all fifteen 250 ms intervals per host,
retains sixteen nine-byte signal receipts per host, and verifies duplicate, malformed, item-bound,
byte-bound, and mismatched-runtime-identity rejection.

The same gate also starts the native `conduit-browser-link-host`. Its std runtime and the browser
WASM runtime independently derive the same boot-scoped plan for the unchanged form. The std source
sends sixteen deterministic `conduit-wire` envelopes as binary RFC 6455 messages over a live
loopback socket. The planned link admits one in-flight item and 64 buffered bytes, the WebSocket
implementation caps every frame/message at 512 bytes, and the semantic payload cap is nine bytes.
The source retains each item until separate accepted and DOM-delivered acknowledgements arrive.
