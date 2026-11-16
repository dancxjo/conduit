# Kernel routed abnormal evidence stock — #6341

API: `FixedScheduler<..., TERMINAL_ROWS, PHASE_BYTES, UNRESOLVED_NODES=NODES>` and `PreparedKernelSession<..., TERMINAL_ROWS, PHASE_BYTES, UNRESOLVED_NODES=N>` preserve default stock. Zero stock is admitted only when the complete active Cord topology has no AbnormalTerminal tracks. Partial node capacities refuse rather than pretending to be a sparse map.

The actual inline unresolved array uses this bound. Safe checked access covers retention, projection, recovery and drained inspection; the zero array is never indexed. CanonicalValue, CordState, phase terminal representation and full 100-byte semantic errors are unchanged. No new unsafe code or alternate runtime was added.

Ordinary constructors and boxed preparation reject incompatible known topology before allocation/provider binding. The in-place writer necessarily first produces topology in guarded physical storage; it is checked before storage escapes and before provider bind/Start. Refused preparation drops owned values once. Rearm preserves immutable topology and clears the possibly empty stock. Region bindings do not replace Cord tracks.

Validation (CARGO_INCREMENTAL=0; leased CPU cache /tmp/conduit-source-member-control-20261111):
- cargo test -p conduit-kernel --all-features --lib unresolved_storage -- --nocapture: 6 passed.
- cargo test -p conduit-kernel --all-features --lib: 265 passed, 3 ignored. Includes additional repeated U0 rearm/cancellation test and existing full-stock routed abnormal test.
- cargo check -p conduit-kernel --no-default-features: passed.
- cargo test -p conduit-kernel --no-default-features --lib: 172 passed, 3 ignored.
- cargo clippy -p conduit-kernel --all-features --lib --tests --no-deps -- -D warnings: passed.

CPU lib tests provide semantic/storage evidence, not ARM size or hardware proof. Parent owns Source-derived selection, target ABI measurement and real MCU preparation/release qualification. No commit/push by child agent.

Environment: three builds hit cache disk quota. No active Cargo process remained; removed only inactive leased incremental cache (734MB), then cargo clean -p conduit-kernel removed 457MiB and subsequently 290.8MiB regenerable outputs. Existing source/evidence/credentials untouched.
