# Jetson Nano ConduitOS port

Native ConduitOS execution is preferred. Linux middleware is an authorized
fallback if hardware support requires it, with any such image explicitly
identified as Linux-hosted rather than native ConduitOS. USB recovery flashing
is a deployment mechanism; a persistent USB connection after native boot
requires a separate device-mode driver.

Development work tracked by [#6478](https://github.com/dancxjo/conduit/issues/6478).
Build the deliverable on the development host and provide a downloadable
compressed image, SHA-256 checksum and flashing instructions. The user will
write the card on their own machine; local USB enumeration is not a deployment
prerequisite.

The requested deliverable is a flashable microSD image for P3450, with
on-screen boot/status output and no required UART adapter. UART is an optional
diagnostic path. There is currently no supported Nano image or physical boot receipt. The
`src/arch/jetson_nano/memory.rs` prerequisite has host conformance for finite
RAM/reservation sets and aligned placement admission. It is not yet connected
to a board entry point. Strict semantic extraction now reads root memory banks
and static reserved-memory children with two address and two size cells,
requires the original Nano/Tegra210 identity, and refuses unsupported layouts. The bounded DTB
envelope/token validator checks block layout, tree grammar and the reservation
map, and protects the DTB itself. Seventeen focused host handoff tests pass, including register-snapshot
admission for EL1, masked exceptions, disabled translation/data cache,
boot-core affinity and a usable counter frequency. EL2 currently refuses;
no transition or board entry assembly is implemented.
Real firmware DTB compatibility and physical boot remain unverified. This
contract describes the original 4 GB Jetson Nano developer kit, not Orin Nano.

## Hardware and provenance

NVIDIA specifies four 64-bit Cortex-A57 cores at approximately 1.43 GHz,
128 Maxwell GPU cores, and 4 GB of 64-bit LPDDR4 with 25.6 GB/s bandwidth.
The RAM belongs to the compute module; its SODIMM-style edge connector is the
whole module interface, not a RAM expansion socket. The developer kit uses
microSD storage; production-module eMMC is a different configuration.

Sources:
- [NVIDIA hardware announcement](https://developer.nvidia.com/blog/jetson-nano-ai-computing/)
- [Upstream U-Boot Nano board identification](https://lists.denx.de/pipermail/u-boot/2020-March/404135.html)
- [Upstream Linux Nano board description](https://github.com/torvalds/linux/blob/v6.12/arch/arm64/boot/dts/nvidia/tegra210-p3450-0000.dts)
- [NVIDIA Nano boot flow](https://docs.nvidia.com/jetson/archives/l4t-archived/l4t-3275/Tegra%20Linux%20Driver%20Package%20Development%20Guide/bootflow_jetson_nano.html)
- [ARM64 image and boot handoff contract](https://docs.kernel.org/arch/arm64/booting.html)

The upstream board identifies `nvidia,p3450-0000`, `nvidia,tegra210`, selects
`serial0:115200n8`, and enables the UART at `0x70006000`. These are reference
facts to compare against the device's actual firmware DTB, not authorization
to touch arbitrary MMIO. Exact carrier/module revision and the running
firmware's device tree must be retained with the physical receipt.

## Boot and memory admission

Retain NVIDIA early firmware for DRAM and power initialization, then use its
U-Boot handoff to enter the freestanding ConduitOS image. Do not write an
Orange Pi bootloader into the Nano boot media. Do not erase existing firmware
as part of building an image.

The image needs the ARM64 header and an actual nonzero effective image size,
including runtime storage. Preserve the incoming DTB pointer before using boot
registers as scratch. Validate DTB alignment, total length, compatible strings,
address/size cell widths, structure/string bounds and finite traversal limits.
Admit RAM from the handed-off memory nodes and exclude both the DTB reservation
map and reserved-memory nodes. Exclude the DTB itself and all live firmware,
image, stack and page-table ranges. Reject overflow and overlapping placements.
A nominal 4 GB specification is not an available-memory map.

Before Rust or BSS writes, check exception level, execution endianness,
MMU/cache state and primary-core identity. Either perform a reviewed EL2-to-EL1
transition with timer access configured or refuse that entry state. Secondary
cores must park before sharing the stack or clearing BSS. The selected image
load address must agree with both the header and linker; firmware addresses
from unrelated boards cannot establish this.

## Host mechanisms and ordinary execution

Keep Tegra addresses, UART register spacing, GIC initialization and timer
routing below the existing architecture seam. Share semantic implementations,
planning, kernel execution and lifecycle state. Authored plots contain no
Nano or firmware facts.

A real timer wake must pass through an installed exception vector, acknowledged
interrupt source and bounded wake state. Do not copy the Orange Pi adapter's
polling timer and software interrupt flag as evidence of hardware IRQ support.
Preserve cancellation, stale-wake refusal and interrupt-mask restoration.

After machine admission, run the ordinary checked text Plot through its
existing Plan and Play, with exact image, Host, Boot, Plan and Play identities.
Expose unsupported USB, storage, network and graphics operations as unavailable.
JetPack's Linux CUDA support is not a ConduitOS GPU implementation.

Protected AArch64 execution must use the actual board memory and interrupt
mechanisms. Review service access, firmware leftovers, DMA and device authority
before claiming parity with the AArch64 virt product. A freestanding serial
bootstrap alone does not establish protected execution.

## Implementation and proof order

1. Add the bounded boot handoff parser and refusal conformance, then the exact
   Nano feature, linker and entry assembly.
2. Add Tegra console, GIC, timer and existing Host mechanism adapters; build
   through a supported `cargo xtask` entrance with an artifact manifest.
3. Produce a flashable microSD image for existing Nano firmware, with explicit
   deployment instructions and no implicit device writes. Supply visible HDMI
   boot/status output so deployment and observation require no UART cable.
   Determine whether existing firmware can provide a retained framebuffer or
   whether a Tegra display mechanism is required; do not assume virt ramfb works.
4. Capture physical ordinary execution, timer wake and cancellation on the exact
   board. Retain firmware identity, DTB and image digests with visible results;
   serial capture is optional. A user without a UART cable must be able to
   flash the image, power on and observe success or a useful refusal over HDMI.
5. Establish protected execution and service authority on the board. Track each
   additional driver and its physical acceptance separately.

Compilation checks code generation and linking. QEMU virt checks its own virtual
machine. Physical Nano acceptance requires the actual board and a source-bound
receipt; neither of the former proves it. The port remains open until its
scoped physical and protection requirements are satisfied.
