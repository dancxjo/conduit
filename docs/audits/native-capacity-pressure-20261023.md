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

Implementation and validation are in progress. No successful QEMU result is
claimed by this preparation note until an exact-source receipt is retained.
