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

`nor::PreparedNorSnapshotWriter` stores one existing immutable snapshot record in an
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
`PreparedNorSnapshot` preserves the existing combined read/publication API by
wrapping that same writer with a separately retained read record buffer. Native
publish-only owners retain the smaller writer and add read workspace only for
recovery. There is one NOR format and publication implementation.
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

## Native one-shot publisher

The explicit `native-host-call` feature enables the existing JSON snapshot Kind's
cooperative native implementation. Default codec/NOR users do not pull in the
kernel, lowering or JSON validator. `ReadySnapshotPublisher` retains the real
exclusive slot and independently checked snapshot/reference grants. Preparation
borrows Root's canonical Kind and exact checked reference configuration; it
retains no decoded Kind tree. The existing native assigned binder checks the
Base identity, exact operation/resource/Fore and rotating bearer capability.
Root must separately admit the content resource, inline/provider/codec backing,
bounded work, actual Start and a flash window excluding timing-sensitive work.

`SnapshotPublishHostCall::invoke` checks output space before media access, then
validates JSON and publishes one exact generation under one operation lease.
Only verified media publication and successful capability completion expose the
exact reference bytes. If the kernel loses that completion, the same request and
exact retained input can retrieve the cached result without a second program.
Changed/stale requests, cancellation, revocation or a media failure cannot retry
side effects. Media commit, kernel commit, remote delivery and reclamation remain
separate; this installs no scheduler, durable catalog, rollover or erase policy.

With `native-host-call`, `native_append::ReadyRecordAppender` retains an actual
recovered `PreparedNorRecordResidence` and independently held current-Boot append
authority. Its plain `machine/nor-record-residence@1` resource unit denotes the
entire affine finite residence, not unlimited bytes/work and not a content
Resource reference. Exact physical bytes, slots, remaining write slots, original
finite call ceiling, append calls and recovery reads are exposed for separate
Root funding. Calls/status work and physical write capacity are distinct bounds.
Preparation uses the ordinary assigned/Fore native binder and rotating Base
capability; it retains no rich Kind or Type tree. The catalog's generic
`resource/try-append-record` consumes existing `value/bytes` (at most 200) and
emits the canonical declared variant `recorded(U64) | capacity(Empty) |
uncertain(Empty) | recovery-required(Empty)`. Build-host derivation links its
immutable schema once; the existing Core composers prepare all result storage.

The exact outcome is cached for same-request/same-byte retry without another
program operation. Only actual kernel acceptance may acknowledge the exact
request/outcome through the owner API and permit the next request. Capacity and
uncertain media are ordinary declared results, allowing authored sampling flow
to continue. After an uncertain attempt, later calls report recovery-required
without further writes; the original uncertain custody remains retained. Scope,
authority and invalid request failures remain hard refusals. Output pressure
never programs media. Revoked retirement preserves committed/uncertain custody
and returns no erase grant. Media publication and kernel acknowledgment do not
claim remote recording, catalog commitment, delivery, reclamation or device Start.
Root must still admit provider memory/work and exclude timing-sensitive operations
through the shared native activity window before invoking synchronous flash work.
