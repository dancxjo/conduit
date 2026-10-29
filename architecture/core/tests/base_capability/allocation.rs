//! Measure mandatory lease work after capability issuance, including rotation.
use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static FAIL_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
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
        if FAIL_AFTER
            .try_with(|n| match n.get() {
                Some(0) => true,
                Some(left) => {
                    n.set(Some(left - 1));
                    false
                }
                None => false,
            })
            .unwrap_or(false)
        {
            return std::ptr::null_mut();
        }
        record();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if FAIL_AFTER
            .try_with(|n| match n.get() {
                Some(0) => true,
                Some(left) => {
                    n.set(Some(left - 1));
                    false
                }
                None => false,
            })
            .unwrap_or(false)
        {
            return std::ptr::null_mut();
        }
        record();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if FAIL_AFTER
            .try_with(|n| match n.get() {
                Some(0) => true,
                Some(left) => {
                    n.set(Some(left - 1));
                    false
                }
                None => false,
            })
            .unwrap_or(false)
        {
            return std::ptr::null_mut();
        }
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

#[test]
fn allocation_refusal_preserves_issuer_and_original_issuance_identity() {
    let args = (
        HostId::from("host"),
        BootId::from("boot"),
        BaseInstanceId::from("base"),
    );
    FAIL_AFTER.with(|n| n.set(Some(0)));
    let result = BaseCapabilityTable::new(args.0, args.1, args.2, 1, [7; 32], 1);
    FAIL_AFTER.with(|n| n.set(None));
    assert_eq!(result.err(), Some(BaseCapabilityRefusal::Capacity));
    for failure in [0, 1] {
        let mut actual = table(7);
        let mut expected_table = table(7);
        let expected = expected_table.issue(request()).unwrap();
        let input = request();
        FAIL_AFTER.with(|n| n.set(Some(failure)));
        let result = actual.issue(input);
        FAIL_AFTER.with(|n| n.set(None));
        assert_eq!(result.err(), Some(BaseCapabilityRefusal::Capacity));
        assert_eq!(actual.inspections().count(), 0);
        assert!(actual.issue(request()).unwrap() == expected);
        assert_eq!(
            actual.inspections().collect::<Vec<_>>(),
            expected_table.inspections().collect::<Vec<_>>()
        );
    }
}
#[test]
fn invalid_issuance_is_checked_before_allocating_and_does_not_consume_identity() {
    let mut actual = table(7);
    let mut input = request();
    input.scope.maximum_in_flight = 0;
    FAIL_AFTER.with(|n| n.set(Some(0)));
    let result = actual.issue(input);
    FAIL_AFTER.with(|n| n.set(None));
    assert_eq!(result.err(), Some(BaseCapabilityRefusal::InvalidBound));
    let expected = table(7).issue(request()).unwrap();
    assert!(actual.issue(request()).unwrap() == expected);
}
