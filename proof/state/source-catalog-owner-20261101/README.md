Raw diagnostic logs referenced below are retained in ignored task output; current checksums and compact results are in `proof/recovery/native-live-membership-20261101/recent-owner-run-evidence.json`.

# Ordinary owner catalog checkpoint qualification

Final combined source: `28f79b2d104c0d5dcdbd79514068ccfad31f9ed9`. Root catalog worker: `a1ca7cf72a`; private admitted-readback witness: `5bf4a3e2d3`; initial helper/tests: `8d8542320c`; exact queue configuration: `91122562bd`.

Final exact command, exclusive existing native cache:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage -- --nocapture
```

Result: exit 0; **11 passed, 0 failed**, 385 filtered; 14.00s tests (`tests-combined.log`). No new warnings. Cache released after terminal completion. No broader target/release/alternate-feature qualification is claimed.

The actual catalog fixture performs publication and independently authorized exact chunk readback on the original Owner/Host before private `verified_reference()` becomes available. The helper uses only that verified reference in `PreparedRecordingCatalog::prepare`, constrained by the selected actual directory profile's existing record/content bounds. It computes the immutable catalog reference from SHA-256 of its canonical content, compiles the existing authored catalog-checkpoint Source, extracts the canonical Body keep's initializer/type/Gear-based State ID/max1024 binding, and selects the exact authored local some(.) expression. Expression refresh precedes actual snapshot+State generation selection. Independent caller publication authority is validated against the new exact selected offer; the std State offer requires no manufactured grant.

The original owner installs the ordinary same-Body workset/proposal and persists it, never returning a sidecar Plan or replacing the Host advertisement/ledger. Both the complete planned State and its actual lowered State must match the extracted stable binding. Internal Cords explicitly select item capacity 1 and byte capacity 1024 within existing Host limits; external catalog input remains bounded to 4096. The actual selected-storage worker observes the exact current Some(catalog reference) after the State transition and returns the original Host. The synthetic accepted source observation, prepared prefix and private chunk-readback witness remain retained, and producer retirement remains unauthorized.

Negative proof refuses catalog preparation before any generation/workset/file mutation when the private readback witness is absent. A separate canonical test proves State binding stability across two snapshot references and the existing State-only read Source. Existing stage/refusal/publication/readback/corruption tests pass together after the shared worker enum change.

Intermediate evidence: `tests-8d854.log` records 2/3 pass with the actual catalog Plan correctly refused because ordinary default internal queue capacity 4 exceeded the selected one-item snapshot limit. The correction selects existing explicit queue limits; it raises no Host/offer/grant limit. `tests-queues.log` records 3/3 passing targeted catalog tests, before removing an unused initial-value allocation/import and performing the final combined check.

This is local actual owner snapshot/catalog/State-path proof using a synthetic accepted codec prefix. It does not prove prior sensing, fresh-Boot State recovery, independently reopened catalog/chunks, Recorder ACK, producer retirement, whole-Root allocator admission or physical power-loss survival. The caller preparation limit is a named input/source volume constraint, not an allocator footprint. Refusal after ordinary selection or replacement may leave that prepared generation/resident in place; no rollback claim is made. No original staged record is consumed.
