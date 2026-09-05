# Integration and promotion

Conduit uses two proof speeds because not every development commit needs to be release-grade; every `main` commit does.

```text
feature branch
      |
      | focused candidate proof
      v
     dev  -- combined-tree integration smoke
      |
      | periodic promotion PR, exhaustive proof
      v
     main -- known-good, releasable, deployable
```

## Branch contracts

`dev` is the construction site and the ordinary pull-request target. Candidate CI explicitly checks out the PR head and uses the existing impact planner to compile affected crates and select relevant semantic, browser, firmware, or ConduitOS proofs. It is an admission screen, not a release certification.

Every `dev` update runs integration smoke on the combined tree. A failure is an integration incident: repairing `dev` takes priority over merging more product work. The PR shepherd owns triage and keeps the queue from turning into stacks of mutually stale candidates.

The earlier candidate reconciliation and workflow-run retirement controllers remain manually dispatchable during migration but do not run on ordinary PR lifecycle events. Native merged-branch deletion replaces the latter in the steady state.

`main` is the stable publication branch. Its only routine input is a same-repository PR from `dev`. The stable `promotion` check forces the complete workspace and product proof graphs, including release fabrication. A feature branch aimed directly at `main` fails the branch boundary rather than acquiring an alternative path.

After a promotion merges, Pages deployment accepts the carrier produced by that exact promotion run. Deployment remains privileged and separate; it does not execute code from an untrusted `pull_request_target` checkout.

The Pages resolver admits `promotion.yml` as an exact carrier producer even though `dev` is the repository default. An explicit recovery deployment still verifies the requested SHA against `refs/heads/main`; default-branch metadata is not publication identity.

## Evidence meanings

| Boundary | Question answered | Cost |
| --- | --- | --- |
| feature PR to `dev` | Is this delta sound enough to integrate? | Focused by actual impact |
| push on `dev` | Does the current combined development tree work in affected domains? | Focused integration smoke |
| `dev` to `main` | Is this exact integrated tree releasable? | Exhaustive |

Passing a feature PR does not promise eternal compatibility with later `dev`. When an old PR is ready to enter integration, rebase it or reapply its clean product delta onto current `dev` and prove that refreshed head.

## Promotion procedure

1. Require green `dev` integration smoke and a controlled open-PR queue.
2. Open or refresh the sole `dev`-to-`main` promotion PR without adding a synthetic product delta.
3. Require the stable `promotion` result at the exact `dev` head. Do not substitute local, candidate, canceled, or older evidence.
4. Merge without rewriting the proven source tree. Verify the resulting `main` tree is the promoted `dev` tree.
5. Let the trusted post-merge deployment upload the already-proven Pages carrier.
6. Close issues only when their stated proof class and stable-main requirements are actually met.

## Emergency queue consolidation

An uber-PR is recovery, not the steady state. The shepherd records the exact head of every stranded PR, combines reviewed deltas once on current `dev`, resolves shared fallout, validates the combined head, and closes an old PR only after its head is an ancestor or its exact patch is demonstrably present. Explicit maintainer authorization may permit a leased force update to `dev`; it never permits an unproven tree to bypass promotion into `main`.

## Roles

Product agents get useful Conduit into `dev`: small owned deltas, focused local proof, and prompt refresh when stale. Agent Fiona is the current PR shepherd: keep `dev` integrable, watch Actions and the open queue, resolve jams, and promote proven batches to `main`. CI work is justified when it protects these two boundaries or removes a demonstrated jam, not merely because a more elaborate evidence theory is possible.
