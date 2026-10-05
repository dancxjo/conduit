//! Measure mandatory lease work after capability issuance, including rotation.
use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountedAllocator;
#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

fn record() {
    if TRACKING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[test]
fn admitted_lease_work_and_rotation_allocate_nothing_under_pressure() {
    for in_flight in [1, 2, 4] {
        let mut table = table(7);
        let mut request = request();
        request.scope.maximum_in_flight = in_flight;
        request.scope.maximum_operations = 64;
        request.authority.maximum_operations = 64;
        let mut handle = table.issue(request).unwrap();
        let claim = claim();
        let mut leases = std::array::from_fn::<_, 4, _>(|_| None);
        ALLOCATIONS.with(|count| count.set(0));
        TRACKING.with(|tracking| tracking.set(true));
        for _ in 0..64 / in_flight {
            for lease in leases.iter_mut().take(usize::from(in_flight)) {
                *lease = Some(table.authorize(&handle, &claim).unwrap());
            }
            assert_eq!(
                table.authorize(&handle, &claim),
                Err(BaseCapabilityRefusal::InFlightFull)
            );
            for lease in leases.iter_mut().take(usize::from(in_flight)) {
                table
                    .complete(&mut handle, lease.take().unwrap(), 16)
                    .unwrap();
            }
        }
        assert_eq!(
            table.authorize(&handle, &claim),
            Err(BaseCapabilityRefusal::Exhausted)
        );
        TRACKING.with(|tracking| tracking.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}
