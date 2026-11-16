# Shared native runner storage profiles — #6341 integration

The ordinary native runner previously named `StoredExecutionKernel` without its queue/terminal-law/phase/unresolved profile parameters. That silently selected the full default profile at its API boundary, preventing a legitimately specialized kernel from reaching the existing execution path.

Runner now appends `QUEUE_SLOTS=C, TERMINAL_ROWS=N, PHASE_BYTES=CanonicalValue::MAXIMUM_BYTES, UNRESOLVED_NODES=N` after its existing ValueStorage parameter. Its Observer, constructors and method impls preserve those same parameters. Pulse, TM1637, relative waits, absolute deadline polling, synchronous native dispatch and their completion helpers forward that exact profile. Existing method-local B provider and Q wait-count generics remain independent. No scheduling, authority, cancellation, owner custody, failure mapping or retirement behavior changed.

New proof: `tests/native_sync/io/profile_runner_tests.rs` uses the existing paired I2C/clock owner fixture. Both capabilities are issued against one sealed Plan before bind. Cold preparation uses a checked linked immutable all-None law table for M0. The admitted active topology has no Cords and reserves Q1 rather than capacity C2 (Q0 is independently invalid). Actual Runner::run_with_i2c_clock executes Q1/M0/PHASE0/U0 with original bounded values, native owners, service buffers and timing activity interlock. It checks exact request/node/call identities, complete canonical results, exactly one physical I2C provider transaction, completion of both owners, cancellation while deadline pending, refusal without activity, owner retirement and no in-flight leases. Allocation tracking surrounds execution only and records zero allocation attempts. This is CPU provider/service evidence, not physical hardware or whole-Body admission proof.

Validation env: CARGO_TARGET_DIR=/tmp/conduit-source-member-control-20261111 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=4; cargo +1.98.1 --locked, standalone targets/mcu/host-runtime/Cargo.toml.
- all-features --test native_sync specialized_profile: 1 passed (three execution/refusal modes).
- all-features --lib --test native_sync: 140 library tests and 21 native_sync integration tests passed.
- clippy --all-features --lib --tests --no-deps -- -D warnings: passed.
- git diff --check: passed.

No commit or push by child. Parent owns Source lowering, MCU admission/target qualification and integration commit.

Environment: initial unqualified debug feature graph and the first correct-env attempt encountered tmpfs per-user quota. Package-native cleanup removed 235.5MiB inactive host-runtime build outputs; correct all-feature debug0 build then succeeded. No unique source, credentials or evidence removed.
