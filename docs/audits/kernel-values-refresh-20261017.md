# Native kernel refresh after value and fan-out changes

Development #5231 / #5395 evidence, 17 October 2026.

Current source `38ebd721d22ed1f58c662c72a25fcf0aaa76ec87` is compared with two captures of baseline `63bda2cf824c283df28e939549f744eb78148eb3`: the earlier retained capture and a new historical-checkout repeat made immediately after the current capture. All use the unchanged harness, Rust/compiler profile, reported i9-12900H CPU and affinity. Each capture has three independent processes, eight trials and sparse/dense 1/8/32/256-node fixtures.

Both comparisons verify exact Sign/remote/output/readiness/value/terminal replay, allocation/copy parity and unchanged recorded work counts across 192 matched cases each. All 60 unique raw capture artifact hashes are checked in the retained archive. Instrumented scheduler objects grow by 24 bytes at every capacity relative to this baseline; test counters are included, so this is not a production-footprint measurement.

| Nodes | Pattern | Earlier baseline mean (process range), ns | Current mean (process range), ns | Repeated baseline mean (process range), ns |
|---:|---|---:|---:|---:|
| 1 | Dense | 796 (773–821) | 987 (894–1,060) | 897 (522–1,243) |
| 1 | Sparse | 470 (291–660) | 603 (464–688) | 753 (602–831) |
| 8 | Dense | 1,448 (1,251–1,711) | 1,869 (1,609–2,287) | 1,897 (1,633–2,108) |
| 8 | Sparse | 464 (276–680) | 676 (498–782) | 744 (674–790) |
| 32 | Dense | 4,241 (3,648–4,964) | 4,635 (4,396–4,867) | 5,322 (4,609–5,791) |
| 32 | Sparse | 478 (305–630) | 688 (503–846) | 738 (543–855) |
| 256 | Dense | 32,530 (28,615–38,319) | 38,756 (32,054–43,514) | 40,533 (35,820–43,898) |
| 256 | Sparse | 554 (321–705) | 745 (591–885) | 903 (718–1,006) |

The current means initially appear higher than the earlier baseline at every shape. The repeated baseline itself shifts upward in most shapes and shows substantial process variation. For one dense node its process means span 522–1,243 ns, overlapping current 894–1,060 ns. This does not establish a stable regression, stable timing equivalence, or causal speedup. The one-node repeated-baseline point estimate is still lower than current; a controlled core/power/load experiment would be needed before setting a regression threshold. All three captures are retained rather than discarding the unfavorable initial comparison.

Timing spans fixture advance with observation/assertion work. Sparse receives one useful value; dense receives one per node. Captures are on a shared uncontrolled host, with no pinning to a specific performance/efficiency core. Matching reported affinity permits descriptive comparison but does not establish identical actual execution conditions. Compilation is outside fixture timing.

The fixtures contain ordinary native one-byte sinks; they do not measure the new large fan-out, Host Call, generation-retirement or quiescence workloads. They check the common path and replay after those changes. No complete checked hosted arithmetic pipeline, FARGAN, browser, physical or stable acceptance is claimed.

Evidence: `evidence/kernel-values-refresh-20261017.json` compares the earlier baseline; `evidence/kernel-values-repeat-comparison-20261017.json` compares the repeated baseline. `evidence/kernel-values-refresh-20261017-raw.tar.gz` contains before-earlier/current/before-repeat directories with raw rows, process CPU/RSS receipts, clean/build logs, compiler/source/executable custody and status hashes. Both measured packages rebuilt with fresh=false; distinct executables are verified.

Reproduction uses the unchanged `proof/performance/kernel-overhead.py --include-large --runs 3` on clean current/historical sources, followed by `verify-kernel-refresh.py`. No CI dispatched or awaited; parent acceptance remains open.

The temporary historical worktree was removed without force after fresh remote
refs, clean tracked/untracked inventory, target-only ignored artifacts, no active
repository-worker cwd ownership and proof its exact HEAD remains reachable from
origin/live. The cleanup receipt records the three protected system service
processes excluded from repository-worker inspection. Observed available space
increased by 566,837,248 bytes (about 541 MiB); the disposable target's apparent
size was 300,698,754 bytes, alongside the removed clean checkout. All branches and
raw evidence are preserved. See evidence/kernel-values-refresh-cleanup.json.
