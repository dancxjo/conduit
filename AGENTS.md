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
and the active issue owns the present scope. If they conflict, identify the
conflict instead of inventing a new architecture. Ordinary features conform
to the canon; changing an invariant requires an explicit architecture issue.

## Work within a clear scope

- Start from current `dev`; ordinary PRs target `dev`.
- State the outcome, scope limits, files/contracts owned, and necessary proof.
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

1. **Forms describe meaning. Hosts offer implementations. Plans make realization exact.**
2. Authored Forms contain no Host, Boot, implementation, OS, device, transport,
   socket, address, DOM, GPIO, stdout, credential, or resource-binding facts.
3. Source, checked Form, expanded Form, Plan, fragment, Play, Sign, and
   Presentation are distinct identities.
4. Kinds, implementations, initialized implementations, capabilities, selected
   capabilities, reservations, and active instances are distinct states.
5. Every executable input/output has an exact typed Port. Emission is
   Port-specific; fan-out is explicit and atomic under pressure.
6. All runtime storage and mandatory work are finite and admitted before Play.
   Hosted profiles may allocate during preparation; Play must not hide growth.
7. Platform effects cross the generic admitted host-operation boundary.
   Adapters do not become schedulers, planners, policy engines, or runtime truth.
8. Availability, reachability, membership, trust, and authority remain distinct.
   Seeing an external subject is not permission to use it.
9. A Line realizes an exact planned Cord. It cannot invent connectivity,
   identity, authority, or retries absent from the Plan.
10. There is one execution kernel. Name and fence fixtures or temporary
    compatibility façades away from production paths.
11. Pressure, failure, cancellation, Sign gaps, stale identities, and unsupported
    behavior remain distinct and machine-readable, including in presentation.
12. Compilation, simulation, browser execution, firmware execution, live
    transport, physical/HIL evidence, and human enactment prove different things.

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
  move; prove lower layers do not depend on orchestration or form cycles.
- Module-splitting PRs require formatting, focused tests, workspace Clippy with
  warnings denied, and the full workspace tests, or a precise infrastructure
  blocker. Establish one seam before assigning another split in the same file.

## Verify and publish honestly

Prefer deterministic conformance below platform tests. Run platform proof for
behavior that needs that environment. Browser acceptance uses the pinned
Chromium project, one worker, zero retries, no forced interactions,
action-performing polling, or screenshot timing theater unless the issue
explicitly changes that contract. A diagnostic retry does not make flaky proof
acceptable. Report missing tools, devices, credentials, and human proof precisely.

Stable `main` receives only the automated `release/<captured-dev-sha>` train
after its exact final head passes exhaustive promotion. Never push directly to
`main` or open an ordinary feature/documentation/maintenance PR against it.
A candidate passing is not stable acceptance. Use the
[CI guide](docs/contributing/ci.md) for current integration and promotion behavior;
automation owns release receipts, retirement, integration refs, reconciliation,
and promotion commits. Investigate actual failures without polling healthy jobs.

Describe what changed, why, the owning issue, the relevant boundary, validation,
and meaningful remaining gaps. GitHub retains commits and check identities;
do not repeat them in prose unless diagnosing a failure. Never close an issue
via PR keywords before all its acceptance criteria and required exact-main
evidence exist.

Keep implementation and stable-acceptance records separate when the claims
depend on promotion. Documentation may accurately describe current development
code and open work as such; reserve accepted-release claims for the required
stable evidence. Preserve old evidence and discussion, correcting stale framing
instead of erasing history.

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
and verify its exact HEAD is remotely reachable. Use `git worktree remove`
without force and retain branches; preserve anything unverifiable.

Recover archived ideas only for a current vertical slice. Record the smallest
reviewed reuse and its provenance in [the reuse ledger](docs/reuse-ledger.md),
with positive and negative proof. Do not rebuild an archived subsystem wholesale.
The canon distinguishes living core, dormant ideas, superseded experiments,
and unresolved dreams. Deferral preserves an idea without making it today's task.
