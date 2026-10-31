# Pico W credential continuity

Green committed its canonical network configuration once, restored it after an
application reflash, then restored it after the operator reported a five-second
USB power disconnection. No JOIN was sent during either recovery check.
`proof.json` retains exact build identities for these phases. The final source
image corrects only the DMA qualification comment; its separate application-only
reflash and automatic restoration are recorded independently of the cold-cycle
proof. A power cut during writing has not been tested.

The feature adds 9,060 bytes of flash and 536 bytes of static RAM to the same
Green Wi-Fi build. Enabled totals are 645,464 flash bytes and 97,768 BSS bytes;
reported heap peak remains 4,524 bytes. One measured commit took 28,718 us.
This measurement does not replace the reviewed 750 ms transaction envelope.
Repeated identical JOIN avoided an erase/revision change. Sensor/display status
remained fault-free; electrical ACKs do not constitute optical acceptance.

The existing shared journal, flash geometry, canonical JOIN wire, and affine
monotonic-wait gate are reused. The two credential sectors precede the preserved
Spore sector. Writes require retained CORE1 custody and the initialized pinned
non-chaining DMA profile; sequential BUSY checks do not establish generic DMA
quiescence. Unknown/corrupt storage refuses writes rather than auto-erasing.

Validation: 29 monotonic-wait tests, strict package Clippy, no_std thumb check,
and Green/White/Yellow firmware release builds passed. Firmware Clippy passes
with only three pre-existing lint classes allowed (`too_many_arguments`,
`chunks_exact_to_as_chunks`, `let_unit_value`); an unmodified strict run fails
on those baseline findings. Enabled/disabled build identities differ and
include feature/dependency inputs. Existing lockfile versions/checksums remain
unchanged.

Only application UF2 pages were written. ROM mass storage exposes no full flash
readback on this setup; the qualified recovery image is retained. This evidence
proves neither MCU Body membership nor observation-history durability. The raw
observer's `flash_write_timing_qualified` remains false: this pilot is not an
admitted Body storage grant.
