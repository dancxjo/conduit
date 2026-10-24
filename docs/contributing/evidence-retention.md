# Evidence belongs beside the release, not inside every checkout

Keep source, reproducible proof harnesses, small canonical regression fixtures,
and compact result/provenance receipts in Git. Save raw run logs, screenshots,
recordings, disk images and compressed captures in CI artifact storage. Evidence
needed beyond CI expiry belongs in a durable archive with exact checksums.
A capture is not a fixture merely because it has been committed before.

`cargo xtask ci pipeline evidence-retention` checks the three historical output
families before candidate compilation. Their byte budgets cannot silently grow.
Raw output needs an exact hash, length and named consumer in
`tools/ci/evidence-retention.json`; changed output or stale exceptions refuse.
New output in these directories is ignored by default. To admit a necessary
fixture, review its consumer and minimum useful size, then explicitly add its
retention entry and source file. Do not raise a budget to retain another run.
Use ignored `target/` output for local proof; upload needed diagnostics from CI.

## Historical audit migration

[The 2026-10-24 archive](https://github.com/dancxjo/conduit/releases/tag/evidence-archive-20261024)
preserves 2,326 removed files (517,929,645 bytes) from source
`67764535f84f7674a84a0119abe727fc970fc753`. The authoritative full source identity
and each original path, length and SHA-256 are in
[the migration manifest](../audits/archived-captures-20261024.json).
Download `audit-captures.tar.gz` from that release and verify SHA-256
`a353ae04525f18d600d92c1162e06befe3f456e42a0b5f50e2967bcc8fee209c`
before extracting into a separate directory. The tar preserves original
repository-relative paths and includes `archive-manifest.json`. Every member of
the downloaded remote archive was verified before deleting its source copy.

Small historical receipts remain in place. Their relative raw-output paths refer
to that archived tree. Markdown links to moved files now download the bundle.
Native Face runtime data, native Source custody PNGs, and scalar-tape reference
measurements remain in source because tests read them. Publication journey assets
and firmware frame captures are retained under explicit budgets until their
consumers support verified external acquisition. They have not been deleted or
relabelled as current execution evidence.

This removes about 494 MiB from the current source tree. Git history and proof
commit identities are unchanged; old full clones retain historical blobs.
