# Immutable snapshot records

Extraction of the existing browser immutable record codec into a shared no-std/
alloc mechanism. Record bytes, key derivation, reference version/extent and
independent authority/residence checks are unchanged. Browser public paths are
re-exports. The existing record fixture is shared rather than copied, with an
additional owner-metadata bound regression test.

`cargo xtask check resource-snapshot` checks the generic codec and browser public
path plus strict codec Clippy. This creates no storage executor, native provider,
Host offer, catalogue seed, scheduler or Body. Linux file effects and shared typed
operation integration remain separate delivery. Refs #6350.

## Fixed NOR residence primitive

`nor::PreparedNorSnapshot` stores one existing immutable snapshot record in an
exclusive logical slot supplied by the residence owner. Slots need program-unit
alignment (power of two, 1–256 bytes), and can share an erase block. No erase,
slot allocation, generation selection, retry, catalog or reclamation operation
is exposed. The owner must preserve adjacent slots and separately admit actual
flash execution windows, write work and the retained record/program workspace.
Publication checks the entire slot is erased, programs a bounded length/digest
envelope and the existing record, verifies every programmed unit by exact media readback, then programs and
reads back a separate final commit unit.
Every media publication attempt is terminal for that handle. Reopening verifies
the marker, envelope digest and existing authority-checked record codec; a torn,
occupied or corrupt slot is never silently repaired or interpreted as success.
The provider contract requires synchronous media visibility and preservation of
all earlier completed units across an interrupted program. CPU cut-injection
tests establish behavior under that contract, not physical power-loss proof.

Media commit is distinct from the kernel's committed publication output,
remote delivery and permission to erase. Accepted RAM samples are not durable.
For the existing recording fixture context, one sample seals as 745 bytes of
JSON / 924 bytes of snapshot record; 64 seal as 3391 / 3570 bytes. Adding this
44-byte envelope and separate 256-byte commit unit programs 1280 / 4096 bytes.
Thus one-reading generations are costly; 64-reading chunks retain a volatile
tail until their admitted publication completes. This primitive does not solve
per-reading stream compression, discover committed generations, install an MCU
provider, or establish restart admission of a Body/Play.

`PreparedSnapshotRecord::prepare_selected` accepts borrowed selected facts without
fabricating a `PlannedGear`. It shares the original semantic/record checks and
requires an independently retained original `AuthorityGrant` plus local
`ResourceReferenceBinding`. Core's existing reference admission checks identity,
version, availability and byte/item ceilings; exact original grant matching
checks operation, subject, capability and current Host/Boot. `AuthorityGrant`
has no wall-clock expiry field; expiring snapshot references remain unsupported.
The trusted Root must supply those independent grants and separately establish
exact assigned correspondence, reservations, revocation/Play lifetime and safe
provider execution. This constructor issues none of those permissions. The rich
preparation API keeps its existing strength and does not invent local grants.
