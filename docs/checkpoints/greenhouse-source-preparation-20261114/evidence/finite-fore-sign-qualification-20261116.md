# Finite duplex Fore evidence window

Implemented reusable checked Sign funding for the first authorized duplex probe. The Source compiler now preserves the original Host construction's physical Sign item/byte ceilings independently of optional schema storage. The sampling factory derives an opaque finite budget for the installed one-input/one-result service: four remote lifecycle Signs per accepted input, plus two terminal closure Signs, while preserving every original local slot.

For this Source, 55 original local slots and the unchanged 64-item/24,576-byte ceilings select **one input, six remote sidecars and 61 total event slots**. The actual HostedSignLog constructor, physical backing metric and explicit observer ingress window use that same budget. The local/no-Line constructor remains at 55 items with zero remote sidecars. No Host profile, physical memory grant, board code or Root was changed.

When the window is spent, the observer pressures the same unaccepted input before touching kernel values, queues or Signs. ACK processing for accepted results continues. The default observer constructor remains unchanged. The budget has no public field constructor and is not received authority.

Qualification:

- 146 runtime lib tests passed, one preexisting ignored.
- A real HostedSignLog/typed bridge test completes two independently funded request/result/ACK rounds, then pressures the next request without mutation, verifies preserved mandatory sidecars, accepts both normal terminal closures and retires normally without cancellation or rearm. It allocates nothing during execution.
- The exact Source budget (55/64 -> one request, six sidecars, 61 events) runs the same success, pressure, closure, evidence-preservation and normal-retirement proof, also without execution allocation.
- 52 Source-build tests passed, including checked ceiling preservation in canonical-only emission.
- Strict runtime lib/tests and Source lib/tests Clippy passed; no-default execution lib check passed.

This is bounded first-probe funding, **not full history scanning**. The native service's 8,192-operation limit is an upper limit, not evidence custody for 8,192 requests. The receiver must explicitly select its one-probe stage resource budget.

Existing InvocationArchive retains exact completed/drained invocation traces; it does not provide live prefix transfer/removal from this continuing sampling kernel. Kernel retirement also requires terminal/drained ownership. Resetting sampling per read or evicting mandatory remote Signs would weaken the contract. A lossless live-stream Sign custody handoff remains the full-scan gate at the existing #6341 workspace-custody / #6409 lifetime intersection.

No target rebuild, flash, Root mutation, live traffic or complete factory peak claim is included in this qualification. The newly generated Source templates still need target compilation before physical probing. The earlier zero-sidecar candidate must not be flashed for history recovery.

Follow-up qualification: added the exact const logical-capacity calculator beside the kernel's physical calculator; its two focused tests and strict kernel/no_std checks passed. Runtime boundary tests and final strict lint passed. Fresh final Blue and Yellow ordinary Makes both passed from clean `da2f2a0b2e`; see `finite-fore-probe-targets-qualified-20261116.md`. No device operation occurred.
