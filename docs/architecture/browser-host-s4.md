# Actual browser host checkpoints

The S4 browser work is intentionally a narrow platform boundary, not a
restoration of the archived browser subsystem. `hosts/browser/signal-dom-host.mjs`
runs in an actual browser document and owns one DOM receipt root for one exact
host/boot identity.

It accepts only a `presentation/signal` effect with the exact plan, active-play,
presentation, and placement identities issued by the runtime boundary. It
decodes the shared nine-byte `value/signal` representation, appends one
machine-readable `output` element, and returns a completion echoing those exact
identities. Duplicate presentation identities, malformed values, and exhausted
item or byte capacity fail before another receipt is retained.

The Chromium proof creates two independent Rust/WASM planner/runtime instances
in one page and retains sixteen ordered receipts in each. A second proof runs
the unchanged form with `flow/pulse` on a native std runtime and
`presentation/show` on browser WASM. A live RFC 6455 binary link carries exact
`conduit-wire` envelopes through a one-item/64-byte planned connection. The
source retains each sequence until separate accepted and DOM-delivered
acknowledgements arrive, then both plan fragments terminate.

This is actual browser execution and a live loopback WebSocket. It does not
prove another browser engine, a non-loopback deployment, authentication, Pico
firmware, or physical behavior. There is one pinned Chromium project with no
engine matrix, retries, forced interaction, or physical claim.
