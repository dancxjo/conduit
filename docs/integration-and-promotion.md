# Integration and promotion

The current workflow is documented in
[CI for contributors and agents](contributing/ci.md).

```text
PR to dev -> inexpensive candidate checks -> combined dev integration
                                              |
                                      automatic release branch
                                              |
                                     exhaustive promotion -> main
```

A running integration or healthy release is allowed to finish. Automation
coalesces waiting releases, keeps proof tied to the exact final release head,
returns release repairs to `dev`, and deploys the accepted Pages carrier.
Contributors open ordinary PRs to `dev` and fix the failures reported for their
change. The linked guide is the single source for contributor procedure and
status meanings.

This page retains its established URL. The former receipt and reconciliation
workflow is described only in the [historical note](ci-candidate-evidence.md).
