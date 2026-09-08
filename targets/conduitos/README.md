# ConduitOS freestanding host

This crate owns the ConduitOS machine and production-kernel boundaries. Its
canonical x86_64 PC product is a `no_std`, `no_main` graphical live system;
IA-32 PC and AArch64, RISC-V64, and LoongArch64 virt Hosts provide distinct
long-lived serial product images. Each normal product image preserves its
checked PROFILE, boot protocol, bounded Host truth, and sole cooperative
`conduit-kernel` execution lane. Architecture proof appliances remain separate
from these product artifacts.

## Canonical live media

Build and boot the common graphical PC Host with:

```console
just conduitos-live
just conduitos-boot
```

These delegate to `cargo xtask conduitos live x86_64 --locked` and
`cargo xtask conduitos live-boot x86_64 --locked`. Both commands name the same
canonical artifact:
`target/conduitos/live/x86_64-pc/conduitos-x86_64.iso`. The compatibility
`cargo xtask conduitos demo --arch x86-64` entrance now builds and boots this
same product ISO; it no longer fabricates an architecture-proof image.

Current live product media are:

| Host type | Artifact | Current experience |
| --- | --- | --- |
| `conduitos/x86_64/pc` | `live/x86_64-pc/conduitos-x86_64.iso` | graphical front door, compositor, keyboard and pointer |
| `conduitos/ia32/pc` | `live/ia32-pc/conduitos-ia32.iso` | long-lived serial product |
| `conduitos/aarch64/virt` | `live/aarch64-virt/conduitos-aarch64.iso` | long-lived serial product |
| `conduitos/riscv64/virt` | `live/riscv64-virt/conduitos-riscv64.iso` | long-lived serial product |
| `conduitos/loongarch64/virt` | `live/loongarch64-virt/conduitos-loongarch64.iso` | long-lived serial product |

All paths above are beneath `target/conduitos/`. Run
`cargo xtask conduitos live-matrix` for machine-readable format, emulator, and
capability-gap truth. ARMv6 Raspberry Pi media and Orange Pi 5 media are not in
this table: their current contracts prove architecture appliances or
deterministic image artifacts, not a normal live ConduitOS product Host.

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

Prove the exact portable keyboard realization separately:

```text
cargo xtask conduitos keyboard-proof --locked
```

That command requires the real bounded HID device path before ConduitOS offers
`input/keyboard`, then checks an ordinary Plan, one production-kernel Play, and
the exact portable values `[4, 0, 0]` (press) and `[4, 1, 0]` (release). The
retained report correlates the boot-local controller, device, interface, and
endpoint identities with the advertised implementation, finite resource
reservations, Plan and active Play. The ordinary Observatory snapshot must
contain the same exact capability for native Patchbay projection. A real boot
without the USB device must refuse without emitting a keyboard offer, and the
focused suite covers stale identity, incompatible or ambiguous devices,
capacity, pressure, transfer failure, device loss, closure, and invalid-value
cases as distinct outcomes. This slice does not add keymaps, text or Unicode
translation, hotplug, multiple-device policy, browser proof, or physical/HIL
proof.

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
