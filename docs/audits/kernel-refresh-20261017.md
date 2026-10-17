# Native kernel refresh after Host and quiescence changes

Local development evidence for #5231 and #5395, 17 October 2026.

Source `de38706add56eca5e13f549f3bd5a5aa55fdf2a4` is compared with `63bda2cf824c283df28e939549f744eb78148eb3` using the unchanged production-kernel capture harness. Both captures report the same i9-12900H CPU, affinity, compiler and release profile. Three independent processes per side run eight trials of sparse/dense 1/8/32/256-node fixtures. The baseline is the retained post-ready-summary capture; this is an aggregate refresh across subsequent changes, not attribution to one optimization.

The verifier checks all 40 raw artifact hashes, forced package rebuild provenance, different executable identities and 192 matching cases per side. Exact Sign/remote event text, outputs, readiness, value ownership and terminal results match. Allocation/copy counters and recorded work counts match. Instrumented scheduler objects grow by 16 bytes at all four capacities; these include test counters and do not establish production layout.

| Nodes | Pattern | Before mean ns (process range) | After mean ns (process range) |
|---:|---|---:|---:|
| 1 | Dense | 740 (603–835) | 796 (773–821) |
| 1 | Sparse | 809 (688–1,001) | 470 (291–660) |
| 8 | Dense | 1,792 (1,740–1,844) | 1,448 (1,251–1,711) |
| 8 | Sparse | 659 (623–678) | 464 (276–680) |
| 32 | Dense | 4,548 (4,478–4,640) | 4,241 (3,648–4,964) |
| 32 | Sparse | 613 (592–626) | 478 (305–630) |
| 256 | Dense | 35,491 (33,736–38,133) | 32,530 (28,615–38,319) |
| 256 | Sparse | 787 (744–867) | 554 (321–705) |

Durations cover the fixture advance with observation/assertion work, not a single logical Step. Sparse receives one useful value; dense receives one per active node. The host is shared and uncontrolled. Process variation is substantial, especially in current sparse cases; these observations establish neither a stable speedup nor a regression threshold. Tiny-host timing equivalence remains unproven.

These native one-byte sink fixtures contain no Host Calls or quiescence tracks. They check the common kernel path after those changes; they do not measure their intended workloads. They also do not cover the full checked hosted Plan/Play arithmetic pipeline, FARGAN, browser execution, physical devices, or stable publication. Parent issues remain open.

The comparison report is `evidence/kernel-refresh-20261017.json`. The complete before/after capture directories, build and clean logs, executable/compiler provenance, raw rows, process CPU/RSS receipts and status hashes are retained in `evidence/kernel-refresh-20261017-raw.tar.gz`.

Reproduce with `proof/performance/kernel-overhead.py --include-large --runs 3` on clean source checkouts, followed by `proof/performance/verify-kernel-refresh.py --before BEFORE --after AFTER --output REPORT`. Capture cleans only the kernel release package, retaining dependencies, and rejects a fresh measured compiler artifact. No CI was dispatched or awaited.
