# Reuse preparation membership across exact source routes

Development slice for #5231 / #5395 cold preparation. Route validation previously
initialized a CORDS-flag array for every installed source-port route. With 256
cords and 256 one-target routes, the regression observed 65,536 initialized flags.

Every valid cord has exactly one local source node/port, which is checked against
every target. A repeated cord in another source route is therefore invalid too;
it never needs a fresh duplicate-detection domain. Validation now lazily creates
one preparation membership bitmap and reuses it across all routes. Graphs with
no routed targets initialize none. The ordinary active-prefix, source, sink and
duplicate checks still refuse InvalidPlan at the same validation boundary.

The scratch bitmap remains bounded by CORDS and exists only during preparation;
its optional wrapper supports lazy initialization. No heap allocation, scheduler
field or Play storage/work is added. This reduces bitmap initialization from
O(installed routes * CORDS) to at most O(CORDS), plus target visits. Other route
and topology validation costs remain; no universal linear preparation or exact
native/physical stack-footprint claim is made.

The fixed preparation regression fails before repair at 65,536 rather than 256
initialized flags and passes afterward. Additional tests cover zero scratch
initialization for ingress-only topology, identical initialization bounds through
the boxed entrance, and continued rejection of duplicate targets within one route
or targets repeated under the wrong source. Initialization counters are test-only
and thread-local; they are absent from product builds. This is deterministic
preparation work evidence, not a measured wall-clock speedup.

Local validation: three fixed-only membership tests and all four alloc membership
tests passed. Alloc and linear-ready-reference kernel suites each passed 207 tests;
formatting, strict alloc all-target Clippy, and diff whitespace passed. Those suites
include the production 10,000-node sparse run. Parent acceptance and optimized
cold-path measurements remain open. No CI was dispatched or awaited.
