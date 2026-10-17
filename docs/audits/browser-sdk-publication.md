# Browser SDK package publication

Development slice for #5510 / #5453. The SDK producer previously created its
public output directory before validating every asset. A late digest refusal
left a partial package that then blocked retry because the output already
existed. Production now uses an exclusive sibling staging directory and renames
it into place only after all assets, SDK helpers and metadata are ready. Failed
attempts clean their staging directory; an existing destination is refused.

Manifest bytes are retained from the bounded reads used for admission. Declared
assets are written from their verified byte buffers rather than reread by path
after hashing. Normal output-directory permissions and asset permissions remain
preserved. Identity, file-count, path and byte limits remain enforced.

The late-digest failure reproduced partial output before repair. The focused SDK
package fixture passes: no output or staging residue after refusal, successful
retry at that path, unchanged existing package on overwrite refusal, independent
hash checks for every packaged bundle file and normal directory permissions.
Its existing npm dry-run, packaged SDK import and static application identity/
closure checks also pass. These use tiny fixture Wasm bytes and prove producer
behavior, not a fresh optimized runtime, actual browser execution, historical
quoted-1kHz provenance or public release identity. Those acceptance claims remain
open.

Consumer-envelope follow-up: SDK packaging now requires the canonical runtime,
checks its declared size against the existing 20 MiB browser consumer ceiling,
and checks the verified bytes for the Wasm v1 header. Release paths must be
unique as well as contained in the IMAGE; otherwise duplicate entries could
omit a declared asset while satisfying the previous cardinality check. The
48 MiB total-package bound and other identity checks remain unchanged.

All six unusable envelopes were accepted before repair: oversized runtime,
truncated/wrong-magic/wrong-version header, absent runtime and duplicated release
path. The focused producer fixture now passes with exact-20-MiB acceptance and
independent byte-hash verification, refusal/no-output/no-staging checks for each
bad case, and the earlier retry/overwrite/permissions/composition checks. The
ceiling fixture is a header envelope, not executable Wasm/ABI proof; optimized
runtime, packaged browser and public release claims remain open.

Inventory-consistency follow-up: distribution files must match the IMAGE inventory
exactly, including order, as required by the browser SDK consumer. The producer
recomputes the distribution digest and compares IMAGE asset sizes, digests and
media types with the release inventory before staging output.

Eight contradictory inventories reproduced acceptance before repair: missing,
duplicated, size-changed or reordered distribution files, a false distribution
digest, and IMAGE/release size, digest or media-type disagreements. The focused
producer fixture now refuses all eight without output or staging residue and
retains successful packaging and the earlier envelope/publication checks. This
is producer fixture evidence; executable runtime, packaged browser, historical
source provenance and public release acceptance remain unproven.

Consumer-metadata follow-up: producer admission now checks the reviewed-
distribution schema, IMAGE runtime ABI and release package/output fields that
the browser SDK requires. All four incompatible variants were accepted before
repair despite correctly hashed payloads. The focused fixture now refuses each
without output or staging residue and retains the valid-package checks. These
metadata fixtures do not establish executable ABI or packaged browser proof.

Non-runtime bound follow-up: the SDK consumer caps each non-runtime payload at
256 KiB, while the producer previously admitted up to 48 MiB per asset. A
correctly hashed 256-KiB-plus-one module reproduced publication before repair.
The focused fixture now accepts exactly 256 KiB with independent hash verification
and refuses one byte over without output or staging residue. The existing
20 MiB runtime and 48 MiB total bounds remain intact. The padded module is an
envelope fixture, not executable module or packaged browser proof.

Boot-declaration follow-up: packaging requires the IMAGE's profile-gated boot
role, canonical boot path and digest to match its verified inventory asset,
mirroring browser Boot admission. Missing declarations, incorrect roles/paths
and stale digests all reproduced publication before repair, even with correctly
rehashed IMAGE and release payloads. The focused fixture now refuses all four
without output or staging residue; its valid fixture includes an exact boot
declaration. This establishes producer consistency, not actual Boot execution.
