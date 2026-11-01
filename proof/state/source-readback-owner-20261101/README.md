# Original owner admitted snapshot readback qualification

Qualified combined source: `988e028daefaf395f53570ab5e94a3ddcb223faf`; production readback extension `7e3d0e8fa0b8ba9da95dff4bf1314ec7603ee8ff`. This change adds owning tests only.

Exact command, exclusive existing native cache:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage::publication::tests -- --nocapture
```

Result: exit 0; **4 passed, 0 failed**, 389 filtered; 11.12s compile, 8.77s tests (`tests.log`). Cache released after terminal completion. No broad/alternate-feature/release build claimed.

The new positive fixture publishes through the actual original Owner worker and selected directory, proves the publication grant cannot authorize reading, then explicitly authorizes the exact ReadPublished offer with a separate read grant. It removes the process-local publication receipt before invoking the ordinary authored readback worker, demonstrating that a retained publication reply is not a read prerequisite. The actual resource/read-json-snapshot Body Play and strict byte-comparing Fore consumer complete successfully. Same Body, Host and Boot remain; the original observation bytes, complete prepared snapshot bytes and backing pointer remain unchanged, with the staged record still pending and producer retirement explicitly unauthorized.

The corruption fixture deliberately replaces the published record with an invalid envelope solely to inject a fault. It then invokes the same admitted readback path, which cannot claim matching content or completed readback. It returns the original Host, clears the current Play normally, and retains the staged prefix. Direct fixture filesystem access supplies neither readback evidence nor authority.

The existing publication positive and stale/missing-grant refusal tests also pass. The accepted input prefix is synthetic codec input; prior sensing/native admission is not claimed by this fixture. Publication plus independent readback still does not prove catalog State commitment, a durable Recorder ACK, producer retirement, whole-Root workspace admission, physical sensing or power-loss survival. No staged data is consumed by either operation.
