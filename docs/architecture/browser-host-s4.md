# Actual browser DOM host checkpoint

The first S4 browser checkpoint is intentionally one platform boundary, not a
restoration of the archived browser subsystem. `hosts/browser/signal-dom-host.mjs`
runs in an actual browser document and owns one DOM receipt root for one exact
host/boot identity.

It accepts only a `presentation/signal` effect with the exact plan, active-play,
presentation, and placement identities issued by the runtime boundary. It
decodes the shared nine-byte `value/signal` representation, appends one
machine-readable `output` element, and returns a completion echoing those exact
identities. Duplicate presentation identities, malformed values, and exhausted
item or byte capacity fail before another receipt is retained.

The Chromium proof creates two independent host instances in one page and
retains sixteen ordered receipts in each. This is actual DOM execution. It does
not yet run the Rust planner/runtime in the browser, provide a live WebSocket,
or prove another browser engine. There is one bounded test with no engine
matrix, retries, forced interaction, or physical claim.
