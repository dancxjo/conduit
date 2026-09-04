# Candidate evidence and integration reconciliation

Conduit CI distinguishes identities that GitHub's default pull-request checkout can make look interchangeable.

```text
PR lifecycle     review, reporting, cancellation namespace
candidate        exact immutable pull-request head commit
integration      prospective composition with a current target base
proof            versioned proposition over exact relevant inputs
receipt          machine-readable result for one exact proof key
artifact         content-addressed object consumed or produced by proof
```

Candidate evidence is immutable. Advancing a PR from B1 to B2 creates a new candidate, but it does not make a successful B1 receipt false. Moving `main` changes the integration question, not candidate history.

Candidate impact uses the unique `merge-base(requested base, candidate)..candidate`
change set. The requested base tip and effective comparison base are reported
separately in the machine-readable plan. Thus commits added only to a moving
`main` never appear as reverse candidate changes. If Git cannot establish one
unambiguous merge base, planning fails closed to the conservative suite.

A bounded extraction recognizer handles the common refactor that moves a final
inline `#[cfg(test)] mod tests { ... }` into a newly added `tests.rs`. It proves
that the entire non-test parent source is byte-identical, and that the moved
test source differs only by indentation, formatting whitespace, or optional
trailing commas. Location-sensitive or unknown macros, raw strings, changed
production tokens, extra paths, and structural ambiguity refuse the shortcut.
Accepted extraction-only candidates run the affected workspace lint/tests but
inherit product, browser, firmware, and ConduitOS proof domains.

## Proof keys

`cargo xtask ci candidate HEAD` reads registered proof specifications and fingerprints their actual Git blobs from `HEAD^{tree}`. Each proof key includes:

- proof ID and contract version;
- relevant source and proof-implementation Git objects;
- the declared runner/toolchain environment class;
- exact consumed artifact digests when the proof declares artifacts.

It deliberately excludes PR number, base SHA, workflow run ID, cache identity, and GitHub synthetic merge SHA. The JSON plan and Markdown summary report each proof as `execute` or `inherited` and explain why.

After a registered proof command succeeds, CI may run:

```text
cargo xtask ci attest-success HEAD PROOF_ID \
  --evidence workflow:NAME \
  --evidence job:NAME \
  --out target/ci-evidence/PROOF_ID.json
```

The canonical receipt has schema `conduit.ci.proof-receipt/v1`. Attestation is a post-proof operation; it does not run or replace the proof. Candidate SHA and source tree are provenance. Equivalence is determined by the proof key.

Receipt reuse fails closed. Unknown schemas, incomplete results, wrong IDs or contract versions, changed input/proof/environment digests, missing artifact identities, and malformed evidence all produce `execute`.

Every registered source and proof-implementation root must resolve in the
analyzed Git tree. A repository ownership move therefore updates the registry
and bumps the affected proof contract atomically. A stale path is a planning
error, never an empty input set that can accidentally inherit an old receipt.

## Reconciliation

`cargo xtask ci reconcile BASE HEAD --receipt RECEIPT...` asks Git for the prospective integration tree with `git merge-tree --write-tree`. It never rebases or mutates `HEAD`.

- A structural conflict reports `candidate_evidence_status` separately from `integration_status: conflict` and schedules no expensive proof.
- A clean merge fingerprints registered proofs directly from the prospective tree.
- An exact successful receipt with the same proof key is `inherited`.
- Only novel proof keys are `execute`.

For example, B1 can retain `browser.tour` evidence after an ESP32-only A1 merges. If A1 changes a browser runtime input, only browser-related keys change; the ESP32 receipt remains inheritable.

## GitHub workflow boundary

Pull-request validation checks out `github.event.pull_request.head.sha` explicitly. Candidate concurrency includes both PR lifecycle and head identity, so unrelated PRs cannot cancel one another and duplicate base movement cannot destroy candidate work. The stable externally required `check` job remains unchanged.

The privileged retirement controller re-resolves the pull request's current
head before it lists candidate runs. A delayed synchronize event for B1 becomes
inert after the PR advances to B2, so only B2 may retire B1; event arrival order
can never reverse that relationship.

