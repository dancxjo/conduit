# ConduitOS freestanding host

This crate owns the first ConduitOS machine and production-kernel boundary
funded by issue #588. It builds one `no_std`, `no_main` x86_64 executable,
boots it through the pinned Limine protocol, normalizes bootloader observations
into bounded ConduitOS data, and gives the sole cooperative execution lane to
the production `conduit-kernel` scheduler. One real PIT interrupt wakes one
exact kernel interest and the resulting value crosses one exact Cord to one
bounded COM1 presentation.

The Limine request and response types are confined to `src/boot/limine.rs`.
Code outside that adapter consumes the boot-neutral types in
`src/boot/observation.rs`.

Run the complete proof with:

```console
cargo xtask conduitos prove --arch x86-64 --locked
```

Inspect the digest-verified architecture matrix and current earned proof rungs
with:

```console
cargo xtask conduitos architecture-matrix --locked
```

Prove the first bounded x86_64 xHCI controller Base separately:

```text
cargo xtask conduitos xhci-proof
```

That command pins one QEMU `qemu-xhci` PCI function, performs real MMIO
halt/reset/start and command/event-ring work, retains exact boot-scoped Base
identity and finite storage/work limits, and separately proves that an absent
controller refuses. It remains freestanding-emulator proof and does not by
itself infer a device or semantic capability.

Prove one bounded root-attached USB device separately:

```text
cargo xtask conduitos usb-proof --locked
```

That command attaches one deterministic QEMU `usb-kbd` below the admitted xHCI
Base and performs real root-port reset, slot/address commands, bounded EP0
control transfers, device/configuration descriptor reads, finite parsing, and
`SET_CONFIGURATION`. The retained report carries exact boot-local
device/interface/endpoint identities and limits. A second real boot with the
controller present but no device must refuse, and deterministic malformed,
oversized, topology, completion, disappearance, and stale-identity vectors must
also pass. Enumeration retains structural HID-class facts for later matching;
it does not parse HID or advertise `input/keyboard`.

Prove one bounded HID boot-keyboard transition stream separately:

```text
cargo xtask conduitos hid-proof --locked
```

That command matches only the enumerated HID boot-keyboard interface and its
single eight-byte interrupt-IN endpoint, selects Boot Protocol, configures the
endpoint through xHCI, and admits exactly two report buffers and transfer TRBs.
The harness waits for the armed guest transfer, then injects acknowledged QMP
key-down and key-up actions through QEMU's input path. ConduitOS must retain
usage `0x04` as one press and one release with exact controller, device,
interface, and endpoint correlation. Deterministic malformed, rollover,
duplicate, pressure, loss, and completion-identity vectors also pass. This
layer performs no report-descriptor interpretation, layout or Unicode
translation, and still does not advertise `input/keyboard`.

That report derives the five supported architecture names from the exact
`BOOT*.EFI` artifacts in the pinned Limine archive and refuses if they disagree
with the architecture-valued command contract. It does not make an unavailable
backend executable.

The command mechanically checks the executable, assembles the same hybrid
BIOS/UEFI ISO twice, requires identical digests, and validates two real QEMU
boots with fresh `HostId` and `BootId` values. Each boot must emit exactly one
bounded boot Sign, one correlated kernel Sign, and one ordinary bounded
Observatory v2 snapshot. The snapshot carries the exact Host offer, seven
machine Bases, resources, Plan, placements, capacity-one Cord, terminal Play,
current and historical Signs, retention accounting, and sealed Limine boot
provenance. The proof feeds the first snapshot through the headless native
Patchbay linear consumer and requires the same exact identities and
distinctions. It writes the evidence record and consumable snapshot to
`target/conduitos/x86_64/kernel-proof.json` and
`target/conduitos/x86_64/observatory-snapshot.json`.

The proof requires `curl`, `make`, `tar`, `xorriso`, and
`qemu-system-x86_64`. Missing tools, unsupported architecture backends,
malformed or absent boot/kernel responses, exceeded bounds, unavailable Bases,
QEMU timeouts, and stale identities are explicit proof refusals.

The production topology is the ordinary authored `time/tick` to
`presentation/tick` Form in `src/ordinary_plan.rs`. Each boot checks that
source, plans against the exact current Host/Boot offer, lowers the sealed
fragment into numeric kernel tables, and binds a distinct active Play. A
boot-scoped 256 KiB arena admits all semantic preparation before Play; the
arena is sealed at Play start and the proof requires its usage to remain
unchanged through terminal completion. The old hand-lowered P2/P3 profile is
compiled only as a regression-test fixture.

The snapshot is prepared inside the admitted arena before Play, bounded to 64
KiB, and emitted only after the expected terminal kernel result is verified.
Limine and firmware facts appear only under `BOOT PROVENANCE [SEALED]`; they
are not live offers, Bases, services, or authority. Patchbay remains read-only
and receives no QEMU-memory or ConduitOS-private inspection path.

This slice adds no preemption, SMP, framebuffer implementation, network,
Patchbay control, second runtime, or additional executable architecture
backend. Each broader ConduitOS profile remains explicitly unavailable until a
separate finite issue earns one architecture and one proof rung.
