# Preserve first-free Host slots with a bounded search hint

Development slice for #5231. Pending Host requests previously searched from
slot zero for every allocation. Filling 64 initially empty slots therefore
inspected 2,080 entries. A fixed-storage production scheduler regression now
observes 64 inspections for the same 64 requests.

One `usize` search hint records a lower bound on the first free slot. Every
entry before the hint is occupied. Allocation searches from the hint, inserts
in the same first-free slot as before, and moves the hint immediately past it.
Consuming a completion lowers the hint to the freed slot if earlier. Cancellation
resets it; fixed and boxed preparation initialize it to zero. No allocation,
extra slot table, runtime growth, or dispatch-policy change is introduced.

The hint may itself point at an occupied slot, and reusing holes can still
require linear search. It is not a constant-time free list. Initial table filling
is linear rather than quadratic in inspected slots. Memory increases by one
usize field per scheduler (8 bytes of field storage on this native host; total
struct size can also depend on alignment). Scan instrumentation is test-only.

The regression admits 64 nodes, fills all 64 pending slots through real Steps,
and checks exact node-to-slot identity and request dispatch order. It completes
nodes 33 and 5 out of order, then consumes each completion and requests a new
call in the same Step. Replacement still chooses the first free slot and keeps
the pending table full with the exact replacement request identities. Cancellation
then drains pending ownership. Before repair the initial-fill work assertion
failed at 2,080 rather than 64 probes; afterward the full scenario passes.

This is deterministic native work evidence, not wall-clock performance or the
parent issue's full Plan/Play acceptance. The reference-ready profile and Host
cancellation/completion suites remain part of local verification.

Local validation: the focused fixed-only regression passed; alloc and
linear-ready-reference kernel suites each passed 198 tests. Formatting, strict
alloc all-target kernel Clippy, and diff whitespace checks passed. No CI was
dispatched or awaited.
