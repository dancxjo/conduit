# Finite print request and lifecycle contract

For #5556, `semantics/process/print.conduit` owns the bounded document reference,
media/color/sides/copies, selected-provider support and lifecycle data. Generated
Native bindings retain one canonical wire; typed support is an ordinary Back
property. Request preparation checks the exact selected support and document
identity/version/profile/access/authority/extents using existing resource access.

`print/submit` is an ordinary effectful, non-replayable Kind with bounded ports.
The three-request Plot checks through the public `conduit check` entrance. An
ordinary sealed Plan needs exact Host/Boot/capability authority. Mono duplex,
color and unsupported-media cases admit or refuse against supplied Back facts.
One finite attempt records queued/dispatched/ACK/completed/failed/cancelled/
uncertain evidence. Unknown ACK, loss and cancellation after dispatch remain
uncertain; known provider job identity survives for later reconciliation.
No helper sends jobs, selects providers, retries or holds document bytes.

Production implementation source is `2ca604151fa5c664b53b70d1f5d43e492cc55f7b`; public integration test and
documentation source is `8b3fe4d719b81fd5cd92bc2701603260e0abab1f`. All 12 process tests and 256
catalog tests pass, plus the public product integration test. Strict semantic
all-target and conduit/xtask binary Clippy pass. The product test verifies empty
public checker diagnostics, absence of a print Back from the real default Host,
and ordinary planning's named `UnknownCapability` refusal. A separate run probe
hit unbound root ports and is not counted as missing-provider evidence.
Strict Clippy for the public product integration test also passed. Exact bounds
and log hashes are retained in
[evidence/print-contract.json](evidence/print-contract.json).

Rust and Plot formatting pass. The standalone print type file and existing
`semantics/process/types.conduit` both reproduce formatter diagnostic
`CND-FRM-019` at EOF. Native Source checking/generation and round trips pass.
A named-payload experiment did not repair that baseline formatter problem and
was reverted; the original wire schema is retained.

This is checked Source/Plan and typed protocol-simulator evidence. It is not a
physical print, installed print provider, durable spooler or duplicate/retry
reconciliation proof. Generic pool execution/fairness, durable uncertain-delivery
reconciliation and opt-in physical proof remain #5557–#5559. Parent #5229 stays open.
