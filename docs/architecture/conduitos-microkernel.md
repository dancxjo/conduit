# ConduitOS privilege boundary

Architecture owner: [#6175](https://github.com/dancxjo/conduit/issues/6175).
Inventory prerequisite: [#6176](https://github.com/dancxjo/conduit/issues/6176).
First isolation milestone: [#6177](https://github.com/dancxjo/conduit/issues/6177).

The target is a hardware-enforced microkernel: privileged Root owns machine
bootstrap, address-space construction, interrupt delivery, bounded execution
and preemption, capability enforcement and IPC. Services own product policy,
Crèche, Face/Mask, retained composition, the Conduit execution engine, network
protocols and eventually drivers. Each service must have its own admitted
memory and authority, independent failure and a bounded restart lifecycle.

This evolves the initial single-address-space profile in #237. The portable
`conduit-kernel` is the execution engine, not the privileged OS nucleus. Its
scheduling and Plan/Play semantics remain authoritative; moving it to a
protected service must not introduce a second semantic scheduler.

## Current graphical path

This inventory describes development source, not completed isolation. The
initial source/dependency audit used development base
`6a8e127b48c66b04ec0801cc4778c8762f36de72`. Cargo resolves the x86_64
`native-compositor` request to `default,native-compositor,native-owner-network`.
That graphical startup reaches these responsibilities:

| Component | Current execution and reachable authority | Intended boundary |
|---|---|---|
| `main.rs`, boot, allocation, architecture setup | Root; physical memory, page tables, interrupts, machine entry | Narrow privileged mechanisms |
| `graphical_startup.rs` | Root; xHCI initialization/enumeration, framebuffer, entropy, offers and product admission | Boot facts and explicit service grants; product preparation outside Root |
| `product_front_door.rs::run` | Root; mutable `XhciReady`, HID sessions, `PixelTarget`, product bases and surface provider | Protected product service with capability-mediated requests |
| `native_face_mask`, front-door presenter | Root; Face projection, rasterization, compositor and display acknowledgement | Protected presentation services |
| `native_compositor` | Root; retained surface pixels, damage, focus/cursor and direct `PixelTarget` writes | Protected retained compositor; Root initially retains scanout |
| Ordinary runtime, planner and Body controllers | Root except explicitly protected regions | Protected runtime/service execution with existing semantic ownership |
| USB, PCI, network, audio and display providers | Root; ports, MMIO, device buffers and physical addresses | Exact mediated grants; driver isolation needs DMA containment |
| `domain/main.rs` | Separate immutable image; explicit command dispatch over a bounded frame, private stack/retained pages | Reuse hardware confinement, extend independently admitted services |

The separate image currently handles text/keymap/editor, Morse and timer
commands; it does not host retained graphical composition. x86_64 has two
fixed domain slots and a 4 KiB exchange frame. A service architecture must
account for simultaneous retained services and finite pixel memory; increasing
slot counts or calling a Root module a service does not establish confinement.
The text image refuses allocation, whereas the retained compositor uses bounded
`Vec`/`String` preparation. A compositor image needs its own admitted arena and
page mappings; weakening the text profile is not a substitute.

## Build evidence

Product/profile builds through the common builder (including
`cargo xtask make conduitos live x86_64 --locked`) retain
`privilege-inventory-<elf-sha256>-<artifact-role>.json` beside `build.json`. The sidecar binds the ELF digest,
source base commit, artifact role, target, requested features, Cargo's resolved
normal/build dependencies and their enabled features, and observed dirty status.
Dry runs do not create this evidence. Collectors must select the current
ELF digest and artifact role and compare the embedded build record. Separate
architecture-appliance builders need not emit an inventory; an older product
inventory is not evidence for their replacement artifact.

Normal dependencies conservatively identify potentially included code; build
dependencies run on the build host and are not automatically Root code. Dead-code
elimination, crate membership and feature selection do not establish runtime
reachability. The separate domain workspace has its own dependency graph.
This sidecar is neither an ELF-symbol audit nor an execution receipt; dirty
source is not uniquely identified by the base commit. A complete audit must
also retain the exact clean-source build, Root and service ELF load segments
and symbols, and observed production service entries. Existing architecture
proof appliances and non-graphical profiles must retain their distinct roles.

## First isolation milestone

Move retained composition into an independently linked, unprivileged image.
Keep surface lifecycle, z-order, damage, clipping, focus/cursor and frame
composition together; a protected pixel-copy helper alone is insufficient.
Use finite versioned messages containing integers and validated lengths,
never Root pointers, controller references or framebuffer addresses.

Root may retain scanout. Its grant must bind the current Host/Boot, service
instance/generation, display resource/generation, output extent and work bound.
Root validates returned damage and pixel ranges before copying to scanout.
A returned frame is not an acknowledged Show: only successful validated
scanout may establish current surface coverage and authorize typed input.
Faults, partial writes, stale replies, cancellation and replacement invalidate
acknowledgement before any fallible operation. Restart cannot reuse old grants
or turn a stale FaceInteraction into an accepted effect.

Prove the ordinary graphical boot and journey preserve their appearance and
behavior, with actual unprivileged compositor entries. Use the production
service path to inject Root/sibling memory access, MMIO/port access, invalid
messages/ranges, stale generations, endless execution and service crashes.
Require Root and sibling sentinels to survive, bounded preemption, revoked
in-flight work, and recovery with a fresh service identity. Retain exact source,
ELF and emulator identities. Emulator confinement does not establish physical
or DMA isolation; every additional architecture needs its own evidence.
