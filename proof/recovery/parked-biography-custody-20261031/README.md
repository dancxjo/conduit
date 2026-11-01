# Biography custody: parked and live CPU slices

The normal sampler/Mask path restores its parked token into Root's actual
`SealedBodyLifecycleSession` before proposal commit and execution. A parked-only
transaction cannot represent that live owner. This slice preserves the parked
API and adds a separate live source-prefix transaction. Neither adds transport,
dispatcher, Body, member, Boot, changed capacity limits, or hardware actions.

The [sanitized receipt](receipt.json) separates copied-media, allocation,
arbitration and native/physical claims. Private inputs, actual identity values,
credentials, raw checkpoints, recovered records and fixture endorsements are
excluded from this directory.

## Ownership and commit boundary

`native_restart::biography::hot::Custody::prepare` consumes the actual
`ParkedBody`. Every preparation refusal returns that same token without another
allocation. The transaction holds the exclusive Store lease; unrelated readers
fail closed. Its offer/export witness reuses Root's existing original-owner
challenge and typed biography endorsement. A different source, generation,
Root, Body, or unconsumed owner context cannot substitute for that custody.

`export_chunk` authenticates the complete source and archive-stream digest before
returning at most 128 bytes. The caller asynchronously frames/flushes that chunk
using the existing biography wire, then yields; it must not accumulate a document
or block on an entire UART export. No owner ACK is accepted before the existing
receiver has verified, reopened and fsynced exact durable custody and obtained a
fresh challenge.

`begin` verifies the Root-endorsed receipt against the exact offer. The bounded
projection preserves the original encoded evidence, admissions, observation
layouts and network Clean cursor, replacing only the signed biography prefix
and removing only the already authenticated sealed archive array. It does not
materialize another Body, admission manager or archive vector. The output uses
the existing private codec and schema.

The existing incremental writer measures the full output before effects and
performs one checked sector/program/readback operation per advance. Complete
readback and typed completion bind the successor before `finish` updates the
actual source/proof and token generation. Canonical Body evidence, Boot, proposal,
foreground, archive summary, original admissions and other custody remain the
same. No observation epoch is erased.

Explicit cancellation authenticates the old affine source and returns its
original owner. An inactive-bank partial write cannot discard that source. An
ambiguous completed commit may become the valid endorsed successor on a later
Boot; cancellation does not falsely adopt it into the running journal. Tests
check both complete-snapshot selection and same-Boot old-owner restoration.

## Live owner and WaitLane handoff

`native_restart::biography::live::LiveCustody` leaves Root's actual Body installed.
It holds the exclusive original Store lease and a small typed scope witness,
not another Body, Plan, archive vector, or raw owner pointer. The live Store
requires `ready=true`, `parked=false`; existing evidence/admissions remain in
their original state. The pending RAM chain must continue the exact committed
summary, including its predecessor digest and compaction head/count.

The integration seam is:

- Root calls `reserve(actual_owner, root_key, current_host, current_boot)` without
  a flash permit. This pins memory scope only; no flash, codec replay, or key
  derivation occurs. The actual Body borrow ends before the owned job moves.
- The admitted WaitLane calls `prepare_source(quiet_permit)`. Root/source checks
  and the complete authenticated export offer are prepared under its actual
  permit. `offer()` and `export_witness()` return `Result` until this succeeds.
- Existing `export_chunk`, `begin`, `planned_bytes`, `operation`, `step`,
  `needs_chunk`, `prepare_chunk`, `advance`, and `is_complete` reuse the same
  Recipe/incremental writer and original-owner wire as the parked path.
- WaitLane calls `finish_source(quiet_permit)` or `cancel_source(quiet_permit)`;
  errors retain the borrowed job and lease. Complete readback updates only the
  committed source/proof/span/prefix/local count. Pending RAM archives are
  neither acknowledged nor cleared; the live realization is never reconstructed.
- Root calls `adopt(owned_job, actual_owner)` without a permit. This checks the
  pinned memory scope without allocation or serialization and releases Store
  custody. A refusal returns the complete job/lease for recovery with the
  original owner. `validate_owner(actual_owner)` is also memory-only.

