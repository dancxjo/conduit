use conduit_core::*;
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
fn exact_record_recipes_preflight_and_preserve_every_field() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    let nominal = StructuredInfoType::nominal(kind_id("test/number"), leaf.clone()).unwrap();
    let sequence = StructuredInfoType::sequence(nominal.clone(), 4).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("test/record"),
        vec![
            StructuredFieldType::new("first", sequence.clone()).unwrap(),
            StructuredFieldType::new("last", leaf.clone()).unwrap(),
        ],
    )
    .unwrap();
    let encoded_type = ty.canonical_bytes().unwrap();
    let (bound, _, _, calls) =
        measure(|| PreparedCanonicalRecordAccess::storage_bound(&encoded_type).unwrap());
    assert_eq!(calls, 0);
    let (refused, _, _, calls) =
        measure(|| PreparedCanonicalRecordAccess::prepare(&encoded_type, bound - 1));
    assert!(refused.is_err());
    assert_eq!(calls, 0);
    let (recipe, requested, peak, calls) =
        measure(|| PreparedCanonicalRecordAccess::prepare(&encoded_type, bound).unwrap());
    assert_eq!(requested, bound);
    assert_eq!(peak, bound);
    assert_eq!(calls, 1);
    for count in [0, 1, 4] {
        let value = StructuredInfoValue::record(
            ty.clone(),
            vec![
                StructuredFieldValue::new(
                    "first",
                    StructuredInfoValue::sequence(
                        sequence.clone(),
                        (0..count)
                            .map(|i| {
                                StructuredInfoValue::nominal(
                                    nominal.clone(),
                                    StructuredInfoValue::leaf(
                                        leaf.clone(),
                                        (i as u16).to_le_bytes().to_vec(),
                                    )
                                    .unwrap(),
                                )
                                .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "last",
                    StructuredInfoValue::leaf(leaf.clone(), 9u16.to_le_bytes().to_vec()).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let view = validate_canonical_structured_value(&value).unwrap();
        for name in ["first", "last", "missing"] {
            let (actual, _, _, calls) = measure(|| recipe.field(view, name).unwrap());
            assert_eq!(calls, 0);
            assert_eq!(actual, view.record_field(name).unwrap());
        }
    }
    let foreign = StructuredInfoValue::leaf(leaf, 1u16.to_le_bytes().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let foreign = validate_canonical_structured_value(&foreign).unwrap();
    let (refused, _, _, calls) = measure(|| recipe.field(foreign, "last"));
    assert_eq!(refused, Err(StructuredInfoRefusal::WrongType));
    assert_eq!(calls, 0);
}
