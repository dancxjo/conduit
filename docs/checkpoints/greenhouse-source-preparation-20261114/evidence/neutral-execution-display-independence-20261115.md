# Source76 measurement survives display NACK

Added a second scenario to the existing neutral_execution harness; shared Root setup, exact installed Source/factory, admission, authenticated Start, typed Fore service and ordinary Runner are reused. No production files or capacities changed. This uses the current original OUT_DIR with EXTERNAL_INGRESS_CORD_COUNT=3 and the corresponding Z3 kernel profile.

The actual CPU I2C provider rejects the first HT16K33 (address112) initialization command 0x21 with I2cDisposition::NotAcknowledged. That becomes the original typed I2C result; the checked Source emits initialization_refused(bus(not-acknowledged)). There is exactly one display transaction and no retry, initialization success or frame write. The frame input receives normal closure without any value after that refusal. It is never staged before initialization or fabricated to force progress.

The same Started Source continues the entire BME280 (address118) protocol: 48 correctly ordered transactions, captured phase14 and an exact numeric observation (2508 centidegrees Celsius, 25767233 Q24.8 Pa, 55588 Q22.10 percent humidity). The Runner drains normally without cancellation. All nine output boundaries close. In declared order the output counts are [captured1, observation1, sensor_refused0, clock_failed0, initialized0, initialization_refused1, written0, write_refused0, unrouted0]. All three input boundaries are accounted: two receive values and close; frame closes without a value.

Execution attempts zero allocations. Both original native capabilities retire Revoked with no in-flight operation. The success scenario still proves 54 transactions and the original captured/observation/initialized/written outputs.

Qualification: both ignored neutral_execution scenarios passed together (59.26 seconds); scoped outer-test strict Clippy (-D warnings) passed; git diff --check passed. Log: outputs/neutral-execution-display-independence-20261115.log.

Command: cargo +1.98.1 test --locked -p conduit-host-rp2040 --test neutral_execution -- --ignored --nocapture --test-threads=1, with the original neutral OUT_DIR and the existing CPU cache, task-local TMPDIR, debug0, incremental0 and jobs4 environment.

This is finite CPU protocol/independence evidence. It does not claim periodic overnight sampling, physical display acknowledgment, target heap fit, a Face/Mask, persistence or installed board readiness. The explicit CPU 32MiB stack and 1MiB preparation ceiling remain unchanged.

Only neutral_execution.rs, neutral_execution/execution.rs, neutral_execution/exercise.rs and neutral_execution/protocol.rs changed. No commit or push by child. Source frozen and CPU cache lease released.
