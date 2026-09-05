# Working agreement for contributors and coding agents

This file governs automated and human changes to Conduit. It is deliberately stricter than ordinary contribution guidance because several agents may work at once and because an attractive local shortcut can quietly create a second runtime, a false proof, or an architecture the project did not choose.

Read these before changing code:

1. [The Conduit canon](docs/conduit-canon.md) defines the durable vision, vocabulary, invariants, and idea-preservation rules.
2. [STATUS.md](STATUS.md) is the checked boundary for what current code actually proves.
3. [Issue #361](https://github.com/dancxjo/conduit/issues/361) owns the forward salvage sequence.
4. The issue assigned to the change owns its exact acceptance criteria and stop line.

When these sources differ, do not improvise a synthesis. Current executable truth belongs in `STATUS.md`; durable architectural intent belongs in the canon; sequencing belongs in the roadmap; the active issue owns the present slice.

## Before starting

- Start ordinary work from an explicit, current `dev` commit and record it in the issue or PR. Only the PR shepherd prepares frozen-`dev` promotion work for `main`.
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

## Local machine disk cleanup

Aggressive cleanup is a valid maintenance option only on Dan's user-owned local-network machines whose hostname is `victus.local` or ends in `.local`. It is never valid in cloud, CI, hosted runner, shared, or otherwise remotely managed environments, even when disk pressure is severe. Verify the hostname and environment before deleting anything; an unknown environment means stop and report the disk-pressure blocker.

On an eligible `.local` machine, recover space in this order:

1. Inventory filesystem usage and the largest directories before changing state. Check for active Cargo, compiler, browser-proof, VM, and other processes that may own candidate artifacts.
2. Remove regenerable build outputs such as inactive Rust `target` directories and tool caches. Preserve outputs used by a running process. Do not delete source trees, repositories, credentials, downloads, virtual-machine images, or other user data merely because they are large.
3. Empty desktop trash and remove stale user-owned temporary artifacts. Do not disturb active sockets, sessions, system-owned temporary paths, or recent artifacts whose ownership is unclear.
4. Use package-native cleanup for package caches and bounded journal retention when available. Do not bypass missing privileges or turn a cleanup into an operating-system reconfiguration.
5. Before deleting a Git worktree, fetch and prune `origin`, verify that the worktree is not the active checkout, verify `git status --porcelain` is empty including untracked files, and verify its exact `HEAD` is reachable from at least one current `origin/*` ref. Retain and report every dirty, untracked, unpushed, unreachable, or unverifiable worktree. Remove qualifying worktrees through `git worktree remove`, then run `git worktree prune`; do not delete their directories directly.
6. Report the before/after free space, what classes of data were removed, what large candidates were deliberately preserved, and any cleanup blocked by permissions.

Disk cleanup is machine maintenance, not permission to change Conduit source or enlarge an issue's implementation scope.

## Change discipline

- Do not push directly to `main`. Stable `main` accepts only an immutable `promote/<full-promotion-sha>` commit whose exact tree and second parent identify the frozen `dev` snapshot, through a promotion PR whose exhaustive gate succeeded.
- Do not open feature, documentation, maintenance, or CI PRs directly against `main`. The sole routine `main` PR is a frozen `dev` snapshot promotion.
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

- `dev` is the construction site. Feature PRs receive focused impact-based proof sufficient to admit them into integration; one PR never cancels another.
- A push to `dev` proves the combined development tree with integration smoke. A red `dev` blocks further admission until repaired.
- `main` is stable and releasable. A same-repository frozen `promote/<full-promotion-sha>` snapshot PR to `main` runs the exhaustive workspace, product, browser, firmware, and ConduitOS gate against its exact head. Its tree equals the chosen `dev` snapshot, with current `main` and that snapshot as its two parents. `dev` remains open for product admission while it is proved.
- Candidate evidence answers whether a change is sound enough for `dev`. Development evidence answers whether the current combined tree basically works. Promotion evidence alone answers whether an exact tree may become `main`.
- Merging or advancing `dev` does not require old feature heads to negotiate with future CI. Refresh stale candidates onto current `dev` when they approach admission.
- Pages deployment consumes the already-proven carrier from the successful frozen-snapshot promotion and runs only after that promotion merges. Privileged deployment code never executes untrusted PR-controlled code.
- Emergency consolidation is exceptional recovery: snapshot every open head, combine only reviewed deltas on a branch from current `dev`, resolve fallout once, prove the combined tree, merge it to `dev`, verify every absorbed head or patch is represented, then close superseded PRs with signed evidence. Never force an unproven recovery tree into `main`.

### PR shepherd

The named PR shepherd keeps the queue moving and the branch boundaries truthful. They monitor open PRs and Actions, redirect PRs aimed at the wrong branch, refresh old product deltas onto current `dev`, sequence overlapping work, resolve integration failures before accepting more changes, close absorbed or obsolete PRs with evidence, and open periodic frozen-snapshot promotions to `main`. They simplify avoidable process rather than adding ceremonial bytes or compatibility layers. They do not weaken proof classes, cross privilege boundaries, or claim a deployment before exact promotion evidence exists.

## PR contract

Every PR description should state:

- the exact `dev` base commit (or exact `main` base for the single promotion PR);
- what changed and why;
- the owning issue and acceptance slice;
- architectural invariants touched;
- explicit non-goals and stop line;
- successful and negative demonstrations;
- commands or workflow runs used for validation;
- what remains open after merge.

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

A request for changes should identify the smallest architectural or proof gap. Avoid style churn when the contract is already clear.

## Idea preservation

Closing or deferring work does not declare the idea worthless. Classify it using the canon:

- **living core**: current, load-bearing, and executable;
- **dormant**: valuable but waiting for named prerequisites;
- **superseded experiment**: retained for lessons, not restoration;
- **unresolved dream**: promising direction whose contract is not settled.

Do not turn dormant ideas into active obligations merely to prevent them from feeling lost. Give them provenance, dependencies, and a future proof, then let the current layer become trustworthy.
