> Current parent #5395 results are in [the final report](runtime-performance-5395-final.md)
> and [acceptance audit](runtime-performance-5395-closure.md). This older #5538
> report remains historical evidence at its original Source.

# Repeated runtime measurements and report-only CI

For #5538, the existing `cargo xtask prove runtime-performance` entrance retains
raw samples, exact source custody, release conformance and per-workload batch
variance. Candidate adds an independent report-only job with no dependency from
the required candidate gate. Timing thresholds remain uncalibrated.

The native Linux capture at clean Source
`b1b01be3fcbdcf70810dafcd4b594961d5bbd3a7` passed all ten release conformance
tests, strict release Clippy and pinned actionlint. The actual hosted command
passed with one million values, three repetitions, 1,000 decisions and 1,000
latency samples. Independent verification checked all 53 retained artifact
hashes, 172 raw records and all 66 repeated-batch distributions against their
original samples. The compact retained receipt is
[evidence/runtime-performance-report.json](evidence/runtime-performance-report.json).

| Workload | Optimized native mean ± sample SD, ms | Prepared reference mean ± sample SD, ms |
| --- | ---: | ---: |
| U64 one add, one million values | 2.788 ± 1.269 | 75.626 ± 3.200 |
| U64 32 adds, one million values | 2.096 ± 0.069 | 1565.026 ± 1.539 |
| Scalar one add, one million values | 2.129 ± 0.031 | 63.859 ± 0.830 |
| Scalar 32 adds, one million values | 5.539 ± 0.092 | 1198.407 ± 2.822 |

The native compiler can legally combine arithmetic; the prepared reference
interprets canonical frames and checked operations. These rows measure that
specific framing/interpretation tax. They do not measure the separate scalar
tape Back or imply a universal native-speed ratio. The large native U64
one-add variation is retained, not filtered.

Ordinary 64-item checked Plot/Plan/std Back/kernel trials hold useful work at
32 additions per value while distributing it over 1/8/32 Gears. Full Host-call
wall means are respectively 0.826 ± 0.261, 2.331 ± 0.040 and 12.214 ± 1.446 ms.
These include reservations, Back preparation, Play and Sign/report translation.
The raw receipt retains exact Source, Plan, Boot and Play, ordered output hashes,
mandatory Signs, capacities, cold Plot/Plan phases and the matched canonical
native loop. Warm expression batches and hosted full-call measurements have
different boundaries. Finer cold/warm inside-kernel phases remain #5536.

This is descriptive variance across three batches in one process. Affinity,
frequency and background load were uncontrolled; process CPU and wall clocks
remain distinct. A single observation has no sample standard deviation, and an
unavailable CPU clock does not become zero. No independent-process confidence
interval, calibrated regression threshold, physical device, browser performance,
or parent #5395 completion follows. CI artifact retention is fourteen days;
failed or missing capture is unavailable evidence, even though it cannot block
merging. See the worker README for the reproducible command and exact boundaries.

The publication refresh onto accepted `dev`
`ca95917e93c9f6f822a31cdd0304d3f5712ee838` preserves the measured report code,
xtask entrance and CI job unchanged. The original capture remains attributed to
its measured Source above; later accepted parser guards and timing markers do
not retroactively change that identity.
