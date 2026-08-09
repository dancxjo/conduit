# Rust Module Inventory

Reviewed commit: 82fec9f1b65ff537148244698cd16744416ce8dc

## 1. Executive summary

* **Number of Rust files inspected**: 50
* **Number at 700+ lines**: 13
* **Number at 1000+ lines**: 10
* **The five highest-priority decomposition candidates**:
  1. [`crates/conduit-std-catalog/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-std-catalog/src/lib.rs) (1,220 lines) - Extract 453 lines of inline production `mod host_profile` into `crates/conduit-std-catalog/src/host_profile.rs`.
  2. [`crates/conduit-kernel/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-kernel/src/lib.rs) (1,356 lines) - Extract 191 lines of inline production `mod hosted` into `crates/conduit-kernel/src/hosted.rs`.
  3. [`crates/conduit-composite/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-composite/src/lib.rs) (3,746 lines) - Extract 1,816 lines of inline unit tests into `crates/conduit-composite/src/tests.rs`.
  4. [`crates/conduit-planner/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-planner/src/lib.rs) (1,982 lines) - Extract 982 lines of inline unit tests into `crates/conduit-planner/src/tests.rs`.
  5. [`crates/conduit-form/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-form/src/lib.rs) (2,042 lines) - Extract 664 lines of inline unit tests into `crates/conduit-form/src/tests.rs`.
* **Large files that should explicitly NOT be split yet**:
  * [`crates/conduit-runtime/src/lowering.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/src/lowering.rs) (974 lines): High semantic risk; plan lowering pass algorithm and identity maps are tightly coupled.
  * [`crates/conduit-embedded-build/src/render.rs`](file:///home/dancxjo/src/conduit/crates/conduit-embedded-build/src/render.rs) (539 lines): Single cohesive responsibility generating embedded C/Rust output.
  * [`crates/conduit-wire/tests/wire_corpus.rs`](file:///home/dancxjo/src/conduit/crates/conduit-wire/tests/wire_corpus.rs) (475 lines): Dedicated integration test suite for wire corpus vectors.
  * [`firmware/conduit-pico-w-signal/src/kernel.rs`](file:///home/dancxjo/src/conduit/firmware/conduit-pico-w-signal/src/kernel.rs) (473 lines): Hardware-bound driver execution loop for Raspberry Pi Pico W firmware.

## 2. Size table

| Path | Lines | Classification | Production / Test / Blank Mix | Primary Responsibilities | Priority |
| --- | --- | --- | --- | --- | --- |
| [`crates/conduit-runtime/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/src/lib.rs) | 4,642 | priority | 3,522 prod (76%), 1,120 test (24%), 157 blank (3%) | Core runtime host engine, operation traits, mandatory Sign logging, composite boundary handling, inline conformance tests | High |
| [`crates/conduit-kernel/src/scheduler.rs`](file:///home/dancxjo/src/conduit/crates/conduit-kernel/src/scheduler.rs) | 4,308 | priority | 2,216 prod (51%), 2,092 test (49%), 176 blank (4%) | Fixed-capacity kernel scheduler, node/cord specs, step execution loop, inline scheduler tests | High |
| [`crates/conduit-composite/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-composite/src/lib.rs) | 3,746 | priority | 1,930 prod (52%), 1,816 test (48%), 126 blank (3%) | Multi-host composite definition, boundary face mapping, composite execution, inline tests | Priority candidate #3 |
| [`crates/conduit-runtime/tests/host_contract.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/tests/host_contract.rs) | 2,262 | priority | 0 prod (0%), 2,262 test (100%), 184 blank (8%) | Integration contract test suite for host runtime lifecycle and Sign verification | Medium |
| [`crates/conduit-form/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-form/src/lib.rs) | 2,042 | priority | 1,378 prod (67%), 664 test (33%), 125 blank (6%) | Form CST parsing, checked form AST validation, profile catalog, inline tests | Priority candidate #5 |
| [`crates/conduit-planner/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-planner/src/lib.rs) | 1,982 | priority | 1,000 prod (50%), 982 test (50%), 94 blank (5%) | Form-to-plan expansion, host placement solver, cord routing, inline tests | Priority candidate #4 |
| [`crates/conduit-wire/src/session.rs`](file:///home/dancxjo/src/conduit/crates/conduit-wire/src/session.rs) | 1,549 | priority | 977 prod (63%), 572 test (37%), 102 blank (7%) | Wire session state machine, frame encoder/decoder codec, inline tests | Medium |
| [`crates/conduit-core/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-core/src/lib.rs) | 1,513 | priority | 1,455 prod (96%), 58 test (4%), 104 blank (7%) | Foundational identity types, plan fragments, host advertisements, authority bindings, inline tests | Low (Canon core types) |
| [`crates/conduit-kernel/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-kernel/src/lib.rs) | 1,356 | priority | 1,046 prod (77%), 310 test (23%), 125 blank (9%) | Kernel ID types, fixed-capacity operation bindings, routes, value store, inline `hosted` module, inline tests | Priority candidate #2 |
| [`crates/conduit-std-catalog/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-std-catalog/src/lib.rs) | 1,220 | priority | 841 prod (69%), 379 test (31%), 87 blank (7%) | Standard contract definitions, profile catalog generation, inline `host_profile` module, inline tests | Priority candidate #1 |
| [`crates/conduit-observatory/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-observatory/src/lib.rs) | 981 | large | 692 prod (71%), 289 test (29%), 40 blank (4%) | Observatory telemetry report construction, text report renderer, inline tests | Medium |
| [`crates/conduit-runtime/src/lowering.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/src/lowering.rs) | 974 | large | 974 prod (100%), 0 test (0%), 52 blank (5%) | Plan fragment lowering pass, kernel identity maps | Do NOT split yet |
| [`crates/conduit-embedded-build/src/render.rs`](file:///home/dancxjo/src/conduit/crates/conduit-embedded-build/src/render.rs) | 539 | review | 539 prod (100%), 0 test (0%), 26 blank (5%) | C/Rust code template renderer for embedded builds | Do NOT split yet |
| [`xtask/src/commands/pico/serial.rs`](file:///home/dancxjo/src/conduit/xtask/src/commands/pico/serial.rs) | 539 | review | 337 prod (63%), 202 test (37%), 40 blank (7%) | Pico serial communication doctor/flasher xtask command, inline tests | Low |
| [`crates/conduit-wire/tests/wire_corpus.rs`](file:///home/dancxjo/src/conduit/crates/conduit-wire/tests/wire_corpus.rs) | 475 | review | 0 prod (0%), 475 test (100%), 52 blank (11%) | Wire frame encoding vector integration tests | Do NOT split yet |
| [`crates/conduit-signal/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-signal/src/lib.rs) | 474 | review | 472 prod (100%), 2 test (0%), 39 blank (8%) | Signal demonstration profile and contract definitions | Normal |
| [`firmware/conduit-pico-w-signal/src/kernel.rs`](file:///home/dancxjo/src/conduit/firmware/conduit-pico-w-signal/src/kernel.rs) | 473 | review | 473 prod (100%), 0 test (0%), 30 blank (6%) | Firmware kernel driver implementation for RP2040 Pico W | Do NOT split yet |

## 3. Detailed candidates

### `crates/conduit-std-catalog/src/lib.rs`

Current responsibilities:
- Standard contract definitions and kind catalog lookup (`standard_contracts`, `find_contract`, `standard_kind_ids`).
- Profile catalog builder (`standard_profile_catalog`, `standard_capability_offers`).
- Inline production module `mod host_profile` (lines 389–841, 453 lines) defining standard host profiles and standard registry installation.
- Inline unit tests (`mod tests` spanning 379 lines).

Recommended first seam:
- Extract inline production `mod host_profile` into `crates/conduit-std-catalog/src/host_profile.rs`.

Why this seam is safe:
- `mod host_profile` is already scoped as a distinct inline module block. Moving it to `src/host_profile.rs` preserves exact module visibility and public re-exports while reducing production code in `lib.rs` from 841 lines down to 388 lines.

Likely destination:
- `crates/conduit-std-catalog/src/host_profile.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- `conduit-form` catalog additions or host runtime execution changes.

---

### `crates/conduit-kernel/src/lib.rs`

Current responsibilities:
- Core kernel identifier types (`NodeId`, `PortId`, `CordId`, `RemoteEndpointId`, `ResourceId`, `SignExpectationId`).
- Fixed-capacity kernel containers (`FixedHostOperationBindings`, `FixedRoutes`, `FixedValueStore`, `FixedSignLog`).
- Operation execution traits and outcomes (`Operation`, `HostOperationOutcome`, `Failure`).
- Inline production module `mod hosted` (lines 598–788, 191 lines) for heap-allocated adapters (`HostedValueStore`, `HostedSignLog`).
- Inline unit tests (`mod tests` spanning 310 lines).

Recommended first seam:
- Extract inline production `mod hosted` into `crates/conduit-kernel/src/hosted.rs`.

Why this seam is safe:
- `mod hosted` is already declared as an inline module block. Moving it to `src/hosted.rs` requires zero changes to re-exports or public API paths.

Likely destination:
- `crates/conduit-kernel/src/hosted.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- `crates/conduit-kernel/src/scheduler.rs` refactoring.

---

### `crates/conduit-runtime/src/lib.rs`

Current responsibilities:
- Definition of core operation execution traits (`OperationState`, `OperationImplementation`) and registry (`ImplementationRegistry`).
- Definition of boundary types (`CompositePortBinding`, `CompositeBoundaryEffect`, `RuntimeOutput`).
- Implementation of primary `HostRuntime` lifecycle (plan preparation, Play start, step pumping, cancellation, release, mandatory Sign collection, composite boundary management).
- Inline conformance test suite (`mod conformance` spanning 1,120 lines).

Recommended first seam:
- Extract `mod conformance` (lines 3523–4642) into `crates/conduit-runtime/src/conformance.rs`.

Why this seam is safe:
- `mod conformance` is a pure test module guarded by `#[cfg(test)]`. Moving it to a submodule file does not alter production runtime semantics and immediately reduces `lib.rs` size by 1,120 lines.

Likely destination:
- `crates/conduit-runtime/src/conformance.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- `HostRuntime` method extractions, generic host boundary redesign, or lowering changes.

---

### `crates/conduit-kernel/src/scheduler.rs`

Current responsibilities:
- Core kernel execution engine (`FixedScheduler`), status tracking (`SchedulerStatus`), and scheduler errors (`SchedulerError`).
- Node, cord, and capacity specs (`NodeSpec`, `CordSpec`, `CordCapacity`).
- Step IO management and driver wrappers (`StepOperation`, `OperationDriver`, `StepIo`).
- Extensive inline unit test suite (`mod tests` spanning 2,092 lines).

Recommended first seam:
- Extract `mod tests` (lines 2217–4308) into `crates/conduit-kernel/src/scheduler/tests.rs` (converting `scheduler.rs` into `scheduler/mod.rs` or `scheduler_tests.rs`).

Why this seam is safe:
- `mod tests` comprises nearly 49% of the file and is completely isolated behind `#[cfg(test)]`. Moving it requires zero edits to `FixedScheduler` execution semantics.

Likely destination:
- `crates/conduit-kernel/src/scheduler/tests.rs` (or `crates/conduit-kernel/src/scheduler_tests.rs`)

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- `FixedScheduler` generic parameter changes or kernel storage refactoring.

---

### `crates/conduit-composite/src/lib.rs`

Current responsibilities:
- Multi-host `CompositeDefinition`, `CompositeBoundary`, `CompositeFaceBinding`, and `ChildHostBinding` data types.
- Multi-host orchestration engine (`CompositeHost`) for stepping, boundary value translation, and sub-host lifecycle management.
- Inline unit test suite (`mod tests` spanning 1,816 lines).

Recommended first seam:
- Extract `mod tests` (lines 1931–3746) into `crates/conduit-composite/src/tests.rs`.

Why this seam is safe:
- Pure test module extraction. Removing `mod tests` reduces `lib.rs` from 3,746 lines down to 1,930 lines cleanly without touching any composite routing logic.

Likely destination:
- `crates/conduit-composite/src/tests.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- `CompositeHost` execution step or multi-host routing changes.

---

### `crates/conduit-runtime/tests/host_contract.rs`

Current responsibilities:
- Integration test suite for `HostRuntime` contract validation.
- Test fixture builders (`SourceImplementation`, `SinkImplementation`, `advertisement()`, `authority_fragment()`, `registry()`).

Recommended first seam:
- Extract fixture builders and common mock implementations (lines 1–479) into `crates/conduit-runtime/tests/common/mod.rs`.

Why this seam is safe:
- Moving test fixture builders into a `common` module follows Rust integration test conventions, deduplicates test setup, and leaves individual test assertions uncluttered.

Likely destination:
- `crates/conduit-runtime/tests/common/mod.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- Production `HostRuntime` changes in `crates/conduit-runtime/src/lib.rs`.

---

### `crates/conduit-form/src/lib.rs`

Current responsibilities:
- Form concrete syntax tree tokens, spans, and diagnostics (`Span`, `CstToken`, `FormDiagnostic`, `FormError`).
- Authoring AST structs (`FormDocument`, `CheckedForm`, `KindDefinition`, `ProfileCatalog`).
- Parser and checked form validator (`parse_document`, `parse`).
- Inline unit test suite (`mod tests` spanning 664 lines).

Recommended first seam:
- Extract `mod tests` (lines 1379–2042) into `crates/conduit-form/src/tests.rs`.

Why this seam is safe:
- Test extraction is purely mechanical. It removes 664 lines from `lib.rs` with zero risk to parsing logic.

Likely destination:
- `crates/conduit-form/src/tests.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- AST parsing changes or `conduit-planner` updates.

---

### `crates/conduit-planner/src/lib.rs`

Current responsibilities:
- Placement choices, options, and errors (`PlacementChoice`, `PlacementChoices`, `PlanningOptions`, `PlannerError`).
- Form-to-plan expansion pass (`plan`, `plan_with_options`, placement solver, cord routing, authority verification).
- Inline unit test suite (`mod tests` spanning 982 lines).

Recommended first seam:
- Extract `mod tests` (lines 1001–1982) into `crates/conduit-planner/src/tests.rs`.

Why this seam is safe:
- Inline unit tests constitute half the file (982 lines out of 1,982). Extracting them reduces `lib.rs` to exactly 1,000 lines without altering planning algorithms.

Likely destination:
- `crates/conduit-planner/src/tests.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- Planning algorithm modifications or `conduit-form` AST changes.

---

### `crates/conduit-core/src/lib.rs`

Current responsibilities:
- Core canon identity types (`FormIdentity`, `ActivePlayIdentity`, `SignIdentity`, `PresentationIdentity`).
- Plan fragment definitions, fragment commitment, and sealing logic (`Plan`, `PlanFragment`, `FragmentCommitment`, `seal_plan`, `verify_plan`).
- Capability offers, host advertisements, authority grants, and link bindings.
- Observation and host lifecycle events (`Observation`, `ObservationKind`, `HostCommand`, `HostEvent`, `PlatformEffect`).
- Inline unit tests (`mod tests` spanning 58 lines).

Recommended first seam:
- Extract `mod tests` (lines 1455–1513) into `crates/conduit-core/src/tests.rs`.

Why this seam is safe:
- `conduit-core` defines public canon types used throughout the workspace. Production code extractions carry high architectural sensitivity. Moving `mod tests` is safe and leaves public re-exports untouched.

Likely destination:
- `crates/conduit-core/src/tests.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- Canon identity modifications or core plan structure edits.

---

### `crates/conduit-wire/src/session.rs`

Current responsibilities:
- Wire session protocol state machine (`SessionMachine`, `SessionBinding`, `SessionHello`, `SessionFrame`, `SessionMessage`).
- Binary frame codec (`encode_session_frame_into`, `decode_session_frame`).
- Inline unit tests (`mod tests` spanning 572 lines).

Recommended first seam:
- Extract `mod tests` into `crates/conduit-wire/src/session/tests.rs` (or `crates/conduit-wire/src/session_tests.rs`).

Why this seam is safe:
- Pure test extraction. Removes 572 lines from `session.rs`, bringing production/test separation clean without affecting binary frame encoding or decoding logic.

Likely destination:
- `crates/conduit-wire/src/session/tests.rs` (or `crates/conduit-wire/src/session_tests.rs`)

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- `crates/conduit-wire/tests/wire_corpus.rs` or protocol framing edits.

---

### `crates/conduit-observatory/src/lib.rs`

Current responsibilities:
- Telemetry data structures (`HostRow`, `CapabilityRow`, `LinkRow`, `PlanRow`, `PlacementRow`, `ConnectionRow`, `SignRow`, `ObservatoryReport`).
- Report generation logic (`build_report`).
- Text report renderer (`render_text_report`).
- Inline unit tests (`mod tests` spanning 289 lines).

Recommended first seam:
- Extract `mod tests` (lines 693–981) into `crates/conduit-observatory/src/tests.rs`.

Why this seam is safe:
- Moving test code to `src/tests.rs` shrinks `lib.rs` from 981 lines to 692 lines cleanly without modifying telemetry aggregation logic.

Likely destination:
- `crates/conduit-observatory/src/tests.rs`

Public API impact:
- none

Semantic risk:
- low

Do not combine with:
- Runtime observation logging format edits.

---

### `crates/conduit-runtime/src/lowering.rs`

Current responsibilities:
- Lowered plan data structures (`LoweredNode`, `LoweredCord`, `LoweredRoute`, `LoweredHostOperation`, `LoweredResource`, `LoweredSign`).
- Identity maps (`KernelIdentityMap`, `KernelExecutionIdentityMap`).
- Lowering pass algorithm (`lower_plan_fragment`).

Recommended first seam:
- none yet

Reason:
- Lowering data structures and the plan lowering pass are deeply intertwined. Attempting extraction before lowering AST stabilization would introduce risk of cyclic dependencies or unnecessary abstraction churn.

---

## 4. Recommended extraction order

1. **Extract `mod host_profile` from `conduit-std-catalog`**: Move inline production `mod host_profile` from [`crates/conduit-std-catalog/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-std-catalog/src/lib.rs) into `crates/conduit-std-catalog/src/host_profile.rs`.
2. **Extract `mod hosted` from `conduit-kernel`**: Move inline production `mod hosted` from [`crates/conduit-kernel/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-kernel/src/lib.rs) into `crates/conduit-kernel/src/hosted.rs`.
3. **Extract unit tests from `conduit-composite`**: Move `mod tests` from [`crates/conduit-composite/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-composite/src/lib.rs) into `crates/conduit-composite/src/tests.rs`.
4. **Extract unit tests from `conduit-planner`**: Move `mod tests` from [`crates/conduit-planner/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-planner/src/lib.rs) into `crates/conduit-planner/src/tests.rs`.
5. **Extract unit tests from `conduit-form`**: Move `mod tests` from [`crates/conduit-form/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-form/src/lib.rs) into `crates/conduit-form/src/tests.rs`.
6. **Extract unit tests from `conduit-wire` session**: Move `mod tests` from [`crates/conduit-wire/src/session.rs`](file:///home/dancxjo/src/conduit/crates/conduit-wire/src/session.rs) into `crates/conduit-wire/src/session/tests.rs`.
7. **Extract unit tests from `conduit-observatory`**: Move `mod tests` from [`crates/conduit-observatory/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-observatory/src/lib.rs) into `crates/conduit-observatory/src/tests.rs`.
8. **Extract conformance suite from `conduit-runtime`**: Move `mod conformance` from [`crates/conduit-runtime/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/src/lib.rs) into `crates/conduit-runtime/src/conformance.rs`.
9. **Extract unit tests from `conduit-kernel` scheduler**: Move `mod tests` from [`crates/conduit-kernel/src/scheduler.rs`](file:///home/dancxjo/src/conduit/crates/conduit-kernel/src/scheduler.rs) into `crates/conduit-kernel/src/scheduler/tests.rs`.
10. **Extract integration test fixtures from `conduit-runtime` host contract**: Move test fixture builders from [`crates/conduit-runtime/tests/host_contract.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/tests/host_contract.rs) into `crates/conduit-runtime/tests/common/mod.rs`.
11. **Extract unit tests from `conduit-kernel` lib**: Move `mod tests` from [`crates/conduit-kernel/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-kernel/src/lib.rs) into `crates/conduit-kernel/src/tests.rs`.

## 5. Explicit non-candidates

- **[`crates/conduit-runtime/src/lowering.rs`](file:///home/dancxjo/src/conduit/crates/conduit-runtime/src/lowering.rs)** (974 lines): High semantic risk. Identity translation maps and plan fragment lowering are closely bound. Defer splitting until lowering interface changes are explicitly owned.
- **[`crates/conduit-embedded-build/src/render.rs`](file:///home/dancxjo/src/conduit/crates/conduit-embedded-build/src/render.rs)** (539 lines): Pure code generation renderer. Single cohesive responsibility; splitting would fragment simple string rendering templates.
- **[`crates/conduit-wire/tests/wire_corpus.rs`](file:///home/dancxjo/src/conduit/crates/conduit-wire/tests/wire_corpus.rs)** (475 lines): Dedicated integration test file containing wire protocol test corpus vectors. Single test responsibility.
- **[`firmware/conduit-pico-w-signal/src/kernel.rs`](file:///home/dancxjo/src/conduit/firmware/conduit-pico-w-signal/src/kernel.rs)** (473 lines): RP2040 firmware kernel driver implementation. Cohesive hardware-bound execution loop.

## 6. Suggested first implementation task

- **Source module**: [`crates/conduit-std-catalog/src/lib.rs`](file:///home/dancxjo/src/conduit/crates/conduit-std-catalog/src/lib.rs)
- **Destination module**: `crates/conduit-std-catalog/src/host_profile.rs`
- **Proposed change**: Extract inline production `#[cfg(feature = "host-profile")] mod host_profile` (453 lines) from `lib.rs` into `src/host_profile.rs` and update `lib.rs` to declare `mod host_profile;`.
- **Justification**:
  - Pure responsibility extraction: moves host profile catalog definitions out of catalog contract root.
  - Low semantic risk: zero production behavior or public API path change (`pub use host_profile::{...}` preserved in `lib.rs`).
  - Production line count impact: reduces `crates/conduit-std-catalog/src/lib.rs` production line count from **841 lines** down to **388 lines** (a 53.8% production line count reduction).

## Follow-up observations

None observed requiring immediate remediation.
