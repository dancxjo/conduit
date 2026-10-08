extern crate alloc;
use bounded_runtime_table::*;
use conduitos::bounded_runtime_table;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
#[derive(Clone, Copy)]
struct Counts {
    enabled: bool,
    requested: usize,
    live: isize,
    peak: isize,
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
        update(|c| c.live -= l.size() as isize);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        charge(n);
        update(|c| c.live -= l.size() as isize);
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOC: Alloc = Alloc;
fn charge(n: usize) {
    update(|c| {
        c.calls += 1;
        c.requested += n;
        c.live += n as isize;
        c.peak = c.peak.max(c.live);
    });
}
fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, isize, usize) {
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
fn table_array_is_admitted_before_allocation_and_never_grows() {
    let (r, _, _, calls) =
        measure(|| BoundedRuntimeTable::<u16, Arc<Vec<u8>>>::storage_reservation(3).unwrap());
    assert_eq!(calls, 0);
    let ((mut table, receipt), requested, peak, _) = measure(|| {
        BoundedRuntimeTable::<u16, Arc<Vec<u8>>>::with_storage_limits(
            3,
            r.preparation_requested_bytes_bound,
            r.retained_array_bytes_bound,
        )
        .unwrap()
    });
    assert_eq!(r, receipt);
    assert_eq!(requested, r.retained_array_bytes_bound);
    assert_eq!(peak, requested as isize);
    assert_eq!(table.array_capacity_bytes().unwrap(), requested);
    for (prep, retained) in [
        (
            r.preparation_requested_bytes_bound - 1,
            r.retained_array_bytes_bound,
        ),
        (
            r.preparation_requested_bytes_bound,
            r.retained_array_bytes_bound - 1,
        ),
    ] {
        let (result, requested, _, calls) = measure(|| {
            BoundedRuntimeTable::<u16, Arc<Vec<u8>>>::with_storage_limits(3, prep, retained)
        });
        assert!(matches!(result, Err(RuntimeTableRefusal::Capacity)));
        assert_eq!((requested, calls), (0, 0));
    }
    let owners = [
        Arc::new(vec![1]),
        Arc::new(vec![2]),
        Arc::new(vec![3]),
        Arc::new(vec![4]),
    ];
    let refs = owners.each_ref().map(Arc::clone);
    let (_, requested, _, calls) = measure(|| {
        assert!(table.is_empty());
        table.try_insert(2, Arc::clone(&refs[2])).unwrap();
        table.try_insert(0, Arc::clone(&refs[0])).unwrap();
        table.try_insert(1, Arc::clone(&refs[1])).unwrap();
        assert_eq!(table.len(), 3);
        assert!(table.iter().map(|(k, _)| *k).eq([0, 1, 2]));
        let (error, key, value) = table.try_insert(1, Arc::clone(&refs[3])).unwrap_err();
        assert_eq!(error, RuntimeTableRefusal::Duplicate);
        assert_eq!(key, 1);
        assert!(Arc::ptr_eq(&value, &refs[3]));
        assert!(Arc::ptr_eq(table.get(&1).unwrap(), &refs[1]));
        let (error, key, value) = table.try_insert(3, Arc::clone(&refs[3])).unwrap_err();
        assert_eq!(error, RuntimeTableRefusal::Capacity);
        assert_eq!(key, 3);
        assert!(Arc::ptr_eq(&value, &refs[3]));
        let removed = table.remove(&1).unwrap();
        assert!(Arc::ptr_eq(&removed, &refs[1]));
        assert!(!table.contains_key(&1));
        table.try_insert(3, value).unwrap();
        assert!(Arc::ptr_eq(table.get_mut(&3).unwrap(), &refs[3]));
        assert_eq!(
            table.array_capacity_bytes().unwrap(),
            r.retained_array_bytes_bound
        );
    });
    assert_eq!((requested, calls), (0, 0));
}

#[test]
fn borrowed_keys_zero_capacity_and_overflow_refuse_exactly() {
    let (mut empty, _) = BoundedRuntimeTable::<String, u64>::with_storage_limits(0, 0, 0).unwrap();
    let key = String::from("original");
    let pointer = key.as_ptr();
    let ((refusal, key, value), requested, _, calls) =
        measure(|| empty.try_insert(key, 17).unwrap_err());
    assert_eq!(refusal, RuntimeTableRefusal::Capacity);
    assert_eq!((key.as_ptr(), value), (pointer, 17));
    assert_eq!((requested, calls), (0, 0));
    let (refusal, requested, _, calls) =
        measure(|| BoundedRuntimeTable::<u64, u64>::storage_reservation(usize::MAX));
    assert_eq!(refusal, Err(RuntimeTableRefusal::Capacity));
    assert_eq!((requested, calls), (0, 0));
    let r = BoundedRuntimeTable::<String, u64>::storage_reservation(2).unwrap();
    let (mut table, _) = BoundedRuntimeTable::with_storage_limits(
        2,
        r.preparation_requested_bytes_bound,
        r.retained_array_bytes_bound,
    )
    .unwrap();
    table.try_insert(String::from("b"), 2u64).unwrap();
    table.try_insert(String::from("a"), 1u64).unwrap();
    let (_, requested, _, calls) = measure(|| {
        assert_eq!(table.get("a"), Some(&1));
        *table.get_mut("b").unwrap() = 3;
        assert_eq!(table.remove("b"), Some(3));
        assert!(!table.contains_key("b"));
    });
    assert_eq!((requested, calls), (0, 0));
}
