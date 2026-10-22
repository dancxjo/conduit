# Matched native Source retention control

Prepared on the pending #6214 dependency (`0e6ffe777`); this is instrumentation
and a controlled measurement entrance, not a measured speedup or issue closure.

`cargo xtask make conduitos native-latency-proof --trials 3 --cache-mode retained`
and the same command with `--cache-mode zero-retention` each build one explicitly
instrumented architecture-proof image and run three fresh sequential QEMU
processes without retries. The ordinary product keeps its existing cache policy.
The zero-retention feature implies latency instrumentation and is selected only
by this observation entrance. No ordinary profile has a disabling option.

Both arms retain exactly the same installed catalogs. The control only disables
canonical checked Source and expanded Plot retention, using the existing
`SourcePreparationCache` zero-limit cold path. This comparison cannot attribute
total catalog-installation reuse versus pre-#6191 code. Current Host offer
planning, lowering, admission, kernel execution, and finite workload remain in
their existing paths. Historical measurements are context, not a matched arm.

Each cache record names the actual static Source digest, plot, mode, limits,
shared-store or private-contention path, preparation outcome, canonical stats
delta and that store's retained counts. Deltas are sampled before and after each
actual preparation. The consumer uses checked sums, including independent cold
fallbacks without resetting shared totals; private caches never claim shared
hits. Records are bounded to 512 per Boot and 768 bytes each; sequence, counter
saturation/reset or formatting exhaustion refuses the observation rather than
changing the operation's outcome. A fallback remains the existing finite cold
path and cannot spin, retry or block preparation.

Mode and limits must match the guest. The parser verifies Source digests against
the exact tool source, rejects absent families, sequence gaps, reordered/stale/foreign records, false
hits and retained entries in the zero-retention arm, and correlates every Face
custody record to the observed Host/Boot. Early records are Boot-scoped just as
early progress records are; they precede identity generation. The family and
per-trial receipts bind clean Source, image hash, selected/observed mode,
profile/build and fresh Host/Boot identity. The existing eleven-field progress
schema, 512-byte/2048-record bounds and all required phases remain unchanged.
Hits emit no manufactured parsing/checking/expansion spans.

No VM family has been run for this change. Before any causal reduction claim,
retain both complete three-process families at the same clean Source and tools,
explicitly preserve the different mode/image identities, compare exact canonical
meaning and action/typed-result/refusal behavior using their original receipts,
and report boot and individual action attribution separately from harness wall
and capture overhead. Raw Plan/Face/Show identifiers legitimately vary with each
fresh Boot and must not be equated across Boots. Raw ticks remain uncalibrated,
nested and nonadditive; emulator measurements establish no physical latency.
