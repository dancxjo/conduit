# Admitted fresh-process catalog and chunk recovery

Qualification source/result entries are completed below after terminal checks. Raw logs belong only to ignored task `target/recovered-source-read-20261101`; checksums and absolute paths are retained in `log-custody.json`.

The fresh-process test starts with an actual original-owner publication, independently authorized readback and catalog/State checkpoint. Its accepted observation prefix is synthetic codec fixture data, not native sensing proof. Ordinary installed checked storage configuration retains the directory device/inode. Parent drops its owner and passes only the installation root to a new `current_exe --exact` child. The child creates a fresh Boot, attaches the actual retained checked storage selection and resumes the original Body. It discovers unknown State current, then uses independently fixture-authorized current-Boot ReadPublished offers for the discovered catalog and its exact indexed chunk. No old grant, stage, witness, expected reference, expected content or serialized Owner crosses the process boundary.

The helper validates private State-token Body/Host/Boot custody, and permits only that catalog or a reference indexed from privately verified catalog contents bound to it. It selects the existing ReadPublished generation, validates the exact independent current read grant, and installs/proposes/persists the ordinary authored recording-readback workset on the original Host. Queues select one item and at most 4096 bytes. Preparation is named source/reference volume, not whole-Root allocator admission; refusal after selection may leave a prepared generation/resident. No rollback is claimed.

Root's worker captures actual content Fore into a fixed 4096-byte buffer, verifies the selected immutable reference digest and bounded canonical catalog/chunk codec after clean completion/quiescence, and accepts private results only after exact issued Play terminal lifecycle persistence. The child checks the complete one-reference catalog is verified and historical Source Boot remains distinct from its fresh collector Boot. Parent compares full canonical recovered bytes, catalog/chunk references and original Body only after child termination. This is admitted readback, not direct file reading dressed as authority.

Negative assertions cover absence of private State recovery before any generation/resident/proposal/file mutation, stale-Boot read authority before generation mutation, chunk selection before admitted catalog contents, and an index outside the verified finite catalog.

This is local actual Owner/selected-storage/fresh-process read proof. It does not authorize ACK or producer retirement, prove prior sensing, installed device execution, physical power-loss survival, or whole-Root allocator custody. No staged producer record is retired by successful recovery.

All qualification commands use:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
```

The initial exact owning run at `914c03542fc4819bbdc85fd6cde2e057e90c28bc`:

```sh
cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage::recovered_read::tests -- --nocapture
```

passed **2/2**, 399 filtered; 29.88s build / 29.07s tests. No new warnings or failure. Test-only actual read completion/no-retirement assertions were subsequently added at `8acfd6280e`, for the final combined run. Final codec boundary source is `2ba21aa03058dc21db95470bb18e3fae0479db9c`.

Intermediate owning codec command at `2ba21aa030` exited 101 before tests: its new boundary test referred to `MAXIMUM_BYTES` ambiguously from measurement and recording glob imports. The ignored failure log is retained; this is a test compilation issue, not a runtime acceptance failure. The final exact rerun below uses the explicitly qualified recording constant.

The corrected owning codec command at `de213ff223` passed **5/5**, 0.19s build / 0.03s tests:

```sh
cargo +stable test --manifest-path bodies/environment/runtime/Cargo.toml --features source-recording --test source_recording
```

This covers existing exact pinned context/Body/session/play and finite append contracts plus candidate canonical decoding without authority, foreign-Body pinned-read refusal, contradictory native Boot, noncanonical bytes and candidate capacity refusal. Candidate decoding preserves historical context; actual immutable digest, catalog membership and current Body checks remain the admitted worker's responsibility.

Final combined source `de213ff22359c73b5345d79eeaf0705eef52c407` passed **16/16**, 385 filtered, 33.83s tests, no new warnings:

```sh
cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage -- --nocapture
```

Required `cargo +stable xtask ci pipeline evidence-retention` passed exit 0. All qualification Cargo processes are terminal and the exclusive native cache was released. No raw logs were added to Git.
