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

## What to keep, archive, or discard

| Material | Action |
| --- | --- |
| Handwritten source, actual tests, proof harnesses, current contracts | Keep in Git. |
| Minimal deterministic fixture read by a named test | Keep in Git with its consumer and provenance; prefer the smallest useful case. |
| Media required by an existing publication verifier | Keep its explicit retention entry until verified external acquisition replaces that consumer. |
| Result summary, source identity, command, proof class, outcome, limits, hashes and durable artifact link | Keep a compact receipt when it establishes a useful guarantee. |
| Raw logs, traces, screenshots, audio/video, firmware/ISO/disk images, compressed run bundles, large JSON dumps | Generate under ignored `target/`; use CI artifacts or a checksummed external archive when retention is required. |
| Repeated captures, duplicate bundles, intermediate successful runs, superseded diagnostics, downloaded archive copies | Discard once no active consumer needs them and required unique evidence is preserved. Do not upload them merely to document the cleanup. |
| Uncommitted work, secrets, active outputs, essential recovery images, only copies of required evidence | Protect locally; do not commit them as a cleanup shortcut. |

A proof receipt must distinguish success, refusal and missing proof. It should
point to retained raw evidence when necessary, without embedding the raw run in
JSON, base64, Markdown, patches or nested archives. Do not create another dated
capture package for each passing rerun. Update the relevant concise result or
link the CI run. A filename below the guard's size threshold is not automatically
worth keeping.

## Instructions for agents on old branches

The archive migration is intentional. Do not restore removed files to satisfy an
old audit link, an obsolete hash list, or a stale branch's idea of completeness.
Retrieve the existing archive into ignored local storage when inspection is
needed, verify its checksum, and keep it outside tracked source. Use the existing
archive URL rather than uploading another copy to Git, CI or a new release.

Before integrating older work, refresh from current `dev`, transfer the smallest
source/test delta, and inspect the staged paths for generated evidence. Preserve
archive removals when resolving conflicts. Never merge an old evidence tree
wholesale or relocate its files to evade `.gitignore` or the retention guard.

If a regression requires historical bytes, extract only the minimum test fixture,
name the test that consumes it, and review that exception explicitly. Do not
force-add raw output or change budgets/allowlists merely to make a gate pass.
Removing a consumer also requires removing its now-unused fixture and retention
entry. These rules apply everywhere in the repository, not only the three
currently inventoried directory families.

Before discarding a required unique capture, verify the durable remote copy and
its exact checksum. Once that is established, delete redundant local copies when
no running job owns them. Temporary CI diagnostics should expire normally;
indefinite retention requires a concrete consumer or unresolved question.
Never delete recovery material, a live run's output or the only useful failure
capture merely because it is large.

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
