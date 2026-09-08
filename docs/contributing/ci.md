# CI for contributors and agents

Open ordinary pull requests to `dev`. The release train handles publication
to `main`; you do not need to operate it to contribute.

The executable definitions live in [the workflow directory](../../.github/workflows/).

## Enter development

1. Start from current `dev` and make one reviewable change.
2. Open a pull request to `dev`.
3. Read the single `candidate` result:
   - **passed** — the change may merge;
   - **failed** — fix the named formatting, patch, or controller failure;
   - **cancelled** — a newer push superseded it; follow the newest head.
4. Merge after review.

Entry is intentionally inexpensive. It checks patch integrity, formatting, and
the lightweight CI controller. It does not compile the full workspace,
fabricate products, start browsers, install firmware toolchains, or boot
ConduitOS.

Do not dispatch reconciliation, copy commit identities into comments, poll every
child job, or preserve an obsolete candidate run. The newest head owns the PR.

## Combined development

Every merge queues integration of that combined `dev` tree. This is where
affected product, browser, firmware, and ConduitOS interactions may report bugs.
A running integration finishes; newer development waits behind it
instead of starving the lane by repeatedly cancelling healthy work.
Only the newest pending update is needed. Both integration suites compare with
the common ancestor of the candidate and accepted `main`, rather than the last
push. Thus a docs-only update cannot hide a runtime change whose pending
integration was coalesced. The captured candidate stays exact while it runs.

A current integration failure is work to repair through an ordinary PR. It does
not retroactively invalidate the history of every contributing PR.

## Automatic release train

After development integration succeeds, automation asks:

1. Is any release open, including one that failed and needs repair? If yes,
   stop. Is release synchronization open? Wait for it too.
2. Read the newest completed integration from GitHub. It must have succeeded;
   a newer running integration can continue while that proven batch releases.
3. Skip a batch already accepted by `main`. Require that it still belongs to
   `dev` and includes all accepted release fixes. A delayed event is a wake-up,
   not an instruction to recreate its old snapshot.
4. Create exactly one `release/<captured-dev-sha>` PR. Later work accumulates
   in `dev`, which is the next-batch queue. No successor release PR is created.

Failure cancels the attempt, not ownership of the batch. Repair that same
release after the attempt becomes terminal. An unrelated dev commit is not
evidence that the release defect was fixed. A closed release is not reopened
automatically for the same captured commit. Explicit abandonment requires a
reviewed replacement decision.

The release branch contains everything accumulated in `dev`. Exhaustive proof
runs there. If it exposes a cross-product bug, the trusted monitor cancels the
known-bad attempt promptly while preserving its exact failure evidence. Only
after that attempt is terminal may a repair advance the release branch and run
as a fresh exact head. A healthy running attempt is never cancelled by newer
development. A release with no progress for 15 minutes
or more than 45 minutes total is escalated once through a
`release-liveness/repair-needed` issue. After merge, automation returns release fixes to `dev`.
The next successful development integration starts the next batch. A ten-minute
admission check also revisits current evidence if completion occurred while the
release or synchronization was still open; it never retries failed proof.

**Promote dev to main** runs this same admission check, including successful
integration, open ownership, and accepted-fix checks. It cannot bypass proof.

The existing lane watchdog still handles early failure, stuck-run escalation,
and superseded unstarted runs left by the old multi-PR policy. New admission
does not manufacture those queues. Installing this change does not authorize
discarding existing release repairs; finish or explicitly resolve those PRs.

Successful promotion has one finalization owner: `finalize-release.yml` verifies
and merges the exact successful head, then dispatches Pages and synchronization.
The long-running release monitor handles approval and early failure; it never
competes to merge or dispatch duplicate publication work.

## Statuses

| Status | Meaning | Action |
| --- | --- | --- |
| `candidate` passed | This exact PR head may enter `dev` | Review and merge |
| `candidate` failed | A cheap entry contract failed | Fix the named failure |
| `dev-integration` failed | The latest combined development tree has a bug | Repair it through an ordinary PR |
| `promotion` failed | The current release batch is not releasable | Fix the release branch |
| `promotion` passed | The exact repaired release head is releasable | None; auto-merge continues |
| `promotion` stuck | The lane exceeded its bounded progress window | Follow the deduplicated release-liveness issue |

Proof keys, receipts, artifact digests, and runner identities are machine-facing
diagnostics. They may appear inside a failed job, but they are not contributor
state and must not become required ceremony again.