When an unchanged candidate needs policy from a newer trusted controller, the
`reconcile-candidate` workflow is requested explicitly with the PR number and
exact current head. Its trusted resolver refuses a stale or closed lifecycle.
It inherits any already-successful `check` or `products-proof` aggregate
attached to that immutable candidate and invokes the current reusable workflow
only for a missing or failed lane. Candidate execution has read-only
permissions; a separate trusted reporter publishes the stable aggregate names
onto the exact candidate SHA. Repeating the same request therefore inherits the
newly published successes rather than replaying work. This replaces empty
commits, policy-only rebases, and close/reopen choreography.

The `book-and-creche-products` workflow follows the same controller model. It
triggers cheaply for every pull request, checks out the immutable candidate and
the current trusted default-branch controller as separate trees, and asks the
typed product proof registry whether the Pages carrier is affected. Unrelated changes retain the
stable `products-proof` result without starting browser, desktop, firmware, or
ConduitOS fabrication. Product ownership therefore lives in Rust rather than a
second workflow path taxonomy. The privileged post-merge Pages workflow stays
separate and executes trusted merged code only.

Merge-group validation identifies its checkout as integration rather than candidate. The privileged `pull_request_target` Pages workflow remains separate and executes only trusted merged workflow machinery. A downloaded carrier is verified against the candidate commit and tree that actually fabricated it. `cargo xtask ci reconcile-product products.pages-carrier CANDIDATE INTEGRATION` compares those two Git trees through the typed product registry. Unrelated tree movement is inherited. When a carrier input changed, the deploy controller calls the unprivileged product workflow on the exact merged commit and admits that newly proven integration carrier instead. Candidate provenance is never rewritten to claim that old bytes came from the later merged tree.

The repository's `cargo xtask` alias first enters the dependency-light
`conduit-xtask-dispatch`. The CI identity operations (`plan`, `candidate`,
`reconcile`, `reconcile-product`, and `attest-success`) use the same typed Rust sources there. At
this migration point its default graph has 35 normal dependency packages,
compared with 265 for full `xtask`; browser Host fabrication is compiled only
for the separate on-demand Host-release feature.

For pull requests, both controllers resolve the current default-branch head once
and materialize that exact commit outside the candidate workspace. Actions
compile that trusted controller in its own temporary Cargo target and run it
from the candidate worktree, so candidate cache/artifact scans cannot traverse
the controller. The PR target SHA remains the requested integration context,
but it does not double as either the candidate comparison base or policy
version. This matters both when `main` advances around an unchanged candidate
and for stacked PRs whose target branch may predate current CI policy.
Controller implementation, requested base, effective candidate comparison
base, and analyzed candidate are therefore separate explicit identities. No
path executes candidate code with privileged credentials.

Repository-policy preflights use that same trusted controller executable while
keeping the candidate checkout as their working tree. A candidate predating a
new CI command can therefore be validated without rebasing merely to acquire
the command implementation; the preflight still inspects that candidate's
exact manifests and locks.

The first registry slice is intentionally broad and conservative. It proves the identity, receipt, and reconciliation mechanism for workspace products, Tour browser proof, and ESP32-C3. Subsequent work can split nodes, teach merge-group orchestration to retrieve retained candidate receipts, model fabricated artifacts as independent graph nodes, remove duplicated path-filtered workflows, batch shared browser/QEMU environments, and make Crèche payload delivery lazy without changing this identity contract.

Product workflow dependencies follow consumed artifacts rather than the final
deployment bundle. The Tour and its embedded real Patchbay consume the browser
Host and Tour runtime, so that proof may start as soon as those exact artifacts
exist. It does not wait for desktop, firmware, or ConduitOS fabrication. The
Crèche machine proofs and final Pages carrier still wait for the complete staged
catalog because they inspect or publish those payloads. The stable
`products-proof` gate joins the distinct results without making the early proof
depend on the later deployment barrier.

## Exact intra-run artifact transport

Candidate-capable workflows name artifacts with `CONDUIT_CHECKOUT_SHA`, never
the synthetic pull-request merge SHA or a workflow run ID. Producers expose the
digest returned by `actions/upload-artifact`; consumers require that exact
digest.

The repository composite downloader permits at most three transport attempts.
It resolves one live exact name inside the exact producer run, downloads the
raw archive, verifies the producer's exact SHA-256, and only then extracts it.
Only network errors, 429/5xx responses, and an explicitly identified
intermediary 403 are retryable. Authorization
failure during producer lookup, a missing or ambiguous artifact, malformed
identity, and digest mismatch fail immediately. The machine-readable transport
record keeps a retry distinct from proof success, and no fallback rebuild can
manufacture supposedly equivalent bytes.
