# Actual browser kernel host checkpoint

This S4 checkpoint is intentionally browser-local, not a restoration of the
archived browser subsystem. Each independent Rust/WASM instance parses and
plans unchanged `proof/fixtures/forms/signal-demo.conduit`, lowers its exact local fragment
through the shared lowering contract, and installs the numeric result into the
same port-aware `conduit-kernel` scheduler model used by production `StdHost`.
The production browser crate has no alternate executor.

The JavaScript adapter accepts only a `presentation/signal` request with the
exact source, checked, expanded, plan, fragment, host, boot, active-play,
node/request/operation, presentation, Sign, and placement identities
issued by Rust. It
decodes the shared nine-byte `value/signal` representation, appends one
machine-readable `output` element, and returns a completion echoing those exact
identities. Duplicate presentation identities, malformed values, and exhausted
item or byte capacity fail before another receipt is retained.

The Chromium proof creates two independent WebAssembly host instances in one
page and retains sixteen ordered receipts in each. Real browser timers and DOM
effects are the only JavaScript-owned platform work. Numeric routes,
operations, values, Sign, identity storage, bounds, cancellation, and
terminal truth remain in Rust. All storage and operation capacity is sealed
before Play start and checked for stable capacity afterward; this does not
overstate JavaScript/WASM allocation instrumentation as an allocation-free
measurement.

The proof also rejects duplicate and malformed completion frames, wrong exact
identity, cancellation, exhausted Sign storage, and platform failure. It
does not provide a live WebSocket or prove another browser engine. There is
one bounded Chromium project with one worker, zero retries, no forced
interaction, and no physical claim.
