# Complete fixed scheduler root preparation

`FixedScheduler::boxed_storage_reservation` returns the exact concrete `Self`
size, including every fixed-capacity topology, queue, route, host/pending, driver,
value-storage and sign-storage inline field. `new_boxed_with_storage_limits`
checks both root request/retained ceilings before invoking the original boxed
constructor, preserving every topology/active-capacity guard and execution path.
No scheduler capacity or Kernel concept is added or changed.

Nested driver Boxes/payloads, dynamic value/sign buffers, caller argument owners,
preparation of those owners, allocator bookkeeping and stack remain separate.
The root receipt must not cause inline embedded storage roots to be charged twice
in the aggregate. This component does not measure the final FARGAN scheduler
instantiation or establish whole working/stack memory admission.

A fresh Kernel-only alloc graph (separate from AI graphs) passes the small actual
scheduler test and library/test Clippy. All fixed root fields request/retain1480B,
exactly the concrete Type size. Both one-under ceilings and original invalid
active-capacity guard refuse with zero allocations. The admitted scheduler drains
through the original execution kernel with zero step allocations. No full model
execution, public target, or #5218 acceptance claim follows from this test.
