# Sealed local pressure policy reaches production kernel Steps

Development proof slice of #5231, 17 October 2026.

The local coalescing fix is now exercised across a sealed Plan and the public
whole-Plan-aware lowering entrance, rather than only hand-built CordSpec tests.
The integration fixture derives two placements from the existing sealed-state
fixture, removes state, connects the exact byte ports locally, seals/verifies the
Plan, then calls lower_plan_fragment_from_plan. It verifies there are no state,
Host Call or activation obligations omitted by this fixture.

For PreserveOrder and CoalesceLatest, the test installs the returned node specs,
cord spec and exact routes directly into FixedScheduler, with fixture Backs.
Both use a two-item/two-byte queue. PreserveOrder commits only the first two
emissions; CoalesceLatest commits the third newest replacement. Queue usage
stays exactly two items/two bytes and value occupancy stays two items. Routing
Signs are exactly two versus three. Global cancellation releases all values.
The separate kernel regression retains exact oldest/newest byte and reference
count coverage for this same production implementation.

The fixture reserves three one-byte value slots (two queued values plus bounded
generated-output staging) and a 64-entry kernel Sign sink. These are explicit
native test storage bounds, not a claim that every hosted storage profile has
been independently admitted or that the Plan's rich mandatory-Sign byte budget
is interchangeable with KernelEvent storage. Authored source checking, real
semantic Backs, host installation, browser and stable publication are not proved.
No production code changes in this slice.

Validation: the complete plan-lowering suite locally with the normal kernel and
again with conduit-kernel/linear-ready-reference-test; strict all-target
plan-lowering Clippy; formatting and diff checks. No CI dispatched or awaited;
parent acceptance remains open.
