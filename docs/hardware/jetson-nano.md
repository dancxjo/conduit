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

The requested deliverable is a flashable microSD image for P3450 with no UART
adapter requirement. The native board binary now compiles and links, runs the
existing ordinary text/timer planning path, and includes retained firmware
scanout diagnostics. Its HDMI, timer interrupts, memory admission and actual
boot behavior remain physically unverified. It has no native USB device,
USB input, storage, network or GPU driver, and no board protection-domain proof.
A successful build is an experimental candidate, not supported hardware parity.

The build entrance is:

```sh
cargo +stable xtask make conduitos jetson-nano-image --output /absolute/path/conduitos-jetson-nano.img.gz
```

It requires a clean committed source tree, an AArch64 bare-metal Rust target,
`aarch64-linux-gnu-objcopy`, Python 3, curl and e2fsprogs. It acquires the official
NVIDIA firmware donor on first use; `--donor` selects a previously acquired
scrubbed donor. No block devices or on-board firmware are written. The output
includes provenance, checksums and NVIDIA license text. See
[media tooling](../../targets/jetson-nano/README.md) for exact firmware layout.

The image links at `0xf0080000`; its ARM64 `text_offset` is `0x70080000` relative
to the Nano DRAM base `0x80000000`. This puts the image above the official donor
VPR candidate pool ending at `0xf0000000`. U-Boot relocates according to the
nonzero effective size and text offset. The actual passed DTB must still admit
the complete image/arena/stack range; neither the donor's map nor these constants
authorize overwriting actual firmware reservations.

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
MMU/cache state and primary-core identity. The board entry implements an EL2-to-EL1
transition with architectural timer access configured, while refusing active
translation/data cache and unsupported endian/exception states. It parks
nonzero-affinity cores before stack/BSS initialization. These paths compile but
still require physical execution proof. Secondary
cores must park before sharing the stack or clearing BSS. The selected image
load address must agree with both the header and linker; firmware addresses
from unrelated boards cannot establish this.

## Host mechanisms and ordinary execution

Keep Tegra addresses, UART register spacing, GIC initialization and timer
routing below the existing architecture seam. Share semantic implementations,
planning, kernel execution and lifecycle state. Authored plots contain no
Nano or firmware facts.

The native GIC-400/virtual-timer implementation must demonstrate that a real
timer wake passes through an installed exception vector, acknowledged
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

## First physical observation and diagnostic revision

The user flashed the candidate from `e23216e56830fcce212228ee9a5795c963b4819a`
and observed a persistent NVIDIA splash after the monitor received independent
power. This confirms firmware HDMI output; it does not establish native kernel
entry, DTB admission, interrupt delivery or ordinary execution. No UART capture
is available, and the candidate has no USB device-mode driver.

The second diagnostic revision accepts documented XRGB/XBGR32 scanout formats,
COLOR_EXPAND, and positioned unscaled windows within the active raster. It also
recognizes an explicitly bypassed display client with the global SMMU enabled,
but still refuses enabled display-client or ASID translation. The exact address,
linear layout, pitch, extent and occupied-memory exclusions remain checked.
Thirty-nine focused handoff/display tests pass. These compatibility changes are
plausible fixes, not evidence that one caused the observed splash.

Display custody is established before final image-placement admission so that
placement refusals can become visible. New messages distinguish accepted
firmware display handoff, interrupt initialization and the planned text/timer
run. Failure before those diagnostics can still leave the firmware splash.
The revised image still needs a physical test; it does not establish hardware
support or protection-domain parity.
