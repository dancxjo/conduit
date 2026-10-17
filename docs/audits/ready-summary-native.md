# Same-host ready-summary production kernel observation

Local #5231 / #5395 evidence compares pre-summary source
`aa0e94966b38e0a34291d7a2e6a44eb6f2fd8f5f` with source
`de38706add56eca5e13f549f3bd5a5aa55fdf2a4` (the full exact identity is retained in the machine-readable report).
Both captures use the same harness digest, Rust 1.98.1, optimized profile,
i9-12900H CPU model and affinity. Each runs three independent processes with
eight trials of sparse/dense 1/8/32/256-node production FixedScheduler fixtures.
Compilation is outside measurements. This is a shared, uncontrolled host.

All 192 matched cases preserve exact Sign/remote lifecycle text, received values,
readiness, live values and terminal outcome. Allocation/copy observations match.
The only recorded work changes are 256-node readiness-byte probes: sparse
96 to 17, dense 544 to 517. Instrumented scheduler object sizes are unchanged
at all four capacities. These are fixture-wide probes, not probes per selection.

## Descriptive wall observations

The table shows each side's mean fixture advance duration and the range of its
three process means, in nanoseconds. The interval includes consumption, return
to Idle, counters and assertions. Sparse fixtures receive one useful value;
dense fixtures receive one per node. These small timings are noisy and are not
a latency percentile study or a stable performance gate.

| Nodes | Pattern | Before mean (process range), ns | After mean (process range), ns |
|---:|---|---:|---:|
| 1 | Dense | 801 (430–1,088) | 740 (603–835) |
| 1 | Sparse | 764 (399–996) | 809 (688–1,001) |
| 8 | Dense | 1,996 (1,545–2,352) | 1,792 (1,740–1,844) |
| 8 | Sparse | 742 (341–992) | 659 (623–678) |
| 32 | Dense | 4,770 (3,731–5,571) | 4,548 (4,478–4,640) |
| 32 | Sparse | 711 (310–964) | 613 (592–626) |
| 256 | Dense | 53,617 (29,264–92,485) | 35,491 (33,736–38,133) |
| 256 | Sparse | 768 (521–968) | 787 (744–867) |

The dense 256-node baseline includes a large process outlier; the sparse ranges
overlap substantially. Tiny cases also vary between processes, although their
capacities retain the direct path. The data do not establish a stable speedup,
a regression threshold, or tiny-Host timing equivalence. Do not attribute the
apparent dense timing difference solely to readiness summaries. The deterministic
probe reduction and exact replay are the strong conclusions.

## Build custody correction

An initial shared-target attempt reported the after checkout as fresh and
returned the same executable hash despite changed kernel source. That attempt
was rejected and retained separately outside these accepted artifacts. The
capture now cleans only `conduit-kernel` release outputs before building, keeps
dependency caches, records clean logs, and requires the measured package's build
record to report `fresh=false`. Both accepted executables were rebuilt and have
different hashes. The comparison verifier also rejects identical executables
for a changed-source comparison. That negative check was exercised with the
actual rejected attempt. Older 144-case captures remain supported.

## Retained evidence and reproduction

`evidence/ready-summary-native.json` verifies all 40 capture artifact hashes,
192 cases per side, exact replay, work counters and descriptive timing summaries.
`evidence/ready-summary-native-raw.tar.gz` retains both complete accepted capture
directories, including raw rows, process CPU/RSS records and build provenance.
Run the current harness against clean pre/post checkouts and new output paths:

```sh
python3 proof/performance/kernel-overhead.py --source-root /tmp/ready-before \
  --include-large --runs 3 --output /tmp/ready-before-evidence
python3 proof/performance/kernel-overhead.py --source-root /tmp/ready-after \
  --include-large --runs 3 --output /tmp/ready-after-evidence
python3 proof/performance/verify-kernel-refresh.py \
  --before /tmp/ready-before-evidence --after /tmp/ready-after-evidence \
  --output /tmp/ready-summary-comparison.json
```

The temporary historical worktree was removed without force after a remote
refresh, clean tracked/untracked/ignored inventory, no active working-directory
owners, and proof that its exact HEAD remains reachable from origin/live.
Observed available space recovered: about 251 MiB. Branches and rejected/accepted
evidence were preserved; only the main checkout remains.

These fixtures are native one-byte sinks, not a whole checked std Host arithmetic
pipeline or FARGAN. They do not exercise 10,000-node execution, Host Calls,
quiescence tracks, physical devices or browser execution. Parent acceptance
remains open. No CI was dispatched or awaited.
