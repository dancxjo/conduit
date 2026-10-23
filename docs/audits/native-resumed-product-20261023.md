# Native product proof after graph-budget repair

Clean Source `84b86ee725dd8950fce6036470466f9dfdcc4c4b` includes current
merged #6255, #6263 codec repair and #6262 Pages checkpoint contract, plus the
Home-capture synchronization correction. No product bound, work budget,
interaction rule or retry was changed for this run.

Supported `cargo +stable xtask make conduitos live x86_64` and
`cargo +stable xtask make host verify target/conduitos/live/x86_64-pc --journey --locked`
both exit zero. The actual product-host ISO SHA256 is
`86fdcfced09c285334eb3b2a43acc62fa2075df31abac803b5f948393b913f66`,
independently matched to the source-bound build manifest. The guest's resolved
image binding is a separate identity.

[Original packet](evidence/native-resumed-product-20261023/manifest.json)
retains 27 ordinary journey and 20 refusal/Details PNGs, original receipts,
byte-exact serial/QMP logs and build/driver/Root-test output (logs are losslessly
gzipped). All 47 PNG lengths, encoded hashes, dimensions, chunk CRC32 values
and independently decoded RGBA hashes pass. Home, Body-start, Patchbay Face and
diagram captures were visually inspected.

The corrected Home capture waits for a new complete return marker. Its serial
prefix ends at byte 1,011,645 with four complete Home-return events. The actual
pixels show **Commons / Hub** and its four Plot choices. The original 50aa
Keyboard-canvas image and qualification remain unchanged; this fresh image
supplies the previously missing Home visual evidence.

All five encountered protected Graph replies complete serialization, validation
and packing. Patchbay opens normally; no Application entry refusal occurs.
Cross-Plot selection succeeds. Initial Edit correctly refuses an existing draft;
its `successful_edit_handoff_enacted` remains false. On the later fresh Play,
the exact 187-byte inspected Source loads, a suffix is entered, and Backspace
restores it on the same Plan/Play. No Source-range selection, save/reopen,
physical or human enactment is claimed. The harness terminates the guest after
successful verification rather than observing a natural exit.

Six production Root broker CPU cases also pass with the delivered correlated
GraphProgress protocol. They retain decoded substituted-return rejection and
quarantine coverage; this is not hostile-code execution proof.

This earns fresh ordinary native enactment for #6263/#6201 and the Home part
of #5608. Relevant CI and merge remain required. #5608's all-four-visible wide
Crèche and pressure/provider matrix, #6257's authorized unplanned Source
inventory, and #5519's meaningful >12-Cord continuation remain open. It does
not close #5994 or the active five-item goal.
