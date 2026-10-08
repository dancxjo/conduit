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
fn types() -> Vec<StructuredInfoType> {
    let mut kind = String::with_capacity(96);
    kind.push_str("value/u64");
    let leaf = StructuredInfoType::leaf(KindId::new(kind)).unwrap();
    let nominal = StructuredInfoType::nominal(KindId::new("test/nominal"), leaf.clone()).unwrap();
    let collection = StructuredInfoType::collection(nominal.clone(), Some(3)).unwrap();
    let sequence = StructuredInfoType::sequence(nominal.clone(), 3).unwrap();
    let mut fields = Vec::with_capacity(11);
    fields.push(StructuredFieldType::new("number", nominal.clone()).unwrap());
    let record = StructuredInfoType::record(KindId::new("test/record"), fields).unwrap();
    let mut cases = Vec::with_capacity(13);
    cases.push(StructuredVariantCase::new("number", nominal.clone()).unwrap());
    let variant = StructuredInfoType::variant(KindId::new("test/variant"), cases).unwrap();
    vec![leaf, nominal, collection, sequence, record, variant]
}
#[test]
fn composer_reserves_all_requests_and_refuses_one_under_without_allocation() {
    for ty in types() {
        if !matches!(
            ty.shape(),
            StructuredInfoTypeShape::Leaf(_)
                | StructuredInfoTypeShape::Record { .. }
                | StructuredInfoTypeShape::Variant { .. }
        ) {
            continue;
        }
        let (reservation, _, _, calls) =
            measure(|| PreparedStructuredComposer::storage_reservation(&ty, 4096).unwrap());
        assert_eq!(calls, 0);
        let ((composer, receipt), requests, peak, _) = measure(|| {
            PreparedStructuredComposer::new_with_storage_limits(
                &ty,
                4096,
                reservation.preparation_requested_bytes_bound,
                reservation.retained_heap_bytes_bound,
            )
            .unwrap()
        });
        assert_eq!(reservation, receipt);
        assert_eq!(requests, composer.owned_heap_bytes());
        assert!(requests <= receipt.preparation_requested_bytes_bound);
        assert!(peak <= receipt.preparation_requested_bytes_bound);
        assert!(composer.owned_heap_bytes() <= receipt.retained_heap_bytes_bound);
        println!(
            "composer {:?}: actual/request/retained bound {}/{}/{}",
            ty.shape(),
            requests,
            receipt.preparation_requested_bytes_bound,
            receipt.retained_heap_bytes_bound
        );
        for (prep, retained) in [
            (
                receipt.preparation_requested_bytes_bound - 1,
                receipt.retained_heap_bytes_bound,
            ),
            (
                receipt.preparation_requested_bytes_bound,
                receipt.retained_heap_bytes_bound - 1,
            ),
        ] {
            let (result, requested, _, calls) = measure(|| {
                PreparedStructuredComposer::new_with_storage_limits(&ty, 4096, prep, retained)
            });
            assert!(matches!(
                result,
                Err(PreparedStructuredCompositionStorageRefusal::Capacity)
            ));
            assert_eq!((requested, calls), (0, 0));
        }
    }
}
#[test]
fn typed_pair_reserves_transients_preserves_exact_types_and_refuses_one_under() {
    let all = types();
    for left in &all {
        for right in &all {
            let (reservation, _, _, calls) = measure(|| {
                PreparedTypedTuplePairEncoder::storage_reservation(left, 4096, right, 4096).unwrap()
            });
            assert_eq!(calls, 0);
            let ((encoder, receipt), requests, peak, _) = measure(|| {
                PreparedTypedTuplePairEncoder::new_with_storage_limits(
                    left,
                    4096,
                    right,
                    4096,
                    reservation.preparation_requested_bytes_bound,
                    reservation.retained_heap_bytes_bound,
                )
                .unwrap()
            });
            assert_eq!(receipt, reservation);
            assert!(
                requests <= receipt.preparation_requested_bytes_bound,
                "actual {requests} exceeds {}",
                receipt.preparation_requested_bytes_bound
            );
            assert!(peak <= receipt.preparation_requested_bytes_bound);
            assert!(encoder.owned_heap_bytes() <= receipt.retained_heap_bytes_bound);
            let ordinary =
                PreparedTypedTuplePairEncoder::new(left.clone(), 4096, right.clone(), 4096)
                    .unwrap();
            assert_eq!(encoder.value_type(), ordinary.value_type());
            assert_eq!(encoder.maximum_bytes(), ordinary.maximum_bytes());
            assert_eq!(
                encoder.value_type().canonical_bytes().unwrap(),
                ordinary.value_type().canonical_bytes().unwrap()
            );
            for (prep, retained) in [
                (
                    receipt.preparation_requested_bytes_bound - 1,
                    receipt.retained_heap_bytes_bound,
                ),
                (
                    receipt.preparation_requested_bytes_bound,
                    receipt.retained_heap_bytes_bound - 1,
                ),
            ] {
                let (result, requested, _, calls) = measure(|| {
                    PreparedTypedTuplePairEncoder::new_with_storage_limits(
                        left, 4096, right, 4096, prep, retained,
                    )
                });
                assert!(matches!(
                    result,
                    Err(PreparedStructuredCompositionStorageRefusal::Capacity)
                ));
                assert_eq!((requested, calls), (0, 0));
            }
        }
    }
}
#[test]
fn bounded_pair_encode_is_allocation_free_and_keeps_original_refusals() {
    let leaf = StructuredInfoType::leaf(KindId::new("value/u64")).unwrap();
    let nominal = StructuredInfoType::nominal(KindId::new("test/nominal"), leaf.clone()).unwrap();
    let value = StructuredInfoValue::nominal(
        nominal.clone(),
        StructuredInfoValue::leaf(leaf.clone(), 7u64.to_le_bytes().to_vec()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    for left in [&leaf, &nominal] {
        for right in [&leaf, &nominal] {
            let r = PreparedTypedTuplePairEncoder::storage_reservation(left, 4096, right, 4096)
                .unwrap();
            let (mut bounded, _) = PreparedTypedTuplePairEncoder::new_with_storage_limits(
                left,
                4096,
                right,
                4096,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            )
            .unwrap();
            let mut ordinary =
                PreparedTypedTuplePairEncoder::new(left.clone(), 4096, right.clone(), 4096)
                    .unwrap();
            let raw = 7u64.to_le_bytes();
            let a = if matches!(left.shape(), StructuredInfoTypeShape::Leaf(_)) {
                &raw[..]
            } else {
                &value
            };
            let b = if matches!(right.shape(), StructuredInfoTypeShape::Leaf(_)) {
                &raw[..]
            } else {
                &value
            };
            let (result, requested, _, calls) = measure(|| bounded.encode(a, b).map(|_| ()));
            assert_eq!(result, Ok(()));
            assert_eq!((requested, calls), (0, 0));
            assert_eq!(bounded.encoded(), ordinary.encode(a, b).unwrap());
            let previous = bounded.encoded().to_vec();
            assert_eq!(bounded.encode(&[1], b), ordinary.encode(&[1], b));
            assert_eq!(bounded.encoded(), previous);
        }
    }
}
