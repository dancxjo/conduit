# Positive whole-workset preparation handoff

New file: `targets/rp2040/make/tests/neutral_source/positive.rs` (currently unused and **not compiler-qualified**). No Cargo run, dependency mutation, native call, admission, Start, or physical operation occurred in this task. Parent owns subsequent inclusion/qualification.

## What current proof actually measures

The existing `neutral_source/exercise.rs` installs `preparation: None, execution: None` and supplies an empty HostCommandInput. Its successful tests cover legitimate refusals, all software-owner initialization, and value-store construction. They never construct the complete kernel, Sign store or execution workspace. The software-plus-values subtotal cannot prove target admission or fit.

## Legitimate next route

1. Preserve the CPU-only Host/Boot/signing identity, but construct inert providers with the SAME independently installed Base IDs, physical implementations, pools, classes and callable capability identities as the generated Host configuration. Publish original owners and the configured resident-work pool. Do not edit compiled layout/template tables to accommodate test identities.
2. Use generated `PREPARATION_DELEGATION` and `EXECUTION_DELEGATION` constants (their exact configured ceilings), not `None`. Root remains the original verified member/credential owner. Existing signer is the pinned fixture owner.
3. Authorize original I2C and clock Issuers with original Root context and these policies, before constructing the command. This creates public original grants; it does not issue a native table or bind a provider.
4. New helper `positive::retain(root, issuers, signer, now)` takes Root's ORIGINAL input buffer through `execution_parts`. It instantiates the exact emitted projection template using matching original Base instances and original grants. It constructs canonical fragment, Plot Plan, Body Plan and Body Play identities through existing Core/Body encoders and checks resource names/classes/capacities against original inventory. It signs an existing issued challenge, authenticates it, checks installed workset delegation and retains the authenticated stamp over exactly those bytes. No current-Boot ownership or authority DTO is manufactured.
5. Include the helper in the generated CPU executable only (not `no_std` library), then invoke the real `neutral_preflight::preflight`. The first positive result must be `Ok(())` while providers remain Ready and call counters remain zero. Then directly call `neutral_factory::prepare` with Root parts/software/values to measure and inspect the complete prepared kernel. Dropping it must restore all provider loans. Admission and Start remain separate gates.
6. Run again with the actual original image heap remainder after charging preexisting Root/equipment/residents. Do not substitute the fixture's 1MiB permissive measurement ceiling for the MCU's installed heap.

The helper uses standard `c::assigned_plan_payload_digest`, so requires no additional dependency. It may need API/type fixes after inclusion; `rustfmt` alone has passed.

## Known fixture mismatches to fix before a positive run

Current CPU `base_id: cpu/i2c`, `base_implementation_id: cpu/i2c-base@1`, clock `cpu/clock`, `cpu/clock-base@1`, and native pools `cpu/i2c/pool` / `cpu/clock/pool` differ from immutable configuration pins. The helper deliberately refuses rather than rewriting them. Use generated Host constants and the neutral board construction's exact original identity rules. Add the configured resident-work resource exactly as `neutral_root.rs` does. Existing actual CPU advertisement has only the two native resources, whereas the compiled resident nodes require their configured work pool.

Expose generated preparation/execution/work constants to the generated CPU `config` module; current harness selects only image/config/assignment/native-operation constants. `neutral_root.rs` is the reference for exact configured policy and original resource pool publication.

## Inline accounting

`positive::report_cpu_inline_sizes(root, equipment, residents)` prints `size_of::<neutral_factory::Kernel<'static, 'static>>()` plus actual Root/equipment/resident object sizes and pointer width. It is a CPU-ABI diagnostic only. A complete preparation interval must include boxed kernel backing, Sign storage, shared workspace, native issuance/tickets, software owners, values, originals and allocator overhead. No numeric inline measurement has been made yet, since compiling this helper while the parent has an active Cargo lease is prohibited.

## Integration mechanics

The current generated exercise is one source string. Add an absolute `#[path = ...] mod positive;` only to the `bin` variant or append the module declaration inside `exercise` string. `positive.rs` imports from `crate` and refers to generated `layout`; it does not need the authoring parser/planner. This confines test construction/signing to CPU; firmware retains the existing signed protocol entrance.

## Integration edits now applied (compiler gate pending)

Parent transferred ownership of existing `neutral_source.rs` and `exercise.rs` after its CPU measurement terminated. These files now expose the exact generated policy/resource/Base constants, construct CPU providers following the same configured identity rules as the physical constructors, publish the configured resident work pool, retain all original refusal assertions, and append the separate positive stage. `positive::measure_preparation` prepares and drops the complete original factory inside an allocation interval before positive preflight is asserted. The helper is included only in the generated executable.

No compilation has yet run because the parent's active ARM build writes the generated OUT_DIR consumed by this test. Exact API compatibility and the positive result remain unproven until that writer releases its lease.
