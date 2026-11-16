# Source76 RAM partition audit — 2026-11-15

**No currently demonstrated, legitimately spare physical region can be reassigned to the heap.** The ELF's apparent remaining RAM is the active stack region, not an unowned arena. The next safe step is exact-target ownership/overlap and stack qualification, followed by removal of proved duplicate immutable preparation backing. Expanding grants or shrinking stack to make the CPU receipt appear to fit is unsupported.

Read-only audit: no builds, source changes, device writes, grant/config changes or runtime execution. Primary artifact `outputs/rp2040-unresolved-stock-20261115.elf`, SHA-256 `c384f6489df37e7094e45cebf489bb24799ddd099c330d20b7d9996291f2b8ff`, checkpoint `0e7bfb3d9a4617d386d239d1f88a947b6d7688b5`. ELF symbols/sections were inspected with readelf. CPU evidence is `outputs/positive-unresolved-factory-20261115.json`; scope differs from this ELF and is not a fit proof.

## Exact physical partition (half-open address ranges)

| Owner / region | Address range | Bytes |
| --- | --- | ---: |
| Complete static extent, including heap | `0x20000000..0x200330c0` | 209088 |
| `.data` | `0x20000000..0x20000094` | 148 |
| alignment gap before `.bss` | `0x20000094..0x20000098` | 4 |
| fixed heap backing `heap::BYTES` | `0x20000098..0x20020098` | 131072 |
| entire `.bss` (includes heap and task owners) | `0x20000098..0x20032ec0` | 208424 |
| `.uninit` | `0x20032ec0..0x200330c0` | 512 |
| downward-growing stack reservation | `0x200330c0..0x20042000` | 61248 |

`__sdata=0x20000000`, `__sbss=0x20000098`, `__ebss=0x20032ec0`, `__suninit=0x20032ec0`, `__euninit=__sheap=0x200330c0`, `_stack_start=0x20042000`. Total covered RAM is270336 bytes. There is no large hole between linker statics and stack. The misleading linker name `__sheap` is the beginning of the free stack-side remainder here: the Rust allocator uses its independent static BYTES array below that symbol, not this remainder.

Large static owners:

* main Embassy task pool: `0x200260b0..0x20031668`,46520 bytes. Root33432 inline is part of this future, not an additional allocation to add again; command input32548 is part of Root.
* resident OWNER: `0x20023d50..0x20026090`,9024 bytes. Already charged in the ELF static extent; do not add again as a heap need. Its inline geometry9016 excludes its storage wrapper/alignment.
* CYW43 radio STATE: `0x20020bb0..0x20023d48`,12696 bytes. It is a live network owner, not disposable scratch.
* USB task680 bytes, Wi-Fi task2096 bytes, network task136 bytes and radio task2568 bytes are also retained `.bss` owners.

Static bytes other than heap total78016. They cannot all become heap merely because software preparation got smaller. In particular, reducing a **heap-boxed** kernel's inline type from earlier sizes to31536 does not automatically shrink the linked main future or expand its fixed allocator array.

## Funding is smaller than the physical heap, not larger

`heap.rs` sets `CAPACITY=HEAP_BYTES`, initializes only static BYTES, and uses linked_list_allocator. `build.rs` takes HEAP_BYTES from checked Host `heap_arena_bytes`; the arena is part of the checked static bound. Its linker assertions separately require static fit and at least16384 stack bytes. That minimum is a linker rejection threshold, **not** a measured safe stack requirement.

`neutral_root::run` constructs/publishes actual equipment, initializes original Root and resident inventory, samples allocator `allocated()`, reserves this **baseline** in `StagedImageMemory`, and leases only `reserve_remaining()` for preparation. Reservations do not create backing. The baseline covers real preexisting Host/Seed/Root/inventory heap allocations. The33432-byte Root value is inline static task storage, while its dynamically allocated contents still contribute to baseline: those are distinct charges.

The CPU64 factory receipt measures151109 retained and153148 peak requested bytes, excluding baseline/equipment/residents/issuer grants, allocator rounding/metadata and stack. Relative to131072 this is already20037 retained and22076 peak excess **on CPU64**, before exclusions. Those differences are warning indicators, not target deficits to fix by moving exactly those amounts. ARM layout differs (kernel31536 versus CPU34176); allocator free-list geometry/fragmentation and target owner allocations require direct accounting.