Root **must fence every canonical Body/admin/renewal mutation** from reserve
through adoption. The memory witness pins Body/Root/current Host/Boot, canonical
sequence/sign, membership revision, pending archive count/head, foreground,
state, and immutable realization Wake/Plan/Play identities. It is not permission
to mutate the Body between phases. Admitted sampler/Mask kernels may continue
independently, but their actual continuity still requires integrated proof.
No ambient Gate introspection or fabricated Root-side permit is used.

The next ordinary successful checkpoint appends the untouched RAM pending tail
on top of the new committed prefix, then acknowledges that tail using the
existing checkpoint contract. Compaction proves custody of the **already
committed prefix**; it does not make newer RAM evidence durable on its own.

The live CPU fixture constructs a canonical actual started realization with a
nonempty pending archive tail. It verifies unchanged owner address, evidence,
realization, same Boot, admissions and Clean cursor; the next checkpoint appends
the tail exactly once. Its 63 storage operations produce 189 NOR cut cases
(zero/half/full effects), retaining the actual live owner and pending tail while
reopening only the old or complete endorsed successor. A separate fresh-process
owned seam test exercises actual shared Gate permits, premature/wrong-owner
adoption refusal and retained-lease recovery. Reservation peaks at 2,644 bytes
including its 2,264-byte job backing on this CPU host; Root adoption allocates
zero bytes. These are representative CPU results, not actual889 dense live
qualification, native memory/timing acceptance, or physical worker evidence.

## Root integration contract (not implemented here)

For the parked API, the management slot must retain the returned `Box<Custody>` across asynchronous
I/O and dispatcher completions, and explicitly finish or cancel it rather than
drop its owner. The API is:

- `prepare(owner, quiet_permit)`, `offer()`, `export_witness()`;
- `export_chunk(offset, &mut [u8; 128], quiet_permit)`;
- `begin(root_endorsed_receipt, quiet_permit)` and `planned_bytes()`;
- `operation()`, `step()`, `needs_chunk()`, `prepare_chunk(quiet_permit)`;
- `advance(existing_permitted_sector_access)` and `is_complete()`;
- `finish(quiet_permit)` / `cancel(quiet_permit)`, returning the actual token.

Read/codec phases borrow the existing flash-window permit so an owned capture
excludes them too. The physical advance performs no codec, allocation, signature
or key derivation. Forebrain owns integration with the actual management slot,
flash domain, sampler wait, pulse provider and original-owner channel.

Borrowing a permit proves exclusion, not duration. Native maximum replay/codec
duration must fit the actual remaining admitted window. The existing per-chunk
SDK timing budget does not qualify a whole source replay. Actual integrated
heap/stack, physical sampling/display continuity, interrupted owner I/O, and
long-duration renewal remain gates before deployment.

## CPU evidence and limits

The ordinary restart fixture passes seven focused tests. The observation-profile
fixture passes ten non-private tests, including 39 real Gate/Mailbox pulse
refusals and 117 NOR cut cases (39 operations times three effect modes). Partial
sector/program effects and failed readback retain a restorable old owner and
select only the old or complete endorsed checkpoint when the copied media is
reopened. Native SDK timing and physical operation were not exercised.

The opt-in private test authenticates copied generation 889. That historical
envelope has an unreconciled Wake; it cannot be presented as an already live
parked owner. Canonical copied-next-Boot resume/seal/park setup obtains the real
fixture token at generation 892. The subsequent hot transaction alone changes
892 to 893, 30,369 to 6,951 encoded bytes, and 24 local archives to zero while
preserving the total archive chain of 332 and the same setup Boot. It measures
408 bytes of preparation peak plus a 2,016-byte CPU job backing, with zero
allocations/reallocations during chunk replay, storage advancement and adoption.
These are CPU-host measurements, not native heap or WCET acceptance.

