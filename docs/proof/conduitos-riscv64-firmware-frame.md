# RISC-V64 normal firmware-frame product proof

Two fresh normal PROFILE-built RISC-V64 products rendered the source-backed
read-only Crèche through the shared checked Face, native Mask and acknowledged
Show in QEMU TCG. Both actual 800×600 PNGs were viewed and independently checked.
This is local freestanding-emulator evidence for #6057, not local input,
physical scanout, GPU acceleration, accepted release or all-five completion.

The [complete packet](../../targets/conduitos/proof/firmware-frame/riscv64-61084bb3a/index.json)
retains the actual compressed ELF/ISO, manifests, two captures, raw launch,
serial and QMP records, independent artifact/pixel/semantic audit, negative
controls and initial failures. Every indexed original and compressed hash was
verified. The [comparison](conduitos-firmware-frame-comparison.md) includes
x86_64, AArch64, IA-32 and this RISC-V product.

## Exact sources and machine

The immutable normal product image was built at runtime source
`61084bb3ae35324bfb7dc2c280b37b6c60b7b00e` from
`conduitos-riscv64-graphical.host.conduit`. Its ISO SHA-256 is
`616ed17feafe529d787c2b780b438d8ff8a05f92a464b0baff6c96e3677118c5`.
The full ELF was independently found inside that ISO at byte 3239936.
The final collector source is `22f9ee6fa0d02a5aa913e75e59e268fa1173c86e`;
only three xtask collector files differ from the runtime source. No guest code,
profile, Make inventory or image bytes changed between them. Launch records
retain both identities; the runtime source is not relabelled as the collector.

The reviewed machine is `virt,acpi=off`, one `rv64` CPU with Zkr enabled and
Sv48/Sv57 disabled, 2 GiB RAM, single-thread TCG, firmware `ramfb`, and no
network or local input provider. The optional graphical profile preserves its
SBI serial provider and adds the finite scanout closure. The default serial
profile and its firmware route remain separately selected.

The selected QEMU is source-built 10.2.1
`conduit-protected-riscv64-graphics`, executable SHA-256
`7cf442811288219dfa1e99f7b1a3766e652aed550377777b3b2ad89cf54038ba`.
OpenSBI 1.8.1 has SHA-256
`b296d711ecb879489689109c0fd7e0698b5b9550725b4e50fac456bbd3ecbcdf`.
The EDK2 pair is pinned to `edk2-stable202508-r1`: CODE SHA-256
`a16aab6e12beab587cf59b4c23f015fcb07a94480b7462198a47310d0d6b1d6f`,
VARS template SHA-256
`ea8094e953b1215444bd001ee1cf22818f1f7f8abcb158e62180cbaa6c1f70af`.
Each boot used a distinct writable VARS copy; the pinned template stayed
unchanged. Source/recipe/license and cold/warm preparation evidence are retained
under [firmware preparation](../../targets/conduitos/tools/riscv64-firmware/README.md)
and [emulator preparation](../../targets/conduitos/proof/tools/riscv64/README.md).

## Mapping, custody and completion

Both boots observed an 800×600, pitch-3200, 32-bit RGB scanout with channel
shifts 16/8/0 and 1,920,000 bytes. Physical start was 4282945536;
virtual start was 18446743803114590208. Before any renderer write, the leaf
required one complete bootloader-reserved framebuffer extent and walked every
page's inherited Sv39 translation: 469 pages and 1,407 descriptors. Reads were
restricted to complete bootloader-reclaimable table pages. Supervisor writable,
already accessed/dirty mappings, exact physical addresses, canonical bounds,
superpage alignment and unsupported attribute refusals were checked. The
inherited PMA is reported without claiming a physical hardware memory type.

An atomic guard admits one writer. Existing `RawDisplay` volatile pixel/span
stores and `fence iorw, iorw` complete before Host Call return and Show
acknowledgment. This completion policy is scoped to the pinned QEMU TCG model:
its source maps guest RAM into `ramfb`, models no RISC-V cache hierarchy, and
translates FENCE into a full memory barrier. The [source review](../../targets/conduitos/proof/tools/riscv64/firmware-231bb012d/scanout-source-review/index.json)
retains exact original files from the verified QEMU archive. It does not prove
physical RISC-V cache coherence or arbitrary device memory behavior.

