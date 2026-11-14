# RP2040 bounded I²C provider

This target leaf adapts the pinned Embassy RP2040 asynchronous controller to
`conduit-i2c-transaction::I2cProvider`. It contains no sensor registers, calibration,
address discovery, retries, Body coordination, or offer/admission policy.
`Rp2040I2c::new` consumes an actual configured HAL controller and an explicit
nonzero cumulative polling allowance. I2C0 and I2C1 have separate native bindings.
The shared request already bounds both written and read payloads to 32 bytes;
combined write/read uses the HAL's repeated-START transaction.

The transfer allowance is a work bound, not a wall-time guarantee. Wakes do not
replenish it. A transfer future is dropped before failure quarantine, so its
borrow of caller storage has ended. The HAL I²C path uses no DMA. On an error or
poll exhaustion the owner masks that controller's interrupts and holds only that
controller in reset. Cleanup has its own bounded observation loop, checking reset
assertion and reset-done deassertion. If that observation fails the result is
`ProviderLost`; the provider remains permanently revoked in either case. There
is no implicit recovery, reinitialization or capacity regrant. Explicit revocation
also quarantines the owned controller. Wrong output storage refuses before IO.
The blocking HAL methods are deliberately unused because their waits are unbounded.

Host construction must supply the finite polling allowance, and Root must supply
current native identity, exact physical attachment, possession and ordinary
assignment before constructing a `ReadyI2cBase` or bound Host Call. A configured
controller alone establishes neither membership, available offers nor a Play.
The firmware source now routes its diagnostic sensor/display drivers through
this owner using the required `maximum_transaction_polls` in the checked I²C
Base initialization. The native HAL adapter supports a single read, write or
combined write/read and refuses other geometry before partial effects. It grants
no additional authority. This integration has linked within existing firmware
bounds, but it has not been deployed or physically exercised. Green's running
firmware remains unchanged. Moving ownership into a planned Host Call, physical
timeout/reset behavior and execution peak memory remain separate proof gates.

Validation (from the repository root):

```sh
cargo +stable test --locked --manifest-path targets/rp2040/i2c/Cargo.toml
cargo +stable check --locked --manifest-path targets/rp2040/i2c/Cargo.toml --features rp2040 --target thumbv6m-none-eabi
cargo +stable clippy --locked --manifest-path targets/rp2040/i2c/Cargo.toml --features rp2040 --target thumbv6m-none-eabi --lib -- -D warnings
```

The CPU fixtures prove finite polling, storage-borrow retirement, completion on
the last allowed poll, and preservation of success/error without retry. They do
not exercise hardware reset registers or provide physical confinement proof.
