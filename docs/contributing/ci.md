# CI for contributors and agents

Conduit has one development entrance and one automatic release train.

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

Every merge starts integration on the latest combined `dev` tree. This is where
affected product, browser, firmware, and ConduitOS interactions may report bugs.
Another merge cancels the now-obsolete integration run and validates the newer
tree instead.

A current integration failure is work to repair through an ordinary PR. It does
not retroactively invalidate the history of every contributing PR.

## Automatic release train

After development integration succeeds, automation asks:

1. Is a release already open? If yes, stop; new work waits for the next release.
2. Does `dev` already have the same tree as `main`? If yes, stop.
3. Otherwise, create `release/<captured-dev-sha>`, open its PR to `main`, and
   enable automatic merge.

The release branch contains everything accumulated in `dev`. Exhaustive proof
runs there. If it exposes a cross-product bug, repair the release branch and let
the new exact head run. After merge, automation returns release fixes to `dev`.
That successful development integration naturally starts the next waiting batch.

**Promote dev to main** remains available as a manual escape hatch. It performs
the same coalescing check and never creates a second simultaneous release.

## Statuses

| Status | Meaning | Action |
| --- | --- | --- |
| `candidate` passed | This exact PR head may enter `dev` | Review and merge |
| `candidate` failed | A cheap entry contract failed | Fix the named failure |
| `dev-integration` failed | The latest combined development tree has a bug | Repair it through an ordinary PR |
| `promotion` failed | The current release batch is not releasable | Fix the release branch |
| `promotion` passed | The exact repaired release head is releasable | None; auto-merge continues |

Proof keys, receipts, artifact digests, and runner identities are machine-facing
diagnostics. They may appear inside a failed job, but they are not contributor
state and must not become required ceremony again.
