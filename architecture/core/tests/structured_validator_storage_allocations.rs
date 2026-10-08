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
fn validator_storage_allocation_bound_and_early_refusal() {
    let leaf = StructuredInfoType::leaf(KindId::new("value/u64")).unwrap();
    let nominal = StructuredInfoType::nominal(KindId::new("test/number"), leaf.clone()).unwrap();
    let collection = StructuredInfoType::collection(nominal.clone(), Some(4)).unwrap();
    let sequence = StructuredInfoType::sequence(collection.clone(), 3).unwrap();
    let mut fields = Vec::with_capacity(19);
    fields.push(StructuredFieldType::new("numbers", sequence.clone()).unwrap());
    let record = StructuredInfoType::record(KindId::new("test/record"), fields).unwrap();
    let mut cases = Vec::with_capacity(11);
    cases.push(StructuredVariantCase::new("present", record.clone()).unwrap());
    let variant = StructuredInfoType::variant(KindId::new("test/variant"), cases).unwrap();
    for (i, ty) in [leaf, nominal, collection, sequence, record, variant]
        .iter()
        .enumerate()
    {
        let (reserve, _, _, calls) =
            measure(|| PreparedStructuredValueValidator::storage_reservation(ty, 262144).unwrap());
        assert_eq!(calls, 0);
        let ((validator, receipt), requests, peak, _) = measure(|| {
            PreparedStructuredValueValidator::new_with_storage_limits(
                ty,
                262144,
                reserve.preparation_requested_bytes_bound,
                reserve.retained_heap_bytes_bound,
            )
            .unwrap()
        });
        let (actual, _, _, calls) = measure(|| validator.owned_heap_bytes());
        assert_eq!(calls, 0);
        assert!(requests <= receipt.preparation_requested_bytes_bound);
        assert!(peak <= receipt.preparation_requested_bytes_bound);
        assert!(actual <= receipt.retained_heap_bytes_bound);
        let (result, _, _, calls) = measure(|| {
            PreparedStructuredValueValidator::new_with_storage_limits(
                ty,
                262144,
                reserve.preparation_requested_bytes_bound - 1,
                reserve.retained_heap_bytes_bound,
            )
        });
        assert_eq!(calls, 0);
        assert!(matches!(
            result,
            Err(PreparedStructuredValidationStorageRefusal::Capacity)
        ));
        let (result, _, _, calls) = measure(|| {
            PreparedStructuredValueValidator::new_with_storage_limits(
                ty,
                262144,
                reserve.preparation_requested_bytes_bound,
                reserve.retained_heap_bytes_bound - 1,
            )
        });
        assert_eq!(calls, 0);
        assert!(matches!(
            result,
            Err(PreparedStructuredValidationStorageRefusal::Capacity)
        ));
        println!(
            "shape={i} requested={requests} peak={peak} actual_retained={actual} bound={}",
            receipt.preparation_requested_bytes_bound
        );
    }
    println!("PASS six_shapes_spare_capacity_zero_allocation_reservations_and_one_under_before_allocation");
}

#[test]
fn contract_storage_covers_all_constraint_payloads_and_capacity_slack() {
    let bytes = |capacity: usize, value: &[u8]| {
        let mut b = Vec::with_capacity(capacity);
        b.extend_from_slice(value);
        b
    };
    let mut members = Vec::with_capacity(9);
    members.push(bytes(128, b"member"));
    let mut transitions = Vec::with_capacity(13);
    transitions.push(TextPatternTransition {
        first_scalar: 0,
        last_scalar: 127,
        target_state: 0,
    });
    let mut states = Vec::with_capacity(7);
    states.push(TextPatternState {
        accepting: true,
        transitions,
    });
    let pattern = CheckedTextPattern {
        states,
        start_state: 0,
        maximum_input_characters: 32,
        maximum_match_steps: 64,
    };
    let mut kind = String::with_capacity(80);
    kind.push_str("value/text");
    let mut constraints = Vec::with_capacity(19);
    constraints.push(ValueConstraint::ByteLength {
        minimum: 0,
        maximum: 32,
    });
    constraints.push(ValueConstraint::UnsignedRange {
        minimum: Some(0),
        maximum: Some(9),
        minimum_endpoint: IntervalEndpoint::Inclusive,
        maximum_endpoint: IntervalEndpoint::Inclusive,
    });
    constraints.push(ValueConstraint::SignedRange {
        minimum: Some(-1),
        maximum: Some(1),
        minimum_endpoint: IntervalEndpoint::Inclusive,
        maximum_endpoint: IntervalEndpoint::Exclusive,
    });
    constraints.push(ValueConstraint::FixedIntegerRange {
        minimum: Some(bytes(41, &[0])),
        maximum: Some(bytes(43, &[1])),
        minimum_endpoint: IntervalEndpoint::Inclusive,
        maximum_endpoint: IntervalEndpoint::Inclusive,
    });
    constraints.push(ValueConstraint::QuantityRange {
        minimum: Some(Quantity::new(1, QuantityUnit::Second)),
        maximum: Some(Quantity::new(2, QuantityUnit::Second)),
        minimum_endpoint: IntervalEndpoint::Inclusive,
        maximum_endpoint: IntervalEndpoint::Inclusive,
    });
    constraints.push(ValueConstraint::FloatFinite);
    constraints.push(ValueConstraint::FloatRange {
        minimum: Some(bytes(47, &0f32.to_le_bytes())),
        maximum: Some(bytes(53, &1f32.to_le_bytes())),
        minimum_endpoint: IntervalEndpoint::Inclusive,
        maximum_endpoint: IntervalEndpoint::Inclusive,
    });
    constraints.push(ValueConstraint::CanonicalMembership {
        members,
        negated: false,
    });
    constraints.push(ValueConstraint::TextPattern {
        pattern,
        anchored_start: true,
        anchored_end: true,
        negated: false,
    });
    // Storage inventory is independent of semantic admission; mixed constraint
    // kinds here deliberately exercise every allocation-bearing representation.
    let contract = CheckedValueContract {
        value_kind: KindId::new(kind),
        maximum_bytes: 32,
        constraints,
    };
    let expected = contract.value_kind.0.capacity()
        + contract.constraints.capacity() * std::mem::size_of::<ValueConstraint>()
        + 41
        + 43
        + 47
        + 53
        + 9 * std::mem::size_of::<Vec<u8>>()
        + 128
        + 7 * std::mem::size_of::<TextPatternState>()
        + 13 * std::mem::size_of::<TextPatternTransition>();
    let (owned, _, _, calls) = measure(|| contract.owned_heap_bytes());
    assert_eq!(calls, 0);
    assert_eq!(owned, expected);
    let (reservation, _, _, calls) = measure(|| contract.clone_storage_reservation().unwrap());
    assert_eq!(calls, 0);
    assert_eq!(reservation.retained_heap_bytes_bound, owned);
    let (copy, requests, peak, _) = measure(|| contract.clone());
    assert_eq!(copy, contract);
    assert!(requests <= reservation.preparation_requested_bytes_bound);
    assert!(peak <= reservation.preparation_requested_bytes_bound);
    assert!(copy.owned_heap_bytes() <= reservation.retained_heap_bytes_bound);
    assert!(copy.owned_heap_bytes() < owned);
    println!("contract_clone_requests={requests} peak={peak} actual_clone_heap={} source_spare_capacity_bound={owned}",copy.owned_heap_bytes());
}
