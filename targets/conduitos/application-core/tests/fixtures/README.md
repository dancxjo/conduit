# Ordinary four-Plot Application admission fixture

`startup.catz` retains the legacy typed Init fixture from the production-shaped four-Plot diagnostic attached to #6207. The host/Boot/body/biography identifiers are synthetic fixture inputs; the projected target graphs are retained in full. This is a deterministic allocator regression, not an emulator or physical-machine receipt.

The original JSON is retained on `origin/prep/6207-buffered-codec-experiment` at `5fba3192a76ce1fbbba130750457de851428027d`, path `docs/audits/evidence/application-codec-capacity-20261021/host-fixture-init.json`.

- Original raw JSON: 857351 bytes, SHA-256 `a6c673eadcf44f6be4c6d05d4d1ff9176c6bdf6016c932866f24440582e96e76`.
- Current canonical JSON: 857519 bytes, SHA-256 `3bae07aa8cdd51a0e44df395c92872d76be901b3c217878b8a08ca78b5045ddf`.
- Admitted compressed wire: 35679 bytes, SHA-256 `8eb19e05b0ccd065cfef9abf696da88ed8019c95c4ce2834e22d2a61ec2c7375`.
- The fixture contains Patchbay targets 1, 2 and 3, selection 1, and the complete Tutorial biography.

The `private_heap` executable maps the production 2 MiB metadata grant at its exact address without replacing any existing mapping, routes preparation through the production 32-byte bitmap allocator, and checks Init, biography projection, all selected graphs and repeated replacement. It runs with ordinary package tests and `cargo xtask check workspace-test-hosts-conduitos` (select the package with `CONDUIT_CI_LINT_FULL=false` and `CONDUIT_CI_TEST_PACKAGES='["conduitos-application-core"]'` for a focused run). Unsupported host platforms report an explicit skip; they do not prove the fixed-address allocator.

The aggregate fixture enum exists only in test code. Tests translate its unchanged
Initial/Target/biography values into the staged production commands; no aggregate
Init compatibility entrance remains in the production Command vocabulary.

The Cord provenance representation adds explicit `source_origin: null` to the
eight retained Cords whose historical fixture has no proven Source origin.
The fixture was decoded and re-encoded through the production codec; no other
JSON value changed and no provenance was fabricated. The original wire remains
in Git history (`b6f1296934144d99e23c7d1ee2a3cb29b96f3a070dd19c4e7adf6706475b4649`).
Exact current wire equality and graph custody digest assertions remain enabled.
