# Original resident workspace integration candidate

The production candidate removes repeated transient backing using the existing borrowed-image/workspace implementation. It does not qualify Green firmware, resolve the work budget, or complete the sampling → standard Mask/HT16 Play.

**Accounting corrections to the initial 261e report:** RP2040 `host/src/resident.rs::OWNER` retains generated residents in static storage. The 208,884-byte original output/scratch payload is static SRAM, not heap allocation; `retained_backing_bytes() == Some(0)` correctly describes its heap backing. Also, 4,997 is the conservative 261e fixture's individual compiler ceiling. The actual I2C BSP passes its 5,000-unit pool directly as that ceiling. Both refusals are now tested explicitly rather than describing4,997 as the BSP limit.

## Actual backing and lifetime

`StructuredResidentEvaluator<I,O,C,B>` owns `[u8; B]` for output and another for scratch. B is the original checked maximum output extent, not a default8192-byte capacity. Across the 52 original resident owners, `sum(B)=104442`; retained payload is208884 bytes. Scratch is needed only while evaluating a Step. Output is staged until that same transactional Step commits; kernel `payload_admission.rs` copies admitted output into its value store before lending the workspace to another Step. Pressure is checked before evaluation. A failed transaction retains neither workspace output nor a consumed input; cancellation drains kernel-owned values. Keeping 52 permanent pairs is unnecessary under the existing API.

`BorrowedWorkspaceStructuredExpressionBack` retains checked immutable child descriptions by reference and reports its real output/scratch extents. The kernel's existing exclusive `StepWorkspace` loan cannot escape a synchronous Step. `PreparedFragment::prepare_execution_workspace()` cold-funds one maximum pair under the original remaining retained ceiling, before any owner bind or whole-workset admission. No new execution allocator, scheduler, driver, unsafe alias, grant or counter was introduced.

Actual regenerated-code component results for the unchanged closed sampling core:

| Quantity | Result |
| --- | ---: |
| Original Owned output+scratch payload, architecture independent | 208,884 bytes |
| Original summed Owned Back inline size, x86_64 | 234,704 bytes |
| New complete borrowed resident inventory, x86_64 | 15,192 bytes |
| One cold shared output/scratch pair | 4,051 + 4,051 = 8,102 bytes |
| Original resident owners / expanded nodes / connections | 52 / 81 / 91 |

The payload reduction is 200,782 bytes after funding the shared pair. The new static-inventory measurement includes its wrappers; the original summed Back measurement excludes inventory wrappers, so they are not presented as an exact whole-inventory subtraction. These are component measurements, not RP2040 whole-static/stack/heap or kernel/value-store totals. SharedWorkspaceSchema was not selected for the target; canonical Cord traffic is preserved.

## Production path

The existing strict specialization receipt accepts optional `"resident_execution_storage":"owned"` or `"shared-workspace"`; omission preserves Owned. Its original captured-byte SHA already enters workset/fabrication identity. No CLI, ambient environment setting, physical address in Source, or alternate Source family was added.

Source checks the receipt and preserves the captured storage. Its default selected-resident preparation now honors that choice and refuses missing build support. RP2040's original I2C helper selects it, preserving original native pair, issuer, IMAGE, pool and exact-work checks. Standard RP Make enables the supporting Cargo feature only from the captured receipt; supporting features alone leave storage Owned. Explicit Shared on the DHT factory refuses instead of being silently ignored.

The existing generator emits uniform optional workspace reports when support is enabled. Empty and Owned/DHT inventories report no workspace; workspace inventories report the actual exclusively loaned owners. RP2040 forwards those original reports for whole and split loans. The existing native I2C/clock fragment constructor funds their single workspace before returning its still-unbound fragment. Original remaining-memory checks charge the real backing once.

## Work is a separate unchanged gate

`resident_work_units` in the old calibration JSON names `CheckedResidentWorkPool::capacity_units`, read from `ResourceBudget.slots` for `kernel/cooperative-step-work@1`. Core's contract defines one unit as one cooperative private-computation fuel unit per Step. It is not RAM, preprocessing storage, CPU cycles or elapsed time. The separate resource `bytes` field funds 16,384 bytes of preparation/bookkeeping storage.

The compiler calculates `PreparedResidentExpression::computation_work()` cold from input/output validation, structured tape operations and child work. `resident_offer.rs` publishes those exact units as a resource demand; assignment identity binds them. Lowering adds unary IO/scheduler fuel 3, while the evaluator checks private fuel+2 before doing work and consumes private fuel per Step. Borrowing immutable metadata and sharing transient bytes changes neither count nor law.

The 52 owners still require 17,282 units against the original 5,000 aggregate pool; ordinary preparation refuses at the first cumulative 5,073. The largest expression requires 5,379: historical conservative fixture limit 4,997 refuses, and the actual BSP individual limit 5,000 also refuses. These are independent of the memory reduction and remain enforced. There is no lawful full Green execution after this refusal.

## Qualification boundary

Manual direct-rustc component/generated-code evidence reused only this lane's warm dependency graph. It rebuilt the exact 26 affected local libraries serially with debuginfo0 and no incremental cache; 60 unchanged dependencies were reused. Reused OUT_DIR files are unchanged typed Forms/schemas produced by Core/Plot, with build-source/file hashes recorded; changed resident inventories were regenerated by the current changed generator. Local byte-pin resolver manifests are explicitly manual proof receipts, not Cargo-produced fingerprints.

Passed checks: strict Source inputs 4 each with support off/on; native constructor checks 2 each off/on; actual52-owner regenerated Owned/workspace storage and original refusals 1; original DHT Owned/no-workspace parity 1; exact empty inventory 2; existing kernel pressure/commit/cancel/drain/workspace guards 5; existing MCU cold-funding/refusal/custody 2; existing Source capture/identity 6. Current Source library, RP Make package and actual I2C build-helper API also compile as components. Offline Cargo metadata accepts both feature declarations. Logs, exact compiler commands and reused-form hashes are in `component-proof/`.

This is **not** regular Cargo feature, ordinary Make/recipe, target compiler/linker, firmware, actual complete-kernel admission, Started, physical I2C or power-cycle proof. RP2040 forwarding and final firmware feature/link closure still require their regular target gate. Original 16 logical queue-item limit, 8192-byte buffer, 49152-byte heap, 131072-byte static limit, 16384-byte runtime/4096-byte assignment bounds and 5,000-unit work pool were not enlarged. Full Cord/value-store geometry and standard Face/Mask/HT16 integration remain unqualified. Ambient's complete Body workset is untouched.
