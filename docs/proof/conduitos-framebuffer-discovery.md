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
framebuffer/mode counts before following their pointers.

The collector stops its own guest. Persistent human launch, checked surface and
scanout admission, source-backed rendering, acknowledged Show, input journeys,
and the comparative product gallery retain their separate acceptance gates.
Raspberry Pi boards remain outside the five live-media profiles and have their
own [framebuffer track](https://github.com/dancxjo/conduit/issues/6059).
