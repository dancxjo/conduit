# Service-slot clearing: paired native protection evidence

This packet records two successful first invocations of the supported
`cargo +stable xtask make conduitos ordinary-domain-proof --locked` command.
It proves the checked architecture-fixture protection controls on each exact
source. It makes no timing, speedup, normal-product journey, physical-hardware,
DMA/driver-isolation, stable-release, or full #6183 completion claim.

The baseline is clean `d2fc6bb5fbc3682cec3d3fc31e2a6b5357c99564`.
The optimized source is clean `7fbb4d733525745c3884db999f94d85898441cd6`,
its direct child. The only source differences are:

- `targets/conduitos/src/arch/x86_64/service_memory.rs`: omit redundant installation clearing.
- `targets/conduitos/src/arch/x86_64/service_memory/storage.rs`: a successful claim begins clear, relying on static zero initialization or prior clearing before Release; mutable views still invalidate cleanliness and Drop still clears before releasing the slot.
- `targets/conduitos/tests/service_slot_lifecycle.rs`: existing 88-line focused lifecycle regression test.

Those three blobs exactly match prepared source
`46e3f0bd9130680ab9fdbf8467602f3f15c2e2bb`. Native fixes, resource grants,
entry budgets and map protections were preserved. No full-store oracle or
historical timing instrumentation was transplanted. `pair-preflight.json` and
`paired-seal.json` record the exact blob/source correspondence.

Both commands ran once, serially, with jobs=2, incremental compilation off,
and dev/test debug information off. Each arm used a separate absolute native
artifact root. Shared-cache producer fingerprint metadata was archived before
each invocation to force source-correct producer compilation; prior producer
executables were retained. Both commands returned zero and both measured
worktrees remained clean. `run.json` records command, source, environment and
first-command log hashes. There were no retries.

Each arm produced four receipts bound to its build identity, actual Root ELF
hash and raw transcript hash, with QEMU q35/max, 256 MiB and one vCPU. Build and
image records retain actual ELF/ISO identities. The checks include CPL3
ordinary computation, all 26 service-memory hostile entries, restart samples
remaining zero, Root/sibling preservation, timer coexistence and actual x87
#NM vector 7/origin 1 followed by terminal refusal and Root/sibling survival.

Service-memory and compositor copy-recovery observations are exactly equal
between arms. Compositor guest and hostile-broker observations differ only in
the source-specific guest `image_sha256` fields; those distinct identities are
retained in full. Excluding only those identity fields, all four observation
objects are equal. The hostile, gate, timer and raw #NM control markers also
match. This is bounded protection behavior correspondence, not binary identity
or a performance comparison. `verified.json` contains each observation set;
`paired-seal.json` records the comparison and limitations. Both independent Root
receipt-binding reviews and the lifecycle source review are included.

The `.gz` files contain lossless first-command output and raw serial bytes.
`compressed-logs.json` records compressed and uncompressed hashes; decompress
before comparing with receipt transcript hashes. `hash-manifest.json` binds
all packet files except itself. The original seal also binds retained native
artifacts outside Git; those entries are not claims that binaries reside in
this documentary packet.

Actual ELF, ISO, producer executables, raw logs and original receipts remain
preserved outside Git under the source packet's absolute paths recorded in
`run.json` and `paired-seal.json`. They were not copied into Git. This evidence
commit is a documentary descendant of the measured candidate; it was not the
measured source. Normal journey comparison awaits its separately accepted
Home collector common base. Full #6183 remains open.
