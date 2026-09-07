# Integration and promotion

Conduit deliberately separates inexpensive development entry from exhaustive
release proof.

```text
feature PR -> candidate -> dev -> dev-integration
                                  |
                       successful integration
                                  |
                         release branch
                                  |
                    repair + promotion -> main
```

## Branch contracts

`dev` is the construction branch and the target for ordinary pull requests.
The `candidate` workflow checks the exact PR head for patch integrity,
formatting, and lightweight CI-controller contracts. It does not compile the
full workspace, fabricate products, start browsers, install firmware
toolchains, or boot ConduitOS. A newer push cancels work for the superseded
head; unrelated pull requests never cancel one another.

Every update to `dev` runs combined-tree integration, including affected
product and machine proofs. Only the newest development head matters. If another
merge advances `dev`, GitHub cancels the obsolete run and proves the new
combined tree. A red current head is useful integration feedback, not a reason
to reconcile every previously merged pull request.

`main` is the stable publication branch. After successful development
integration, the release controller creates `release/<captured-dev-sha>`
containing everything then in `dev` and opens its PR to `main`—unless a release
is already running or the two trees already agree. The branch is intentionally
repairable: exhaustive proof may expose interactions that inexpensive
development entry did not. Repair those bugs on the release branch until its
exact final head passes.

After a release merges, automation opens an auto-merged sync back to `dev`.
Release repairs therefore become part of later development rather than being
rediscovered.

## Contributor procedure

- To change Conduit: open a pull request to `dev`, then review the single
  `candidate` result.
- Publication normally starts automatically after successful development
  integration. **Promote dev to main** is the manual escape hatch.
- If development integration fails: repair `dev` through another ordinary PR.
- If promotion fails: repair the release branch and let the exact new head run.

No person or agent supplies integration identities, operates proof receipts,
creates ceremonial merge commits, dispatches reconciliation, or polls every
child job.

See [CI for contributors](contributing/ci.md) for status meanings. Historical
candidate-reconciliation details remain only as a
[short archaeology note](ci-candidate-evidence.md).

## Guarantees retained behind the interface

- candidate code never receives privileged deployment credentials;
- each gate evaluates its exact current head;
- entry to `dev` is cheap, while combined and release failures remain visible;
- release proof is exhaustive and applies to the final repaired release head;
- a successful older run never substitutes for a failed newer head;
- release fixes return to `dev`; and
- Pages deploys only a carrier produced by the accepted promotion.

These are implementation requirements, not contributor chores.
