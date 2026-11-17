# Green closed sampling integration boundary

Current production reduction and corrected accounting: [WORKSPACE-REDUCTION.md](WORKSPACE-REDUCTION.md). The initial261e evidence below remains historical.

**Accounting correction:** RP2040 retains these Owned Backs in its static `OWNER` inventory (`host/src/resident.rs`), funded by the static IMAGE reservation. The 208,884-byte payload is permanent inline/static SRAM, not a heap allocation. Its comparison with the 49,152-byte arena is a budget reference, not an existing heap charge. Shared workspace moves only the maximum transient pair into cold-funded kernel backing.

This candidate does **not** qualify Green firmware or a complete environmental Play. It isolates the first actual production blocker without replacing the application with the old open-Fore environmental service or a diagnostic sensor loop.

Base: `823fd1b0d15631fec18fd7cf7296a955a91a2063`, a descendant of frozen qualified MCU Source `8cebe9cddd771d70fb2ff287031b1753978cd3b2` (their merge-base is exactly 8ce). New authoring input SHA256: `361a1ca98a0eb9e2b0f4a2721a51a6930563d070cfeb52219f06efc9bbb3aa70`. This is a distinct candidate, not the frozen ACA Source/fabrication identity. No Make IMAGE, fabrication identity, sealed artifact, admission or Started receipt was produced.

## Concrete cut

`src/bme_sampling_build_tests.rs::selected_source` extracts the original checked finite repeated sampler and exact standard-family selections. Its existing test still passes. `src/green_closed_source_tests.rs` is registered beneath installed-value tests so it can reuse the original private fixture configuration path without changing any production API.

The new core has no input bindings: original `InstalledValueBindings` emits checked canonical `BmeSamplingBegin` once; original `machine/i2c/transact` closes request/result feedback; the original sampler owns the clock and repeated acquisition policy. Fixture-only equipment material selects address 118, count 2, completion-relative period 1000ms after checking Source. The Source itself does not select a physical attachment. Captured state, semantic observation and failures remain terminal output Fores. They are **not** standard Mask/display consumers, and no Start is attempted.

## Measured refusal before adding Mask/HT16

| Item | Exact result |
| --- | ---: |
| Expanded nodes / Cords | 81 / 91 |
| Resident / software owners | 52 / 27 |
| All resident computation work | 17,282 units |
| Original aggregate work pool | 5,000 units |
| Largest resident / conservative261e fixture ceiling | 5,379 / 4,997 units |
| Largest resident / actual BSP individual ceiling | 5,379 / 5,000 units |
| Ordinary aggregate preparation refusal | first cumulative 5,073 >5,000 |
| Owned canonical output + scratch payload alone | 208,884 bytes |
| Original entire heap arena | 49,152 bytes |
| Independently compiled x86_64 Owned Back inline storage | 234,704 bytes |
| Scoped preparation heap retained / peak | 0 / 0 bytes |

The 208,884-byte lower bound is architecture independent: every original Owned resident retains output and scratch `[u8; B]`, and B is its original exact output contract extent. It excludes native metadata, software owners, Cord/value store, kernel, I2C/clock and allocator overhead. The x86_64 inline number is not an RP2040 layout measurement. No runtime execution, full-kernel preparation peak or whole application workspace/heap number is claimed. There is no lawful application runtime to measure after these earlier refusals.

To inventory demand, only the **host diagnostic compiler envelope** allows all individual expressions to be described. This envelope is never given to calibration, sealing, admission or target configuration. The original target pool remains 5000 and its real refusal is asserted separately. No 8192-byte buffer, 49152-byte heap or other profile grant is changed. SharedWorkspace/SharedWorkspaceSchema is not silently substituted for the BSP's actual Owned selection.

## Native reuse boundary and responsibility

Existing `mechanisms/io/bosch-env/src/lib.rs::Sensor` is a bounded async register driver with cancellation-invalidated readiness. Its current only Green integration, `targets/rp2040/firmware/host/src/environment.rs::run`, is explicitly a diagnostic polling/display loop; using it as the application would bypass original factory, admission and Start.

It also cannot be aliased to original Source compensation: `plots/device-protocols/main.conduit::BmeObservation` retains pressure in q24.8Pa; `mechanisms/io/bosch-env/src/calibration.rs::compensate` converts that intermediate into rounded centipascal `Reading`. The native driver exposes different failure/range/BMP behavior. An exact checked adapter cannot recover discarded pressure precision.

The production seam is `source_composition/native_i2c_clock_factory.rs::prepare_fragment` plus `host-runtime/src/execution_factory_native.rs::{PreparedCallSet,PreparedNativeDrivers}` and original selected resident identities. It currently retains resident/software and original independently issued I2C/clock tickets; no Bosch contract/provider is initialized or admitted there. `software.rs::SoftwareRole` has a DHT observation owner but no Bosch one. A legitimate native realization must retain original semantic/profile correspondence, exact Source compensation and protocol/failure laws, original bus/clock possession and all resource charges; merely calling `Sensor::measure` or adding another board loop does not implement that seam.

Next ownership: Source integration can narrow the actual repeated Source/state representation and remeasure its unchanged original pool, or supply exact Source-versus-native compensation/protocol correspondence to a reusable admitted provider. Native-provider realization/factory dispatch must coordinate with the native machinery owner before expanding those paths. Only after this core fits should Source connect semantic observation through the original Face/standard Mask renderer and HT16 output, then qualify the complete factory/admission/Started runner with realistic finite I2C samples, redraws, cancellation and Fore loss. None of those downstream gates has passed here. Ambient's complete workset remains intact; no partial Body/Wake replacement is authorized by this candidate.

## Reproduce

From this isolated worktree:

```sh
cargo test -p conduit-mcu-source-build --lib closed_sampling_core_accounts_original_owner_work_before_mask --no-default-features -- --nocapture
cargo test -p conduit-mcu-source-build --lib repeated_sampling_selects_real_finite_state_and_pairing_owners --no-default-features
```

Both commands passed. First additionally compiles the actual emitted resident owners as executable and no_std library, measuring only their preparation/storage. Logs are adjacent. There were no hardware, Root/Pi, network/service, seed, key, history, grant or deployment effects.
