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
creates the exact remote train ref and passes the expected source to Integration,
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
and successful completed Candidate evidence. A newer failed or running attempt
cannot be replaced with an older green attempt. Missing or ambiguous evidence
refuses. The bounded manifest is created exclusively; an existing file is
preserved. Its deterministic train branch name binds the base and selected heads.
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
cargo xtask ci pipeline train-land /absolute/train.json INTEGRATION_RUN_ID /absolute/archives /absolute/new-landing
```

Landing restores and validates all products, refreshes source/PR admission,
requires the latest exact-source Integration attempt and its successful required
Candidate aggregate, and checks live protection and rules. Unknown required
checks/rules or human-review requirements refuse. It writes an exclusive intent
before a single non-forced ref update to the exact proven commit, then checks
the acknowledged source/tree and writes a separate landing receipt. A concurrent
development update outside that source cannot be fast-forwarded over. This is
a graph guard, not a strict compare-and-swap to the captured base.

There is no forced update, protection override, PR-merge fallback or automatic
mutation retry. Existing evidence and ambiguous outcomes require inspection.
GitHub may reject a direct update under a pull-request requirement, including a
zero-review requirement. Such a rejection is terminal; this command does not
change branch policy. A returned landing receipt is not stable publication.

Integration now earns the protected `candidate` context from its exhaustive
proof. The aggregate runs even after a guard refusal or cancelled dependency and
accepts only a successful proof; skipped proof cannot satisfy that context. This
does not authenticate a caller-provided product bundle. Landing requires the
archive authentication, producer custody and live checks above; local receipt
hashes alone cannot authorize it. Full train acceptance and publication reuse
remain distinct from fixture tests of these adapters.

After landing, the dev Integration job looks for the latest manual train proof
of its exact source. It authenticates the original manifest archive, reconstructs
the source tree, verifies successful exhaustive proof/Candidate aggregation and
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
Pages composition uses the authenticated bundle returned by publication.
