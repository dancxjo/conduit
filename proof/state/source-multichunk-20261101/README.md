# Whole Source prefix qualification — 2026-11-01

Production source `057f48d32ce3223ce1530ddc1f50f2d53e3affe7` retains every accepted slot, freezes the finite prefix at first checkpoint preparation, and selects the next existing4096-byte codec chunk only after private admitted readback verification of the current chunk. Explicit index/SHA compare-and-select admits only the current chunk; repeating the immediate previous successful request returns the same next chunk. The archive retains bounded verified references/ranges, not duplicate chunk content. Catalog preparation refuses until contiguous verified coverage includes the complete frozen prefix. Per-prefix clock/counter validation prevents chunk boundaries resetting Source continuity; generic catalog readers are unchanged.

The process fixture stages16 synthetic canonical Available observations, the canonical maximum256-byte attachment identifier, actual4096-byte records, selected State1024, and selected snapshot-record capacity16. Natural codec fit yields two chunks. Original canonical slots/global recording sequences remain retained through publication, independent readback, both chunks and the catalog; no producer retirement or ACK is authorized. A new subprocess receives only the installation root, reattaches the checked selected-directory identity to the same Host/new Boot, recovers unknown State, independently reads the catalog and both chunks through ordinary workers, and returns content evidence. Parent compares all16 original `(sequence, canonical bytes)` and both original Source scopes/digests. Corruption of all three snapshot files fails closed without private read token/content evidence.

Qualification uses the existing native cache with `CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`, and `cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit FILTER -- --nocapture`.

- `source_recording_stage::advance::tests`:2/2 passed at `a31e5c6ca2`,29.82s build/18.64s tests. Subsequent `057f48d32c` removes only an unused test import.
- `source_recording_stage::recovered_read::tests`:2/2 passed at `057f48d32c`,11.44s build/45.84s tests.
- `source_recording_stage`:18/18 passed at `057f48d32c`,51.15s tests.

- `body_recording`:8/8 passed at `a62836bb13`,22.11s tests. This includes original six operations and authenticated next-chunk/no-Play/idempotent response qualification. An earlier7/8 run at `057f48d32c` exposed eager test-array read-authority selection before publication existed; `a62836bb13` changes only that test to publish/finish before selecting ReadPublished. Production remains byte-for-byte source-equivalent to `057f48d32c`.
- `native_return`:6/6 passed at `a62836bb13`,1.70s tests, including actual control change between challenge open and signing.
- `native_part_custody`:2/2 passed at `a62836bb13`,0.86s tests.

The initial `2a8b84b917` build stopped before product compilation because shared-cache std-offers metadata lacked the already committed bounded-State factory. Inspection confirmed the function and wildcard exports in source; package-native `cargo clean -p conduit-std-offers` removed84.4MiB of regenerable artifacts. The exact rerun compiled and passed. No production correction was needed. One unrelated merged `RetainedPart::open` dead-code warning remains. Raw logs are ignored outside the repository; checksums and paths are recorded in the companion custody manifest.

This is local product/process qualification. It does not prove deployment, physical MCU observations, salvage of the older live executable's retained RAM prefix, producer retirement, or ACK. Root owns live installation and its existing custody hold.

Required `cargo +stable xtask ci pipeline evidence-retention` passed exit0 with the new compact proof staged. All requested Cargo/test processes are terminal; native cache released. The shared native cache was not used concurrently.
