# Working agreement

Start with [CONTRIBUTING.md](CONTRIBUTING.md) for setup and the PR workflow.
This file records the constraints contributors and coding agents need when
changing code. Keep routine work focused and explain the behavior and proof
that matter to a reviewer.

## Read the relevant contracts

Before changing code, read the [canon](docs/conduit-canon.md),
[current status](STATUS.md), and the owning issue's acceptance criteria.
The [roadmap](docs/roadmap.md) points to current work; the old R1 milestone
[#361](https://github.com/dancxjo/conduit/issues/361) is completed history.

The canon owns architectural intent, STATUS owns capability and proof claims,
and the active issue owns the present scope. For documentation changes, use
[documentation ownership](docs/documentation-policy.md) to keep introductions,
exact contracts, status, and historical evidence distinct. If they conflict, identify the
conflict instead of inventing a new architecture. Ordinary features conform
to the canon; changing an invariant requires an explicit architecture issue.

## Work within a clear scope

Follow the canonical [ownership and handoff flow](CONTRIBUTING.md#keep-ownership-and-handoffs-small).

- Start from current `dev`; ordinary PRs target `dev`.
- state the outcome, scope limits, files/contracts owned, and necessary proof.
  Check open PRs for overlap before substantial work.
- Keep changes reviewable. Do not add unrelated refactors, dependencies,
  generated files, broad renames, or speculative abstractions.
- A needed change outside the agreed scope is a blocker to report, not
  permission to absorb another issue or start a downstream milestone.
- Parallel work needs disjoint file ownership. Do not assign several agents
  to the same core crate, CI surface, or architecture document.
- Reserve root manifests, README, STATUS, reuse ledger, justfile, and workflows
  for the integration owner unless explicitly handed off.
- Do not modify another agent's branch, rewrite its history, or absorb its work
  without a handoff. Refresh stale candidates onto current `dev` using their
  smallest clean product delta; avoid compatibility machinery for stale ancestry.
- If combined `dev` is red, repair it before admitting more work. The shepherd
  owns integration fallout and should fix it or revert the smallest offending
  change through a reviewable PR.

## Preserve the architecture

1. **plots describe meaning. hosts offer implementations. plans make realization exact.**
2. Authored plots contain no host, boot, implementation, OS, device, transport,
   socket, address, DOM, GPIO, stdout, credential, or resource-binding facts.
3. Source, checked plot, expanded plot, plan, fragment, play, sign, and
   face are distinct identities.
4. kinds, implementations, initialized implementations, capabilities, selected
   capabilities, reservations, and active instances are distinct states.
5. Every executable input/output has an exact typed port. Emission is
   port-specific; fan-out is explicit and atomic under pressure.
6. All runtime storage and mandatory work are finite and admitted before play.
   Hosted profiles may allocate during preparation; play must not hide growth.
7. Platform effects cross the generic admitted Host Call boundary.
   Adapters do not become schedulers, planners, policy engines, or runtime truth.
8. Availability, reachability, membership, trust, and authority remain distinct.
   Seeing an external subject is not permission to use it.
9. A line realizes an exact planned cord. It cannot invent connectivity,
   identity, authority, or retries absent from the plan.
10. There is one execution kernel. Name and fence fixtures or temporary
    compatibility façades away from production paths.
11. Pressure, failure, cancellation, sign gaps, stale identities, and unsupported
    behavior remain distinct and machine-readable, including in face.
12. Compilation, simulation, browser execution, firmware execution, live
    transport, physical/HIL evidence, and human enactment prove different things.

Canonical implementation vocabulary uses one noun at each altitude: a gear
invokes a kind through its fore; a host offers a back; a plan selects exact
backs; a play advances in bounded steps; and a step may cross the host boundary
with a host call. Host backs and Plot backs share eligibility laws but retain
their distinct realization and provenance facts. Reserve capability vocabulary
for actual possession/authority, especially Base capabilities, rather than as a
generic synonym for an implementation.

## Use the supported entrances

Public workflows enter through `conduit`. Repository development, validation,
proof, hardware work, and demonstrations enter through `cargo xtask`.
Document runnable capabilities through those entrances. `just` may only provide
optional thin aliases; direct Cargo package commands, npm/npx, raw test runners,
platform tools, environment switches, and implementation binaries are internal
conveniences, not required user interfaces. Named credential-environment
references accepted by xtask are permitted so secrets stay out of arguments.
Promoting a repository demo into `conduit` is an explicit product-boundary change.

## Keep Rust responsibilities separate

- Keep `lib.rs` and `main.rs` as façades and composition roots. Put substantive
  implementation in responsibility-named modules.
- New or materially expanded production Rust files should normally stay below
  500 lines. Explain an exception and why a coherent split would be worse.
- Before growing an existing file over 500 lines, extract a coherent
  responsibility in this change or a prerequisite. Files over 800 lines are
  frozen against unrelated growth.
- Split tests by contract or proof surface; share fixtures in a small common
  module instead of duplicating them across giant test files.
- Split by stable responsibility and dependency direction, never numbered or
  miscellaneous shards. Identify inputs, outputs, invariants, and private
  collaborators first; preserve public paths with narrow re-exports.
- Prefer extraction-only PRs. Avoid semantic cleanup or redesign in the same
  move; prove lower layers do not depend on orchestration or plot cycles.
- Module-splitting PRs require formatting, focused behavioral tests, and the
  owning package checks. Use broader checks when a shared contract changes;
  an extraction alone does not require rerunning unrelated workspace tests.
  Establish one seam before assigning another split in the same file.

## Verify and publish honestly

Prefer deterministic conformance below platform tests. Run platform proof for
behavior that needs that environment. Browser acceptance uses the pinned
Chromium project, one worker, zero retries, no forced interactions,
action-performing polling, or screenshot timing theater unless the issue
explicitly changes that contract. A diagnostic retry does not make flaky proof
acceptable. Report missing tools, devices, credentials, and human proof precisely.

Stable `main` receives only the automated `release/<captured-dev-sha>` train
after its exact source passes full integration and the publisher verifies the
retained artifacts and identical accepted tree. Never push directly to
`main` or open an ordinary feature/documentation/maintenance PR against it.
A candidate passing is not stable acceptance. Use the
[CI guide](docs/contributing/ci.md) for current integration and promotion behavior;
automation owns release receipts, retirement, integration refs, reconciliation,
and promotion commits. Investigate actual failures without polling healthy jobs.

An open release owns its batch through failure and repair. Later development
accumulates in `dev`; do not create successor release PRs while a release or its
synchronization is open. Running integration finishes and pending updates
coalesce; compare the accumulated delta with accepted `main`, never only the
last push whose intermediate checks may have been skipped.

Describe what changed, why, the owning issue, the relevant boundary, validation,
and meaningful remaining gaps. GitHub retains commits and check identities;
do not repeat them in prose unless diagnosing a failure. Close implementation issues when their scoped behavior is merged and relevant
checks pass. Track automatic publication separately unless the issue explicitly
owns release acceptance. Do not describe Candidate proof as stable acceptance.

Keep implementation and stable-acceptance records separate when the claims
depend on promotion. Documentation may accurately describe current development
code and open work as such; reserve accepted-release claims for the required
stable evidence. Preserve needed historical evidence through checksummed external
archives and compact references; keep discussion and correct stale framing.
This does not require retaining raw captures in the source tree.

## Keep origin branches purposeful

- Before publishing a branch, refresh `dev` and check current open consolidation
  work. A local checkout or old branch name is not evidence of missing work.
- Do not recreate an audited deleted branch under `archive/`, `recovery/`,
  `checkpoint/`, another prefix, or a new name. Do not bulk-push local heads or
  publish a recovery-map branch during cleanup. Preserve uncertain source with
  a kept local ref or verified local bundle until its useful delta is reviewed.
- Salvage missing functionality and regression coverage into focused current-dev
  changes. Do not merge an obsolete subtree, restore old execution machinery,
  carry historical captures, or publish bookkeeping merely to preserve a tip.
- A remote branch needs a current issue/PR purpose and named ownership. Delete
  redundant aliases after verifying retained source and current PR/release
  dependencies; use expected-head checks so concurrent new work is not erased.
- Keep `dev`, `main`, the live publication branch, and active PR/release work.
  Never infer disposability solely from a branch prefix, age, closed issue or
  patch similarity. Preserve any unique useful work until actual salvage lands.

## Keep generated evidence out of source

Follow [evidence retention](docs/contributing/evidence-retention.md) for every
proof run and handoff, including work already underway on older branches.

- Keep source, proof harnesses, minimal fixtures read by named tests, required
  publication assets, and compact provenance/result summaries in Git.
- Write raw logs, traces, screenshots, recordings, firmware/disk images and run
  archives to ignored `target/` output. Use CI artifacts for temporary diagnostics
  and checksummed external archives for evidence that must outlive CI expiry.
- Never restore archived captures from Git history, old branches, downloads or
  release bundles into tracked source. Do not reupload an existing archive, copy
  it under another directory, rename it, split it, or encode it as text to bypass
  this rule. Link its existing archive and checksum instead.
- Refresh older work onto current `dev` using only the smallest product delta.
  Preserve cleanup deletions; inspect staged paths before pushing. Do not merge
  stale evidence directories back with an otherwise useful implementation.
- Do not force-add generated output, add blanket retention exceptions, increase
  budgets to fit another run, or weaken the guard. A new fixture needs a named
  executable consumer and the minimum bytes needed to exercise its contract;
  an old capture or audit link alone is not such a consumer.
- Discard duplicate and superseded local output after its useful result is
  summarized and any required unique evidence has verified external custody.
  Protect active runs, uncommitted source, credentials, recovery images and the
  only copy of required evidence. Preservation does not mean committing it.
- Run `cargo xtask ci pipeline evidence-retention` before submitting evidence
  changes. This guard covers known historical directories; the policy applies
  throughout the repository, including newly invented output directories.

## Review the substance

Check scope, exact identities and bounds, pressure, cancellation, terminal
outcomes, and the claimed proof class. Look for duplicate runtime truth, hidden
allocations or retries, invented connectivity, and ambient authority. A planner
decision or serialized grant is not hostile-code confinement; reject paths
that can exercise authority beyond their admitted realization. Request the
smallest architectural or proof correction and avoid unrelated style churn.

## Preserve work and ideas

Follow [environment stewardship](docs/contributing/agent-operations.md) before
large builds or cleanup. Protect active artifacts, dirty or unpushed work,
unique evidence, credentials, and personal data. Before removing a worktree,
refresh remote refs, inspect tracked/untracked changes and active ownership,
and verify its exact HEAD is retained by a kept local/remote ref or a verified
local bundle outside that worktree. Preserve dirty/untracked work separately.
Use `git worktree remove` without force and retain the needed local branches;
preserve anything unverifiable. Do not publish an origin archive merely to
satisfy this custody check.

Recover archived ideas only for a current vertical slice. Record the smallest
reviewed reuse and its provenance in [the reuse ledger](docs/reuse-ledger.md),
with positive and negative proof. Do not rebuild an archived subsystem wholesale.
The canon distinguishes living core, dormant ideas, superseded experiments,
and unresolved dreams. Deferral preserves an idea without making it today's task.
