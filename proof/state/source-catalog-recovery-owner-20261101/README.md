# Fresh-process ordinary owner catalog State recovery

Production/helper source qualification: `4aa1104ef7`; final combined source after removal of one unused test import: `b50ae33863063a402086033b55aa6917a17de5ad`. Helper source starts at `573e2b1c69`; owning test corrections/negatives are included in the final source. Raw logs stay in ignored task output; exact paths, lengths and SHA-256 hashes are in `log-custody.json`.

All commands use the existing exclusively owned native cache and this environment:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
```

Commands and terminal results:

- `cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage::recovery::tests -- --nocapture` at `4aa1104ef7`: **3 passed**, 396 filtered, 21.57s build / 22.52s tests.
- `cargo +stable test -p conduit-std-host --lib --features linux-snapshot declared_name_ -- --nocapture` at `4aa1104ef7`: **3 passed**, 1007 filtered, 0.23s tests. Two new ledger-generation tests and the existing bounded text presentation test. One unused test import warning was removed mechanically at final source; no behavioral change or warning suppression.
- Final combined `cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage -- --nocapture` at `b50ae33863`: **14 passed**, 385 filtered, 25.77s tests; no new warnings.

The positive parent runs actual publication, independently authorized chunk readback and catalog checkpoint on the original Owner. Ordinary checked Host configuration selection retains the real storage directory device/inode before process handoff. Parent drops its Owner and launches `current_exe --exact` with only the installation root as custom recovery context. Child reads the installation, attaches its retained checked selection to a new Host Boot, and calls ordinary `resume_service`. It has no volatile recording stage, verified snapshot witness, expected catalog reference or old grants.

The State-only helper parses the actual authored `catalog-seed-read.conduit`, reuses canonical seed extraction, selects the exact Body binding/type/default-none/1024-byte profile on the child's original Host, and proposes/persists the ordinary same-Body Plan with explicit one-item queues. It returns no sidecar Plan. The actual selected-storage worker discovers unknown current Some; clean joined lifecycle persistence precedes private recovered token creation. Parent compares child's recovered canonical reference bytes, stable binding and Body with its oracle only after child termination, and verifies the Boot changed. Child asserts actual Current observation, recovered reference and no producer-retirement authority. No snapshot/read grant is fabricated for State recovery.

Negatives prove absent State emits explicit None without a recovered reference, and a zero caller preparation-volume limit refuses before generation, resident, proposal or State-file change. The preparation quote is named source/input volume, not whole-Root allocator admission.

The first compile at `573e2b1c69` exposed a moved relative path in Root's start-method extraction and owning test serialization mismatches. Corrected compile at `104ae0d82d` reached the true child and correctly refused a stale ledger generation after checked declared-name attachment. The canonical metadata repair at `4aa1104ef7` validates exact idle inventory and advances only the existing ledger generation alongside advertisement metadata; it preserves original pool/instance objects. No recovery check was weakened.

This qualifies local fresh-process recovery of the catalog reference through ordinary Body State, using a synthetic accepted Source codec prefix. It does not qualify recovered catalog/chunk reads, full recording reconstruction, ACK, producer retirement, prior sensing, installed physical-device operation, allocator admission, or physical power-loss survival. Publication/readback/catalog still leave staged producer records unacknowledged. Raw evidence is local ignored custody, not a published archive.

Required `cargo +stable xtask ci pipeline evidence-retention` passed (exit 0) on final source. All Cargo processes for this qualification are terminal; the shared native cache is released. No raw logs were added to Git.
