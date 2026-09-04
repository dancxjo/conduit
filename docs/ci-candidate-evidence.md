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

## Reconciliation

`cargo xtask ci reconcile BASE HEAD --receipt RECEIPT...` asks Git for the prospective integration tree with `git merge-tree --write-tree`. It never rebases or mutates `HEAD`.

- A structural conflict reports `candidate_evidence_status` separately from `integration_status: conflict` and schedules no expensive proof.
- A clean merge fingerprints registered proofs directly from the prospective tree.
- An exact successful receipt with the same proof key is `inherited`.
- Only novel proof keys are `execute`.

For example, B1 can retain `browser.tour` evidence after an ESP32-only A1 merges. If A1 changes a browser runtime input, only browser-related keys change; the ESP32 receipt remains inheritable.

## GitHub workflow boundary

Pull-request validation checks out `github.event.pull_request.head.sha` explicitly. Candidate concurrency includes both PR lifecycle and head identity, so unrelated PRs cannot cancel one another and duplicate base movement cannot destroy candidate work. The stable externally required `check` job remains unchanged.

Merge-group validation identifies its checkout as integration rather than candidate. The privileged `pull_request_target` Pages workflow remains separate and executes only trusted merged workflow machinery; it promotes an already-proven, source-tree-sealed carrier.

The first registry slice is intentionally broad and conservative. It proves the identity, receipt, and reconciliation mechanism for workspace products, Tour browser proof, and ESP32-C3. Subsequent work can split nodes, teach merge-group orchestration to retrieve retained candidate receipts, model fabricated artifacts as independent graph nodes, remove duplicated path-filtered workflows, batch shared browser/QEMU environments, and make Crèche payload delivery lazy without changing this identity contract.
