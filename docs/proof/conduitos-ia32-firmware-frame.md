# IA-32 normal firmware-framebuffer product

For #6057, runtime source `64d5130753466f44c0b605ad6b366d54083eeafd` built the separately selected normal graphical PROFILE and passed two fresh QEMU UEFI32 boots. Both produced a checked source-backed Crèche Face, acknowledged native Mask Show and real 1280×800 QMP PNG. This is a read-only emulator product frame; local input, physical display, GPU acceleration and stable release acceptance remain unproved.

![Actual IA-32 Crèche](../../targets/conduitos/proof/firmware-frame/ia32-64d513075/first/creche-read-only.png)

The [complete packet](../../targets/conduitos/proof/firmware-frame/ia32-64d513075/index.json) retains original captures, serial/QMP and launch records, actual compressed kernel ELF and ISO, manifests, focused validation, negative probes and earlier nonpassing attempts. Evidence-only descendants preserve the stated runtime source.

## Selected product and machine

The normal [graphical profile](../../targets/conduitos/profiles/conduitos-ia32-graphical.host.conduit) selects the existing IA-32 serial entrance plus `display/scanout`, the existing native Presenter/compositor and four finite surface slots with 8 MiB. Exact lowering rejects missing, foreign, duplicated or unbounded closure members. It selects `ia32-product` and `native-compositor-core`, with no local keyboard/pointer or proof instrumentation. Rendering, Face ownership, checked Mask/Plan/Play, surface authority and acknowledged Show use the same `FirmwareFace` composition as AArch64.

QEMU is `qemu-system-i386`, `q35`, CPU `max`, 512 MiB, one CPU, default standard VGA, display disabled for QMP capture and debugcon on port `0xe9`. The normal Multiboot product uses the reviewed CODE/VARS pair from F0; each run verifies a fresh writable VARS copy against the immutable template before guest execution. Launch receipts retain emulator, CODE, template, copy and image digests. No firmware is downloaded during boot.

The graphical Make selects Multiboot video mode 0. The separate serial profile retains mode 1 and its BIOS text entrance. Graphical BIOS/VGA-text proof is explicitly unavailable in the collector; it cannot borrow a serial text receipt. Current graphical evidence is UEFI32 only.

## Mapping, ownership and bounds

Observed scanout is 1280×800, pitch 5120, 32 bpp, RGB shifts 16/8/0 and 4,096,000 bytes at physical and virtual `0x80000000`. The IA-32 leaf verifies Root's known flat protected-mode segments and disabled paging. It validates the original bounded firmware memory map and excludes Root image/arena, module, handoff and memory-map storage before retaining an exclusive writer.

Both boots report 18 memory-map entries. The framebuffer lies outside all reported nonreserved memory; no single reserved entry covers it. This is not relabelled as reserved RAM. Observation remains separate from the selected provider's ownership. Bounds and aligned cache-line coverage are checked before stores. CPUID independently supplies CLFSH, SSE2 and a 64-byte line; volatile word stores complete through MFENCE, CLFLUSH of modified lines and MFENCE before Show acknowledgement. No MTRR/cache-memory-type claim is made.

The profile admits a 256 MiB runtime arena. Both actual runs report peak live allocation of 18,542,560 bytes. Each first scanout writes 1,024,000 pixels; the Mask receipt records 29 kernel signs and three Fore endpoints. Spawn-to-QMP-capture observations are 7.203 and 6.679 seconds, including firmware and ordinary product preparation. These are observed upper bounds for first visible pixels, not calibrated hardware latency or a cross-architecture performance comparison.

## Actual joins and semantic correspondence

ISO SHA-256 is `8c43ed41cabd747706f129ad759a3d8511996b8cb39e093266e0b14ead7b52bf`. The [independent audit](../../targets/conduitos/proof/firmware-frame/ia32-64d513075/independent-audit.json) verifies actual kernel/ISO digests and the complete kernel ELF inside the ISO, two distinct Host/Boot pairs, Profile/Build/logical Image joins, framebuffer Base and mapping joins, Face/revision/Mask Plan/Play/Show acknowledgement, PNG chunk CRCs and independently decoded RGBA pixel digests. PNG hashes are `fd2623f8a997a06096f564aa782db7e04c1c9f76c1933561090493e69cc0bb9f` and `5a89302d7c425d91ead1d6948935c556ff46f70808dfae8608d1b01838781ac2`.

The authored projection Source/Checked/Expanded identities and Mask plot identities equal the retained x86 and AArch64 references. IA-32 and AArch64 also agree on all current owner-action availability and five typed Plot subjects. Four Body worksets remain `native-body-current-host-unavailable`; Birth reports `EmptySelection`. The Mask separately reports read-only/no local input and hides input hints. Boot-generated names and concrete Host/Boot/Face/Show identities differ. This establishes correspondence with explicit capability differences, not runnable-workset or input parity. See the [partial three-architecture comparison](conduitos-firmware-frame-comparison.md).

## Refusals, baseline and validation

Removing VGA from the same immutable graphical ISO produced an actual guest `display-absent` refusal in 4.128 seconds and no graphical Show. Dirty source refused through the supported verification command before guest spawn: no launch receipt or guest serial bytes were produced, the image stayed unchanged and the owned marker was removed. Both original negative packets are retained.

The normal serial baseline at parent source `dea3571c98c49ded26096403cd07f774a650dfc2` passed two fresh UEFI32 boots and one BIOS boot, with actual VGA text validation and distinct Host/Boot IDs. The subsequent one-file correction only exposes portable product Bases under `native-compositor-core`, which the serial profile does not select. Serial baseline source and image identity remain separately labelled.

Focused validation passed 21 lowering, 346 tooling, nine Multiboot, three scanout-policy and seven Make/offer tests. Their original source-file receipt precedes tooling-only edition formatting and the native-only module-gate correction. Final runtime source passed strict combined all-target host Clippy, all 345 supported preflight tests, normal graphical build and both actual UEFI32 captures. An initial tooling-test invocation used a library-only flag on the binary xtask package and ran no tests; the corrected invocation supplies the stated passing tooling suite. Earlier formatting refusal, missing local actionlint prerequisite, and pre-correction native compile failure remain nonpassing evidence; the passing serial baseline is not lost when the later graphical compile fails. The original artifact audit assumed a Plot identity prefix; the corrected audit uses the typed `Plot` role on the same retained bytes, with no guest retry.

## Supported reproduction

At the runtime source with the documented IA-32 build tools:

```sh
cargo xtask make conduitos prepare-ia32-firmware
cargo xtask make host build targets/conduitos/profiles/conduitos-ia32-graphical.host.conduit --output target/host-build/ia32-graphical
cargo xtask make host verify target/host-build/ia32-graphical --boot
```

Verification deliberately stops QEMU after receipt/capture. The persistent launcher and the RISC-V64/LoongArch64 normal graphical frames remain required by #6057. This report supplies one architecture slice, not the complete five-architecture acceptance.
