# Normal AArch64 product USB preparation and receipt

For #5587, extending ordinary-product integration evidence from the existing
#5437 transport and the preparation repair tracked by #5867. This uses the
canonical `conduitos-aarch64-virt.host.conduit` image, not the dedicated
protocol proof profile.

The unchanged canonical image first panicked in a fresh CDC run. A bounded,
nonallocating product panic report exposed a 434,606-byte allocation failure
with 5,574,752 bytes live in the declared 8 MiB arena. The canonical profile
now declares the same finite 256 MiB preparation arena as the existing
protocol Source profile. A second run then refused at the genuine graph
admission gate: the checked class expands to 49 gears, exceeding eight
preparation operation slots. The declared preparation ceiling is now 64.
This ceiling counts gears, not USB transfer operations. Root control/read/write
counts, authority and attachment checks remain unchanged.

The ordinary AArch64 product boot verifier now supplies 512 MiB instead of
256 MiB so the declared arena fits beside the kernel and firmware. Its small
architecture fixture keeps its separate configuration. Tests cover explicit
heap ceilings on PC/protocol/AArch64 profiles and the unchanged static fallback
on an actual zero-heap RISC-V profile.

At `fc6907fae2fe13e02f01c5f1a57c5e8e8930d594`, the canonical image built,
two ordinary product boots emitted fresh Host/Boot identities and the expected
zero-Body serial presentation, and fresh CDC success/refusal guests passed.
Independent wire validation observed five class controls and sixteen exact
OUT/IN pairs, or zero class effects for the mismatched Root. Play reported zero
allocations and reallocations. Two arena tests and strict all-target xtask
Clippy passed. Exact current identities and hashes are in
[evidence/usb-normal-aarch64-product.json](evidence/usb-normal-aarch64-product.json).

A separately retained comparison at `99cd2c5461a8f4cec1b2bf298708769a20cba5f0`
executed the same Source package and Root on canonical PC and AArch64 images
at one code revision. It retains each architecture's own kernel, controller,
provider, attachment, Plan and Play identities. That earlier comparison is
not relabeled as the refreshed source above.

Proof class is freestanding emulator, not physical compatibility. The larger
arena increases the normal AArch64 memory requirement; no automatic growth,
new provider, retry or class policy is added. Parent #5253 remains open for its
current full Integration and stable-publication gates. The accepted #5586 STALL
repair (#5873) and #5867 PC preparation retain their own source-pinned proof.

## Accepted-base product refresh and cross-architecture joins

At `e564c88a5aa5cab4d8e33f82d9eb14542557a513`, after accepted PC preparation,
the canonical AArch64 product rebuilt and two ordinary boots emitted distinct
Host/Boot identities. The front-door presentation retains absent Body identity;
that presentation is separate from the selected USB Root's real Plan/Play.
Fresh CDC success and mismatched-Root refusal both pass.

The canonical PC product then built on that same source revision. The exact
same checked package succeeds with sixteen TX/RX pairs on each architecture;
the same mismatched Root refuses with zero class controls/transfers on both.
Each native Play retains zero allocation/reallocation counters. Physical
compatibility remains unverified.

The retained correlation audit joins package, class Source/checked/expanded,
controller PCI/BAR and attachment port/slot/epoch, selected Base/provider/resource,
Host/Boot, static Plan, and distinct Body Plan/Play identities. It verifies raw
serial hashes without text newline normalization. The offline checker at
`targets/conduitos/proof/tools/check_cdc_product_correlations.py` accepts four
retained receipt directories, expected source and a fresh output path. Three
altered identity records were refused even after their transcript hashes were
recomputed; these are offline verifier tests, not new native provider refusals.
Exact receipts and tool hashes are in the evidence file. The earlier source-pinned
proofs remain intact. Parent #5253's integration and publication gates remain open.
