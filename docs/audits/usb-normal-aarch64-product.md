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
current full Integration and stable-publication gates. The queued #5586 STALL
repair and #5867 preparation work retain their own acceptance.
