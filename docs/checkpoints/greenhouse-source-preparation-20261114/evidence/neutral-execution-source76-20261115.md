# Actual checked Source76: complete CPU admission and execution

The test in `targets/rp2040/make/tests/neutral_execution.rs` compiles the actual neutral RP2040 board modules against their original ordinary Make OUT_DIR, then executes them with explicitly owned CPU I2C and clock providers. No production source, factory, refusal, service, firmware or resource grant was changed. It reuses the existing exact signed-workset fixture helper in neutral_source/positive.rs.

## Proven path

- Original checked Source hash and exact independent installed definitions/identities; 76 nodes, all active Cords and all 12 local Fore pins. Original projection validation: 49 resource requirements, 2 native Calls, zero remote bindings.
- Original Root Seed and authenticated membership receipt; native Issuers authorize exact original equipment, and the signed 30,314-byte complete workset is retained within the unchanged 32,548-byte input bound.
- Original neutral_factory::prepare constructs the complete workset and actual value/schema/Sign/workspace stocks.
- Whole PreparedLocalWorkset admission uses the original Root ledger and an observation for every original pool. Observation backing is conservatively funded by its measured cold-construction peak, including dynamic text backing.
- Current authenticated Start reborrows and checks the same retained command and live complete ledger before binding original native providers. A repeated Start is refused.
- The ordinary shared native Runner services its actual specialized Source kernel and original I2C/clock owners. All three typed ingress contracts are validated before enqueue; configuration and begin are closed after their values, and frame is supplied only after initialized.
- Exact CPU register transcripts execute 48 BME transactions and 6 HT16K33 transactions (five initialization and one frame write), once each and in per-device order. Clock observations are monotonic and satisfy Source-owned deadlines. No provider returns an already computed environmental observation.
- All nine typed egress boundaries are validated and serviced. Captured, observation, initialized and written each emit once; sensor_refused, clock_failed, initialization_refused, write_refused and unrouted emit no values and close normally. Every output closes and the whole kernel drains.
- Borrowed allocation-free assertions check captured phase 14 and the actual Source compensation result: temperature 2508 centidegrees Celsius, pressure 25767233 Q24.8 Pa, humidity 55588 Q22.10 percent.
- Execution attempts zero allocations. Both native capabilities finish Revoked with zero in-flight operations.

## Qualification

Final explicit ignored test: 1 passed, 27.47 seconds. Log: `outputs/neutral-execution-source76-20261115.log`.

Environment: TMPDIR is task/work/source-proof-tmp-20261115; CARGO_TARGET_DIR=/tmp/conduit-source-member-control-20261111; CARGO_INCREMENTAL=0; CARGO_PROFILE_DEV_DEBUG=0; CARGO_PROFILE_TEST_DEBUG=0; CARGO_BUILD_JOBS=4.

Command: cargo +1.98.1 test --locked -p conduit-host-rp2040 --test neutral_execution -- --ignored --nocapture, with CONDUIT_NEUTRAL_CPU_OUT_DIR pointing to the selected original neutral firmware OUT_DIR.

The outer test/typed-fixture harness also passes scoped strict Clippy: cargo +1.98.1 clippy --locked -p conduit-host-rp2040 --test neutral_execution --no-deps -- -D warnings. Embedded probe modules are compiled and executed by rustc inside that test. git diff --check passes.

## Deliberate limits

This is a CPU execution proof, not MCU fit or physical sensor/display evidence. The CPU thread has an explicitly funded 32 MiB stack; the preparation/admission ceiling is 1 MiB. Root/equipment/native fixture metadata and CPU allocator overhead are not a claim to fit the RP2040's original 131,072-byte heap. Parent target-ABI/link/preparation measurements remain separate.

This exact authored Source is a finite environmental exchange. It does not author periodic sampling, a Face, a Mask, temperature-to-display mapping, durable history or autonomous restart. The supplied display frame is a typed RAM fixture; it is not a fabricated thermometer Mask. Those are next authored/installed composition obligations. The production neutral service correctly remains unable to Start without its own installed external Fore service. No partial subset is reported as a continuing Body Play.

New files only under neutral_execution*. No commit or push by child agent.