Even treating22076 as a hypothetical transfer would leave only39172 stack bytes. No current evidence qualifies that partition, and changing it would also change the checked Host heap/static partition. This audit authorizes neither.

## Stack evidence and its limits

`stack_probe::initialize` marks only `__sheap..(MSP-256)` before task spawning, with interrupts masked. `scan_spare` returns contiguous untouched marker bytes from the low end. `checkpoint` captures phase/MSP and scans only once per fixed phase, leaving IRQs enabled during scanning; it is observation, not reservation or worst-case proof. The top256-byte exclusion and already-used initialization frames are not marked as freely reclaimable. A marker scan is conservative for writes it actually observes but cannot prove never-executed paths, nested interrupts, recursion or future workload maxima.

No physical deployment or matching Source76 runtime watermark is supplied by the current artifact. Historical evidence must not be reused as a current bound:

* `yellow-recovery93-later-memory-fault-20261111.json`: heap_limit110592, heap_used109232, peak109744; stack_spare4676, MSP0x2003b738, latest phase18/phaseMSP0x2003c130. Different firmware and partition; it proves neither spare61248 nor current fit.
* `source96-stack-audit-20261112/assessment.json`: physical stack59092 versus identified direct-call frames64664, exceeding it by5572 before deeper calls/IRQs. Different ELF and known failure.
* `rp2040-in-place-stack-frames-20261113.json`: old preflight local frame64236; `rp2040-cold-software-in-place-entry-frames-20261113.json`: another ELF preflight31580, near-quantity validation31892, numeric projection15556, mathematical decode15148. Entry frames are not summed-call bounds. Improvements between these artifacts do not establish today's deepest reachable call chain.

The flash margin49708 and immutable `.rodata` are not writable RAM. The512-byte `.uninit` must retain its actual owners; it is not a meaningful solution to the measured overlap.

## Is Root command/projection deduplication required next?

It is a promising **existing #6341 custody** seam, but this audit does not prove that every retained command/projection byte is redundant or that this is the only possible reduction. `HostCommandInput<N>` already owns one immutable caller-funded command buffer with attribution stamp. `execution_workset_receiving::receive` reborrows it and returns `DelegatedBodyFragment<'w>` values borrowing that backing; the inspected path does not blindly clone the whole command into each fragment. Likewise terminal contracts now support immutable linked backing. Do not remove the original authenticated command or claim a duplicate just from simultaneous bytes with related meanings.

Root's fixed command input32548 and original30314-byte assignment make the command lane a substantial owner, but a potential remaining ~2KB capacity difference is not a tens-of-KB solution and changing checked bounds requires requalification. Audit actual projection/table/value-schema allocations and provenance lifetimes before selecting the next reducer: retain original checked bytes once, borrow checked immutable tables where their lifetimes permit, and retire only preparation temporaries whose last use is proved. Do not silently replace command authority with unverified linked metadata.

## Next smallest safe qualification

1. Use this exact retained ELF to produce bounded entry-frame/direct-call-chain evidence for Root receive/preflight, selected kernel preparation and nested numeric/schema validation; include cold boot, Wi-Fi and IRQ frame allowance. This needs no firmware change or speculative heap increase.
2. Attribute target-specific heap requested/rounded live stock by original Root baseline, equipment/resident/issuer baseline, command decoding/projection, software initialization, value pools and kernel construction. Reconcile retained and peak overlap against **the remaining lease**, not total SRAM or CPU's larger arena. Fresh counters require a separate reviewed diagnostic change if current evidence cannot supply them.
3. Only if the unchanged-budget preflight is safe to execute, run the original authenticated full preparation entrance with current heap grant, capture phase watermarks and heap baseline/peak, then drop it. No admission/Start claim: `neutral_preflight` still deliberately drops its result and the required Fore/Mask/application integration remains unfinished. Preserve the existing working firmware until this bounded test is qualified.
4. If target heap still refuses, remove the largest **proved** duplicate/overlap under6341 and repeat this narrow qualification. Consider physical repartition only after a defensible exact-target stack bound plus explicit checked partition review; there is no demonstrated free region to spend now.
