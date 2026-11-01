# Durable State receipt barriers qualification

Initial source: `c54609c4a0d137f27a220842be9dc435d832ea26` (base `843b78b300`), owning 13/13 passed. Final source: `b02e9f4bf9`, with selected-root Linux gates and owning test extraction; shared barrier behavior unchanged.

Exact command (exclusive native cache, no product/integration target):

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit-std-host --features linux-snapshot --lib state_value::durable:: -- --nocapture
```

Final result: exit 0, 13 passed / 0 failed; 986 filtered out, 2.17s test execution.

Output: `durable-tests.log` (initial) and `durable-tests-final.log` (final).

No non-Linux compile/run is claimed. Selected-root APIs and retained descriptor backing are Linux-only; the shared portable idempotent receipt barriers remain unconditional.

New tests inject only test-local synchronization failures: rename-visible but journal-sync failure cannot yield a receipt on a subsequent failing current-file or directory barrier; a successful retry returns the same exact receipt and only then advances the installed generation. Selected-root construction requires journal and actual parent barriers on initial creation and AlreadyExists retries, retains its directory descriptor across root rename, and refuses a symlink child.

These are local syscall/barrier and State protocol tests. Process-abort recovery is not physical power-loss proof. The legacy recursive directory creation API remains compatible; this change does not establish synchronization of its newly created root ancestors. Parent owns installed Body execution/kernel selected-root wiring and Recorder product qualification.
