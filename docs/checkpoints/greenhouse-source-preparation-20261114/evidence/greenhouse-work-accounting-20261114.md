# Greenhouse MCU work: preservation and handoff accounting

The shared preparation path has made substantial, measured progress, but this is **not a completed greenhouse appliance**. The complete checked Source component now passes CPU preparation; the newest phase-capacity integration passes complete CPU preparation but has not received target qualification. No device was flashed during this memory-reduction sequence.

## What is saved

- **#6837:** shared integration/delivery branch. Last complete MCU/runtime/factory/ARM qualification: `c7e14f11e2`. Latest additional shared-kernel commit: `4c3999e4e4`, qualified at kernel scope only.
- **#6902:** draft recovery branch, preserving all current experimental MCU source, tests, public evidence, reproduction scripts and `CONTINUE-HERE.md`. The checkpoint process verifies source files byte-for-byte without resetting the working checkout or its normal index.
- Private credentials, Body keys and observation journals remain outside Git. Generated ELF files are retained locally as regenerable artifacts; their hashes and build receipts are saved in the checkpoint.

## Implemented shared changes

### 1. Remove repeated immutable assignment metadata

An explicit installed-resource-table profile resolves the original checked resource bindings from the installed image instead of transmitting their repeated text with every assignment. The original resource metadata was 6,030 bytes; its reference is 46 bytes. Including the changed envelope length, this saves 5,985 bytes: the actual assignment fell from 36,299 to **30,314 bytes**, fitting the unchanged **32,548-byte** input.

This is an explicit profile, not permission to infer possession from discovery or accept arbitrary metadata references. Source/layout, scope, authentication, native authority and resource checks remain. Ordinary receivers without that profile refuse it.

### 2. Get complete checked preparation through its real gates

The exact component has **76 Gears, 95 Cords and 97 queue slots**, with **27 software, 47 resident and two native owners**. Work included:

- Independently funding queue slots rather than assuming one slot per Cord; two feedback Cords retain their actual two-item capacities.
- Covering 49 resource requirements with the existing fixed scratch budget rather than a 32-entry tracker.
- Checking the 721-row Source layout with bounded reusable bitmap scratch, retaining exact multiset and inactive-row checks.
- Funding the numeric kernel event log separately from the Plan's named mandatory-Sign receipt budget; neither is a substitute for the other.
- Propagating the firmware's emitted execution-workspace cfg into the CPU proof, so it tests the actual selected owner profiles.

The **original, uninstrumented factory** prepares the whole kernel and execution workspace from authenticated Root input. Original no_std modules compile, Root preflight succeeds, all native loans return, and no I2C or native-clock operation occurs. This is preparation, not admission or Start.

### 3. Share physical buffers without shortening reference lifetime

`SharedSchemaValueStore` separates logical references from physical payload buffers. It preserves logical slot order/bounds, generations, reference counts, canonical offsets and byte quotas. Physical buffers recycle only after the final reference releases them. The 65,536-cycle test compares retirement behavior with the dense store; pressure and stale-reference checks remain.

Selection uses **actual prepared-owner retention reports and exact emitted Source topology**, not device names or Kind guesses. It funds all queue slots, simultaneous outputs of one Step, retained state and native input pins separately. Unknown lifetimes keep the original physical stock. Execution allocation/rollback checks and backing accounting are covered.

Earlier work also removed duplicated software output workspace, retained linked schemas rather than repeated copies, and reduced authored Source constructor duplication. Those scoped measurements are not added to the whole-factory savings below.

### 4. Keep checked terminal contracts in flash

The same scheduler can retain the checked immutable terminal matrix rather than copy it into RAM. The default fixed/no_alloc profile remains; explicit `TERMINAL_ROWS=0` selects linked storage. Actual-Back equality, inactive rows, native incompatibility, projected recovery, topology and physical storage-budget checks remain. A changed by-value rebind refuses; unchanged contracts remain valid.

Actual ARM geometry for the same code/profile is **64,216 bytes inline versus 47,192 bytes linked**, removing the **17,024-byte** table. No additional heap owner hides that allocation. The default inline profile's small storage handle is charged through its actual size.

### 5. Latest kernel-only phase-capacity work

ARM layout inspection identified large abnormal-terminal state: Cord state 13,680 bytes, terminal phases 8,512 bytes and unresolved abnormal state 8,056 bytes. These were measured, not guessed.

Only the retained abnormal-finalization payload has a justified narrower capacity now. The current Source has no `FinalizeThenPropagate` contract. The new `PHASE_BYTES` profile defaults to 100; smaller profiles refuse any selected abnormal finalizer before allocation or effects. Public abnormal values, Cord storage and unresolved abnormal truth **retain their full 100-byte capacity**. `NotAccepted` was deliberately not treated as proof that abnormal emission is impossible.

