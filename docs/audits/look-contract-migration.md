# Finite canonical Look and Phosphor migration

Issue #5633 owns this contract; #5627 tracks the broader migration. Parent
#5368 and authored syntax #5377 remain open.

The old ApplicationTheme reader, Rust owner, serialized identities and browser
module are retired. Phosphor is now a checked `LookInfo` resource identified as
`conduit.look/phosphor@1`, version 1, with independently optional visual, spoken
and generative facets. Its canonical Native type and value are pinned across
Rust and JavaScript; no parallel serialization was introduced. The retained
legacy wire is a negative fixture verified against the old encoder before its
removal. Existing Native bounds, exact field identities and final Source
constraints apply; encoded Look admission is bounded to 65,536 bytes.

Visual data contains 15 bounded RGB roles, ten finite metrics and the existing
VisualExpression. Structural breakpoints and grid placement stay with the Mask.
Spoken delivery/verbosity and generative register/verbosity are closed data
parameters. They carry no provider, prompt, Face selector, action or topology.
Neither new provider consumption nor authored Look declaration syntax is claimed.

Facet availability distinguishes unsupported requests, absent facets, observed
Host unavailability and selected data. Accessibility and hard Host bounds retain
precedence. Visual admission keeps the original resource immutable and records
requested versus selected Phosphor fallback. Impossible constraints refuse.
Browser no-Look manifestation clears the previous Look's owned paint and metadata
without changing Face meaning or unrelated renderer styles.

Owned consumers include Patchbay native CSS, actual browser WASM nucleus exports,
application manifestation, current HTML selectors, static bundles and workspace
staging. Old exports/imports are removed together. Historical receipts and the
negative wire fixture retain their original identities.

## Proof boundaries

The full native Face suite passed 278 tests across 55 targets, with one existing
ignored test requiring an explicitly retained real Todo Face. Final strict
all-target Face Clippy passed after correcting fixed-size fixture byte helpers.
Five actual Chromium/WASM/DOM tests passed during development, including keyboard
revision behavior, semantic links, product contrast/reflow and no-Look clearing.
Canonical wire refusal tests passed four Node cases. Existing packaging/CSP/graph
checks and native Patchbay consumer tests also passed during development.
A fresh release WASM build passed, followed by all five Chromium tests.
Strict browser Host/runtime/Patchbay consumer Clippy also passed. Exact source,
artifact and log hashes are retained in
[evidence/look-contract-validation.json](evidence/look-contract-validation.json).
The current-dev refresh adds accepted parser guards; it does not relabel these
measured Look captures. Native tests, browser execution and physical/human proof remain distinct.

This does not close #5368, #5377 or #5627, admit Todo #5659's missing prerequisites,
or establish spoken/generative output. Source grammar, real application/body
integration and broader component/Mask proof remain owned by their open issues.
