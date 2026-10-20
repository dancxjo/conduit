# #5992 file-size and ownership review

Reviewed exact `6f579f684500e6bf54c5dd7b5870c5c5c6fb7d6d` against `00525`, read-only. No source edits, fetch, Actions observation or GitHub writes.

One actionable policy concern: `targets/conduitos/src/tour_workspace.rs` grows from 536 to 548 lines. The metric constant and feature-specific snapshot are directly related to the font correction, but the explicit rule for growing an existing file over 500 lines asks for a coherent extraction. The smallest seam is its existing complete `#[cfg(test)] mod tests` (starts at line 358): move that unchanged to `tour_workspace/tests.rs` and retain `#[cfg(test)] mod tests;`. Production then remains about 358 lines; the approximately 190-line test module retains the same `super::*` imports and test paths. No layout redesign or broader renderer extraction is justified. Validate formatting, the owning package checks and the existing focused Tour tests; do not rerun unrelated workspace tests solely for this extraction.

Coherent exceptions requiring no further extraction:

- `make/xtask/mod.rs` is 815 lines, up three. Its two module declarations and one dispatch arm are composition-root wiring for the relevant headless proof. No substantive implementation was added to this frozen façade; the work lives in responsibility-named `headless_profile.rs` (30 lines), its tests (69), and existing `headless_proof.rs` (212). An unrelated command-dispatch extraction would expand scope without improving this change.
- `make/xtask/build.rs` is 555 lines, up five. The added optional manifest field/pass-through and three-line delegation are the minimum integration seam to `headless_profile::configure_symbols`; the profile policy has already been kept outside the orchestration file. A broader build extraction would be speculative here. Record this narrow related-glue exception in the PR description.

All other changed/new Rust files are below 500 lines. No root manifests, STATUS, README, reuse ledger, workflows or justfile changed. File ownership stays inside the ConduitOS typography/native layout and its supported proof entrance.

## Resolution

Root extracted the exact existing test body in f0a913758, then applied only three rustfmt wrapping changes in 3860faa7e. Production statements remain byte-identical. tour_workspace.rs is now 360 lines and its tests are 188 lines. All 19 focused tests passed before formatting, which changed no names, literals or operators. Final formatting passed. Required full-workspace validation is running on the final source.
