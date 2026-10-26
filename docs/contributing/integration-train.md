# Integration train preparation

This is the development implementation of the integration train. Assembly and
capture change no GitHub refs; dispatch creates a guarded train ref and requests
proof. Landing updates development to an already proven exact source. Development
and publication can adopt that source's original proof/products. A real
end-to-end train acceptance remains pending.

Assemble explicitly selected ready PRs in a new local worktree:

```sh
cargo xtask ci pipeline train-assemble owner/repository /absolute/new-worktree /absolute/new-train.json PR_NUMBER...
```

Assembly checks live readiness first, fetches only the exact selected commits
and development base, and merges those heads on a deterministic local train
branch. It rechecks input readiness and base freshness before emitting a
manifest. The original worktree is preserved. Existing destinations and branch
names are never overwritten. A conflict or late stale-state refusal preserves
the new branch/worktree for diagnosis and does not emit a success manifest. No
retry, cleanup or merge-abort is performed implicitly. Keep the manifest outside
the candidate source worktree so it does not become an untracked source input.

From the clean candidate checkout, dispatch its exhaustive proof:

```sh
cargo xtask ci pipeline train-dispatch /absolute/train.json
```

Dispatch refreshes live readiness and reproduces local commit/tree custody. It
uploads the locally assembled commit with Git push and creates the exact remote
train ref under an empty expected-ref lease. Only a new-branch acknowledgement
grants dispatch ownership; an identical existing ref or creation race refuses.
It reads back the exact source, then passes the expected source to Integration,
whose guard refuses a mismatching source before expensive proof starts. Existing
queued, running or successful exact-source runs are reused. Failed runs and
changed refs refuse. An existing ref with no visible run is an ambiguous intent;
inspect that ref/run before operator recovery rather than automatically sending
another workflow request. A transport error preserves the ref as evidence.
The command reports a known reused run ID or an acknowledged dispatch with no
run ID yet; neither result claims that proof completed.

The dispatch carries the bounded manifest as a separate workflow input. Before
expensive proof, the guard independently reconstructs live admission and the
selected merge tree, retains the manifest as an Actions artifact, and supplies
its digest to every target producer. A source-only manual request cannot enter
the train route without this manifest.

From a clean checkout of the proposed combined source, capture explicitly
selected PRs through the supported pipeline entrance:

```sh
cargo xtask ci pipeline train-capture owner/repository SOURCE_SHA /absolute/new-train.json PR_NUMBER...
```

`SOURCE_SHA` must be the exact checked-out commit. Git must contain the current
development base and every selected head as ancestors of that commit. The
captured source tree must also equal Git's default conflict-free merge of the
selected heads in canonical PR-number order onto that base. Capture reconstructs
this tree using temporary Git objects, without changing refs, the index or the
working tree. Extra source changes and manual conflict resolutions refuse;
resolve those changes in a selected PR and prove the new selection instead.
Ancestry alone is insufficient admission. The command reads the live development
ref, selected PRs and their latest exact-head
Candidate runs using the existing `gh` authentication. It requires open,
non-draft, mergeable same-repository PRs targeting the captured development base
and exact completed Candidate run/attempt evidence. A successful Candidate
remains eligible. A bounded `native-deferred` tier may also admit a failed
Candidate when its exact-attempt complete job inventory proves preflight,
package admission and every selected portable/unit/host/non-native target
successful, with failures confined to the known ConduitOS product lanes and
their aggregate wrappers. An explicitly skipped unselected unit/target placeholder
is recorded; a missing or skipped selected job refuses. Unknown jobs, incomplete
matrices, cancelled/running jobs or changed attempts refuse. A newer failed or
running cheap-admission attempt cannot be replaced with older green evidence.
The v2 manifest binds exact run/attempt/head, every job ID/name/conclusion and
all deferred native failures into its digest. It never reports the input PR's
Candidate as successful or makes it directly mergeable. No ordinary Candidate
workflow or branch protection is weakened; only eligibility for full combined
proof changes. Every native product lane must still pass on the combined exact
source before landing. Missing or ambiguous evidence refuses. The bounded
manifest is created exclusively; an existing file is preserved. Its deterministic
train branch name binds the base, selected heads and exact admission receipts.
The live development ref is authoritative: GitHub PR metadata can retain an older
`base.sha` even when its current merge preview contains the new development tip.
An old metadata value does not replace or invalidate the captured live base.
Target branch/repository, actual Git ancestry/tree and fresh dev-ref checks still
must agree.

Once the exact source on that train branch has completed an exhaustive manual
Integration run, acquire its immutable archives into a new directory:

```sh
cargo xtask ci pipeline train-acquire /absolute/train.json INTEGRATION_RUN_ID /absolute/new-archives
```

