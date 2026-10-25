# Historical evidence externalization

The [immutable historical shipment](https://github.com/dancxjo/conduit-evidence/releases/tag/historical-evidence-20261025-ced59408ceab)
retains the exact original bytes of 36 generated Conduit evidence objects outside
source Git history. It contains 598,444,017 payload bytes plus complete frozen
rehearsal ref/commit/tree maps and verification metadata.

The 600,688,640-byte tar has SHA256
`ced59408ceabc5dc7032aee630285fa95a96c9845df767e4d3a30c1161665b35`.
The accompanying `manifest.json` has SHA256
`296e0a56aefd9abfd081a709a5252da16b5a9286fc0e9c5258796c4f297236ba`.
It records original Git blob SHA1, SHA256, size, historical paths and change
commits. The release also retains checksums and a standalone verifier.

All five uploaded release assets were downloaded afresh and matched server
sizes/digests. Independent verification checked every original blob and all
eleven metadata members without the original Git repository. Publication was
then observed with `immutable: true`, retaining the same asset IDs and digests.
Release ID: `423603093`; repository: `dancxjo/conduit-evidence`.

Download the assets into a new directory, verify `SHA256SUMS`, and run
`python3 verify-export.py DIRECTORY`. The verifier reads fixed archive members
without extracting arbitrary paths. Independent original Git backup remains
necessary against repository/account loss.

This shipment does not assert an origin history cutover. Its maps describe the
rehearsal snapshot; current refs must be captured again and final mappings and
clone/integrity proofs verified after integration. Original proof identities
remain original identities. Rewritten commits are not claimed to have produced
historical evidence.
