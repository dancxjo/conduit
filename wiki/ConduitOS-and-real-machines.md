ConduitOS is Conduit's freestanding host environment. It runs the same architectural model without assuming Linux, Windows, a browser, or another general-purpose OS underneath it.

## Current product targets

The development tree has product paths for:

| target | current experience |
|---|---|
| x86_64 PC | graphical QEMU product |
| IA-32 PC | serial product |
| AArch64 virt | serial product |
| RISC-V64 virt | serial product |
| LoongArch64 virt | serial product |

Board make also exists for families such as RP2040, AVR, ESP32, Raspberry Pi, and Orange Pi, with different proof levels.

## Build and boot x86_64

```bash
cargo xtask make conduitos live x86_64 --locked
cargo xtask make conduitos live-boot x86_64 --locked
```

The first creates the live ISO. The second boots it in QEMU.

The graphical path includes Crèche/body birth, resident forms, Tour, Patchbay, pointer/keyboard paths, and USB-line exercises.

## Profile is not boot truth

Conduit keeps this ladder explicit:

```text
host source / profile
 -> build
 -> image
 -> boot
 -> current host offers/resources
 -> plan
 -> play
```

A compiled driver does not prove a device initialized. An image is not a body. A boot is not a plan. A profile does not grant authority by listing a capability.

## Forms all the way down

Above a small irreducible machine membrane, the architectural preference is ordinary semantic forms/backs instead of a monolithic driver runtime.

A native path may descend roughly like:

```text
boot observations
 -> ACPI / PCI discovery
 -> admitted machine resources
 -> controller/protocol work
 -> portable audio/presentation semantics
 -> mask
 -> show
```

At the bottom, native Rust earns its place for irreducible operations such as:

- exact MMIO/PIO;
- interrupt binding;
- DMA/IOMMU mapping;
- barriers/cache maintenance;
- architecture state.

Those mechanisms remain bounded and capability-bound.

## Source order is not hardware order

If reset must precede configure, the dependency should be semantic:

```conduit
request >> reset
reset.ready >> configure
configure.ready >> enable
```

Declaration order is not a hidden imperative initialization script.

## Headless is first-class

A headless host is not a broken graphical host.

If a profile provides no graphical realization, planning should refuse graphical placement truthfully or use another admitted host/mask route.

That matters for screen-free speech and distributed bodies.

## Emulator proof is not physical hardware proof

The x86_64 visual journey is strong **freestanding-emulator** evidence.

It does not by itself establish:

- that an old physical laptop boots;
- that its storage controller works;
- that its real network/audio/input hardware works;
- that a human can use the full experience unattended.

The physical laptop campaign keeps those stages separate.

## Pico and physical lines

Physical RP2040/Pico work has its own explicit path:

```bash
cargo xtask make pico build --usb-remote
cargo xtask make pico flash --usb-remote
cargo xtask prove std-pico-usb --interactive
```

Flashing changes hardware. The interactive proof checks a running boot and exact session evidence. Firmware compilation alone is not that proof.

## Pocket Theremin is a useful hardware boundary example

The portable form talks about Distance, Frequency, and bounded PCM.

```conduit
distance >> map.distance
map.frequency >> frequency
frequency >> tone.frequency
tone.audio >> audio
```

It contains no HDA, PCI, DMA, WebAudio, ALSA, or framebuffer identity.

Those belong to backs, bases, host calls, and resources.

The severe test of portability is not "can it compile twice?" It is "can materially different realizations preserve one semantic form while retaining honest mechanism evidence?"
