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
| `product_front_door` | Root; platform adapters own hardware input, rescue and execution providers; policy receives typed events and checked display effects | Protected product service with capability-mediated requests |
| `native_face_mask`, front-door presenter | Root; Face projection, rasterization, compositor and display acknowledgement | Protected presentation services |
| `native_compositor`, compositor service | x86_64 native guest owns retained pixels, damage, visual focus/cursor and composition; Root validates scanout | Extend evidence and independent recovery; other architectures still need service backends |
| Crèche draft service | x86_64 native guest owns canonical BirthDraft, naming and Birth Face projection; Root checks immutable snapshots/selections | Move remaining controller and presentation policy out of Root |
| Home launcher/command bar | x86_64 Home guest owns canonical HomeModel, keyboard composition and refusals; guest also constructs scene/local Face content; Root validates replies and retains checked lifecycle effects | Relocate remaining general presentation and product control |
| Native Body workset runtime | x86_64 Runtime guest owns the canonical fixed scheduler, values, Cords and Signs; Root checks effect requests and input provenance | Extend to remaining execution paths and establish runtime evidence |
| Native-workset retained applications | Application guest owns canonical Tutorial/Tour/Patchbay models; Root checks action proposals against prior views and live admission facts | Migrate separate lifecycle Tutorial projection and remaining controller paths |
| Ordinary/Tour runtime, planner and Body controllers | Root except explicitly protected regions | Protected runtime/service execution with existing semantic ownership |
| USB, PCI, network, audio and display providers | Root; ports, MMIO, device buffers and physical addresses | Exact mediated grants; driver isolation needs DMA containment |
| `domain/main.rs` | Separate immutable image; explicit command dispatch over a bounded frame, private stack/retained pages | Reuse hardware confinement, extend independently admitted services |

The ordinary separate image handles text/keymap/editor, Morse and timer
commands. x86_64 additionally has independently admitted compositor, Draft, Home, Runtime and Application
service slots, each with its own address space and a 4 KiB exchange frame.
The ordinary two-slot pool remains separate. A service architecture must
account for simultaneous retained services and finite pixel memory; increasing
slot counts or calling a Root module a service does not establish confinement.
The text image refuses allocation. Retained service images use closed private
arenas and page mappings; the Draft profile also admits a bounded private RW/NX
data template. Ordinary text and compositor executable admission remain
immutable. The native configuration passes a targeted local compile; earlier
compositor emulator evidence predates the later Crèche service work. Full
service/runtime/driver isolation is not established by this source inventory.

## Build evidence

Product/profile builds through the common builder (including
`cargo xtask make conduitos live x86_64 --locked`) retain
`privilege-inventory-<elf-sha256>-<artifact-role>.json` beside `build.json`. The sidecar binds the ELF digest,
source base commit, artifact role, target, same-invocation Root link map and
its digest, requested features, Cargo's resolved
normal/build dependencies and their enabled features, and observed dirty status.
Dry runs do not create this evidence. Collectors must select the current
ELF digest and artifact role and compare the embedded build record. Separate
architecture-appliance builders need not emit an inventory; an older product
inventory is not evidence for their replacement artifact.

Normal dependencies conservatively identify potentially included code; build
dependencies run on the build host and are not automatically Root code. Dead-code
elimination, crate membership and feature selection do not establish runtime
reachability. The separate domain workspace has its own dependency graph.
Release ELFs strip symbols. The retained `.map` companion records their
pre-strip linked symbols and addresses without enabling privileged behavior.
The map filename includes the ELF digest and artifact role, and the inventory
binds its content hash. It supports classification of linked responsibilities;
it does not prove which code executes. This sidecar is not an execution receipt; dirty
source is not uniquely identified by the base commit. A complete audit must
also inspect the exact clean-source build, Root and service ELF load segments
and their linked responsibilities, and observe production service entries. Existing architecture
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

## Enforced current graphical inventory (#6176)

The v2 inventory contract audits the concrete `native-compositor` x86_64 product,
not a generic claim about every architecture. Run its deterministic controls:

```sh
cargo xtask check privilege-inventory
```

Build a clean committed normal product, then verify its sealed BUILD and common
builder evidence explicitly (the verifier does not build or boot):

```sh
cargo xtask make host build targets/conduitos/profiles/conduitos-native.host.conduit --output target/privilege-product --locked
cargo xtask make conduitos verify-privilege-inventory --build-dir target/privilege-product --inventory-dir target/conduitos/x86_64 --output target/privilege-receipt.json
```

