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

A current integration failure is work to repair through an ordinary PR. It does
not retroactively invalidate the history of every contributing PR.

## Automatic release train

After development integration succeeds, automation asks:

1. Does `dev` already have the same tree as `main`? If yes, stop.
2. Otherwise, create `release/<captured-dev-sha>` and open its PR to `main`.
3. Preserve one started healthy release and at most the newest
   queued successor; close older unstarted releases as superseded.

The release branch contains everything accumulated in `dev`. Exhaustive proof
runs there. If it exposes a cross-product bug, the trusted monitor cancels the
known-bad attempt promptly while preserving its exact failure evidence. Only
after that attempt is terminal may a repair advance the release branch and run
as a fresh exact head. A healthy running attempt is never cancelled by newer
development or by a queued successor. A release with no progress for 15 minutes
or more than 45 minutes total is escalated once through a
`release-liveness/repair-needed` issue. After merge, automation returns release fixes to `dev`.
That successful development integration naturally starts the next waiting batch.

**Promote dev to main** remains available as a manual escape hatch. It requests the same release workflow; the lane controller keeps one active
attempt and coalesces queued successors.

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