Acquisition refreshes readiness, reconstructs the selected source tree, and reads
the actual successful run before and after downloading. It requires the complete
target archive set, unique artifact IDs, matching repository/source/run IDs,
unexpired archives and API SHA256 digests. Each exact artifact ID is downloaded
with streamed byte limits, then checked for size and digest. Existing outputs
refuse; failed evidence is retained. A changed run attempt refuses. Downloads are
bounded to 4 GiB per archive and 32 GiB total with at least 1 GiB initial disk
reserve. This initial path refuses artifact inventories above 100 entries rather
than silently ignoring a page.

`acquisition.json` binds the retained archives to the observed run and attempt.
It does not extract archives or authorize landing. Actions artifact metadata
lacks producer attempt identity: observing an unchanged attempt cannot prove an
older artifact was produced by that attempt.

Restore authenticated archives through the checked parser:

```sh
cargo xtask ci pipeline train-restore /absolute/train.json INTEGRATION_RUN_ID /absolute/archives /absolute/new-restoration
```

Restoration re-reads live metadata for every immutable artifact ID and verifies
the local ZIP hashes again. It accepts one named target tar per ZIP and only
regular files/directories beneath that target. Links, special files, traversal,
duplicate paths, malformed headers and oversized expansion refuse. It preserves
executable modes, enforces a disk reserve during writes, and checks the complete
restored product bundle through the existing receipt verifier.

New target products contain `producer.json`, sealed into the product file set.
It binds source/tree, workflow/source/run/attempt/job, observed tool versions,
verified tool acquisition, runner image/environment fingerprint and selected
command inventory. Restoration refuses legacy products without this custody or
products from another run attempt, even when the archive itself is authentic.
Environment values are fingerprinted without serializing credentials. Command
inventories bind source recipes, arguments and effective environment; they do
not claim individual test names or every nested rustc invocation were separately
observed. Full source/feature/test/environment policy acceptance and the
publication handoff still need an end-to-end real train proof.

For an already restored product bundle, verify its retained products:

```sh
cargo xtask ci pipeline train-verify /absolute/train.json INTEGRATION_RUN_ID /absolute/product-bundle
```

Verification checks the local source tree, refreshes live development and PR
state, requires the successful exact-source train-branch Integration run, and
uses the existing exhaustive target-set/file-size/hash/proof-class receipt
verifier. It reports source, tree and run identities only after those checks.
Successful run metadata without products is insufficient. Manifest digests are
integrity checks, not signatures or permission to change a protected branch.

Land the exact proven source with authenticated archives and a fresh evidence
directory:

```sh
cd /absolute/trusted-controller
cargo xtask ci pipeline train-land /absolute/train.json INTEGRATION_RUN_ID /absolute/archives /absolute/new-landing /absolute/reviewed-machine-policy.json CONDUIT_TRAIN_APP_JWT CONDUIT_TRAIN_POLICY_OBSERVER /absolute/proposed-source
```

Landing restores and validates all products, refreshes source/PR admission,
requires the latest exact-source Integration attempt and its successful required
`train-accepted` aggregate, and checks live protection and rules. Unknown required
checks/rules, any pull-request requirement (including zero required reviews),
or checks that exempt administrators refuse. It writes an exclusive intent
before a single non-forced ref update to the exact proven commit, then checks
the acknowledged source/tree and writes a separate landing receipt. A concurrent
development update outside that source cannot be fast-forwarded over. This is
a graph guard, not a strict compare-and-swap to the captured base.

Landing additionally requires the [dedicated machine writer contract](train-machine-writer.md):
an active exact-dev repository update ruleset whose sole bypass actor is a
reviewed dedicated Integration App, independent of the classic proof gate.
The external configuration and separate App JWT/installation/trusted observer credential channels
must be supplied explicitly. Missing real identities keep activation off.
Run the privileged command from a separately pinned trusted controller checkout;
the explicit proposed-source argument controls only Git and data operations.
The proposed source must
never provide the executed controller or build its tooling with these credentials.

There is no forced update, protection override, PR-merge fallback or automatic
mutation retry. Existing evidence and ambiguous outcomes require inspection.
The existing dev policy requires a PR and exempts administrators; that policy
cannot authorize this direct landing path. Before live train landing, an
explicit branch-policy migration must require the distinct Actions-owned
`train-accepted` check, require its enforcement for administrators, and replace the
PR requirement with exact train admission. Exact-proof protection alone does not
establish exclusive machine writer authority; the separate update ruleset above
must also be installed and verified. This command refuses the old policy
and does not change it itself. A server rejection remains terminal. A returned landing receipt is not stable publication.

Integration now earns the protected `train-accepted` context from its exhaustive
proof. The aggregate runs even after a guard refusal or cancelled dependency and
accepts only successful exhaustive proof or authenticated exact-source train
adoption. Ordinary Candidate success cannot satisfy this context. This
does not authenticate a caller-provided product bundle. Landing requires the
archive authentication, producer custody and live checks above; local receipt
hashes alone cannot authorize it. Full train acceptance and publication reuse
remain distinct from fixture tests of these adapters.

