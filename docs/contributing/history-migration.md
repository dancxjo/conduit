# Preserve historical capture identity during a source-history migration

Historical evidence names the original commit that produced it. Keep those
capture IDs and proof classes unchanged. A rewritten commit did not execute
that historical proof. Live train admission continues to require actual Git
ancestry; it does not use this historical capture bridge.

`requireCaptureLineage` defaults to the existing complete ancestry check. A
deliberately committed `docs/audits/history-capture-lineage.json` can authorize
the historical relation after a source-preserving migration. The compact policy
pins immutable full-map and original-source-backup asset identities, the full
map's Merkle root, and original capture/parent inclusion witnesses. Original raw
commit bytes must hash to the original capture ID. Their mapped commit must
preserve the entire source tree, authorship, timestamps, message and parent
order, translating only the parent IDs through the same authenticated map.
Original commit signatures are retained in the witness/backup and removed from
the translated comparison; they are never claimed to sign rewritten history.

The mapped source must be an actual ancestor of publication in a complete
checkout. Shallow history, grafts, replacement tricks, missing objects, dirty or
symlinked policies, changed source trees and mismatched map witnesses refuse.
Unknown captures still require actual ancestry; a map cannot make them silently
pass. The policy checkout must equal publication HEAD. Returned custody keeps
`captureSource`, `mappedCaptureSource` and `publicationSource` distinct.

Prepare a policy outside both repositories through the supported entrance:

```sh
cargo xtask ci pipeline history-lineage-create /absolute/original.git /absolute/rewritten.git /absolute/commit-map /absolute/captures.json /absolute/custody.json /absolute/new-policy.json
```

The capture inventory is an explicit JSON array of original full commit IDs.
The map is the complete `git-filter-repo` `old new` commit map; dropped commits
are refused. `custody.json` supplies `mapAsset` and `originalBackup`, each with
`releaseId`, `assetId`, `url`, `sha256` and `bytes`. Preparation reads the actual
GitHub release/asset metadata and requires published immutable releases and
matching exact asset identities. It checks local map bytes against that digest,
constructs Merkle witnesses and verifies the actual original/mapped objects.
It emits a fresh separate file and does not install a policy or change refs.

The committed policy's Merkle root is the runtime trust anchor. Its relationship
to the full map and immutable custody is established during preparation and
review. Runtime checks verify inclusion locally; they do not fetch a mutable
map or claim to verify a cryptographic release attestation.

Before committing a real policy or updating source refs, independently verify
the final frozen remote/local union backup, complete ref/commit maps, unchanged
unselected source/data, the exact reviewed generated-object removals, and all
original capture root trees. Use a final filter that preserves commit-message
hash literals and topology. Re-run the capture inventory against the final
source. A historical rehearsal is insufficient for final cutover.

Run preflight, retained-evidence/site validation and train proof in a fresh
rewritten clone with the proposed policy. Preserve original source backups,
payload exports, immutable custody, final maps and refusal evidence. The policy
does not replace those global migration checks, authorize a force push, or make
the rewritten source inherit historical runtime acceptance.

The original documentary website archive is preserved independently in the
[immutable documentary-history release](https://github.com/dancxjo/conduit-evidence/releases/tag/documentary-history-20261025-0218a5040676)
(release423665080, archive asset676137304). Site assembly reads that archive
with the unchanged SHA256
`0218a50406761390aba1f00f84ccd178018782dfa89527f293d8ac66b9aa76c2`,
without depending on the old source commit remaining downloadable. Its5,581,556
bytes match the previously pinned codeload archive; all295 journey files match
the original commit's blob bytes and modes. The release manifest records that
inventory, and all three release assets passed a fresh download/hash check
before publication and immutable-state observation. Site custody retains the
original capture commit and archive identity; this is no new execution proof.
