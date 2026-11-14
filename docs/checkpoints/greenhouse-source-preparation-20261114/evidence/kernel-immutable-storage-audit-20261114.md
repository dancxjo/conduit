# ExecutionKernel immutable-storage next gate

Read-only audit; no build, experiment or production edit. Actual ARM geometry reports ExecutionKernel inline64,192B (`outputs/arm-shared-schema-geometry-20261114.json`). Actual positive CPU factory after shared stock retains189,469B/peaks191,508B; neither proves fit in the unchanged131,072B target heap.

## Verified substantial duplicate: exact terminal contracts

`architecture/kernel/src/scheduler.rs::FixedScheduler` embeds `terminal_transductions: [[Option<AssignedTerminalTransduction>; PORTS]; NODES]`. Source76 has N=76,P=2: **152 entries**. Each entry contains three enums supporting32-byte law identities. Exact removable array bytes are **152 × size_of::<Option<AssignedTerminalTransduction>>()**, minus a small storage handle; this audit makes no guessed ARM ABI-size claim.

The independently checked immutable contracts already exist in generated `GENERATED_TERMINAL_TRANSDUCTIONS`, narrowed to actualP as `neutral_workset.rs::TERMINALS`. `targets/mcu/host-runtime/src/execution.rs::prepare_inner` passes the expected table to `execution_terminal_gate.rs::check_actual`, which compares every actual prepared Back report including inactive rows. `prepare_storage` then passes that table by reference to the boxed constructor. **`architecture/kernel/src/scheduler/boxed_preparation/table_writer.rs:128` copies `*contracts` into the heap allocation.** This is retained immutable duplication under #6341.

The contracts are set during construction/explicit `bind_terminal_transductions`, then read during Play. They differ from mutable terminal phases, emitted counts, cancellation progress and abnormal values, which must remain funded.

## Minimal reusable change

Select an explicit immutable terminal-table storage profile for the ordinary scheduler. Its linked constructor retains the checked `&'static [[Option<AssignedTerminalTransduction>;P];N]` instead of copying the array. Preserve whole-table actual-Back equality, native/compact incompatibility checks, inactive rows, projected-recovery checks, and original assignment/layout/authority admission. Immutable bytes alone authorize nothing.

A storage type parameter can select a small borrowed representation while preserving existing inline owned/no_alloc callers. An alloc-only alternative is `Owned(Box<array>) | Linked(&'static array)`, but the owned fallback still allocates/charges the full array and needs correct preparation/drop accounting. **`Owned(full_array) | Linked(reference)` inside one inline enum saves no struct RAM**, because the largest variant determines its size. Simply boxing every existing array relocates memory and is not the saving.

Update boxed field writing, exhaustive unbound cleanup, retained-storage accounting and bind/rebind paths consistently. Failed preparation must not bind providers. New Plans require new checked custody; an existing live Play's table cannot change underneath it. Other immutable topology tables may be future candidates, but are not needed for this first change.

## Required evidence

Dense/linked traces must agree for normal/terminal/abnormal/cancellation/recovery behavior and refusals. Foreign/tampered contracts and inactive/native mismatches refuse before provider transfer. Test failed construction/drop and next-Play custody. Then repeat original positive whole-factory preparation, actual ARM type size, allocator retained/peak and cold-stack measurement with the original128KiB target heap. This is a proposed kernel storage change, not a fit, admission or physical-execution claim.
