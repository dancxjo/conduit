# Global model-memory and uncertified deadline refusal

The global planner's model-memory regression uses ordinary checked
`llm/generate` Gears, the existing local-model provider fixture catalogue,
source-bound forecasts, current resource observations and sealed Plans.
The GPU and CPU are explicitly modeled offers; no device is opened and no
inference is timed.

Each of two model instances requires eight MiB of model memory. Both Hosts
offer enough compute, inference slots and queue capacity for both instances.
The supplied GPU forecast is one millisecond per operation, versus ten on the
CPU. With sixteen MiB of GPU memory, both placements select the GPU. With
eight MiB, the Plan reserves eight on each Host. If CPU memory falls below
eight MiB too, global search refuses the entire assignment exhaustively.
Every successful result passes ordinary Plan verification. The ample-memory
control distinguishes memory admission from unrelated slot or queue limits.

`favorable_forecasts_cannot_certify_a_deadline_request` also exercises the
global entrance with `PerformanceIntent::BoundedResponse`. The existing
generic selector returns `UnsupportedIntent` before global search can produce
a Plan. A favorable forecast is therefore not a WCET certificate.

This is an explicit unsupported boundary, not an integration of deadline
regions into global placement. The separate
[deadline/WCET contract](../architecture/deadline-wcet-regions.md) remains
responsible for admitting exact worst-case timing bases. This regression does
not claim physical timing, automatic deadline replanning, model-memory
deduplication, physical GPU availability, or execution speedup.

The executable cases live in
`architecture/planner/tests/global_model_memory.rs` and are included in the
ordinary planner integration-test selection. The parent issue's other
global-placement, transfer, authority, incremental and processing proofs
remain independently necessary.
