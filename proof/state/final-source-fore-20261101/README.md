# Final Source Fore custody after worker join

Qualified source: `71c030a863498175135602b814197cd1c77232d0` (base `bb5289d01aa4d7e0d32680bb7fce9561aa4df6f9`). Production scope is `body_owner/continuing.rs`; dedicated owning fixture is `continuing/final_fore_tests.rs`.

Exact command, exclusive existing native cache:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit final_fore_tests -- --nocapture
```

Result: exit 0; **2 passed, 0 failed**, 386 filtered; 3.65s test execution (`tests.log`). Cache released after terminal completion; no broader test or release build claimed.

The fixtures run an actual Owner/Clock Body worker with its original Host and start/lifecycle handshake. A gated wrapper waits until the consumer has observed the original one-slot receiver Empty, then publishes its final Fore and exits before join returns. The production post-join completion helper must retain that final delivery as an explicit typed refusal before writing the lifecycle receipt, or preserve it as the trailing value of an existing refusal. The fixture deliberately uses foreign Fore identity: acceptance must refuse, without dropping the bytes or creating another stop request after producer quiescence. No timing sleep establishes the final-slot ordering.

This proves local completion ordering and refusal custody, not acceptance of foreign identity, a Recorder durable ACK, physical sensing, or publication durability. Existing exact typed acceptance remains the only route for a valid final Fore. No Source semantics, advertisements, registry entries or grants change.
