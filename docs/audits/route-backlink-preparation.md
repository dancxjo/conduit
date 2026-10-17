# Prove cord backlinks from validated route membership

Development slice for #5231 / #5395 preparation cost. After checking every routed
target's exact source and sink, validation previously walked that same route
again for each local-source cord to prove its backlink. A 256-way fan-out made
256 forward visits plus 32,896 reverse visits: 33,152 target visits in total.

The shared preparation bitmap already proves which cords appeared in the first
pass. A marked cord has been checked against its exact source node/port and sink;
its identity and resource bounds are still checked in the cord pass. That pass
now uses membership instead of repeating the target walk. This removes the
quadratic backlink work from fan-out without adding heap allocation, scheduler
state, or Play work. Other preparation checks and their costs remain.

When membership is absent, a source-route lookup still preserves the original
error distinction. An uninstalled route returns Routing(RouteMissing). An
installed empty route or a route omitting a declared cord returns InvalidPlan.
The lookup need not walk targets because the completed first pass already proves
absence. Source/sink mismatches, duplicate targets and inactive references still
refuse through their existing checks.

The real fixed-scheduler 256-way fan-out regression failed before repair at
33,152 target visits and now observes 256. A preparation-only iterator seam counts
actual yielded targets through both passes; its counter is thread-local and
absent from product builds. Negative vectors explicitly cover missing, empty
and incomplete routes. Existing fixed/boxed membership and duplicate/source
vectors remain. This is deterministic validation-work evidence, not an optimized
wall-clock or physical-memory claim.

Local validation: five focused fixed-only membership tests passed; alloc and
linear-ready-reference kernel suites each passed 209 tests. Formatting, strict
alloc all-target Clippy, and diff whitespace passed. Parent acceptance remains
open. No CI was dispatched or awaited.
