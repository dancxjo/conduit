#![cfg(feature = "alloc")]
use conduit_kernel::*;
use std::alloc::{GlobalAlloc, Layout, System};
#[derive(Clone, Copy)]
struct Counts {
    enabled: bool,
    requested: usize,
    live: usize,
    peak: usize,
    calls: usize,
}
std::thread_local! {static COUNTS:std::cell::Cell<Counts>=const{std::cell::Cell::new(Counts{enabled:false,requested:0,live:0,peak:0,calls:0})};}
fn update(f: impl FnOnce(&mut Counts)) {
    let _ = COUNTS.try_with(|cell| {
        let mut c = cell.get();
        if c.enabled {
            f(&mut c);
            cell.set(c);
        }
    });
}
struct Alloc;
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        charge(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        charge(l.size());
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        update(|c| c.live -= l.size());
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        charge(n);
        update(|c| c.live -= l.size());
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOC: Alloc = Alloc;
fn charge(n: usize) {
    update(|c| {
        c.calls += 1;
        c.requested += n;
        c.live += n;
        c.peak = c.peak.max(c.live);
    });
}
fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize, usize) {
    COUNTS.with(|c| {
        c.set(Counts {
            enabled: true,
            requested: 0,
            live: 0,
            peak: 0,
            calls: 0,
        })
    });
    let value = f();
    let counts = COUNTS.with(|c| {
        let mut v = c.get();
        v.enabled = false;
        c.set(v);
        v
    });
    (value, counts.requested, counts.peak, counts.calls)
}
#[test]
fn store_preparation_counts_all_slots_and_refuses_one_under_without_allocation() {
    for (items, maximum, logical) in [(4, 8, 8), (1024, 16384, 1024 * 16384)] {
        let (r, _, _, calls) =
            measure(|| HostedValueStore::storage_reservation(items, maximum, logical).unwrap());
        assert_eq!(calls, 0);
        let ((store, receipt), requested, peak, _) = measure(|| {
            HostedValueStore::new_with_storage_limits(
                items,
                maximum,
                logical,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            )
            .unwrap()
        });
        assert_eq!(r, receipt);
        assert_eq!(requested, r.preparation_requested_bytes_bound);
        assert_eq!(peak, requested);
        assert_eq!(store.owned_heap_bytes().unwrap(), requested);
        assert_eq!(
            r.slot_payload_requested_bytes,
            items as usize * maximum as usize
        );
        assert_eq!(
            r.retained_heap_bytes_bound,
            r.slot_array_requested_bytes + r.slot_payload_requested_bytes
        );
        assert!(r.retained_heap_bytes_bound > logical as usize);
        println!(
            "hostedstore {items}x{maximum}, logical{logical}: slotarray {}, payload {}, total {}",
            r.slot_array_requested_bytes, r.slot_payload_requested_bytes, requested
        );
        for (prep, retained) in [
            (
                r.preparation_requested_bytes_bound - 1,
                r.retained_heap_bytes_bound,
            ),
            (
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound - 1,
            ),
        ] {
            let (result, requested, _, calls) = measure(|| {
                HostedValueStore::new_with_storage_limits(items, maximum, logical, prep, retained)
            });
            assert!(matches!(
                result,
                Err(HostedValueStorePreparationRefusal::Capacity)
            ));
            assert_eq!((requested, calls), (0, 0));
        }
    }
}
#[test]
fn store_budget_guard_and_pressure_release_parity_preserve_fixed_capacities() {
    for (items, maximum, logical) in [
        (0, 8, 8),
        (1, 0, 8),
        (1, 8, 0),
        (1, 8, 9),
        (u16::MAX, u32::MAX, 1),
    ] {
        assert!(matches!(
            HostedValueStore::new(items, maximum, logical),
            Err(StorageError::InvalidBudget)
        ));
        let (result, requested, _, calls) =
            measure(|| HostedValueStore::storage_reservation(items, maximum, logical));
        assert_eq!(
            result,
            Err(HostedValueStorePreparationRefusal::Storage(
                StorageError::InvalidBudget
            ))
        );
        assert_eq!((requested, calls), (0, 0));
    }
    let r = HostedValueStore::storage_reservation(4, 8, 8).unwrap();
    let (mut bounded, _) = HostedValueStore::new_with_storage_limits(
        4,
        8,
        8,
        r.preparation_requested_bytes_bound,
        r.retained_heap_bytes_bound,
    )
    .unwrap();
    let mut ordinary = HostedValueStore::new(4, 8, 8).unwrap();
    let heap = bounded.owned_heap_bytes().unwrap();
    let (_, requested, _, calls) = measure(|| {
        let a = bounded.store(&[7; 8]).unwrap();
        let b = ordinary.store(&[7; 8]).unwrap();
        assert_eq!(a, b);
        assert_eq!(bounded.store(&[1]), ordinary.store(&[1]));
        assert_eq!(bounded.store(&[0; 9]), ordinary.store(&[0; 9]));
        bounded.retain(a).unwrap();
        ordinary.retain(b).unwrap();
        bounded.release(a).unwrap();
        ordinary.release(b).unwrap();
        assert_eq!(bounded.reference_count(a), ordinary.reference_count(b));
        bounded.release(a).unwrap();
        ordinary.release(b).unwrap();
        assert_eq!(bounded.get(a), ordinary.get(b));
        let a2 = bounded.store(&[4]).unwrap();
        let b2 = ordinary.store(&[4]).unwrap();
        assert_eq!(a2, b2);
        assert_ne!(a, a2);
        bounded.clear();
        ordinary.clear();
        assert_eq!(bounded.used_items(), 0);
        assert_eq!(bounded.used_bytes(), 0);
        assert_eq!(bounded.owned_heap_bytes().unwrap(), heap);
    });
    assert_eq!((requested, calls), (0, 0));
}
