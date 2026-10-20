# Conduit performance audit

Repository: `dancxjo/conduit`, current `dev` source and retained performance reports

The strongest finding so far is that Conduit's execution overhead is often larger than the computation it's performing. The project has already made real progress on scheduling and expression specialization, but improving the scheduler alone will not solve the largest problems.

Three findings stand out immediately:

1. Execution and coordination: The [runtime performance audit](https://github.com/dancxjo/conduit/blob/dev/docs/audits/runtime-performance-5395-final.md?utm_source=chatgpt.com) finds substantial overhead compared with optimized native arithmetic. Additional Gears introduce dispatch, Cord, validation, and Host coordination costs.
2. Large computational graphs: Conduit's [full FARGAN performance report](https://github.com/dancxjo/conduit/blob/dev/docs/audits/scheduler-fargan-completion-20261017.md?utm_source=chatgpt.com) describes 768 nodes and 1,211 Cords for a 160-sample audio frame. It records roughly 2.8 seconds of fresh preparation per invocation, with preparation dominating wall time. That is a serious warning about executing fine-grained numerical graphs.
3. Data movement: The [fusion audit](https://github.com/dancxjo/conduit/blob/dev/docs/audits/pure-chain-fusion.md?utm_source=chatgpt.com) demonstrates a dramatic reduction in scheduler selections, but all 1,984 intermediate canonical values in its 32-Gear fixture were still materialized.

These suggest a common direction: retain extremely detailed logical semantics while compiling longer stretches of computation into compact physical execution regions.

That is more promising than simply making each individual Gear a little faster.

## Where the strongest opportunities appear

Priority by expected leverage

Engineering judgment, not measured speedup

| Very high   | Keep prepared computation alive across repeated values and audio frames      |
| ----------- | ---------------------------------------------------------------------------- |
| Very high   | Fuse pure numerical work without materializing every intermediate Cord value |
| High        | Reduce Host boundary allocations, cloning, and canonical decoding            |
| High        | Specialize the native compositor's per-pixel work                            |
| High        | Reuse checked preparation, catalogues, and eligible Plan structures          |
| Conditional | SIMD, multicore, and GPU acceleration for suitably large workloads           |

A particularly concrete issue is already visible in [the hosted allocation audit](https://github.com/dancxjo/conduit/blob/dev/docs/audits/kernel-overhead-attribution.md?utm_source=chatgpt.com): its Flow-output path creates an owned delivery, then clones it for an acknowledging adapter. That accounted for six allocations per additional output, or 378 allocations for 63 outputs, in the measured fixture.

This is a cleaner optimization target than speculative scheduler micro-tuning. The desired change is ownership transfer or a prepared shared delivery, while preserving separate acknowledgement, cancellation, and Sign semantics. It needs a before/after benchmark, but the existing report gives us a precise starting point.

## 1. The highest-value engineering changes

My first recommendation is to target the lifetime of prepared computation, not just the speed of its individual instructions.

1

Make prepared graph execution reusable

Expected leverage: Very high · Primary consumers: FARGAN, DSP, long-lived Bodies

FARGAN currently measures about 2,764 ms of fresh owner preparation and 21.8 ms of Play for the 160-sample frame fixture. These are distinct costs, so amortizing preparation is potentially enormous for repeated frames.

Build a reusable prepared-owner/session profile that retains immutable code, graph bindings, weights, numeric representations, and reservations across consecutive admitted frames. State transitions must still commit only after successful output acceptance.

[Existing direction: FARGAN streaming #5715](https://github.com/dancxjo/conduit/issues/5715?utm_source=chatgpt.com)

2

Compile numerical islands rather than executing each elementary operation as a Gear

Expected leverage: Very high · Primary consumers: AI, signal processing, scientific computation

Keep the 768-node FARGAN graph as authoritative Source, but let the Planner lower eligible subgraphs into compact prepared numeric kernels.

A matrix-vector multiplication, activation, and elementwise addition need not require separate scheduler dispatches or canonical transport between every elementary operation. Internally, they can execute as a bounded numeric region with preallocated scratch and exact source-node attribution.

This extends the fusion and expression work already begun in [#5396](https://github.com/dancxjo/conduit/issues/5396?utm_source=chatgpt.com) and [#5397](https://github.com/dancxjo/conduit/issues/5397?utm_source=chatgpt.com), rather than inventing a new computational ontology.

This is also the natural entrance for SIMD, neural inference providers, and eventually GPUs.

3

Remove unnecessary Host Flow delivery cloning

Expected leverage: High for small frequent outputs · Confidence: High in identified allocation source

The specific code is in [`targets/std/src/installed_std.rs`](https://github.com/dancxjo/conduit/blob/dev/targets/std/src/installed_std.rs?utm_source=chatgpt.com). It constructs a delivery from borrowed bytes, then clones the entire delivery for the adapter.

Try a prepared owned-delivery handoff or a bounded reference-counted shared payload. Pre-intern stable port and Kind identities where semantics permit.

Preserve output acknowledgement as a distinct event from ownership transfer. Compare exact allocations, bytes copied and throughput with the existing reference path.

4

Make ConduitOS graphics substantially cheaper per pixel

Expected leverage: High for rendering · Confidence: High in source pattern, unmeasured wall impact

I found a particularly conspicuous hot path in [`native_compositor/frame_composition.rs`](https://github.com/dancxjo/conduit/blob/dev/targets/conduitos/src/native_compositor/frame_composition.rs?utm_source=chatgpt.com).

For every pixel in a damaged region, it searches visible surfaces to select the highest z-order surface, tests focus and cursor overlays, and calls a checked pixel writer. The raw framebuffer writer emits four volatile byte stores per pixel.

Conduit already has bounded damage rectangles, so this is not a proposal to add dirty rectangles. It is an opportunity to make the existing damage compositor more efficient using clipped spans, precomputed surface order, cached geometry, and a separately proven fast scanout writer.

The current surface update also clears and rerenders its whole retained surface. Incremental raster reuse may help text editing and cursor-heavy interfaces, although revision and failure semantics must be preserved.

Benchmark full redraws, tiny damage, scrolling, hover, and rapid text input before choosing a rewrite.

5

Cache immutable checking and preparation, with exact dependency invalidation

Expected leverage: High for cold starts and interactive editing · Primary consumers: Browser, ConduitOS, Planner

The small one-Gear benchmark spends about 5 ms checking Source before execution. FARGAN's first Source checking takes approximately 147 ms, and its check/expand/Plan phase around 807 ms per repetition in the measured fixture.

The right strategy is not an unsafe global cache. It's content-addressed reuse of immutable checked declarations, expanded subgraphs, Type specializations, accepted laws, and eligible prepared Back fragments.

The existing [incremental planner](https://github.com/dancxjo/conduit/blob/dev/architecture/planner/src/incremental.rs?utm_source=chatgpt.com) already provides finite candidate caching. Extend that discipline through the preparation pipeline, with dependencies identifying exactly what a change invalidates.

Source editing should invalidate the smallest provably affected graph, not automatically every expensive preparation.

## 2. Further opportunities across the architecture

| Subsystem        | Opportunity                                                                                                                | Qualification                                                                                                                           |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Kernel and Signs | Compact internal Sign records, intern immutable identities, defer expensive human-readable reconstruction until inspection | Mandatory events and failure attribution cannot disappear                                                                               |
| Planner          | Cheap greedy fast path plus bounded global optimization when estimated placement benefit exceeds search cost               | Existing [#5230](https://github.com/dancxjo/conduit/issues/5230?utm_source=chatgpt.com) owns global placement                           |
| Replanning       | Hysteresis, exact dependency-based cache invalidation, preservation of resident resources                                  | Existing [#5233](https://github.com/dancxjo/conduit/issues/5233?utm_source=chatgpt.com) owns this                                       |
| Multicore        | Parallelize independent ready regions while preserving deterministic commit and resource ownership                         | Needs real physical overlap evidence, not just multiple lanes in a Plan                                                                 |
| Cross-host Lines | Measure canonical serialization, transport framing, buffering and redundant copies; reuse bounded prepared codecs          | No evidence yet of the dominant cross-host bottleneck                                                                                   |
| Browser/WASM     | Demand-driven modules and smaller runtime footprints, preserving stable semantic interfaces                                | Recent [PR #6003](https://github.com/dancxjo/conduit/pull/6003?utm_source=chatgpt.com) already repaired a major browser-size regression |
| CI/builds        | Narrow dependency invalidation, reuse generated artifacts and avoid broad rebuilds                                         | Much of the infrastructure is already optimized; verify rather than redo                                                                |

The Sign path deserves particular attention in large graphs. The measured FARGAN fixture retains 6,976 Signs per frame. That's important evidence, but the way we store and reconstruct evidence could be substantially cheaper than constructing a fully expanded descriptive object every time.

A compact internal Sign record containing event identity, sequence, and references to Plan-owned metadata could preserve the same logical evidence. We would still need to prove retention limits, ordering, correlation and observability independently.

## 3. What is already working, and shouldn't be rewritten

The fixed scheduler is not the original naive scanner anymore. [#5231](https://github.com/dancxjo/conduit/issues/5231?utm_source=chatgpt.com) is closed, and its retained audits show sparse readiness improvements, fixed-store slot search repairs, Host pending-call indexing, and Sign-capacity improvements.

The 10,000-node, two-ready fixture reduced examined scheduler candidates from 30,000 to four, plus bounded summary inspection. That is meaningful algorithmic progress. Chasing another percentage point in ready selection would be a poor first choice while expression Host Calls and graph preparation dominate FARGAN's measured costs.

Likewise, build-script optimization already produced a substantial improvement: the measured generator's median runtime fell from 124 seconds to about 7 seconds after build dependencies received optimization level 1. [Build audit](https://github.com/dancxjo/conduit/blob/dev/docs/audits/build-dependency-throughput.md?utm_source=chatgpt.com).

The current `dev` head also includes browser metadata compression work. PR #6003 reports an optimized runtime of approximately 14.56 MB, below its 20 MiB limit. This is an excellent example of why performance work should begin with current-source measurements instead of re-fixing historical bottlenecks.

## 4. The ConduitOS responsiveness problem deserves its own measurement

I would introduce stage-to-stage measurements around actual startup and graphical interaction.

ConduitOS already emits boot-stage markers from [`graphical_startup.rs`](https://github.com/dancxjo/conduit/blob/dev/targets/conduitos/src/graphical_startup.rs?utm_source=chatgpt.com). Those give us natural instrumentation points, but a complete, repeatable breakdown from boot entry to first acknowledged interactive frame is still needed.

The metrics I want are first visible paint, first usable input, redraw latency, pointer-to-visual feedback, steady typing latency, and the separate cost of building, sealing, preparing, and running the required Plots.

This will distinguish slow hardware enumeration or initialization from slow semantic preparation and slow rasterization. They demand quite different fixes.

The graphical optimization should also respect the existing Face/Mask architecture. A faster compositor must not invent application state or accept input against an obsolete Show revision.

## 5. A practical delivery sequence

I would organize the next performance push around actual user-visible and compute-visible gains.

1. Measure a stable cross-subsystem baseline

   One short arithmetic Plot, one repeated Quantity computation, one FARGAN trajectory, one native graphical journey, and one multi-Host workload. Record cold/warm latency, allocations, bytes materialized, scheduler work and required Signs.
2. Eliminate concrete copying and preparation waste

   Fix the hosted Flow cloning path. Reuse prepared owners across eligible repeated invocations. Preserve a reference Back for exact behavioral comparison.
3. Generalize compact numerical regions

   Start with a pure fixed-width computation, then a real FARGAN numerical segment. Prove that internal canonical values are genuinely avoided, not merely passed through a faster scheduler.
4. Optimize the native graphical path

   Measure the compositor, then introduce a proved span-based realization and improved retained-raster reuse. Compare actual QEMU frames and responsiveness.
5. Add parallel hardware Backs where throughput justifies them

   CPU SIMD first, then multicore regions and a GPU candidate when resource reservations, transfer costs, numerical semantics and source-level attribution are all supported.

## My larger architectural conclusion

Conduit has unusually strict source, execution, and evidence boundaries. Their purpose is to make different realizations interchangeable without lying about what happened.

That strictness can become an optimization advantage.

A conventional execution system sometimes has to rediscover dynamically that a quantity has a particular type, a resource is authorized, an operation is pure, an expression has bounded range, or a transformation can be reassociated.

Conduit can establish many such facts beforehand. Its difficulty is that some execution paths still repeatedly perform expensive work whose invariant parts were already established.

I would give the compiler and Planner a more ambitious physical-lowering job:

Take the largest eligible region of checked, meaningful work, prove its implementation contract, and prepare the cheapest bounded realization the current Host can genuinely provide.

The authored graph stays intact. The logical observations stay intact. The physical execution becomes smaller.

One architectural distinction is essential: a fast region is not permission to erase a boundary that matters. If a debugger, authority transition, pressure policy, temporal interaction, or external effect needs a boundary, that boundary survives or prevents fusion.

This is how Conduit can get the advantages of compilation, batching, vectorization, resource locality, and accelerator execution without degenerating into a collection of opaque machine-specific shortcuts.

If I were choosing one decisive experiment, it would be the full FARGAN frame: keep its actual Source, semantic contracts and oracle outputs unchanged; reuse preparation across frames; lower an eligible numerical subgraph into one prepared physical region; then compare cold time, warm frame time, allocations, memory traffic, and every required Sign against the existing implementation.

That single experiment would test Conduit's central performance proposition more forcefully than another round of scheduler micro-optimizations.