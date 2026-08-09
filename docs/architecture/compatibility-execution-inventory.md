# Compatibility-execution fence and caller inventory

This inventory concerns the legacy hosted executor, not functional callable compatibility. Functional compatibility is canonical checked-face equality as established by #522.

`conduit-runtime` now has two compile-time surfaces:

- default/no features: current exact-plan `lowering` only;
- `compatibility-executor`: `HostRuntime`, its implementation registry and operation-state protocol, compatibility bases, pump, queue ownership, lifecycle, and clue machinery.

Production std and browser hosts and the Pico firmware build depend on `conduit-runtime` with default features disabled. They can lower exact plans but cannot name or instantiate `HostRuntime`. Enabling a fixture elsewhere in a whole-workspace build does not change the feature declaration in a production image; each production package is also checked independently by the required std, WASM, and firmware commands.

## Complete current caller classification

| Caller | Classification | Fence and disposition |
| --- | --- | --- |
| `crates/conduit-runtime/src/compatibility_executor.rs` | legacy compatibility implementation | API exists only with `compatibility-executor`; retained as source material and fixture support |
| `crates/conduit-runtime/src/conformance.rs` | legacy test support | compiled only inside the feature-gated compatibility module |
| `crates/conduit-runtime/tests/host_contract.rs` and `host_contract_fixtures` | legacy test support | integration test declares `required-features = ["compatibility-executor"]` |
| `hosts/std/src/lib.rs` / `LegacyStdFixtureHost` | legacy fixture driver | both the type and dependency feature are gated by the explicitly named `legacy-fixture-driver`; production `StdHost` uses the kernel |
| `fixtures/browser-sim` | simulation fixture | explicitly enables `conduit-runtime/compatibility-executor` and Signal's `legacy-fixture-driver` |
| `fixtures/pico-sim` | simulation fixture | its `compatibility-fixture` feature explicitly enables the compatibility executor and Signal legacy driver; the no-default carrier remains free of them |
| `crates/conduit-composite` | composite compatibility fixture | the crate's explicitly named `compatibility-fixture` feature enables the retained child `HostRuntime`; deletion waits for a kernel-backed composite replacement |
| `crates/conduit-signal/src/host_profile.rs` | legacy implementation/registry support | separated from current Signal contracts and catalog; available only through `legacy-fixture-driver` |
| `crates/conduit-std-catalog/src/host_profile.rs` and its runtime conformance test | legacy implementation/registry support | available only through the explicitly named `compatibility-fixture`; form-catalog consumers disable defaults |

## Current production and adaptation users

The following are not compatibility-execution callers. They import only `conduit_runtime::lowering`: production `StdHost`, browser/WASM runtime, `conduit-embedded-build`, and the Pico firmware build script. The planner's tests also use lowering only. No production Pico target dependency includes `conduit-runtime`; its host-only build dependency has default features disabled.

Catalog and contract construction in `conduit-signal` is current semantic/adaptation support and no longer depends on the legacy executor. This preserves current lowering and checked-face planning without dragging a second scheduler into production images.

## Mechanical proofs

The `browser_readiness` architecture tests pin the feature names and production dependency declarations. CI separately compiles production std, browser/WASM, no-default/Thumb surfaces, and the Pico firmware with its locked standalone graph. Fixture tests continue to exercise the explicitly enabled compatibility paths. Source guards retain the stronger rule that production std/browser modules contain no `HostRuntime`, legacy host command, pump, or registry use.
