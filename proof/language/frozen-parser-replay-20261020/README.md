# Frozen parser replacement diagnostic

Recovery for #6076 and parent #4907. This record is distinct from the original
29-of-34 partial run preserved at `prep/4907-campaign-checkpoint-20261019`.

The original executable, raw traces and source-pin file were not recovered.
A full 34-history replacement was frozen before execution from source
`26b13d753200a291768949a02b0c4dfe88276c0f`, using the original fitted model,
case, cohort, fit, trainer, gate, specification and lexical-profile hashes.
No original partial report is used in the replacement aggregate. Original
source-pin equivalence is not established. The evaluator is byte-identical to
its archived compiled-source copy; the loader's sole change uses fixed-size
`as_chunks` in ABI verification instead of `chunks_exact`.

`replay-plan.json` records the boundary; `source-pins.json` binds the preserved
source snapshot, and `worker-custody.json` binds the rebuilt executable.
The relocated manifest retains historical absolute evidence paths, not a
portable product interface. The runner is an internal diagnostic test, not an
installed language consumer. `verify.py` refuses an incomplete report set and
checks the retained receipts and raw canonical Source trace files.

The first replacement terminated after 28 reports when the workspace filesystem
ran out of space while retaining canonical Source observations. Its terminal
receipt is `first-replacement-terminal.json`; it is incomplete and excluded
from any aggregate. A fresh whole34 run uses the identical worker, manifest,
models and schedule, with output on a separate tmpfs to isolate workspace disk
pressure (`second-replacement-custody.json`). No reports are mixed across runs.

The second run completed all 34 histories with exit status 0.
[COMPARISON.md](COMPARISON.md) and `comparison.json` retain the complete paired
result. Neither arm is accepted as a new model/default or installed Current
controller; no broad accuracy or real-time claim follows. Numerical/canonical prerequisites #6062/#6063 and normal
reviewed acceptance gates remain separate.

## Local completion and next consumer

The complete second worker finished 34/34; no third run was started. The
comparison retains attachment/vocative/control outcomes, commitment timing,
revisions, contradictions and actual time/resource limits. The raw traces and
all reports have a durable local copy recorded in `retained-artifacts.json`.
No fitted inputs or scheduling policy changed.

The next consumer is [#5755](https://github.com/dancxjo/conduit/issues/5755),
compact vocative realization from supplied committed structure. Its accepted
#5724 gate is recorded in release #6160. Reuse #5725's lexical/IPA projection and
the common prosody carrier for initial, medial and final vocatives, with
source-linked phrase/boundary/pitch and PCM comparisons, comma-only controls,
stale/session refusals and immutable played-output controls. This selection
starts no implementation, changes no parser default, and does not grant
installed Current authority. #6073/#6074/#6075/#6077 remain separate work.

At the user's request, this is a local-work-first draft: automated proof and
CI shepherding are deferred. Existing source-bound evidence is retained with
its actual limits. Merge/publication acceptance is not claimed.
