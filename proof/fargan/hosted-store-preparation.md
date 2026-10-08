# Fixed hosted store preparation inventory

The existing hosted store now exposes allocation-free reservation, construction
with preparation/retained ceilings, and actual owned requested-capacity inventory.
It preserves the original budget predicate and store/release/generation behavior.
Every slot's full maximum byte buffer is charged, independently of the smaller
logical active-byte limit. Slot metadata array storage is charged separately.
The root value, allocator bookkeeping and stack remain outside this receipt.

The unchanged 1024-slot × 16384-byte profile requests 16,777,216 payload bytes
plus 32,768 slot-array bytes: 16,809,984 total. Actual allocation requests, peak
and retained capacities equal that reservation. Both one-under ceilings refuse
without allocation. A small profile proves logical-byte pressure, stale handles,
retain/release and reuse parity with the existing constructor without allocation
or capacity growth. All 76 Kernel unit tests plus two allocator tests pass;
Kernel alloc all-target Clippy passes. No scheduler/capacity/transport semantics
change. Evidence: project outputs/fargan-hosted-store-preparation.
