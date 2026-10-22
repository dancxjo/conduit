//! Observe only already-prepared session operations, not receipt construction
//! or terminal destruction. The allocator is local to this integration binary.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

pub fn without_heap_activity(work: impl FnOnce()) {
    let (_, observation) = allocation_probe::observe(work);
    assert_eq!(observation.allocations, 0, "session operation allocated");
    assert_eq!(
        observation.reallocations, 0,
        "session operation grew storage"
    );
    assert_eq!(
        observation.deallocations, 0,
        "session operation destroyed retained data"
    );
}
