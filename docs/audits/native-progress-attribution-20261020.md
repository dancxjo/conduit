# Bounded Root preparation progress for #6180

The existing completed-interval timing stream remains unchanged. A separate
`CONDUIT_NATIVE_PROGRESS` diagnostic stream emits before a long operation and
when it finishes or returns its existing refusal. It is enabled in graphical
x86_64 bare-metal code independently of the optional latency feature. Hosted
calls are no-ops; these records are Root observations, not lifecycle Signs,
Plot meaning, scheduling authority or a new semantic declaration wire.

Each event carries a Boot-scoped sequence, span/parent, finite phase, bounded
context, outcome and raw fenced TSC ticks with unknown frequency. Phases cover
xHCI/USB/machine setup, identity generation, display/input preparation, first
decoded-input wait, optional owner network admission, Face construction, Mask
preparation, first Show, damage-bearing compositor scanout, native Plot build,
catalog installation, parsing, checking, expansion, resident identity resolution,
Host/capability preparation, planning, lowering and kernel preparation.

The first-input interval begins after front-door setup and ends at the first
decoded key/pointer input. It includes polling/service/harness work and does not
claim time-to-input-readiness or replace the existing exact first usable Show ACK.
Idle composition calls still execute their ordinary acknowledgement path; only
pending damage consumes scanout diagnostic spans, avoiding an idle-loop flood.

## Finite storage and failure behavior

- At most 1,024 spans / 2,048 events per Root Boot, 64 ASCII context bytes and
  a 512-byte fixed formatting buffer including prefix/newline. No diagnostic
  heap allocation or retained variable-length runtime event collection is added.
- Three atomic counters and a failure flag track synchronous Root nesting.
  Other CPU domains must not publish this stream; this is not an SMP claim.
- An observer bound, bad nesting, invalid context or reversed clock produces one
  explicit observer refusal and freezes further observations. Underlying operation
  results and mandatory Signs retain their existing paths.
- Operation `Err` results produce structured refused events. A dropped scope is
  abandoned, never inferred success. The proof parser rejects abandoned,
  incomplete, reordered, replayed, malformed, overflowing or cross-Boot families.
- The producer's phase/sequence/counter data is bound to the same journey's
  independently validated native Face/Show Host/Boot custody. The network phase
  is explicitly optional for an unprovisioned local product, and the report
  records whether it was exercised.

Raw clock reading is shared with the original observer. No calibration,
tick-to-time conversion, interval summation into exclusive CPU time, new asset,
font/paint change, graphics capacity increase or Source-validation bypass occurs.
All existing static Source checking and expansion still happen; this is the
attribution step before reuse, not an optimization or speedup claim.

## Hosted gate

48 focused checks pass: five progress-state checks, nine native parser/timing
checks (including three new progress checks), 19 native workset checks and 15
compositor checks. Two existing manual compositor observations remain ignored.
The tests cover nesting/outcomes, one-shot exhaustion, clock/order/context
refusal, actual Root protocol parsing/custody, workset admission and raster/ACK
regressions. Formatting and diff checks pass. [Retained logs](evidence/native-progress-attribution-20261020/hosted/).

The initial unit compilation hit local disk exhaustion; after the compiler
stopped, inactive regenerable caches were removed while preserving executables,
source, images and captures. Intermediate compile errors during editing are not
passing evidence. The focused results above are from the corrected compiled
candidate. Native execution and measured attribution require a separate clean,
source-pinned family. #6180 remains open until reuse and controlled measurements
satisfy its original acceptance; #6158/#6055/#6155 remain skipped.

## Actual attributed family before reuse

Clean unchanged source `83c25d3159ffb1a02f07f230a879da194e21b109` completed
`cargo xtask make conduitos native-latency-proof --trials 3` with exit 0, three
fresh QEMU processes, one image/profile/workload and no retry. Exact ISO SHA-256:
`72a06912c093769e017833e406e324aec7cf2d5c0de4184edb26e998a0420b78`.
The family `2389930-1792554956576196096` includes the combined #6188 dev changes.

| Fresh run | Full journey wall seconds | Progress events / spans | Boot → first framebuffer commit, ticks | Boot → actual usable input ACK, ticks |
| --- | ---: | ---: | ---: | ---: |
| 0 | 87.227176703 | 1,360 / 680 | 10,182,299,639 | 11,031,309,679 |
| 1 | 87.072837347 | 1,360 / 680 | 10,106,732,947 | 10,961,099,939 |
| 2 | 88.674289081 | 1,362 / 681 | 10,220,407,966 | 11,051,637,490 |

Every stream has matched start/end scope custody, all required local-product
phases, no observer refusal or abandoned scope, and fewer than 2,048 events.
Each run has 105 PlotBuild operations: Keyboard canvas 38, Memory Lantern 29,
Tour 23, Patchbay 14 and Hello 1. The Hello Source remains outside the installed
native workset; checking its identity does not confer execution eligibility.
The third run has one additional damage-bearing scanout (109 versus 108),
explicitly retained rather than discarded. The optional owner-network path was
not exercised in these unprovisioned guests. All operation spans completed
without refusal; refusal behavior is covered by the hosted protocol tests.

The added Source child phases reveal where the repeated preparation time goes:

| Phase | Calls in each run | Aggregate elapsed ticks, run0 | run1 | run2 |
| --- | ---: | ---: | ---: | ---: |
| Catalog installation | 105 | 48,774,165,392 | 48,742,008,709 | 49,641,143,280 |
| Source parsing | 105 | 85,120,292 | 81,004,281 | 81,447,458 |
| Source checking | 105 | 30,170,714,750 | 30,129,225,699 | 30,779,609,821 |
| Canonical expansion | 105 | 64,644,362,436 | 64,606,974,384 | 65,676,317,490 |

Expansion, catalog installation and checking dominate these observed child
intervals; parsing alone is small. The enclosing PlotBuild totals are
146,076,529,021 / 145,925,215,790 / 148,612,783,226 ticks. Kernel preparation is
16,682,713,461 / 16,667,234,650 / 17,065,814,196 ticks. These include observer
cost, can nest with planning/workset intervals and must not be summed into
exclusive CPU time or converted using an inferred TSC frequency. The independent
record check reconstructs parents/phase totals from original serial events and
matches the produced receipts exactly.

All 93 actual PNG captures (31/run) independently matched original byte lengths,
file SHA-256, dimensions and decoded RGBA hashes. All 4,082 original progress
events independently match their receipts; recomputed phase counts/ticks and
closed parent scopes agree. All three original frame manifests are complete
with no failure. [Full captured family, raw serial/QMP, timing/typing/reading
receipts, PNGs, image metadata and verification](evidence/native-progress-attribution-20261020/captured/).

This is the instrumented **architecture-proof-appliance** exercising the ordinary
graphical journey. Full journey wall includes QEMU, scripted inputs, polling,
capture and teardown; first input ACK includes harness work and is not a readiness
time. Host/OS/compiler caches are uncontrolled. Normal-product execution without
the latency feature, physical/human proof, calibrated time, SMP and a speedup
are not claimed. The older 21777e523 baseline remains separately attributed;
its different source/observer cannot establish an optimization gain here.

The next step is a finite checked/expanded Source inventory reusing #6087's
canonical preparation mechanism, with fresh Host planning and unchanged semantic
results. Preserve the actual parse/check phase boundaries when adding reuse;
do not add a dummy parse or relabel a combined preparation cost as checking.
Then repeat this exact profile/workload with three fresh processes and record
controlled counts, timings and semantic regressions. #6180 remains open.
