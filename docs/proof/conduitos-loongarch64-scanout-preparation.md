# LoongArch64 scanout preparation

This report retains the isolated preparation through source `2994f4de9` for
the remaining LoongArch64 frame in #6057. At those sources it changed no Host
offer, profile, renderer, boot collector or default firmware route and proved
no new graphical product frame. The historical held-file descriptions below
refer to that preparation, before normal-product wiring.

RISC-V PR #6412 subsequently merged, and [applicable combined Integration](https://github.com/dancxjo/conduit/actions/runs/40107606433)
passed at descendant `b0bc8d714c1d34d30231c75a7e3ea02c7395b6b9`, retaining the
merge. The preparation is now refreshed onto current development and wired into
an optional normal graphical profile. The [normal-product report](conduitos-loongarch64-firmware-frame.md)
now retains its two actual graphical boots and negative controls. Those are
separate later evidence; the preparation packets do not establish that claim.

The [source packet](../../targets/conduitos/proof/tools/loongarch64/scanout-source-review/index.json)
retains seven exact original QEMU 10.2.1 files, their licensing, hashes and
comparisons to the retained prepared source trees. The fixed source archive
SHA-256 is `a3717477d8e2c84d630bfffbc20f6cd3293eb45aa1e6dac6d0cc27689991c9e1`.
The comparison explicitly retains the prepared variant's CSR-MISC registration
difference in `trans_privileged.c.inc`; it does not mislabel patched source as
an identical upstream file. The cache-operation implementation in that file is
unchanged by the retained difference.

`ramfb_create_display_surface` maps guest physical RAM for the display surface.
LoongArch's `trans_dbar` emits a full TCG memory barrier; `trans_cacop` is a
privileged no-op in this model. `get_physical_address` checks direct address
mode and direct-map windows before page-mapped translation. These facts
constrain an emulator-only completion policy; they do not establish physical
cache behavior, inherited mapping permissions or exclusive framebuffer custody.

