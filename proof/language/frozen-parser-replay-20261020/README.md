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

The second run is in progress. No complete aggregate, accepted model, new default,
installed Current controller, broad accuracy or real-time claim follows from
this preparation. Numerical/canonical prerequisites #6062/#6063 and normal
reviewed acceptance gates remain separate.
