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

## Deep single-owner stacks

The trusted default-branch controller classifies open same-repository pull
requests by their exact head and base branch identities. A chain is collapsible
only when every edge is unique and every PR has the same GitHub owner. Forks,
duplicate head refs, siblings, owner boundaries, cycles, stale events, and
unknown API results fail closed to ordinary independent candidate proof.

Within a valid chain, only the immutable cumulative tip schedules the expensive
workspace, browser, product, firmware, and ConduitOS candidate graphs. Every
intermediate PR still receives a stable `check` and `products-proof` result,
but those runs retain only exact head/base provenance, the complete ordered PR
chain, the cumulative tip identity, and `git diff --check` slice evidence. They
do not install Rust, compile the impact planner, fabricate products, or start
machine proofs.

This changes proof scheduling, not history. Each intermediate PR, commit,
author, issue link, and review conversation remains intact. The cumulative tip
proves the composed tree. Intermediate PRs may be retired only after admission
establishes that their content is present in the integrated result.

## Exact generic Rust toolchain

`rust-toolchain.toml` owns the reviewed generic Rust compiler, Clippy, and
rustfmt identity used by repository development and CI. Generic Actions setup
and any explicit Cargo toolchain selector name that exact version; neither may
resolve the moving `stable` channel independently. Target-specific toolchains,
such as the separately named ESP Xtensa installation, remain distinct.

The exact generic version participates in the proof registry environment
identity. Updating it is therefore an intentional proof-contract change: the
affected proof keys change and current lint/test evidence is regenerated. A
candidate cannot acquire a new warning merely because the upstream `stable`
pointer moved between local proof and GitHub execution.

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

`cargo xtask ci candidate HEAD --impact-plan PLAN` reads registered proof specifications, selects the exact proofs applicable to the candidate, and fingerprints their actual Git blobs from `HEAD^{tree}`. A missing or malformed retained applicability plan conservatively selects the complete registry. Each proof key includes:

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

`cargo xtask ci reconcile BASE HEAD --impact-plan PLAN --receipt RECEIPT...` asks Git for the prospective integration tree with `git merge-tree --write-tree`. The retained candidate impact plan distinguishes a proof that was intentionally inapplicable from missing evidence for an applicable proof. It never rebases or mutates `HEAD`.

`cargo xtask ci integration BASE HEAD` owns the lower-level composition
decision. It first uses Git's genealogical merge base. If that reports a
conflict, the controller may retry only when a direct candidate parent's exact
Git tree ID equals a commit tree reachable from `BASE`. That reachable commit
becomes the explicit effective merge base, preserving a stacked candidate
after its parent was squash-merged without treating patch similarity as
identity. The JSON and job summary report the effective commit, tree, and
selection method. No exact tree match, or a conflict after the exact retry,
remains a structural conflict and fails closed before proof jobs start.

- A structural conflict reports `candidate_evidence_status` separately from `integration_status: conflict` and schedules no expensive proof.
- A clean merge fingerprints registered proofs directly from the prospective tree.
- An exact successful receipt with the same proof key is `inherited`.
- Only novel proof keys are `execute`.

For example, B1 can retain `browser.tour` evidence after an ESP32-only A1 merges. If A1 changes a browser runtime input, only browser-related keys change; the ESP32 receipt remains inheritable.

## GitHub workflow boundary

The authoritative impact, candidate-identity, receipt, integration, product
reconciliation, toolchain, and bounded-monitor contracts are compiled into the
internal `conduit-xtask-dispatch` target from the same Rust source files used by
`cargo xtask ci`. `cargo test --locked --package conduit-xtask-dispatch` runs
that controller suite with only Git/planning dependencies; it does not compile
ConduitOS, browser runtimes, firmware, semantic products, or fabrication
packages. Controller-changing PRs run this small suite in the classifier before
fan-out. `cargo xtask` remains the documented repository entrance.

Pull-request validation checks out `github.event.pull_request.head.sha` explicitly. Candidate concurrency includes both PR lifecycle and head identity, so unrelated PRs cannot cancel one another and duplicate base movement cannot destroy candidate work. Candidate workflows retain `check` and `products-proof` as immutable evidence lanes. Branch protection uses the distinct reconciliation-owned `admission` gate. This separation is necessary because GitHub preserves a failed native workflow check in the commit rollup even after a newer same-name Checks API result succeeds; reusing a native lane name cannot unambiguously replace stale admission state.

The privileged retirement controller re-resolves the pull request's current
head before it lists candidate runs. A delayed synchronize event for B1 becomes
inert after the PR advances to B2, so only B2 may retire B1; event arrival order
can never reverse that relationship.

