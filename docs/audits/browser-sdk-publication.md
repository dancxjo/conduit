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

Bundle-build graph follow-up: reviewed module declarations must be unique,
each dependency list must fit the SDK loader's existing 16-entry bound, every
dependency must name a declared module, and the graph must be acyclic. Admission
uses active/completed visitation over at most 32 modules and 16 edges per module;
shared dependencies remain valid. Refusals distinguish invalid declarations,
missing dependencies and cycles before emitting an IMAGE.

Self-cycles, two-module cycles, duplicate declarations, dependencies on an asset
without a module declaration and oversized dependency lists all reproduced
acceptance before repair. Three focused builder tests now pass, including all
five negative graphs and a positive shared-dependency graph, using the pinned
Playwright runner with one worker and zero retries. These tests call production
builder code in Node; they do not exercise browser module imports, establish
public provenance or close #5453/#5510.

Distribution aggregate/order follow-up: BUILD now recomputes the declared
distribution digest from its ordered inventory and refuses duplicate or
unbounded declarations before constructing an IMAGE. Verified assets are emitted
in manifest order, so transport payload order cannot produce an IMAGE inventory
that the SDK consumer refuses for disagreeing with the original distribution.

A false aggregate digest and duplicate inventory entry both reproduced BUILD
acceptance before repair. All four focused Node-side builder tests pass through
the pinned Playwright runner (one worker, zero retries), including reordered
payload transport with exact manifest/IMAGE/release inventory agreement. These
are local content-admission fixtures, not public package/source corroboration
or browser execution proof.

Independent IMAGE-verification follow-up: recomputing a self-consistent IMAGE
hash is insufficient to bind its claims to the caller's inputs. Verification
now verifies distribution payloads and reconstructs the expected IMAGE payload
from the checked configuration, selected implementations and reviewed
distribution. BUILD and verification share the exact payload construction;
existing implementation and Boot refusals retain their distinct codes.

Six rehashed forgeries reproduced acceptance before repair: changed PROFILE,
configuration, BuildId, source commit, distribution digest and file size. Tests
recompute the IMAGE id, binding hash and release aggregate so stale outer hashes
cannot account for the refusal. All five focused Node-side builder/verifier
tests pass through the pinned runner with one worker and zero retries. This
proves these input-binding checks, not public provenance, browser execution or
complete hostile-release admission.

Release-payload binding follow-up: verification requires the release file
inventory to match its payload sizes, digests and media types, then binds each
non-IMAGE payload identity to the verified distribution. A self-consistent
release aggregate cannot authorize replacing the IMAGE's declared runtime.

Five variants reproduced acceptance before repair: replaced/rehashed runtime,
changed payload media type, false declared size, false declared digest and an
absent release inventory. All six focused Node-side builder/verifier tests now
pass through the pinned runner (one worker, zero retries); these five variants
refuse with `ArtifactBindingMismatch`. This proves the tested content bindings,
not runtime execution, publisher authority or public release provenance.

Builder consumer-bound follow-up: manifest admission now checks positive safe
integer asset sizes against the existing browser consumer limits (20 MiB runtime,
256 KiB other assets) before hashing or copying distribution payloads. A declared
large bundle budget no longer permits individually unusable assets.

Empty runtime and one-byte-over runtime/module envelopes all reproduced BUILD
acceptance before repair. Seven focused Node-side builder/verifier tests pass
through the pinned runner (one worker, zero retries), including exact-boundary
acceptance and all three refusals. Padded/header payloads establish envelope
bounds, not executable module, runtime ABI or packaged browser proof.