The same private test separately exercises the unchanged cold implementation on
original generation 889 without first adding a Boot: 29,115 to 7,848 bytes at
generation 890. Its copied canonical next-Boot checkpoint is 9,251 of 32,608
bytes. Both paths preserve original Body/Root, admissions, Clean and layout
receipts. All endorsements are explicit CPU fixtures, never real receiver ACKs.

Internal proof entrances are the existing standalone `native_restart` test
filtered by `biography::hot`, and `native_observed_cold` compiled with its
documented final-crate `observation-store` cfg. The private opt-in additionally
requires `CONDUIT_PRIVATE_BODY_BACKUP`, `CONDUIT_PRIVATE_BODY_ID`,
`CONDUIT_PRIVATE_PRODUCER_HOST`, and an independently pinned
`CONDUIT_PRIVATE_ROOT_PUBLIC`; no private input is bundled here.

Strict Clippy on the opt-in observation harness is **not green**. The unchanged
base and this candidate produce the same 15 diagnostic messages and locations,
with no new-module diagnostics. Existing harness lint policy was not weakened;
the receipt records that baseline comparison rather than claiming strict success.

## First migration staging; do not execute as an unattended recipe

The separately built clean `4076538797` maintenance image uses the actual retained
birth/biography and owner public key. Its supported `biography-maintenance-image`
build and linked maintenance packaging passed. Its application SHA-256 is
`c0019543e39a3f5191ff908eb9c51001cf9d46d2136a05fbc039d83d432bf2f7`;
it is 1,269,072 bytes and reserves 71,724 stack bytes. It has not been installed.

1. Forebrain first qualifies the integrated hot-capable normal image, original
   owner client and current capacity. Preserve the prior qualified app and all
   existing durable archive prefixes/receipts; do not copy observer identities.
2. Coordinate one attended interruption. Read and privately retain fresh full
   flash in ROM without normal-booting it. Authenticate exact chip, Body/Root,
   credentials, partition map, current checkpoint and all custody cursors.
   The copied-889 results above cannot replace this fresh check.
3. Preflight cold-maintenance effects, including its existing observation
   `resume_pending` path. Do not silently reclaim an observation epoch; verify
   prior receipt/custody and exact recovered records or refuse the migration.
4. If approved, write only the qualified maintenance app and verify full readback
   and all non-app bytes before starting it. This is a hardware interruption,
   even though maintenance does not admit a new canonical Body Boot.
5. Using the original physical-owner key and independently pinned Root, use the
   existing biography-window/offer/data protocol. Verify the complete archive
   chain against the already retained prefix, then reopen/fsync exact records
   privately. Request a fresh unchanged-source challenge; only then may an
   authorized genuine owner ACK obtain Root's endorsed compaction receipt.
6. Require one-generation prefix rewrite and physical readback, unchanged Body,
   Root, original admissions, Clean/layout and observation history. Retain the
   genuine receipt separately. If that proof fails, do not discard or rewind
   either source or receiver custody.
7. Preserve fresh post-compaction flash without admitting a normal Boot. Decode
   it and qualify the exact next normal Boot/park path with the final app; check
   actual serialized margin rather than raising the unchanged 26,000-byte guard.
8. Only after coordination and qualification restore the app partition with the
   final normal image and full non-app readback. Verify restored identity, Boot,
   real sample/Mask counters, checkpoint bounds, saved Wi-Fi and later signed
   observer return. No second restart is implicit.

This staged cold pause is a first-migration option, not acceptance of cold pauses
as ongoing autonomous history custody. No reset, flash, genuine ACK, erase, new
observer or Body membership transaction was performed for this slice.

The combined developer checkout is additionally verified by
[combined-dev-73a00bc772.json](combined-dev-73a00bc772.json). The actual opt-in
`native_observed_cold` harness selected all four live-custody tests: three passed
normally, including 189 modeled NOR cuts, and the Boot-global STORE test passed
alone in a fresh process. This corrects an earlier invocation that selected zero
tests. These are CPU custody and refusal results; the native Root caller/fences,
phase timing and continuous physical sampler/Mask proof remain outstanding.
