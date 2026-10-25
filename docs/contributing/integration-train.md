# Integration train preparation

This is the development assembly/admission/evidence layer for the integration
train redesign. Workflow dispatch, landing and publication reuse are still
pending. These commands do not change GitHub refs or merge GitHub PRs.

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

From a clean checkout of the proposed combined source, capture explicitly
selected PRs through the supported pipeline entrance:

```sh
cargo xtask ci pipeline train-capture owner/repository SOURCE_SHA /absolute/new-train.json PR_NUMBER...
```

`SOURCE_SHA` must be the exact checked-out commit. Git must contain the current
development base and every selected head as ancestors of that commit. The
command reads the live development ref, selected PRs and their latest exact-head
Candidate runs using the existing `gh` authentication. It requires open,
non-draft, mergeable same-repository PRs targeting the captured development base
and successful completed Candidate evidence. A newer failed or running attempt
cannot be replaced with an older green attempt. Missing or ambiguous evidence
refuses. The bounded manifest is created exclusively; an existing file is
preserved. Its deterministic train branch name binds the base and selected heads.

Once the exact source on that train branch has completed an exhaustive manual
Integration run, verify its retained products:

```sh
cargo xtask ci pipeline train-verify /absolute/train.json INTEGRATION_RUN_ID /absolute/product-bundle
```

Verification checks the local source tree, refreshes live development and PR
state, requires the successful exact-source train-branch Integration run, and
uses the existing exhaustive target-set/file-size/hash/proof-class receipt
verifier. It reports source, tree and run identities only after those checks.
Successful run metadata without products is insufficient. Manifest digests are
integrity checks, not signatures or permission to change a protected branch.

A future landing adapter must refresh state at its mutation boundary and use a
race-safe operation that preserves the tested tree. These observational results
are not a compare-and-swap, merge authorization or publication receipt. The
current development and stable publication requirements remain in force.
