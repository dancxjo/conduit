# Working agreement for contributors and coding agents

This file governs automated and human changes to Conduit. It is deliberately stricter than ordinary contribution guidance because several agents may work at once and because an attractive local shortcut can quietly create a second runtime, a false proof, or an architecture the project did not choose.

Read these before changing code:

1. [The Conduit canon](docs/conduit-canon.md) defines the durable vision, vocabulary, invariants, and idea-preservation rules.
2. [STATUS.md](STATUS.md) is the checked boundary for what current code actually proves.
3. [Issue #361](https://github.com/dancxjo/conduit/issues/361) owns the forward salvage sequence.
4. The issue assigned to the change owns its exact acceptance criteria and stop line.

When these sources differ, do not improvise a synthesis. Current executable truth belongs in `STATUS.md`; durable architectural intent belongs in the canon; sequencing belongs in the roadmap; the active issue owns the present slice.

## Before starting

- Start ordinary work from current `dev`. Git and CI retain the exact identity;
  do not copy it into issue or PR prose unless a failure investigation needs it.
- State the one outcome being attempted, its non-goals, and the proof needed to accept it.
- Identify the files and contracts the work is expected to own.
- Check open PRs for overlapping files or architectural surfaces.
- Treat a required change outside the agreed scope as a blocker to report, not automatic permission to enlarge the task.
- Do not begin a downstream milestone because a useful dependency appears nearby.

## Architectural invariants

Every change must preserve these rules unless an explicit architecture issue changes the canon first.

1. **Forms describe meaning. Hosts offer implementations. Plans make realization exact.**
2. Authored forms do not contain host, boot, implementation, operating-system, device, transport, socket, address, DOM, GPIO, stdout, credential, or resource-binding facts.
3. Source documents, checked forms, expanded forms, plans, fragments, plays, Signs, and presentation are distinct identities.
4. Kinds, implementations, initialized implementations, capabilities, selected capabilities, reservations, and active instances are distinct states.
5. Every executable input and output has an exact typed port identity. Emission is port-specific; fan-out is explicit and atomic under pressure. Never restore implicit broadcast semantics.
6. All queues, buffers, values, operation slots, routes, Signs, resources, and mandatory work are finite and admitted before Play start. A hosted convenience profile may allocate before Play start, but Play start may not hide unbounded growth.
7. Platform effects cross a generic admitted host-operation boundary. Platform adapters do not become schedulers, planners, policy engines, or sources of runtime truth.
8. Availability is not authority. Reachability is not membership. Membership is not trust. A link observation is not permission to use an external subject.
9. A Line realization carries an exact planned Cord. It does not invent connectivity, retry semantics, identity, or authority absent from the Plan.
10. There is one execution kernel. Fixtures and temporary compatibility façades may exist only when named honestly and fenced away from production paths.
11. Failures, pressure, cancellation, Sign gaps, stale identities, and unsupported cases remain distinct and machine-readable. Do not convert them into success, retries, generic errors, or presentation-only state.
12. Simulation, compilation, browser execution, firmware execution, live transport, and physical/HIL proof are different proof classes. Never promote one into another.

## Scope and concurrency

Parallel work is encouraged only when ownership is clear.

- Prefer separate issues and branches with disjoint file allowlists.
- Avoid assigning multiple agents to `conduit-core`, `conduit-kernel`, `conduit-plan-lowering`, root manifests, CI, or the same architecture document at once.
- Reserve integration files such as `README.md`, `STATUS.md`, `docs/reuse-ledger.md`, root manifests, the `justfile`, and workflow files for the integration owner unless a sidecar issue explicitly owns them.
- Sidecar PRs should add isolated tests, fixtures, scripts, or documents without opportunistic refactors.
- Do not edit another agent's branch, rewrite its history, or absorb its issue without an explicit handoff.
- Ordinary branches and pull requests target `dev`. Refresh stale work by rebasing or reapplying its smallest clean product delta onto current `dev`; do not build compatibility machinery merely to preserve stale candidate ancestry.
- If `dev` is red, restoring it outranks admitting more work. The PR shepherd owns combined-tree fallout and should either repair it promptly or revert the smallest offending integration through a reviewable PR.

## Environment maintenance and disk cleanup

Agents must assess the actual environment, ownership, active processes, and storage pressure when deciding when and how to clean. A `.local` hostname suffix is not required and does not by itself establish ownership.

On Dan's machines `forebrain`, `victus`, and `envie` (including their fully qualified hostnames), agents are responsible for proactively maintaining usable disk space. Check headroom before substantial builds and recover space from verified disposable artifacts when needed; do not wait for disk exhaustion or ask again for routine cleanup permission. This responsibility does not authorize deleting uncommitted work, unique evidence, or personal data. These hosts own their workspaces; agents may manage workspace lifecycle according to the host's local policy.

In cloud, CI, hosted runner, and other managed environments, the agent's own environment-specific rules, permissions, and cleanup lifecycle govern. This repository imposes no blanket prohibition on cloud cleanup and grants no additional privilege there. Follow those environment rules for workspace and cache retention, shared resources, and disposal. If the environment or ownership is uncertain, investigate and limit cleanup to verified disposable artifacts within the agent's authority; report any remaining blocker.

On user-owned machines, recover space in this order:

1. Inventory filesystem usage and the largest directories before changing state. Check for active Cargo, compiler, browser-proof, VM, and other processes that may own candidate artifacts.
2. Remove regenerable build outputs such as inactive Rust `target` directories and tool caches. Preserve outputs used by a running process. Do not delete source trees, repositories, credentials, downloads, virtual-machine images, or other user data merely because they are large.
3. Empty desktop trash and remove stale user-owned temporary artifacts. Do not disturb active sockets, sessions, system-owned temporary paths, or recent artifacts whose ownership is unclear.
4. Use package-native cleanup for package caches and bounded journal retention when available. Do not bypass missing privileges or turn a cleanup into an operating-system reconfiguration.
5. Report the before/after free space, what classes of data were removed, what large candidates were deliberately preserved, and any cleanup blocked by permissions.

Disk cleanup is machine maintenance, not permission to change Conduit source or enlarge an issue's implementation scope.

## Change discipline

- Do not push directly to `main`. Stable `main` accepts the current repairable
  `release/<captured-dev-sha>` branch only after its exact final head passes the
  exhaustive promotion gate.
- Do not open feature, documentation, maintenance, or CI PRs directly against
  `main`. The sole routine `main` PR is the automatic release train.
- Keep PRs reviewable. A large milestone may use several small PRs, but closing the parent issue requires the complete acceptance proof.
- Do not introduce broad renames, compatibility layers, dependencies, generated files, or cleanup unrelated to the owned outcome.
- Do not rebuild archived subsystems wholesale. Recover the smallest reviewed concept demanded by a working vertical slice and record its provenance in `docs/reuse-ledger.md`.
- Do not add a placeholder abstraction solely because a future feature might need it.
- Do not close an issue through a PR-body keyword unless every acceptance criterion is complete and exact-main evidence exists.
- Preserve old evidence and discussion. Correct stale claims in place; do not erase history to make the present look cleaner.

## Executable entrances

`conduit` is the product entrance. `cargo xtask` is the repository-development entrance.

- Public executable workflows MUST enter through `conduit`.
- Repository development, validation, proof, hardware development, and demonstration workflows MUST enter through `cargo xtask`.
- A documented runnable capability MUST have one of those entrances.
- `just` may provide optional thin recipes only when each delegates directly to `conduit` or `cargo xtask`; it may not own behavior or become a required interface. Direct Cargo package invocations, npm/npx, test-runner commands, platform build commands, raw implementation environment switches, and implementation binaries remain internal conveniences and MUST NOT be required user interfaces. Named credential-environment references accepted by `cargo xtask` remain permitted so secrets do not enter arguments or logs.
- Promoting an experience from `cargo xtask demo ...` to `conduit ...` is a product-boundary change. Do not merely alias repository paths into the installed CLI.

## Rust module boundaries

Large Rust files are an architectural warning, not a badge of productivity. Do not make a crate root, host adapter, scheduler, parser, or integration-test file the dumping ground for an entire subsystem.

- Keep `lib.rs` and `main.rs` primarily as façades: module declarations, narrow public re-exports, top-level composition, and genuinely crate-wide types. Put substantive implementations in responsibility-named modules.
- A new or materially expanded production `.rs` file should normally remain below 500 lines. Crossing that threshold requires an explicit explanation in the PR and a reason a coherent module boundary would be worse.
- Do not add more responsibility to an existing production file above 500 lines without extracting at least one coherent responsibility in the same change or in a prerequisite extraction PR. Files above 800 lines are frozen against unrelated growth.
- Test files are not exempt. Split integration tests by contract or proof surface before they become a chronological grab bag. Shared fixtures and builders belong in a small `common` support module, not duplicated across giant test files.
- Split by stable responsibility and dependency direction, not by arbitrary size. Use names such as `identity.rs`, `validation.rs`, `admission.rs`, `scheduler.rs`, `sign.rs`, or `tests/authority.rs`; never use `part1.rs`, `misc.rs`, `helpers2.rs`, or numbered shards.
- Preserve public paths deliberately with narrow `pub use` re-exports when compatibility matters. Do not make every extracted item public merely to satisfy the compiler.
- Prefer extraction-only PRs: move one coherent responsibility, preserve behavior and public contracts, and avoid semantic cleanup, renaming, or redesign in the same diff.
- Before extraction, identify the module's inputs, outputs, invariants, and private collaborators. After extraction, prove there are no dependency cycles and that lower-level modules do not import orchestration layers.
- Every module-splitting PR must run `cargo fmt --all --check`, relevant focused tests, `cargo clippy --workspace --all-targets -- -D warnings`, and the full workspace test suite unless the PR documents a precise infrastructure blocker.
- Agents working in parallel must own disjoint source files. Do not assign two agents to split the same monster file at once; establish and merge the first seam before opening work on the next seam.

Line count is a smoke alarm rather than the design itself. A 300-line file with five responsibilities still needs separation; a compact table or generated declaration may justify more lines when its ownership and contract remain singular.

## Proof and CI

A green check proves only the commands and environments it actually ran.

- Prefer deterministic conformance below platform tests.
- Use real platform tests only for behavior that cannot be established below the platform boundary.
- Browser acceptance uses one pinned Chromium project, one worker, zero retries, no forced interaction, no action-performing polling, and no screenshot timing theater unless the owning issue explicitly changes that rule.
- A retry may diagnose infrastructure; it does not convert an invalid or flaky proof into acceptance.
- Exact-main acceptance means the merged commit, not merely a PR head or local workspace, passed the named required jobs.
- If a tool, board, device, credential, or environment is absent, report the verification gap precisely. Do not manufacture a substitute claim.

### Integration and promotion

- `dev` is deliberately easy to enter. A feature PR runs formatting, patch
  integrity, and lightweight controller checks, not product fabrication or
  machine proof.
- Combined `dev` trees run affected integration proof in one finish-active lane.
  A materially started healthy run finishes; newer development waits rather
  than cancelling it. Repair failures through ordinary PRs.
- After successful integration, the release controller starts a release only
  when none is open and `dev` differs from `main`. New work accumulates for the
  next release while the current one runs.
- A release branch may receive bug fixes discovered by exhaustive proof only
  after the failed attempt is terminal. Its exact final head—not its initial
  snapshot—must pass before `main` accepts it. A healthy running attempt owns
  the lane; only the newest unstarted successor remains queued.
- After merge, automation returns release fixes to `dev`. Pages deploys only
  the carrier produced by the accepted promotion.
- Do not manually manage proof receipts, candidate retirement, integration
  refs, reconciliation labels, or promotion commits. See
  [CI for contributors and agents](docs/contributing/ci.md).

### PR shepherd

The PR shepherd resolves actual failures and overlapping changes. They do not
poll healthy child jobs, narrate machine identities, retire superseded runs, or
manually create promotions. Automation owns that bookkeeping.

## PR contract

Every PR description should state what changed and why, its owning issue when
one exists, the relevant architectural boundary, and any meaningful proof gap
or remaining work. GitHub already records commits, checks, and run identities;
do not duplicate that metadata as prose.

Implementation and acceptance-record changes should normally be separate when claims depend on stable promotion evidence. Update `README.md`, `STATUS.md`, the roadmap, and audit records only after the implementation reaches accepted exact-main evidence.

## Review contract

Reviewers should ask:

- Does the change establish the claimed proof class?
- Does any platform or fixture become a second runtime or source of truth?
- Are all exact identities and bounds preserved through the changed boundary?
- Are pressure, cancellation, failure, closure, and terminal evidence tested?
- Are hidden allocations, retries, ambient authority, or invented connectivity present?
- Does the PR remain inside its stop line?
- Is a dormant idea being promoted without its prerequisites?
- Can an implementation exercise ambient authority beyond its exact admitted realization? Reject such paths; distinguish cooperative authority checks from proven hostile-code confinement. A serialized grant identity, planner decision, or Sign is not an enforcement mechanism.

A request for changes should identify the smallest architectural or proof gap. Avoid style churn when the contract is already clear.

## Idea preservation

Closing or deferring work does not declare the idea worthless. Classify it using the canon:

- **living core**: current, load-bearing, and executable;
- **dormant**: valuable but waiting for named prerequisites;
- **superseded experiment**: retained for lessons, not restoration;
- **unresolved dream**: promising direction whose contract is not settled.

Do not turn dormant ideas into active obligations merely to prevent them from feeling lost. Give them provenance, dependencies, and a future proof, then let the current layer become trustworthy.