The [official LoongArch Volume 1 manual](https://loongson.github.io/LoongArch-Documentation/LoongArch-Vol1-EN.html)
(section 2.1.7) distinguishes SUC, coherent cached and WUC access types and
reserves MAT value 3. Its LDDIR/LDPTE and TLB-entry sections distinguish huge
page-table entries from final TLB fields. A validator must interpret the actual
selected paging regime rather than assume those representations coincide.

The existing protection-domain leaf describes a reviewed four-level 4 KiB
regime with 48-bit virtual addresses and checks PWCL/PWCH, page size and DMW
state. This is a useful local contract, not a substitute for observing the
actual framebuffer boot. The future scanout leaf must observe its CSR regime,
exclude or prove direct-window matches, validate every reserved framebuffer
page and retained table read, reject mismatched addresses/permissions/attributes,
retain exclusive bounded write custody, then render the shared checked
Face/Mask/Show and obtain two real normal-profile QMP captures. Local input,
physical scanout and GPU acceleration remain separate claims.

## Held mapping interpretation model

The responsibility-local mapping model is not linked into any product or Host
offer. Six [standalone conformance tests](../../targets/conduitos/proof/tools/loongarch64/mapping-model-preparation/index.json)
passed using the repository's pinned Rust compiler and an existing local serde
dependency. Source-file hashes stayed unchanged through formatting, compilation
and tests. This is deterministic interpretation proof over supplied descriptors,
not a normal Cargo suite or actual firmware mapping observation.

The model requires the exact four-level 4 KiB regime, 48-bit virtual addressing
without reduced-address mode, PLV0 page-mapped operation and no matching direct
window. It bounds every page and descriptor, distinguishes table pointers from
huge/ordinary leaves, checks V/D/software-W and PLV, removes huge HGLOBAL from
the physical address, requires exact physical identity and uniform nonreserved
MAT. The future machine reader must separately prove complete table-page
custody and supply actual CSR and descriptor observations before this model
can contribute to a graphical product claim.

## Observed historical normal-product CSR prerequisite

The [actual CSR probe](../../targets/conduitos/proof/tools/loongarch64/paging-csr-probe/index.json)
boots the preserved normal product at source `79a04199e` with the prepared
graphics emulator, pinned ROM and ramfb. It reaches the existing
`observatory-framebuffer-unrepresented` refusal, with no graphical Show. A
read-only QMP monitor query then observes CRMD `0xb0`, PWCL `333763884`,
PWCH `615`, STLBPS `12`, nonzero page-aligned PGDH, DMW0 `0x11`, other DMWs
zero and RVACFG zero. These match the model's paging prerequisites. This does
not observe the framebuffer's HHDM pointer, actual table descriptors or custody.

Only the named CSR whitelist is retained; general/FPU registers and the raw
register dump are discarded. Fresh ephemeral fw_cfg entropy is not retained
or hashed. Inputs and the unchanged firmware ROM were independently rechecked.
The original inherited `template_vars_unchanged` field is explicitly qualified
as ROM-hash stability: this launch uses `-bios` and selects no CODE/VARS pair.
The running guest was not paused for observation. This evidence neither
relabels the historical image as current source nor proves a graphical Frame.

## Held machine adapter

The unlinked `arch/loongarch64/framebuffer/mod.rs` prepares the future machine
adapter without changing any shared entrance or Host offer. It reads CPUCFG
and the actual CSR regime, validates all reserved framebuffer pages through the
pure model, permits descriptor reads only from complete bootloader-reclaimable
table pages, and rejects a changed paging regime. An atomic lease bounds the
single scanout writer; volatile pixel stores complete with `dbar 0` under the
reviewed QEMU TCG policy. Interrupt-enable changes alone are excluded from the
translation comparison. No page-table or CSR mutation is introduced.

This file has passed formatting, patch hygiene and a [compile-only LoongArch64 target check](../../targets/conduitos/proof/tools/loongarch64/scanout-adapter-compile/index.json). The check compiles the actual held adapter and mapping files against explicitly nonfunctional boot/display interfaces. It proves no custody, normal product build or guest execution. The adapter remains unlinked and has not been booted or used to acknowledge a Show. After
the RISC-V merge's required combined Integration passes, its normal product
wiring and cross-target checks must establish those separate claims. Existing
pure-model tests and historical CSR observations do not validate this adapter.

## Actual historical framebuffer translation observations

Two additional [read-only QMP probes](../../targets/conduitos/proof/tools/loongarch64/framebuffer-translation-probe/index.json)
resolve the historical ELF's exact Limine request symbols and response pointers.
The framebuffer is 800×600, pitch 3200, physical `0x0da90000`, with HHDM
`0xffff800000000000`. A second fresh boot observes all 473 unique descriptors
needed for its 469 pages. The held mapping model accepts these observations
with uniform MAT 2 (WUC), using explicitly assumed 48-bit CPUCFG widths; those
widths have not been measured by this probe. Actual product wiring must read
CPUCFG itself and separately prove table-page and scanout custody before stores.

Both guests retain the expected `observatory-framebuffer-unrepresented`
refusal and acknowledge no graphical Show. The queries neither pause the guest
nor change registers or memory. Full transcripts, selected memory reads, inputs,
collector scripts, compile/check output and original failures remain retained.
This is stronger mapping preparation, not a current graphical product frame.

## Held shared-Face composition

The unlinked `bin/loongarch64_product/graphical.rs` composes the same
`FirmwareFace` used by AArch64, IA-32 and RISC-V with the LoongArch machine
adapter. It consumes a fresh 32-byte key through the existing bounded fw_cfg
entropy provider and emits the shared Face plus a bounded mapping record.
There is no new Face authoring, input route or entropy source. The record
names actual MAT observation and the QEMU-only `dbar 0` completion policy.
This is formatted preparation only; no normal product wiring, cross-build or
Show claim follows from this unlinked file.

A third fresh read-only probe follows the actual Limine memory-map response:
46 regions, within the current 128-region bound. Exactly one FRAMEBUFFER region
covers the full scanout, and each of the five complete descriptor pages lies
inside exactly one BOOTLOADER_RECLAIMABLE region. This establishes historical
map coverage for the observed translations; it does not grant the future
current product its exclusive writer lease or acknowledge a graphical Show.

## Held normal-product collector

The unlinked `make/xtask/loongarch64_product_boot/graphical.rs` prepares the
normal-product collector using the common source/Face/Mask/Show capture join,
the reviewed graphics emulator, pinned single ROM, fresh ephemeral fw_cfg
entropy and a read-only product image. It retains the finite 45-second/1-MiB
bounds, static no-input capture policy, guest cleanup, unchanged ROM check,
mapping/MAT completion policy and exact Base/sealed-boot joins.

Formatting and patch hygiene are the only checks on this held collector.
It intentionally requires the existing firmware helper's visibility and the
common capture target to be wired after the required integration gate; it
cannot execute as currently held. Its exact Base inventory and canonical
records still require normal-product compilation and runtime validation.

Three [isolated hosted collector-mapping tests](../../targets/conduitos/proof/tools/loongarch64/collector-mapping-tests/index.json)
now pass against the actual held validation module. They reject disagreement
in scanout geometry/page counts, missing custody/completion assertions,
malformed or shadowing direct windows, unsupported paging regimes and extents
outside the supplied CPU address width. Contract fixtures label supplied
custody/completion claims separately from historical measured addresses.
This does not compile the complete collector or establish normal firmware proof.
The collector's inherited RISC-V refusal marker was also corrected to LoongArch.
