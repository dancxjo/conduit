# ConduitOS firmware framebuffer discovery

The G0 collector for [#6057](https://github.com/dancxjo/conduit/issues/6057)
observes a normal PROFILE-built product image through the existing bounded QMP
transport. It records firmware framebuffer handoff and real display bytes.
It does not admit a graphical Host, publish a Face, acknowledge a Show, or prove
physical scanout or GPU acceleration. A firmware logo or uniform display remains
diagnostic evidence.

Build the normal image with `cargo xtask make conduitos live ARCH --locked`.
Use already prepared local firmware and, for RISC-V, its exact U-Boot EFI input.
The collector never downloads. IA-32 preparation is the separate F0 route:
`cargo xtask make conduitos prepare-ia32-firmware` followed by
`cargo xtask make conduitos doctor-ia32-firmware`.

For example, probe the AArch64 firmware candidate and its absence separately:

```sh
cargo xtask make conduitos framebuffer-probe --arch aarch64 --provider ramfb \
  --image-directory target/conduitos/live/aarch64-virt \
  --firmware /usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
  --output-directory target/framebuffer-aarch64-ramfb
cargo xtask make conduitos framebuffer-probe --arch aarch64 --provider none \
  --image-directory target/conduitos/live/aarch64-virt \
  --firmware /usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
  --output-directory target/framebuffer-aarch64-absent
```

An output directory must be new, UTF-8 and contain no commas. Existing evidence
is never replaced. `discovery.json` records the product manifest, distinct tool
and image source identities, source dirtiness, input hashes/sizes, exact launch,
QEMU version, raw display modes and explicit refusals. `serial.log`,
`qemu-stderr.log` and bounded `qmp.jsonl` retain the observations. The existing
bounded PPM→PNG capture writes `observed-display.png` when available; its health
refusal remains separate from framebuffer discovery. Missing screenshot support
is reported without fabricating pixels or erasing a valid handoff observation.

The same command accepts all five live-media architectures:

| `--arch` | Candidate provider | Additional local input |
| --- | --- | --- |
| `x86-64` | `vga` | `--firmware /usr/share/seabios/bios-256k.bin` |
| `ia32` | `vga` or investigated `ramfb` | CODE as `--firmware`, immutable `--vars-template` |
| `aarch64` | investigated `ramfb` | local QEMU EFI firmware |
| `riscv64` | investigate `ramfb` and firmware-initialized `vga` independently | OpenSBI as `--firmware`, U-Boot ELF as `--boot-kernel` |
| `loongarch64` | investigated `ramfb` | exact prepared QEMU EFI ROM; QEMU 10 or newer |

`--provider none` supplies the absent-device comparison. An optional `--emulator`
selects an exact existing local QEMU executable, including an owning target's
prepared emulator. The tool records its hash and version; selecting a device
never establishes firmware support. In particular, RISC-V firmware builds may
have Bochs video support without a ramfb GOP provider. That result is specific
to the retained firmware and emulator, not a universal architecture capability.

IA-32 uses GDB internally to stop at the exact ELF entry and read the Multiboot
handoff before the kernel consumes EBX. The supported `cargo xtask ci pipeline setup conduitos-ia32` entrance supplies
GDB for that probe.
Its 128-byte record follows the pinned Limine 12.5.2 structure, whose RGB fields
begin at byte 112 after an explicit reserved word. A legacy text console is
retained as text, not relabeled as linear graphics. Other architectures read the
pinned Limine request through QMP, verify its guest ID/revision, and bound
framebuffer/mode counts before following their pointers. Discovery admits at most
four framebuffer records and 128 modes per record (the retained x86 firmware
actually offers 66). RISC-V requires a complete product or refusal line and
100 ms of serial quiescence before pausing, to avoid reading a Root virtual
address during an SBI console call. An unreadable address remains a refusal,
never an absent framebuffer or graphical acceptance.

The collector stops its own guest. Persistent human launch, checked surface and
scanout admission, source-backed rendering, acknowledged Show, input journeys,
and the comparative product gallery retain their separate acceptance gates.
Raspberry Pi boards remain outside the five live-media profiles and have their
own [framebuffer track](https://github.com/dancxjo/conduit/issues/6059).

## Retained development discovery

The [source-pinned capture package](../../targets/conduitos/proof/framebuffer-discovery/firmware-g0-776157c1abf7/index.json)
retains the original reports, serial/debugger/QMP transcripts and real diagnostic
PNGs with hashes. Input product sources and the collector source are recorded
separately. This is development discovery, not accepted-release or graphical
product evidence. The IA-32 and x86 captures are uniform. None of these captures
has an acknowledged Conduit Show; nonuniform firmware or QEMU diagnostic pixels
do not establish one.

| Live Host | Candidate | Observed current framebuffer | Advertised modes |
| --- | --- | --- | --- |
| x86_64 PC | standard VGA, BIOS | 1280×800, pitch 5120, RGB32 | 66 |
| IA-32 PC | standard VGA, UEFI32 GOP | 1280×800, pitch 5120, RGB32 | Multiboot current mode only |
| AArch64 virt | ramfb, UEFI GOP | 800×600, pitch 3200, RGB32 | 3 |
| AArch64 virt | absent device | no framebuffer response | 0 |
| RISC-V64 virt | ramfb | no framebuffer response | 0 |
| RISC-V64 virt | firmware VGA | 1280×1024, pitch 5120, RGB32 | 1 |
| LoongArch64 virt | ramfb, UEFI GOP | 800×600, pitch 3200, RGB32 | 3 |

The packed x86 firmware mode records are decoded as bounded bytes, without
creating aligned Rust references to guest memory. The input image, kernel,
firmware, immutable template and emulator hashes were checked again after each
run. Private writable VARS and temporary entropy bytes are excluded from the
capture package. The repository-local full run retains the writable VARS with
private permissions; it is not a public artifact or a replacement template.
