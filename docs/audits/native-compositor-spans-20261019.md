# Native compositor spans and checked scanout

The development compositor selects visible surfaces by horizontal span instead
of searching every surface for every damaged pixel. It prepares the bounded
surface order once, splits rows at coverage and overlay edges, and uses checked
row writes for interiors. One-pixel damage retains a direct selection path.
The authored Face, Show revision, retained raster and input-routing contracts
remain unchanged.

`PixelTarget::write_span` defaults to the original ordered pixel writes, so
custom targets keep their exact first-error behavior. `RawDisplay` can check a
valid row once. Its existing constructor retains byte stores; a separate unsafe
constructor explicitly requires permission for aligned 32-bit volatile stores.
Unaligned pixels and odd pitches use byte stores. The Limine RGB32 binding opts
into word writes. These changes do not establish physical display performance.

## Memory target comparison

The measured source is `08c0cd8a81c8857904b4dd5f2ca8489cd40968c1`, retained on
`perf/compositor-measured-20261019`. The supported command is
`cargo xtask make conduitos compositor-performance`. It checks both compositor
and raw scanout scopes, requires their measurement markers, and refuses failed
or empty proof processes. The current helper uses one Cargo invocation for both
libtest filters, keeping them on the same test image.

The public capture passed five compositor tests and four raw scanout tests.
Three additional processes replayed the retained optimized executable on CPU 4;
each passed all nine tests and emitted 25 compositor and 15 raw scanout batches.
The test image uses optimization level 3 with debug assertions. Reference and
span realizations alternate batch order in the same image. These observations
come from one shared host with concurrent compilation, and have no confidence
interval or universal timing threshold. They measure memory targets rather than
QEMU or input-to-display latency.

| Damage fixture | Median of process span/reference ratios |
|---|---:|
| Full 640 by 480 redraw | 0.0351 |
| One pixel | 0.8744 |
| Scroll rectangle | 0.0428 |
| Hover rectangle | 0.2201 |
| Typing rectangle | 0.0577 |

The raw writer fixture writes sixteen 640 by 480 frames per batch. Its median
process ratio is 0.7849 for checked word pixels versus byte pixels, and 0.3478
for checked word rows versus byte pixels. All raw observations are retained;
the first process had substantially longer absolute batches than the others.

The [measurement receipt](evidence/native-compositor-spans-20261019/evidence.json)
binds the executable digest and raw logs. Its timing aggregate uses only the
three retained-worker processes, not the initial public capture. The
[source comparison](evidence/native-compositor-spans-20261019/source-bridge.json)
verifies the listed owned source files are byte-identical after refreshing onto
development containing the audio and borrowed-Flow changes. That comparison
does not substitute for testing the refreshed candidate.

## Correctness and acceptance

The frozen per-pixel reference checks 768 combinations of bounded surfaces,
damage and injected target failures, comparing pixels, coverage, counts and
partial failure results. It also checks invalid damage and focus/cursor edges,
with an allocation guard over composition. Placement already replaces the
raster buffer and invalidates its receipt when surface dimensions change.

Raw scanout tests compare byte, word and span paths across four alignments and
six pitches, including odd row pitches, guard bytes and padding. They check
loss, invalid and empty spans, zero allocation, the default first-error path,
and forwarding through mutable target references.

Workspace Clippy with warnings denied and focused tests passed on the measured
scanout implementation. The earlier span-only implementation also passed
1,143 ConduitOS library tests. A monolithic local workspace build was stopped
before exhausting available disk space; it did not reach test execution and
provides no workspace test result. Final candidate CI must cover that scope.

The refreshed helper still needs its supported-command replay. Actual QEMU
journey and responsiveness proof, startup-stage measurements and incremental
retained-raster reuse remain separate required work. This is a development
measurement, with stable acceptance reserved for the automated release train.