When an unchanged candidate needs policy from a newer trusted controller, the
`reconcile-candidate` workflow is requested explicitly with the PR number and
exact current head. Its trusted resolver refuses a stale or closed lifecycle.
It uses an already-successful `check` or `products-proof` aggregate attached to
that immutable candidate only as a location for immutable receipt artifacts.
The aggregate name is never proof equivalence. The trusted current-main
controller verifies each recognized receipt against the prospective integration
tree and the retained candidate applicability plan. A lane inherits only when
every applicable proof disposition in that lane is `inherited`; missing,
malformed, ambiguous, or unknown evidence produces fresh execution. The resolver freezes the current prospective
integration commit separately from the candidate head; novel lanes execute
against that integration commit, while inherited candidate receipts remain
historical evidence about the immutable head. A main-only repair therefore
participates in integration proof without being copied into every PR branch.
Integration execution has read-only
permissions; a separate trusted reporter publishes one aggregate `admission`
decision onto the exact candidate SHA and records the base and prospective
integration identities in its summary. Repeating the same request therefore
inherits the underlying lane successes rather than replaying work. This replaces empty
commits, policy-only rebases, and close/reopen choreography.
The controller publishes `admission` only after both lanes succeed or inherit
exact success. A refusal fails the reconciliation run without creating a
sticky failed admission check; absence of the required admission is the
fail-closed state.

The `book-and-creche-products` workflow follows the same controller model. It
triggers cheaply for every pull request, checks out the immutable candidate and
the current trusted default-branch controller as separate trees, and asks the
typed product proof registry whether the Pages carrier is affected. Unrelated changes retain the
successful product evidence without starting browser, desktop, firmware, or
ConduitOS fabrication. Product ownership therefore lives in Rust rather than a
second workflow path taxonomy. The privileged post-merge Pages workflow stays
separate and executes trusted merged code only.

Merge-group validation identifies its checkout as integration rather than candidate. The privileged `pull_request_target` Pages workflow remains separate and executes only trusted merged workflow machinery. A downloaded carrier is verified against the candidate commit and tree that actually fabricated it. `cargo xtask ci reconcile-product products.pages-carrier CANDIDATE INTEGRATION` compares those two Git trees through the typed product registry. Unrelated tree movement is inherited. When a carrier input changed, the deploy controller calls the unprivileged product workflow on the exact merged commit and admits that newly proven integration carrier instead. Candidate provenance is never rewritten to claim that old bytes came from the later merged tree.

An expedited main admission does not need a ceremonial pull request solely to publish Pages. An explicit `book-and-creche-pages` dispatch may name the exact current `main` SHA. The trusted controller verifies that identity against the default-branch ref, derives its exact tree and parent, fabricates the carrier through the same unprivileged product workflow, verifies the resulting carrier, and only then promotes it. A stale or non-main SHA refuses before fabrication.

Merged branches are retired by repository machinery rather than GitHub's immediate automatic deletion. The trusted closed-PR controller first retargets every open dependent to `main`, verifies that each immutable child head stayed exact, explicitly dispatches reconciliation for that PR/head, rescans for dependents, and only then deletes the merged branch. GitHub suppresses workflow events caused by a workflow's own `GITHUB_TOKEN`, so relying on the retarget's base-edit event would silently omit reconciliation. Any malformed response, failed retarget, refused dispatch, changed child head, or remaining dependent retains the branch and fails the controller visibly.

Branch protection's stable `admission` context is the final lightweight job in the reconciliation workflow, not an ad hoc check run racing unrelated check-suite completion. Even when every proof is inherited, reconciliation reaches that job, publishes a separately named `admission-evidence` detail record, retires its temporary integration ref, and lets the job's own terminal result satisfy protection. Duplicate dispatches do not cancel one another, so a later canceled suite cannot replace the established success merely as bookkeeping.

Candidate proof begins with one typed shared-compilation prerequisite selected
from changed Cargo packages that feed more than one scheduled proof world. The
prerequisite runs once, under the exact Rust toolchain, before the workspace and
product graphs are released concurrently. Both graphs still run their trusted
classifiers, planners, and independent artifact or lock prerequisites when the
shared compile fails. Proof nodes that compile or realize candidate code remain
blocked, so they cannot rediscover the same compiler failure or allocate a
browser, firmware, Host, or ConduitOS consumer. Each stable aggregate emits a
`conduit.ci.causal-block/v1` record naming `workspace.shared-compile`, its exact
package set, and the dependent proof domain. Reconciliation applies the same
proof-node boundary to the prospective integration tree. An empty shared-package
set is an explicit no-op, not evidence, and unknown/global impact keeps the
existing conservative graph.

