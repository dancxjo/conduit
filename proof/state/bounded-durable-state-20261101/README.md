# Selected durable State value-bound specialization

Exact source `68a0fbd1ca1e722acd09cc32d25785a0c51935c6`. The selected storage profile allowed1024B and its canonical keep requested1024B, but generation preparation demanded capacity≥generic4096B implementation because the factory advertised4096B Host Calls. Refusal was correct for that advertised offer; increasing capacity/grants is not this fix.

New general `state_value_durable_std_offer_with_bound` validates1..4096B, borrows the default factory's already encoded canonical initializer to verify it fits, and narrows only maximum queue bytes, commit input bytes and recovery output bytes. The existing default4096 factory body/descriptor, semantic kind, implementation ceiling/identity, retention, receipt/recovery-metadata bounds, resources and authority requirements remain unchanged. Selected generation now calls it with the exact binding maximum after the existing configured-capacity check. No new kind/protocol/authority, Host/registry/ledger clone, provider release/regrant or Source mutation is introduced. Existing Core prepared update/scoped ledger refresh still performs exact inventory transition.

Unit proof covers exact4096 descriptor equality with default, selected1024 limits, canonical default exactfit/unfit, zero/over4096 refusal. Generation proof configures actual1024, refuses4096 before mutation/files, then accepts exact1024 and preserves existing offer payload allocation, unrelated resources/capabilities and Host Sign/Play cursors. The actual product original-Owner fixture's selected state capacity is corrected4096→1024, so ordinary catalog checkpoint, fresh-Boot State recovery and independently admitted catalog/chunk reads exercise the deployed profile instead of hiding the mismatch. The product accepted observation is synthetic codec fixture data, not physical sensing proof or a new actual native-Source Base initialization.

Exclusive existing cache environment:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
```

- `cargo +stable test -p conduit-std-offers --lib bounded_durable_offer -- --nocapture`: **2/2 passed**,52filtered;1m47s build/0.00s tests.
- `cargo +stable test -p conduit-std-host --lib --features linux-snapshot body_storage_generation::tests -- --nocapture`: **5/5 passed**,1006filtered;28.97s build/0.48s tests.
- `cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage -- --nocapture`: **16/16 passed**,391filtered;33.84s build/42.03s tests, including actual1024 catalog/recovery/read and corruption refusal.

Raw logs remain ignored task output with exact local paths/sizes/SHA256 in log-custody.json. No raw logs are tracked.

This fixes future-image admission. It does NOT modify or salvage the currently running older Owner. Root explicitly holds PID2522454 with16 accepted volatile records, first9 published/readback verified but not catalog committed and7 unpublished. No restart/reconfiguration was performed by this work, and no all16 durable/ACK/producer-retirement or physical restart success is claimed. Current API has no fullstage export/import or prepared-chunk advance; see task outputs/live-stage-state-bound-audit-20261101.md for the precise custody gap and conditional existing Face-evidence avenue. Root separately owns optimized release and any operator custody decision.

- `cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit body_recording -- --nocapture`: **6/6 passed**,401filtered;25.12s tests, actual authenticated original-Owner six-operation path now selects1024State.

Required `cargo +stable xtask ci pipeline evidence-retention` passed exit0, including current tracked Root physical deployment receipt/summary working-tree updates. All child Cargo processes are terminal; native cache released. Root's separate release build and live custody hold remain Root-owned. No failed test run/source correction or new warning occurred in this qualification.