The second directory is the owning proof/evidence root, independent of Cargo's
compiled-artifact cache. Use the path actually printed by the build. Verification
requires the sealed manifest's source, normal product role, target and actual
kernel hash to match both its digest-qualified inventory and **current**
`build.json`. A different later build replacing `build.json` refuses, even if the
older inventory remains. Preserve an exact evidence directory when retaining a
build; never choose a convenient sidecar by filename age or overwrite a refusal.
The map filename and hash, requested and complete resolved graphical feature
closure, clean source, reviewed source hashes and linked ELF observations must
also match. Full verification requires a clean checkout at the sealed Source:
it recomputes the entire locked, offline, target-specific Cargo tree and compares
all transitive dependencies/features, normalizing only the checkout root path.
Changing a transitive feature while retaining the root line refuses.
Old v1 observations remain historical evidence; they cannot pass this stronger
contract by relabelling their schema. An inventory cannot set its isolation flag
or promote its proof class to hardware/executed evidence.

### Current authority and startup classification

The reviewed machine-readable responsibility table lives in
`make/xtask/build/privilege_inventory/source.rs`. Each entry records exact source
bytes and anchors; the verifier reads those bytes from the stated Git commit.
The entry is `main -> graphical_startup -> product_front_door::run -> platform::run
-> run_controller`. Startup initializes PCI/xHCI and USB attachment, machine
state, entropy, firmware scanout and offers. `platform::run` receives mutable
`XhciReady`, HID sessions and USB devices; its RootPlatform retains those hardware
handles while the controller receives typed inputs and a checked display sink.
`RootRuntime`, lifecycle admission, Body planning, separate Tour paths, Root
facades and effect providers remain privileged. A restricted Rust signature
reduces authority flow but does not itself enter an unprivileged address space.
USB/PCI, device buffers, audio and framebuffer providers have not thereby become
isolated drivers; DMA containment and physical evidence remain separate work.

The x86 service backend constructs page tables, checks images and owns trap,
preemption and capability machinery. `ServiceMemoryLayout` declares image,
exchange, transfer, stack, metadata and optional pixel ranges; its values do not
prove the resulting installed mappings. RX/RO images, RW+NX private ranges and
unmapped gaps remain the mapping contract. `service_memory/storage.rs` owns
**nine** independently claimed physical profile stores; the separate ordinary
two-slot domain pool is additional. Old descriptions of six slots are historical,
not the current inventory.

| Profile | Source-owned responsibility | Authority retained outside it |
| --- | --- | --- |
| Compositor | Retained pixels, surfaces, damage, focus/cursor and composition | Root scanout grant, range validation and ACK |
| Draft | Canonical Birth draft/naming and projection | Root lifecycle effects and immutable selection validation |
| Home | Home controller and local presentation; independent instance of the Draft/Home image | Root product lifecycle and effect dispatch |
| Runtime | Canonical native-workset scheduler/values/Cords/Signs | Root admitted hardware effects; remaining ordinary paths |
| Application | Native retained application state | Root checks against prior views and admission facts |
| Network | Bounded network protocol state | Root network device and transport handles |
| OrdinaryRuntime | Separately leased ordinary text/Tour execution | Root provider/machine authority and remaining runtime paths |
| Mask | Checked retained workspace layout/presentation policy | Root Face/Show/input custody and scanout validation |
| Tls | Bounded TLS protocol state | Root socket/device transport and product policy |

Default native Cargo closure is `default,native-compositor,native-owner-network`.
Build dependencies execute on the build host. Normal dependency membership is
potential code, not privileged reachability. Source-reviewed calls establish
possible paths; optimization may inline or remove symbols. The Root link map
supports exact linked responsibility classification, while the v2 ELF observer
records the Root and valid embedded ELF64 images with content hashes, entry points
and load-segment flags/addresses/extents. Embedded image bytes establish neither
which service profile selected them nor an executed privilege level. Image
identity, source-selected entry, installed mapping and actual service entry remain
separate evidence. The ordinary image can serve more than one admitted profile;
shared immutable bytes never imply shared mutable instance ownership.

This issue's contract and negative checks cover stale source/hash/map/role,
feature-profile substitution, dirty source, malformed ELF segments and unsupported
isolation claims. A fresh build-bound receipt is source/link proof only. Existing
QEMU receipts may establish their own exact-image service entries or interactions;
they are not automatically imported or upgraded by this verifier. No new QEMU,
physical/DMA confinement, full microkernel, recovery or #6175 completion is claimed.