The final collector joins the same framebuffer Base to eight actual Bases
(seven ordinary plus scanout), the current Host/Boot, and the sealed UEFI64 boot
provenance, geometry, Build and logical Image. It checks the Face/Mask/Show
identities and acknowledgment before QMP capture. The shared firmware presenter
produces one frame and exposes no repaint or local-input operation. Final QMP
captures therefore leave the guest running; the receipt explicitly records
`acknowledged-one-shot-no-local-input`, then the verifier terminates its guest.
This is distinct from a persistent user launcher, which remains open in #6057.

## Frames, semantics and finite costs

| Fresh boot | PNG SHA-256 | Boot to capture |
| --- | --- | --- |
| [First](../../targets/conduitos/proof/firmware-frame/riscv64-61084bb3a/first/creche-read-only.png) | `95f854f8ccee636e611db82401a313bebfe23e1de4fb40b0f1e9ff1e0d13352d` | 10,794 ms |
| [Second](../../targets/conduitos/proof/firmware-frame/riscv64-61084bb3a/second/creche-read-only.png) | `d86b5869d238bcb30f36003aaeb8c7a6bfd5c02b48d78a799b9f9633264fd240` | 10,791 ms |

These are measured boot-to-capture upper bounds, not calibrated time to the
first visible pixel. Host/Boot/Face/Show identities are fresh. The names differ,
but authored Source/Checked/Expanded projection and Mask identities, all five
Plot subjects and owner action availability equal the retained AArch64 frame.
That frame is already joined to the IA-32 and x86 references. Pixel equality is
not asserted. No Body was born and no local action was exercised on RISC-V.
The visible notice is read-only with no local input; it shows no navigation-key
hints. The full retained Face includes content beyond this first viewport.

The profile admits a 256 MiB runtime arena and four surface slots totaling
8 MiB. One scanout contains 480,000 pixels; each first Show wrote 480,000
scanout pixels, with 29 kernel Signs and three Fore endpoints. The first capture
records 504,970 offscreen pixels written. These are finite preparation/first-frame
counts, not frame-rate or incremental-update performance measurements. Arena
high-water usage was not independently instrumented in this RISC-V receipt.

## Refusals and validation

Removing only `ramfb` from the same image's launch produced the actual
`display-absent` refusal after 8,535 ms and no graphical Show. The image and
firmware template stayed unchanged. Supported verification with an owned dirty
source marker refused before the normal product guest launch; no normal launch
receipt or serial bytes appeared, and the private VARS remained unchanged.
Prepared-tool validation can run bounded paused empty-machine QMP probes before
that refusal; those execute no guest instructions. The marker was removed and
the source restored clean.

Four mapping tests, 23 lowering tests, the producer-record collector refusal
test, RISC-V library/product cross-checks, repository preflight (345 tests),
strict combined all-target xtask/ConduitOS native-compositor Clippy, and actual
two-boot verification passed. Cross-target feature-dependent and legacy
`memcpy`/`memset` warnings remain in the raw logs; this is not a warning-free
cross-target build claim.

Initial attempts remain explicit: the first library check lacked a RISC-V cfg
on the raw extent observation; the first captured frame's final validator used
`pitch_bytes` instead of the common producer's `pitch`; a later second capture
timed out waiting for QMP stop; and the first direct static two-capture run met
an obsolete serial-only Patchbay expectation of seven Bases. The final code
uses the actual shared producer record in its schema test, captures the
acknowledged static frame without VM pause, and validates the exact eight-Base
inventory and boot provenance. None of the failed outer attempts is reported
as a successful full verifier run.

## Supported entrances

```sh
cargo xtask make conduitos prepare-riscv64-domain-emulator --graphical
cargo xtask make conduitos prepare-riscv64-firmware
cargo xtask make conduitos doctor-riscv64-firmware
cargo xtask make host build targets/conduitos/profiles/conduitos-riscv64-graphical.host.conduit --output target/host-build/riscv64-graphical
cargo xtask make host verify target/host-build/riscv64-graphical --boot
```

Use the packet's exact recorded source/command pair to reproduce its retained
artifact. Building a later source produces that source's distinct Make/Image
identities. The preparation tools require their documented local prerequisites;
the locally prepared graphics emulator also retains the explicitly recorded SDK
runtime-library dependency. Execution does not acquire firmware. Input parity,
physical Pi/display work and GPU acceleration remain separate follow-ons.
