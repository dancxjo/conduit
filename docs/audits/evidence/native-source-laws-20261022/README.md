# Native Source-law admission: local proof, 22 October 2026

Issue: #6064. Measured implementation/test source: `b3baad60b` (full commit and
per-file SHA-256 identities in `manifest.json`), based on current `dev` source
`a56001540`. This is a narrow recovery from checkpoint `26b13d753`, including
post-recovery ownership, capacity, current Beam and nested-Native corrections.
It is not a merge of the campaign checkpoint.

`cargo +stable xtask check native-source-laws` passed all 15 tests with one test
worker: 3 core borrowed-projection tests, 6 Native admission tests, 3 actual
occurrence tests, 2 optional tests and 1 actual four-slot Beam test. The final
Beam test took 183.32 seconds. Builds used two jobs, no incremental compilation,
and zero dev/test debug info in an exclusive existing target directory.

The private catalogue retained by syntax checking grants Native authority;
public inspection metadata can detect conflicts but cannot add an owner. The
opaque witness borrows the exact checked document, Native metadata and retained
canonical bytes. Admission traverses actual nested record, variant, optional,
sequence and nominal occurrences with prepared evaluators and finite storage.
Absent optional alternatives are not fabricated. Pressure/cancellation preserve
evidence and cannot produce a witness. Granted limits, prepared counts and
canonical/output capacities are inspectable; these are not total heap/RSS bytes.

The checked nested-alias fixture proves an outer scalar contract (`in 0..=7`)
and an inner actual invariant (`where . >= 2`) independently, with allocation
and reallocation counts of zero during admission. A proposed outer numeric
literal where law is rejected by the existing checker with `CND-FRM-058`; its
failed preflight is preserved and that unsupported Source is not claimed
admitted. The new borrowed projection removes exactly one nominal wrapper;
the existing all-wrapper `representation()` behavior is unchanged.

The Beam uses the current parser Source/Plan/Play initializer through the
existing kernel fixture, then existing Native state/scorer constructors. All
four slots, including inactive slots, retain their actual checked Native laws.
Independent root-anchor, cycle and committed-prefix mutations refuse. Runtime
allocation observations concern the calling test thread's prepared admission
and composition; they are not a claim about whole-process memory or unrelated
Source/Plan preparation.

Core and plot owning strict Clippy pass with `-D warnings`. The Beam target's
owning strict result and exact command are recorded in `strict.json` (its
`--no-deps` flag restricts lint ownership). Unrestricted Beam strict on the
inherited base fails at the unchanged `conduitos` `Ingress.plan_id` dead-code
warning; that negative remains in this packet. It requires the separately
owned current-dev repair and accepted-base refresh, not a lint suppression here.

Five exact successful test executables are preserved locally outside the repo;
their hashes are recorded for custody. This public packet contains source
identity and compressed raw logs, not those local executable downloads.
Historical compile/fixture/refusal failures remain beside the passing run.

Generated Rust materialization (#6065), installed Current/epoch/lifecycle work
(#6071), and hostile-code confinement of trusted StartupCatalog configuration
are separate scopes. No hosted Candidate, merge, Integration or shared
publication acceptance is claimed. #6064 remains open pending those gates and
review; this recovery ref creates no PR or Candidate.