Kernel scope passes **258 tests, three ignored**, focused phase tests, no_alloc compilation and strict library/test Clippy. The MCU adapter and Source-derived zero-byte selection initially remained unqualified; the bounded follow-ups below now qualify them together for CPU preparation, while ARM remains unverified. The small fixture's savings are not a Source76 or ARM saving claim.

## Comparable full-factory measurements

These are CPU64 requested allocations, not target RAM fit:

| Preparation stage | Retained bytes | Peak bytes |
| --- | ---: | ---: |
| Before shared physical buffers | 210,354 | 212,393 |
| Shared buffers selected | 189,469 | 191,508 |
| Shared buffers and linked terminal table | 172,453 | 174,492 |
| Plus Source-derived phase capacity (CPU only) | **164,853** | **166,892** |

The verified combined CPU64 saving is **45,501 bytes** in both retained and peak allocation. Exclusions remain preexisting Root, equipment, resident inventory, issuer grants, allocator overhead and stack. The separately funded CPU test arena is **not** the MCU's unchanged 131,072-byte heap. The newest row includes the separately qualified7,600-byte full-factory phase-capacity saving; no new ARM saving is inferred.

At `c7e14f11e2`, MCU runtime tests passed **139, one ignored**; topology tests passed five; scoped strict Clippy passed. Kernel tests then passed 255, three ignored, before the latest kernel-only phase change.

The last ARM preparation-entrance release links with **49,844 bytes flash margin**, **209,088 bytes static extent** and a **61,248-byte stack region**. ELF SHA-256: `4f83f51d2e6b7b1b3041c65836e9fdb993451ca08f36a45f9e574d54d5ab518f`. Type geometry and linking do not prove heap/stack safety or that the complete reachable application fits.

## What is running, and what is not established

The devices remain on their previous firmware; the live fleet was not reverified in this preservation pass. Earlier sensing/display and admission evidence must not be represented as execution of this new Source path. Threebee's UI/startup work is coordinated separately through #6895 and the shared handoff file; this accounting does not claim its latest package is deployed or that its graph contains every thermometer.

The experimental MCU entrance still prepares/drops and refuses **`structured-fore-mask-unavailable`**. It does not Start the complete application. This checkpoint does not establish a single fully operating Body across all devices, independent sampling/display, complete committed overnight history, restart recovery, NAT interruption recovery, provisioning/failover, browser joining, or a unified live/history Face. General extraction of remaining application-specific runtime dependencies is also unfinished.

## Resume here

1. Qualify the phase-capacity profile on target when budget permits. The complete CPU/no_std proof now passes; the last ARM figures belong to `c7e14f11e2`.
2. Qualify actual target heap, startup/preparation overlap and stack, including the original Root/equipment/allocator. Keep grants truthful; do not increase them merely to hide refusal.
3. Finish autonomous authored Source composition and actual Fore/Face/Mask admission and execution. Link the whole reachable application before claiming flash fit.
4. Verify real sensing/display, durable committed history, power cycles and owner/network loss on hardware before replacing working firmware. Preserve the same Body and provenance.
5. Continue integration with the shared display/history lane and deliver reviewed slices into dev. At the last fetch, current dev was already included; preservation is not a merge claim.

This work belongs principally to **#6341 and #6457**. Native Mask/Fore acceptance remains relevant to #6374/#6378; lifetime correctness to #6340/#6409; durable member/history acceptance to #6461/#6465 and the parent #6295. These preparation results do not close those acceptance obligations, and no new MCU memory epic is needed.

## Bounded follow-up: shared MCU adapter

The saved shared MCU phase-capacity adapter now compiles and passes the complete runtime library suite: **139 passed, one ignored** (`mcu-phase-terminal-tests-20261114.log`). Its three forwarding files are committed separately. This updates only the adapter's qualification: the RP2040 Source-derived selection, whole-factory preparation, new memory savings and ARM target remain unverified after phase specialization. The complete measurements above still belong to `c7e14f11e2`. No target build, device change or live-service change was made in this follow-up.

## Bounded follow-up: complete phase-profile CPU preparation

The original uninstrumented signed Source76 factory, Root preflight and original no_std compilation now pass with the Source-derived zero-byte abnormal-finalization profile. CPU64 requested retained/peak allocation is **164,853 / 166,892 bytes**, compared with172,453 /174,492 for the linked-table profile: **another7,600 bytes saved**. The combined measured saving from210,354 /212,393 is now **45,501 bytes**. All earlier exclusions remain. Public abnormal values, Cord state and unresolved abnormal truth keep100-byte capacity. Native calls remain0 and all loans return; no admission or Start occurs.

Receipt: `positive-phase-terminal-factory-20261114.json`; test log alongside it. These are CPU64 measurements. The newest profile has **not** been built or measured for ARM, and the last ARM ELF/geometry figures above still belong to c7e14f11e2. No hardware or service changes were made. Resume with target qualification rather than repeating this unchanged CPU proof.
