# Borrowed canonical assignment records — local Core proof

Issue [#6306](https://github.com/dancxjo/conduit/issues/6306) adds a bounded
representation input to the existing assigned-plan validator. A constrained MCU
can reuse canonical computation payloads already in firmware while retaining the
standard assignment header, canonical framing/digest and logical byte ceiling.

Both contiguous and borrowed inputs use one validator. Borrowed input has an
exact header, at most 13 × 255 records, u16 payload lengths and checked total
length bounded by the supplied existing maximum. Incremental SHA-256 hashes the
same tag/length/payload sequence. Existing schema, Host/Boot, inventory and
computation checks remain in force. The helper owns no payload and allocates no
storage. It grants no Source provenance, native authority, Body or Play.

The scoped branch starts from dev c8019a3d9. Seven Core controls and five legacy
assignment goldens pass. The borrowed/contiguous comparison mutates every header
and fixture payload byte across all three schemas, including resealed payloads.
Hash comparison covers every split at padding/block boundaries through 1,024
bytes. Capacity refusals and allocation probes remain explicit. Strict Core
all-target Clippy and the Cortex-M0 no_std library build pass. Logs, source hashes
and proof scope are retained in [the receipt](evidence/6306-borrowed-assignment/receipt.json).

This is local Core conformance. Runtime table construction, compact transport,
independent firmware provisioning, native possession and device lifecycle work
remain separate admission slices of the environmental appliance. Candidate and
accepted Integration are not established by these local logs.
