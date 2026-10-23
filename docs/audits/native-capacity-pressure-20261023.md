# Native compositor capacity pressure

This is a focused #5608 preparation slice. It extends the existing
`cargo +stable xtask make conduitos compositor-semantic-recovery-proof`
appliance, retaining its production Mask, protected compositor and explicit
Root copy-failure recovery. It does not complete #5608 or #5994.

## Boundaries

The scene case fills the actual `GraphicsScene` with 48 commands, each with
192 UTF-8 payload bytes. The 49th command and a separate 193-byte payload
must refuse atomically. The valid scene goes through the production Root
typography rasterizer into its admitted staging lease before any semantic
Show exists. The protected compositor accepts pixels, not scene commands;
this is not a claim of protected scene interpretation or diagnostic Show.

The surface case uses the current Mask's actual service session. Seven
additional invisible 1×1 fixture Root surfaces are admitted and uploaded.
Together with the canonical Mask surface they occupy the fixed eight slots.
A ninth admission must return `OwnerError::Capacity` without entering the
guest, changing service cost, or replacing the session or service instance.
After removing only the seven fixture surfaces, the canonical Mask must
recompose and retain the same source Face, Show and draft revision with
keyboard routing available. Fixture admissions do not give the Mask's plan
an eight-surface reservation.

The existing later recovery case still requires a real fresh typed
interaction to advance the source draft. That later action is distinct
from the unchanged-source assertions at the surface-pressure boundary.

This is a source-pinned freestanding QEMU appliance using the existing
native Make profile. It is not the ordinary product image, physical scanout,
human observation, a screenshot acceptance case, guest CPU-fault recovery,
or malformed guest reply recovery. No limits or memory profile increase.

## Validation record

Clean Source `27723dc559ffe8852c011e14afee792749154aae` passes the supported
entrance with outer exit zero. The collector requires QEMU guest exit 33,
the original exact semantic-recovery receipt, matching native Make admission,
and the separate strict pressure receipt before publishing evidence schema @3.
The [original packet](evidence/native-capacity-pressure-qemu-20261023/)
retains the complete serial transcript, build/image records, compressed full
build/proof log, focused test log, independent verification and verifier.

All 48 valid commands complete in the production typography rasterizer,
writing 470,016 pixels. Both capacity refusals leave the scene unchanged.
Seven distinct extra handles complete admission/upload/commit and removal.
The ninth admission leaves the full service cost unchanged; guest entries
remain exactly 1,161 before and after refusal. Canonical recomposition writes
307,200 pixels and preserves the exact original Face, Show and draft revision,
with keyboard routing available. The subsequent original copy-loss recovery
still retires old input, acknowledges a fresh service's Show atomically, and
accepts a fresh typed interaction that advances the canonical draft.

Independent verification checks the clean source, actual ELF and ISO bytes,
serial hash, unique complete receipt lines, observed counters and semantic
custody, Make identity correspondence and unchanged 268,435,456-byte runtime
arena/ceiling. ISO SHA256 is
`18c29a750423268ac8381d81161553fa4773271dea453af88454597edbb3d6c0`.
All three focused collector/profile/recovery tests pass, including missing,
changed, zero-counter, duplicate and overclaim pressure negatives. The supported
build's six-target common-backbone compile gate also completes; existing unused
code warnings remain in the original log. No extra workflow run was queued.

This packet adds no screenshot or ordinary-product pressure acceptance. The
broader #5608 readable layout, separate visible refusals and combined-head
merge/check requirements remain open. Earlier copy-loss and ordinary-product
evidence retain their own exact sources and proof classes.
