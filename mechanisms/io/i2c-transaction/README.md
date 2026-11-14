# Shared I²C transaction realization

This `no_std` + `alloc` mechanism owns finite, class-neutral I²C transactions and their exact admitted Host Call boundary. Register addresses, initialization, calibration, compensation and retry policy belong in authored checked Plots.

The request and result Forms come from `plots/device-protocols/i2c-types.conduit`. Existing Kind, Fore, implementation, execution-profile and authority identities remain unchanged. `conduitos::i2c_base` forwards this API; the Intel i801 physical controller remains in ConduitOS.

The optional `operation-factory` feature supplies the rich host scheduler factory through `conduit-composite`; ConduitOS enables it explicitly. The default portable contract/owner path avoids that crate’s pointer-atomic `Arc` requirement on ARMv6-M.

Preparation constructs the exact contract, canonical decoder/encoder and scheduler Back without bus traffic. A trusted composition root supplies exclusive native attachment truth and binds the selected Plan, lowering, active Play and capability possession. Runtime calls preserve bounded sequence/lease accounting, canonical outcomes and revocation. Transactions permit at most 32 written and 32 read bytes; combined write/read requires repeated START. A provider must return quiescent or remain quarantined.

`I2cContract::prepare` decodes immutable schemas checked from Source during fabrication. The default `source-catalog` feature retains authoring catalogs; MCU consumers disable default features to omit the runtime parser dependency. Result composers borrow linked schemas and reserve output lengths derived from the full finite checked result shape, within the unchanged Host Call ceiling. Their retained heap payload is inspectable before admission; this does not make all preparation allocation fallible or prove MCU peak RAM. Provider installation, same-Body admission and sensor Plot execution remain separate.

Validation:

```sh
cargo +stable test --locked --manifest-path mechanisms/io/i2c-transaction/Cargo.toml
cargo +stable clippy --locked --manifest-path mechanisms/io/i2c-transaction/Cargo.toml --all-features --all-targets -- -D warnings
cargo +stable check --locked --manifest-path mechanisms/io/i2c-transaction/Cargo.toml --target thumbv6m-none-eabi --no-default-features
```

Private-owner tests move with the implementation. ConduitOS retains its public contract, installation, i801 and authored protocol integration suites against the forwarding API.

Compact assigned binding also supports an exclusive prepared ticket. Preparation
checks exact assignment/Fore/possession and retains codecs while the controller
stays in its original ready slot. Dropping the ticket cancels preparation;
consuming it transfers that same controller without allocation. Root must still
admit the complete workset and authorize Start and retirement independently.

Assigned preparation also validates the actual opaque bearer against its owned
capability table before creating a ticket. A valid serialized assignment with a
foreign, revoked or exhausted local grant cannot take the initialized controller.
The ticket exclusively owns that non-Clone table and move-only bearer, so safe
public APIs cannot change its lifecycle after preflight. Its read-only backing
receipt and node inspection neither authorize nor mutate work. Binding remains
allocation-free and transfers the original provider once; Root still controls
whole-workset admission, Start and retirement. Electrical address checks and
finite attachment/operation bounds are unchanged.
