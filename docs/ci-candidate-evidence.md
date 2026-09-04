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

The `book-and-creche-products` workflow follows the same controller model. It
triggers cheaply for every pull request, checks out the immutable candidate and
the current trusted default-branch controller as separate trees, and asks the
typed product proof registry whether the Pages carrier is affected. Unrelated changes retain the
stable `products-proof` result without starting browser, desktop, firmware, or
ConduitOS fabrication. Product ownership therefore lives in Rust rather than a
second workflow path taxonomy. The privileged post-merge Pages workflow stays
separate and executes trusted merged code only.

Merge-group validation identifies its checkout as integration rather than candidate. The privileged `pull_request_target` Pages workflow remains separate and executes only trusted merged workflow machinery; it promotes an already-proven, source-tree-sealed carrier.

The repository's `cargo xtask` alias first enters the dependency-light
`conduit-xtask-dispatch`. All four CI identity operations (`plan`, `candidate`,
`reconcile`, and `attest-success`) use the same typed Rust sources there. At
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

The first registry slice is intentionally broad and conservative. It proves the identity, receipt, and reconciliation mechanism for workspace products, Tour browser proof, and ESP32-C3. Subsequent work can split nodes, teach merge-group orchestration to retrieve retained candidate receipts, model fabricated artifacts as independent graph nodes, remove duplicated path-filtered workflows, batch shared browser/QEMU environments, and make Crèche payload delivery lazy without changing this identity contract.

Product workflow dependencies follow consumed artifacts rather than the final
deployment bundle. The Tour and its embedded real Patchbay consume the browser
Host and Tour runtime, so that proof may start as soon as those exact artifacts
exist. It does not wait for desktop, firmware, or ConduitOS fabrication. The
Crèche machine proofs and final Pages carrier still wait for the complete staged
catalog because they inspect or publish those payloads. The stable
`products-proof` gate joins the distinct results without making the early proof
depend on the later deployment barrier.