An exact successful `admission-evidence` identity includes candidate, base, and
prospective integration identities. A later lifecycle event with that same
triple inherits the complete admitted proposition without re-running its lane
proofs. Unknown contracts or any identity mismatch fall back to proof planning.

Required PR-head admission is requested through the bounded `ci:reconcile`
label. The `pull_request_target` event attaches the stable job to the immutable
PR head while every privileged step still comes from canonical `main`; the
controller attempts to consume the label before planning so it can be requested
again. That cleanup is best-effort bookkeeping: API congestion may leave the
label present, but it cannot suppress exact PR/head validation.
Reopen and base-edit lifecycle events reconcile automatically. A manually
dispatched run remains useful diagnostic evidence, but its job belongs to the
selected dispatch ref. After exact reconciliation succeeds, that trusted run
publishes the required `admission` check directly onto the verified PR head;
ordinary event-driven reconciliation continues to use the job gate itself.

When a duplicate candidate workflow is deliberately cancelled, its aggregate
`check` and `products-proof` joins do not manufacture new failure records while
the cancellation propagates. Genuine dependency failures still reach those
joins and fail normally; only an explicit workflow cancellation suppresses the
redundant terminal aggregate.

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

The registry remains intentionally broad and conservative, but every active
`check` proof node retains its own exact receipt: CI-controller contracts,
all workspace shards, standalone fabrication locks, every ESP32 target, Limine
and ConduitOS tool admission, every selected x86 and architecture proof, and
the AArch64 product. The classifier builds the trusted dependency-light
attestation controller once and distributes that immutable executable to proof
jobs; matrix rows do not rebuild it. Product CI separately retains exact Tour,
Patchbay-debugger, workspace-product, and Pages-carrier receipts. The trusted
on-demand reconciler retrieves and verifies these receipts before scheduling,
so a lane is inherited only when every applicable proof key is present.

Selected ConduitOS x86 propositions share one Ubuntu runner and one prepared
toolchain through `cargo xtask conduitos prove-many`. The command admits at most
eight named proofs and admits a bounded concurrency ceiling. Hosted CI uses two
children so QEMU's existing bounded interaction deadlines remain meaningful on
the two-core runner while still overlapping independent proofs. The rescue
proposition performs several timing-sensitive guest boots and therefore owns
the local QEMU environment while it runs; ordinary x86 propositions retain
two-way overlap before that barrier. Each child
has a distinct short-lived target root (short enough for UNIX QMP socket
bounds), QMP/socket namespace, logs, evidence, result JSON,
and proof receipt. A failure is collected without cancelling its siblings, so
environment batching never turns eight propositions into one proof. The
workflow copies only bounded JSON/log/PNG outputs into retained evidence and
removes the temporary machine roots; its shared Cargo compilation tree is cache
material, not evidence and is not uploaded. This replaces up to eight
simultaneous x86 runners with one while leaving the stable `check` aggregate and
exact proof keys intact.

Reconciliation does not stop at a lane-level decision. The trusted controller
projects its bounded list of novel check proof IDs through the typed proof
registry into exact workspace, ESP32, ConduitOS x86, architecture, and
controller selections. The reusable check workflow uses that projection in
place of ordinary path impact for integration execution. Inherited proof IDs
therefore do not produce new proof receipts merely because one sibling
proposition is novel. A selected machine proof may still prepare an inherited
tool or image environment that it actually needs, but that preparation is not
reported as a newly executed proof. Unknown, duplicate, oversized, or
product-lane IDs fail before fan-out.
Ordinary pull-request candidates and merge-group planning continue to use the
normal impact graph when no reconciliation delta is supplied.

Later work can split broad nodes, model fabricated artifacts as independent
graph nodes, remove duplicated path-filtered workflows, batch the shared
browser environment, and make Crèche payload delivery lazy without changing
this identity contract.

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

## Bounded run observation

Use `cargo xtask ci monitor RUN_ID...` when shepherding Actions runs. The
monitor batches the latest run states into one GitHub request, waits two minutes
between ordinary observations, and retains every requested run identity until
GitHub reports it terminal. `--interval-seconds` and `--max-requests` may make a
bounded observation window smaller, but neither may be zero.

Rate limits and transient observation failures are not CI results. The monitor
honors `Retry-After` or `X-RateLimit-Reset`, otherwise applies bounded
exponential backoff with jitter, and never restarts or cancels a run. If a
tracked run falls outside the returned batch or the request budget expires, it
returns an actionable error without inferring success, failure, cancellation,
or disappearance.
