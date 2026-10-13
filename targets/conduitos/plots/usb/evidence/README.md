# USB architecture acceptance evidence (#5258)

The current execution index is `current-usb-classes-a783be37/`. CDC serial,
read-only BOT recovery and Audio use byte-identical packaged Source and Root
requests on x86_64 q35 and AArch64 virt-10.2. ECM executes on x86_64 through the
separately admitted network Root. Each receipt retains its actual kernel identity;
these are execution observations of a783be37/a405f218, not execution of the final
PR head.

| Architecture | Collected product execution | Remaining disposition |
| --- | --- | --- |
| x86_64 q35 | CDC, ECM, BOT and Audio; exact bytes, zero Play allocation, acknowledged retirement | Hosted closure gates |
| AArch64 virt-10.2, neoverse-n2 | Same CDC, BOT and Audio Source/Root bytes; zero Play allocation and retirement | ECM network Root deferred to #5490; hosted closure gates |
| IA-32 PC | Explicitly unsupported | Native product USB provider follow-up #5489 |
| RISC-V64 virt | Explicitly unsupported | Native product USB provider follow-up #5489 |
| LoongArch64 virt | Explicitly unsupported | Native product USB provider follow-up #5489 |

`architecture-matrix.json` records exact class dispositions and receipt hashes.
Fresh BOT recovery includes both a successful independent second request and a
failed clear-IN that drains without a fabricated result or fresh request. Audio
retains 72 exact isochronous packets and typed normal drain. CDC retains sixteen
bidirectional request pairs; ECM retains full-MTU frames and 1460 TCP bytes in
each direction. These observations complement deterministic fault tests; a
successful guest alone does not prove every negative case.

`class-lifecycle-acceptance.md` maps the composed serial/ECM negative tests
to shared native enforcement and states their simulation boundaries.

Historical receipts remain immutable evidence for their original builds.
Their `remaining` lists and integration fields describe collection-time gaps,
not the current aggregate audit. In particular, older BOT receipts demonstrate
patterned block-after-CSW publication and sixteen repeated reads on both
architectures; newer recovery receipts supply the repaired graph drain and
retirement proof. No claim is retroactively added to an older receipt.
Physical/HIL compatibility remains unverified. Compilation is never substituted
for execution. This index alone does not close #5253–#5258.

Supported native rows use Root-owned coherent DMA and bounded event-ring polling;
no USB IRQ handler ownership is claimed. The matrix records shared ordering and
architecture mapping sources. Kernel identities belong to individual receipts,
not one historical row-wide candidate source. AArch64 HID device execution remains
uncertified in this four-class milestone. IA-32, RISC-V64 and LoongArch64 have no
admitted USB provider; their unsupported dispositions have explicit follow-up owners. This does not imply those
architectures lack other product capabilities.

The `hosts-conduitos` CI shard discovers ConduitOS integration targets through
`tools/xtask/src/suites/host_protocol_shards.rs`; it includes
`usb_architecture_evidence`, which verifies the five target dispositions and
exact retained receipt hashes. The HID and protocol-Plot integration binaries
are assigned to their existing browser and workbench shards. This gate checks
retained evidence integrity; it does not substitute a device execution claim.

Deferred native USB providers for IA-32, RISC-V64 and LoongArch64 are owned by
[#5489](https://github.com/dancxjo/conduit/issues/5489). AArch64 ECM network Root
admission is owned by [#5490](https://github.com/dancxjo/conduit/issues/5490).
The matrix certifies collected emulator execution evidence for the four owning
class profiles; exact-head Candidate, merged-tree Integration and automated stable-main
acceptance are separate closure gates. HID remains an explicitly uncertified profile. Receipt-level
integration fields preserve their collection-time status.