After landing, the dev Integration job looks for the latest manual train proof
of its exact source. It authenticates the original manifest archive, reconstructs
the source tree, verifies successful exhaustive proof/`train-accepted` aggregation and
checks accepted development ancestry. With valid adoption it skips a duplicate
exhaustive build. With no manual train it runs the ordinary complete proof. A
failed, malformed or stale train refuses; it cannot silently become a green
adoption or fall back over a known failed proof.

Publication still requires a successful dev push Integration. Its supported
entrance is `cargo xtask ci pipeline publish DEV_INTEGRATION_RUN_ID`. The
publisher independently acquires and safely restores the selected run's exact
archives before promoting anything: original manual train products for adopted
sources, or actual dev products otherwise. Local bundles cannot substitute for
those archives. Producer source/tree/workflow/run/attempt/digest, tool acquisition,
environment and command custody are checked again. No build/test is run in this
handoff. Release manifests keep original proof and development acceptance
identities distinct; the accepted main tree must still equal the proven tree.
Before any release branch, status, main merge, tag or release mutation, the
publisher composes the authenticated browser site and exact-source ConduitOS
journey in a fresh directory and checks all local links. A composition or link
failure retains the output for diagnosis and refuses promotion. Only the
verified directory is exposed as `pages-site` for the subsequent Pages upload.


Input observations use the attempt-specific jobs API, bounded to 100 jobs and
100 run records with exact total-count equality. Run/attempt/head and latest-run
state are rechecked after jobs are read; assembly, dispatch, acquisition,
restoration and landing reobserve and compare canonical admission custody.
Neither a fresh rerun nor a newly created run silently substitutes for the
manifest's original admission. Oversized custody refuses before assembly
creates a worktree; the manifest retains its 64 KiB bound without truncation.
Historical v1 train manifests are refused explicitly and must be recaptured as
v2 rather than relabelled. Global cheap PR admission still requires a separate
enforced machine-only landing policy; this bounded train-input tier does not
activate it.

CI-only changes to the exact standard-library Rust workflow contract and Python
train restorer use preflight proof instead of compiling the full xtask dependency
graph twice. Preflight executes the complete canonical Node corpus once,
independently of its Cargo wrapper. It then compiles the unchanged real Rust
launcher and executes it against bounded real passing and failing Node tests.
The private native launcher proxy verifies the complete canonical argument set,
working directory, exactly one invocation and failure propagation. Missing or
changed arguments, a skipped launch or ignored failure refuse. This verifies
the actual Rust launcher without replaying all semantic tests. Ordinary Cargo
execution of that Rust test still runs the complete Node corpus. Private outputs
and time bounds remain enforced, and editing the wrapper cannot remove the
independent full-corpus proof. Other Rust/Python paths keep normal conservative package admission.
The prior hosted run recorded230seconds of lint compilation and564seconds of
test compilation for these two tooling entrances; future hosted timings must
measure the improvement rather than assume those costs disappear.

Linux hosted compiler objects travel in separate, bounded seed artifacts. They
never replace a product archive, task target directory, or current-source Hello
proof. Preparation writes a fresh external configuration and snapshot. Seed
selection examines at most eight successful dev Integration acceptances, verifies
current dev ancestry, and delegates adopted train custody to the same exact-tree
adoption verifier used for products. The selected original producer's exact run
attempt, artifact API digest, namespace policy, inventory and object hashes must
all match. Missing or expired compatible seeds give an explicit cold build;
invalid selected custody refuses instead of silently choosing another producer.

The Linux target uses private build/proof outputs and seals only compiler objects
after successful current-source proof. A producer may upload its seed before its
whole Integration finishes; discovery admits it only after exhaustive successful
dev acceptance. Candidate and unaccepted manual train artifacts are ineligible.
The workflow requests only read access for acquisition and uploads a separate
`compiler-cache-hosted-linux-*` artifact with fourteen-day retention. There is no
shared Cargo target restore for this lane.

`compiler-cache-hosted-retire` requires explicit artifact IDs and verifies each
managed namespace and producer before deletion, records durable intent, and
requires absent readback. It does not claim to clear the whole namespace or all
GitHub artifacts. Local audited cache clearing remains separate and preserves
private task outputs. Remote transfer, snapshot preparation and full-history
checkout costs must be included in hosted performance measurements; local cache
hits alone do not prove a net pipeline speed improvement.

The exact `tools/ci/pipeline/hosted-cache-seed.py` helper is also covered by
preflight rather than rebuilding xtask's Rust dependency graph. The seed corpus
executes that real Python helper for archive sealing/extraction and malformed
archive refusals. This exception does not cover adjacent Python files, compiler
profile JSON/TOML, product Rust, or runtime dependencies.
