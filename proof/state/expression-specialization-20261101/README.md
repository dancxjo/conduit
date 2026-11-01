# Retained std expression specialization qualification

Final combined source: `1e966cb614774b4d9ec09ee76467b5bce44d0117` in `work/dev-sync-eb55` (original source `f2567c9674`, combined merge `87123a73dd6bdf48a7b0fb62e5dbbdd76b08f4fb`, production/constructor correction `7a344d3bf0`).

Both commands ran with the exclusively assigned existing native cache:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit-std-host --features linux-snapshot --lib expression_specialization:: -- --nocapture
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit-std-host --features linux-snapshot --lib kernel_preparation::storage_refresh::tests::scoped_software_refresh_preserves_exact_reserved_backing_and_refuses_active_child -- --exact --nocapture
```

Final results: expression tests 5/5 passed (`expression-tests-qualified.log`, 0.26s); scoped ledger test 1/1 passed (`scoped-ledger-test.log`, 0.23s). No all-target test/build or alternate-feature/non-Linux qualification is claimed. Cache released after both terminal exit-zero results.

Coverage: exact factory selection and retirement, unrelated payload pointer preservation, duplicate/count/canonical-size/descriptor-limit/unknown-program refusals without inventory mutation, allocation-free canonical length parity and serialized counting-writer parity. The scoped ledger fixture checks active child refusal and preservation of the exact existing pool object/reserved backing token through generation refresh and final release. That accounting fixture is not physical Source owner/product proof. Parent owns actual installed Recorder integration and original complete Plan validation.

The caller limit governs a named post-construction descriptor-volume preparation estimate: two serialized descriptor volumes plus explicitly named fixed/vector/identity, canonical factory scratch, scoped ledger and vector-growth charges. It is not a Rust allocator footprint, RSS measurement, whole-Root admission or closure of #6341. No general JSON metadata visitor/tree remains.

Intermediate evidence is retained: `expression-tests.log` records the initial compile failure (missing MIDI/Whisper constructor fields and closure error inference); `expression-tests-final.log.gz` records the intermediate 3-pass/2-fail test run. The latter expectations incorrectly treated zero as an invalid canonical input maximum and assumed fixture-only appended offers were sorted. The final tests use the actual canonical maximum and exact offer identities/payload pointers. No production behavior was changed for those test corrections.

Intermediate uncompressed log SHA-256: `e02361f54f95f90e976332cd3539300e0e468b706592b401e4ac335af74ff2a5`.
