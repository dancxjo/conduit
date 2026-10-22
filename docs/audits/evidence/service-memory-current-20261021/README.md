# Current x86_64 service-memory backend evidence (#6181)

The supported `cargo +stable xtask make conduitos ordinary-domain-proof --locked`
completed successfully from clean source
`08a8352297ec35a18138b5e02e958cce5331d395`. This packet preserves that source
identity independently of the later evidence commit. It is an architecture proof
appliance, not a normal graphical product build or accepted release.

The exact Root ELF SHA-256 is
`53da1a141c18c1d6dc9789aea94dd1fafba22358865fb1bbe64d51fd83764c98`;
the ISO SHA-256 is
`66e5ea74e21cd4bee96c24af36d7b6540c0fe6f540fbe8c10f1a26e30fe5778d`.
The actual profile is QEMU x86_64/q35, CPU max, one vCPU and **256 MiB** RAM.
Historical 128 MiB receipts describe their own earlier images; they do not
establish that the current, larger linked product fits that profile.

## Retained custody

[manifest.json](manifest.json) records SHA-256 and byte counts for every packaged
file and its original uncompressed bytes. Gzip files decompress byte-for-byte to
the preserved originals; no logs are normalized or redacted. Paths printed in
logs identify the actual measured checkout and local compiled-artifact cache.
ELF and ISO bytes remain in the local owning evidence directory and are excluded
from Git. Their hashes and the original retention record are retained.
Original retention records may list additional local historical artifacts;
`manifest.json` alone defines the files included in this documentary packet.

- [Current service-memory receipt](current/service-memory-fixture-proof.json)
  binds the exact build, QEMU profile, raw serial digest and completed 26-case
  backend fixture.
- [Guest compositor receipt](current/compositor-guest-fixture-proof.json),
  [hostile broker receipt](current/compositor-hostile-broker-fixture-proof.json)
  and [copy recovery receipt](current/compositor-copy-recovery-fixture-proof.json)
  retain their own explicit fixture and execution boundaries. The hostile broker
  receipt includes exact guest-image hashes and preserved Root/sibling sentinels.
- [Current raw serial](current/ordinary-domain-proof.log.gz) and
  [build/proof output](current/build-proof.log.gz) retain successes and terminal
  completion together. Build/image records and dependency/link inventory/map are
  beside them; link observation is not an isolation proof by itself.
- [Text admission diagnostic](negative-text-0f221/ordinary-domain-proof.log.gz)
  preserves source `0f2218d1a32dd0577b71b577536281c842b78a88`: the selected actual
  Face binding allows 256 completion bytes, while the migrated broker rejected it
  under an obsolete zero-output assumption. The original raw diagnostic and
  kernel refusal are retained.
- [Morse failure](negative-morse-f306/ordinary-domain-proof.log.gz) preserves
  source `f30633a9d`: ordinary HELLO and hostile gate checks progressed, then
  `morse-product-prepare` refused. It is not a completed backend proof. The same
  stale text-Face completion assumption remained in Morse admission; the custom
  indicator contract correctly continues to allow zero completion bytes.

The canonical completion correction matches the existing selected Source/Plan
contract; it changes no effect grant, service memory ceiling, timeout or semantic
scheduler. Shared Root validators are exercised with actual prepared/lowered
ordinary and Tour Plans, rather than a separately fabricated binding fixture.

## Scoped acceptance audit

CPU image/layout contracts in `targets/conduitos/src/service_memory/tests.rs`
cover unchanged ordinary 128 KiB image/16 KiB retained bounds; the separate
compositor service image and 8 MiB retained plus 8 MiB scratch arenas; malformed,
wrong-target, overlapping, oversized and writable/executable image rejection;
and guard, overflow and alignment checks. This packet retains **19 CPU tests** passing at the exact same clean tested
source: 12 ordinary text controls, two Morse contract controls and five focused
service-memory image/layout controls, with zero failures or ignored cases.
The [service-memory test log](cpu/6181-service-memory-tests.log.gz) was appended
after its owning process completed; the tested source remains `08a835229`.

The focused strict Clippy command also passed with exit 0 on the same clean
`08a835229` source:

```sh
cargo +stable clippy --locked -p conduitos --all-targets --features hosted-tools,native-compositor -- -D warnings
```

The [raw Clippy log](cpu/6181-focused-clippy.log.gz) and its original/compressed
hashes are retained. This local lint result is separate from Candidate or
combined Integration acceptance.

The actual backend fixture in
`targets/conduitos/src/ordinary_domain_proof/service_memory.rs` checks CPL3,
exclusive slot ownership, charged memory, stopped-guest transfer staging and
retained private writes. Its 26 hostile entries exercise Root/sibling/MMIO reads
and writes, image write protection, NX private ranges, exterior/interior guards,
port I/O denial, two bounded loops and invalid gates. Root and sibling sentinels
survive; the original sibling subsequently executes correctly.

Faulted instances refuse further entry, exchange, output and transfer access.
Restart obtains a fresh monotonically increasing boot-local instance. Output
bytes are fully inspected for zero; metadata/pixel/scratch boundaries are sampled
beside the backend's charged whole-slot clearing. This is not a claim that every
private byte was independently sampled after restart.

The memory backend grants no scanout or device capability. Higher compositor
paths separately check Host/Boot/offer/display grants and service-instance/session
namespaces before scanout. A boot-local instance number alone is not a capability.
Copied Root mappings remain supervisor-only; guest code receives the closed
private mapping profile, not device or privileged pointers.

## Remaining gates and limits

The local backend result does not close #6181 before reviewed repair delivery and
required Candidate/combined Integration/publication gates. The fixture does not
close #6177 or #6175: full production retained-compositor integration, hostile
production interaction, service recovery and unchanged product journey retain
separate acceptance obligations. No physical, DMA, driver or other-architecture
isolation is established. A separate normal graphical journey on graph-repair
source `9cfc7ff59` is not relabelled as execution of this source or backend packet.
