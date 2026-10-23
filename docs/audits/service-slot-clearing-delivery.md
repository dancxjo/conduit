# Service-slot clearing delivery (#6183)

Service-slot installation cleared storage that had already been cleared at retirement. The delivered change removes that duplicate pass. A mutable lease marks storage dirty; retirement clears dirty storage before releasing ownership. Static storage and retired leases start clean. Privilege maps, grants, admission and the conservative CR3 path remain unchanged.

Measured setup clearing accounted for 93.5304–96.6607% of the outer setup interval in fourteen exact instances. This identifies the cost addressed here; it is not a calibrated hardware latency or a causal end-to-end speedup claim.

## Source-bound evidence

Each packet retains its measured source. These runs are not relabeled as executions of the delivery commit.

| Scope | Retained evidence commit / path under docs/audits/evidence |
|---|---|
| Combined setup, CR3, register/IRQ components, staged IPC and native protection | `4e363400a9f714d7f534b4ac7a0b55c7726b5f56` (combined measured source `96a9bb5c1f312f6beca5efb8f9c6c2316bafa22c`) |
| Baseline/optimized native protection, restart zeroing, hostile guest and sibling controls | `7deda45596fae3747166f23dbdcef2f9552195f4` / `service-clearing-native-pair-20261023` |
| Matched normal latency and graphical journeys | `f19f77960` / `service-clearing-normal-pair-20261023` |
| Actual Root flags restoration and bounded software trap envelopes; full native appliance | `47f1c6e423587b3613cb2d0298fb32ddef2ac8ae` / `service-trap-envelope-native-20261023/b0883a753` |
| Unmodified timing records, delivery correspondence and focused lifecycle test | `47f1c6e423587b3613cb2d0298fb32ddef2ac8ae` / `service-clearing-normal-timing-20261023` |

The trap run measures baseline storage, not optimized storage. Twenty rows preserve actual Root flags; preemption has a null terminal envelope rather than an invented zero. Entry, terminal dispatch and overlapping restoration intervals are software envelopes. They do not isolate hardware trap entry, pure IRET, or full IRQ preservation cost. The native protection pair separately covers optimized storage.

## Kernel and responsive-journey observations

The matched normal sources are baseline `1f7278327cfeb41e0fb2b86218efcde1cffaea83` and optimized `2ffef51ca29975f2fe858d32f10d1385934db419`.

| Trial | Baseline kernel prepare ticks | Optimized ticks | Baseline journey seconds | Optimized seconds |
|---|---:|---:|---:|---:|
| 1 | 934570184 | 802026038 | 34.020398339 | 25.762650910 |
| 2 | 965352676 | 835772072 | 32.321365726 | 26.036856503 |
| 3 | 939970852 | 761758792 | 35.445986180 | 24.397296595 |

Kernel records use `x86-tsc-lfence` with unknown frequency. Build durations are excluded from this table. Arm order and caches were uncontrolled; these observations establish neither a causal percentage gain nor physical-machine latency. Both graphical arms passed their historical 24-step/27-frame contract, with 54 actual PNGs reviewed. Later Source/Collection/Patchbay routes are not covered by those historical journeys.

## Delivery correspondence and checks

The two production blobs and lifecycle test in delivery source `dc429ecabe6b887013e06d321c94fa5f640389d0` exactly match the optimized measured pair. The delivery starts from dev `e57706b15e65f075d6cfa9783de3c5fe44836b2c`; 130 selected owning files remain identical to the measured normal baseline. This is selected backend/grant/lifecycle correspondence, not whole-tree identity. Broader UI and hosted adapter code changed.

The focused lifecycle test passed once on that delivery source: one passed, zero failed or ignored. It exercises actual storage zeroing across all regions, duplicate-claim refusal, dirty mutation after explicit clearing, retirement/reclaim and partial-failure reclamation. Its executable and raw log are retained in the timing packet. This was an internal focused Cargo check, not another native wrapper invocation.

PCID was unavailable in the measured QEMU profile; no unsupported ASID scheme or privilege relaxation was introduced. Existing staged IPC retains its bounded grant checks. Relevant Candidate and accepted native Integration checks remain required before implementation closure. New custom runtime experiments are warranted by relevant semantic drift or an actual failure, rather than unrelated route changes alone.
