# Shared I²C transaction realization

This `no_std` + `alloc` mechanism owns finite, class-neutral I²C transactions and their exact admitted Host Call boundary. Register addresses, initialization, calibration, compensation and retry policy belong in authored checked Plots.

The request and result Forms come from `plots/device-protocols/i2c-types.conduit`. Existing Kind, Fore, implementation, execution-profile and authority identities remain unchanged. `conduitos::i2c_base` forwards this API; the Intel i801 physical controller remains in ConduitOS.

The optional `operation-factory` feature supplies the rich host scheduler factory through `conduit-composite`; ConduitOS enables it explicitly. The default portable contract/owner path avoids that crate’s pointer-atomic `Arc` requirement on ARMv6-M.

Preparation constructs the exact contract, canonical decoder/encoder and scheduler Back without bus traffic. A trusted composition root supplies exclusive native attachment truth and binds the selected Plan, lowering, active Play and capability possession. Runtime calls preserve bounded sequence/lease accounting, canonical outcomes and revocation. Transactions permit at most 32 written and 32 read bytes; combined write/read requires repeated START. A provider must return quiescent or remain quarantined.

`ReadyI2cBase::check_assigned` and `bind_assigned_retained` also accept the
existing compact assigned-Plan projection, including borrowed record storage.
They validate the current Host/Boot, native provider and resource generations,
exact operation and locally issued capability before transferring the retained
controller. Refusal keeps that controller in Root; codec preparation performs
no bus traffic. The resulting Host Call retains the same address interval,
canonical outcomes, sequence accounting and revocation as full-Plan binding.
It does not borrow the received assignment after preparation. These APIs do not
install a physical MCU provider or authorize one from serialized facts.

`I2cContract::prepare` remains a preparation API using the checked Source parser. Extraction does not make live parsing appropriate for a small MCU, install an MCU provider, prove same-Body admission, or execute a sensor Plot. Those composition steps remain separate.

Validation:

```sh
cargo +stable test --locked --manifest-path mechanisms/io/i2c-transaction/Cargo.toml
cargo +stable clippy --locked --manifest-path mechanisms/io/i2c-transaction/Cargo.toml --all-features --all-targets -- -D warnings
cargo +stable check --locked --manifest-path mechanisms/io/i2c-transaction/Cargo.toml --target thumbv6m-none-eabi
```

Private-owner tests move with the implementation. ConduitOS retains its public contract, installation, i801 and authored protocol integration suites against the forwarding API.
