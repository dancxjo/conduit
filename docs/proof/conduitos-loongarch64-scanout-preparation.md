# LoongArch64 scanout preparation: source review only

This held preparation supports the remaining LoongArch64 frame in #6057.
It changes no Host offer, profile, renderer, boot collector or default firmware
route, and proves no new graphical product frame. RISC-V's current PR retains
its own Candidate/combined-Integration gate before overlapping implementation.

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
