# Working agreement for contributors and coding agents

This file governs automated and human changes to Conduit. It is deliberately stricter than ordinary contribution guidance because several agents may work at once and because an attractive local shortcut can quietly create a second runtime, a false proof, or an architecture the project did not choose.

Read these before changing code:

1. [The Conduit canon](docs/conduit-canon.md) defines the durable vision, vocabulary, invariants, and idea-preservation rules.
2. [STATUS.md](STATUS.md) is the checked boundary for what current code actually proves.
3. [Issue #361](https://github.com/dancxjo/conduit/issues/361) owns the forward salvage sequence.
4. The issue assigned to the change owns its exact acceptance criteria and stop line.

When these sources differ, do not improvise a synthesis. Current executable truth belongs in `STATUS.md`; durable architectural intent belongs in the canon; sequencing belongs in the roadmap; the active issue owns the present slice.

## Before starting

- Start from an explicit, current `main` commit and record it in the issue or PR.
- State the one outcome being attempted, its non-goals, and the proof needed to accept it.
- Identify the files and contracts the work is expected to own.
- Check open PRs for overlapping files or architectural surfaces.
- Treat a required change outside the agreed scope as a blocker to report, not automatic permission to enlarge the task.
- Do not begin a downstream milestone because a useful dependency appears nearby.

## Architectural invariants

Every change must preserve these rules unless an explicit architecture issue changes the canon first.

1. **Forms describe meaning. Hosts offer implementations. Plans make realization exact.**
2. Authored forms do not contain host, boot, implementation, operating-system, device, transport, socket, address, DOM, GPIO, stdout, credential, or resource-binding facts.
3. Source documents, checked forms, expanded forms, plans, fragments, plays, evidence, and presentation are distinct identities.
4. Kinds, implementations, initialized implementations, capabilities, selected capabilities, reservations, and active instances are distinct states.
5. Every executable input and output has an exact typed port identity. Emission is port-specific; fan-out is explicit and atomic under pressure. Never restore implicit broadcast semantics.
6. All queues, buffers, values, operation slots, routes, evidence, resources, and mandatory work are finite and admitted before activation. A hosted convenience profile may allocate before activation, but activation may not hide unbounded growth.
7. Platform effects cross a generic admitted host-operation boundary. Platform adapters do not become schedulers, planners, policy engines, or sources of runtime truth.
8. Availability is not authority. Reachability is not membership. Membership is not trust. A link observation is not permission to use an external subject.
9. A connection provider carries an exact planned cord. It does not invent connectivity, retry semantics, identity, or authority absent from the plan.
10. There is one execution kernel. Fixtures and temporary compatibility façades may exist only when named honestly and fenced away from production paths.
11. Failures, pressure, cancellation, evidence gaps, stale identities, and unsupported cases remain distinct and machine-readable. Do not convert them into success, retries, generic errors, or presentation-only state.
12. Simulation, compilation, browser execution, firmware execution, live transport, and physical/HIL proof are different proof classes. Never promote one into another.

## Scope and concurrency

Parallel work is encouraged only when ownership is clear.

- Prefer separate issues and branches with disjoint file allowlists.
- Avoid assigning multiple agents to `conduit-core`, `conduit-kernel`, `conduit-runtime`, root manifests, CI, or the same architecture document at once.
- Reserve integration files such as `README.md`, `STATUS.md`, `docs/reuse-ledger.md`, root manifests, the `justfile`, and workflow files for the integration owner unless a sidecar issue explicitly owns them.
- Sidecar PRs should add isolated tests, fixtures, scripts, or documents without opportunistic refactors.
- Do not edit another agent's branch, rewrite its history, or absorb its issue without an explicit handoff.
- Rebase or merge current `main` before final acceptance and rerun the relevant proof at the resulting exact head.

## Change discipline

- Do not push directly to `main`.
- Keep PRs reviewable. A large milestone may use several small PRs, but closing the parent issue requires the complete acceptance proof.
- Do not introduce broad renames, compatibility layers, dependencies, generated files, or cleanup unrelated to the owned outcome.
- Do not rebuild archived subsystems wholesale. Recover the smallest reviewed concept demanded by a working vertical slice and record its provenance in `docs/reuse-ledger.md`.
- Do not add a placeholder abstraction solely because a future feature might need it.
- Do not close an issue through a PR-body keyword unless every acceptance criterion is complete and exact-main evidence exists.
- Preserve old evidence and discussion. Correct stale claims in place; do not erase history to make the present look cleaner.

## Proof and CI

A green check proves only the commands and environments it actually ran.

- Prefer deterministic conformance below platform tests.
- Use real platform tests only for behavior that cannot be established below the platform boundary.
- Browser acceptance uses one pinned Chromium project, one worker, zero retries, no forced interaction, no action-performing polling, and no screenshot timing theater unless the owning issue explicitly changes that rule.
- A retry may diagnose infrastructure; it does not convert an invalid or flaky proof into acceptance.
- Exact-main acceptance means the merged commit, not merely a PR head or local workspace, passed the named required jobs.
- If a tool, board, device, credential, or environment is absent, report the verification gap precisely. Do not manufacture a substitute claim.

## PR contract

Every PR description should state:

- the exact base commit;
- what changed and why;
- the owning issue and acceptance slice;
- architectural invariants touched;
- explicit non-goals and stop line;
- successful and negative demonstrations;
- commands or workflow runs used for validation;
- what remains open after merge.

Implementation and acceptance-record changes should normally be separate when claims depend on exact-main CI. Update `README.md`, `STATUS.md`, the roadmap, and audit records only after the implementation reaches accepted exact-main evidence.

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
