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
