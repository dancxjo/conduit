# Current hosted runtime performance capture

Local #5395 / #5231 evidence at clean source `3779e178308f6f65c583a89c30d33d9cf4897664`, 17 October 2026.

The supported command completed with one million expression values, three repetitions, 1,000 selection decisions, 1,000 event samples and the hosted profile enabled. All 16 optimized conformance tests passed before measurement. Compilation is outside the measured phases. The retained metadata records Rust 1.98.1, release profile, i9-12900H topology/frequency snapshots, affinity, OS, binary size and process CPU scope. No core pinning or controlled background load is claimed.

## Independent retained-data verification

The new developer verifier `proof/performance/verify-runtime-capture.py` checks the complete 89-file manifest, successful source/conformance custody, all 190 raw records and summary-to-raw metrics. It independently rebuilds all 78 repeated-batch distributions (mean, sample SD and nearest-rank percentiles) from raw samples. All 12 expression cases preserve output sums/digests across modes and repetitions.

All 27 ordinary/local/fused 64-output pipeline trials match the independently calculated SHA-256 of canonical U64 outputs 32 through 95. Each retains exactly 64 RemoteValueDelivered Signs, increasing unique event sequences, and unchanged admitted value capacity. The nine ordinary trials additionally match retained Plan identities and exact Host/Boot/Play observation custody. These checks do not re-run authored source checking or certify arbitrary Plan semantics.

Five negative cases reject changed artifact bytes, an altered/rehashed mean, a removed/rehashed delivery Sign, Python -O assertion disabling and attempted receipt creation inside the immutable capture. The negative receipt pins the verifier digest. Output receipts use exclusive creation.

## Expression-only warm observations

Means ± sample SD across three batches, milliseconds for one million values. Native uses optimized checked arithmetic/registers; prepared reference interprets canonical frames. Legal compiler folding remains allowed, so nominal source operators are not a hardware instruction count.

| Case | Optimized native, ms | Prepared reference, ms |
|---|---:|---:|
| u64-add1 | 2.924 ± 1.553 | 74.144 ± 0.133 |
| u64-add32 | 2.069 ± 0.024 | 1546.720 ± 0.513 |
| scalar-add1 | 2.022 ± 0.015 | 63.712 ± 0.045 |
| scalar-add32 | 5.382 ± 0.014 | 1194.036 ± 4.280 |

The large native U64 one-add variation is retained. The prepared-reference interpretation/framing cost remains substantial in this fixture; this does not characterize the separate installed local Back or imply a universal native-speed ratio.

## Ordinary hosted observations

Each trial performs the same 32 additions per value over 64 ordered values through checked Plot, sealed Plan, std Back, production kernel, typed cords and mandatory Signs. Full-call timing includes reservation, Back preparation, Play and report translation; cold parse/check/expansion/selection/sealing phases remain separately retained in raw rows.

| Logical Gears | Full Host call, mean ± sample SD, ms |
|---:|---:|
| 1 | 0.919 ± 0.048 |
| 8 | 2.027 ± 0.151 |
| 32 | 6.341 ± 0.093 |

The explicit local-computation profile is separately measured with and without physical pure-chain fusion:

| Gears | Local unfused, ms | Fused, ms |
|---:|---:|---:|
| 1 | 0.776 ± 0.300 | 0.841 ± 0.127 |
| 8 | 1.675 ± 0.124 | 1.754 ± 0.050 |
| 32 | 3.435 ± 0.130 | 3.349 ± 0.028 |

Those local profiles select a different realization from the ordinary std Host-call fixture. Physical fusion preserves intermediate values/cords and changes dispatch; its timing samples do not establish a stable win. More logical Gears still increase total full-call cost for equal useful arithmetic, but this capture does not isolate that cost to one kernel function. Independent-process/core/load controls remain required for regression thresholds.

Worker process resident high-water is 367,808 kB, including measurement/harness and retained data. The recorded executable is 22,053,376 bytes. Neither number is a production kernel footprint. Post-first-output allocation windows include Host/collection work; no whole-Play zero-allocation claim follows from unchanged value capacity.

## Evidence and reproduction

`evidence/hosted-runtime-refresh-20261017.json` retains the verified receipt, statistics, metadata and hashes. `evidence/hosted-runtime-refresh-20261017-raw.tar.gz` retains all raw samples, source/Plan/Host artifacts, conformance logs and command status (about 287 MiB uncompressed). All 89 archived artifact hashes were checked. `evidence/hosted-runtime-verifier-negative-20261017.json` retains the negative checks.

```sh
cargo +stable xtask prove runtime-performance --output /tmp/conduit-hosted-capture \
  --values 1000000 --repetitions 3 --decisions 1000 --samples 1000 \
  --hosted-pipeline true
python3 proof/performance/verify-runtime-capture.py \
  --capture /tmp/conduit-hosted-capture --output /tmp/conduit-hosted-receipt.json
```

The verifier is a developer evidence tool for hosted captures, not another product entrance. The original measurement worker/fixtures are unchanged. No historical before/after speedup is claimed across different sources or CPUs. Million-value end-to-end streaming, DSP/pressure/fairness, independent-process uncertainty, hardware counters, browser/physical/stable acceptance and parent closure remain open. No CI dispatched or awaited.
