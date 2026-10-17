# Independent single-CPU hosted runtime observations

Local #5395 / #5231 evidence at source `b72fa900f925d9c64205d4916fbe026cd7724a4e`, 17 October 2026.

Three separate supported CLI invocations each ran the 16 optimized conformance tests and the full one-million-value, three-batch, 1,000-decision/sample hosted profile. All succeeded. The source stayed clean and unchanged throughout. The first invocation rebuilt dependencies after an incoming Face change; later invocations reused the release build. Compilation and conformance are outside the timed worker phases.

## Observed process control

A scoped subprocess wrapper selected allowed CPU 4 from reported maximum-frequency limits and restricted only itself/its children to that CPU. A separate lightweight /proc observer recorded each actual measurement worker while live. Both process status and worker metadata report affinity 4. The normal task process retained its original 20-CPU allowed set; no governor or machine-wide settings changed.

| Worker | Observed PID | CPU affinity | Start-time ticks |
|---:|---:|---:|---:|
| 1 | 4036848 | 4 | 44301926 |
| 2 | 4038438 | 4 | 44311789 |
| 3 | 4039347 | 4 | 44319582 |

The workers report the same i9-12900H model, compiler/profile/configuration and exact frozen Plot digests. CPU 4 metadata snapshots were approximately 5,000 MHz; these are point observations, not a locked frequency. The worker executable reports 22,053,616 bytes in all three captures. Thermal state, interrupts, governor behavior and other shared-host load remain uncontrolled.

## Correct independent statistical unit

`compare-runtime-processes.py` invokes the existing capture verifier on each directory, checks source/config/CPU/control/process consistency and rejects reused capture/worker/Play identities. It retains each process's three raw batch samples, then derives one mean per process. Sample SD is calculated across the three process means, not across nine pooled batches. This is descriptive between-process dispersion, not an inferential confidence interval or calibrated regression gate.

The study verifies 267 capture artifact hashes, 570 raw rows and 234 within-process repeated distributions. All 81 ordinary/local/fused pipeline trials preserve the independently checked 64-output digest, required delivery Signs, Plan/work shape and value capacity. Each capture retains exact source/Plan/Host/Boot/Play custody.

## Ordinary full-call wall observations

Every trial performs 32 nominal additions per value over 64 useful ordered outputs. Full-call scope includes reservation, Back preparation, Play and report translation. Values below are milliseconds.

| Gears | Process 1 mean | Process 2 mean | Process 3 mean | Mean ± sample SD of process means |
|---:|---:|---:|---:|---:|
| 1 | 0.864 | 0.929 | 0.808 | 0.867 ± 0.060 |
| 8 | 2.062 | 2.119 | 1.892 | 2.024 ± 0.118 |
| 32 | 6.261 | 6.232 | 6.188 | 6.227 ± 0.037 |

## Explicit local/fused profile

This profile selects a different local-computation realization from the ordinary std Host-call fixture. Fused execution retains all logical intermediate cords and values.

| Gears | Local unfused, ms | Fused, ms |
|---:|---:|---:|
| 1 | 0.741 ± 0.005 | 0.826 ± 0.043 |
| 8 | 1.707 ± 0.074 | 1.805 ± 0.135 |
| 32 | 3.368 ± 0.068 | 3.380 ± 0.055 |

Fewer physical scheduler decisions do not establish a consistent full-call timing gain in these 64-output trials. The result is retained rather than selecting favorable batches. More detailed separation of Back preparation, validation, dispatch, canonical transport and report work remains necessary before attribution. No historical source/CPU speedup comparison is made.

## Retained data and refusal checks

`evidence/hosted-runtime-pinned-20261017.json` includes all 78 metric groups and three verified capture receipts. `evidence/hosted-runtime-pinned-20261017-raw.tar.gz` holds capture-1/2/3, control.json and processes.json (267 artifact hashes checked after archiving). Separate control/PID receipts are also retained alongside the report. The raw archive is about 17 MiB compressed.

Five negative cases reject duplicate capture roots, reused worker identities, an observed affinity mismatch, a rehashed worker-affinity mismatch and Python -O assertion disabling. Their receipt pins the comparison tool digest in `evidence/hosted-runtime-process-negative-20261017.json`.

To rerun the fixed measurement profile, invoke the supported command three times with distinct output paths under an allowed single-CPU affinity, recording actual worker PIDs/start times and masks. The retained control files describe the scoped wrapper and observed process facts for this study. To reproduce the analysis from the extracted archive:

```sh
python3 proof/performance/compare-runtime-processes.py \
  --capture /tmp/runtime-study/capture-1 \
  --capture /tmp/runtime-study/capture-2 \
  --capture /tmp/runtime-study/capture-3 \
  --control /tmp/runtime-study/control.json \
  --process-observations /tmp/runtime-study/processes.json \
  --output /tmp/runtime-process-comparison.json
```

The original recorded output paths remain inside capture status and match the process receipt even after archival relocation. The comparison is a developer evidence tool, not a new product entrance. Hardware instruction/copy counters, million-value end-to-end streaming, DSP/pressure/fairness, browser/physical/stable acceptance and parent completion remain open. No CI dispatched or awaited.
