# Scoped scalar tape performance

For #5542, a release benchmark at clean Source `8ad979ea88b5b833547d9b21161a3b29ed5404c3` compares the existing reference evaluator and explicit std scalar tape on the same checked 1/3/8/32-addition corpus. The archived JSON retains all 128 raw product/micro rows, exact Source, Plan/Play/provider/artifact identities, log digest and eight-row variance summaries. Native elapsed wall time is measured; this is not CPU time or source event time.

Optional no-op-by-default markers identify installed preparation start, actual kernel Play start and finish. The benchmark separately records authoring/Plan creation, evaluator preparation, warmed prepared evaluator throughput, an optimized Rust checked-multiplication reference, and the actual installed kernel Play. It verifies result parity before accepting every sample. Timing is report-only; no correctness or CI threshold depends on it.

| Addition operations | Reference warm evaluator ns/value | Tape warm evaluator ns/value | Reference actual Play µs | Tape actual Play µs |
|---|---:|---:|---:|---:|
| 1 | 74.72 | 89.70 | 12.15 | 10.49 |
| 3 | 175.78 | 143.92 | 10.76 | 10.89 |
| 8 | 423.70 | 278.35 | 11.56 | 11.33 |
| 32 | 1630.06 | 935.61 | 15.72 | 14.30 |

These are eight-row medians. The one-operation tape is slower in the prepared evaluator loop. Tape evaluator preparation also costs more: approximately 3.39 µs versus 1.10 µs for one operation, and 9.61 versus 7.61 µs for 32 operations. The warm evaluator gains at 3/8/32 do not justify a universal selection rule. Whole-Play differences are smaller and noisy; the three-operation tape median is slightly worse. Kernel scheduling, Host Calls, validation and Fore delivery remain real costs.

The optimized Rust loop is approximately 0.43 ns/value throughput for this independent repeated operation; it is neither pipeline latency nor equivalent kernel/validation work. The large gap remains visible rather than being labeled native speed. Row zero includes first-use cache effects. The process was unpinned, with concurrent development builds on the host; raw variance is retained and these samples are not calibrated production thresholds.

The tape declares 128 maximum instructions and 5120 bytes of fixed evaluation scratch on this native ABI, even for tiny expressions, plus prepared instruction/literal storage and output buffers. That cost matters for small Hosts. No tiny-device regression or suitability has been physically measured, and no installed no_std tape profile is offered. Browser currently advertises `browser/kernel-pure-expression@1` and prepares the reference evaluator; an API compiling for a target is not an installed tape realization. These unsupported eligibility boundaries remain explicit.

The parent #5396 retains its semantic/resource/cross-target gates and #5541 its full differential/refusal suite. This report adds no JIT, general code generation, universal speedup, physical timing guarantee or automatic Back preference.

Validation also passed all 116 existing installed-host regression tests with the
no-op default timing observer; strict library/benchmark Clippy denied warnings.
The archived samples retain their measured Source, independent of this report
commit or later dependency refreshes.
