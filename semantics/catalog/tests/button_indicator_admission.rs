use conduit_core::InfoBool;
use conduit_semantic_catalog::{
    button_transition_value, map_button_transition_to_indicator, PreparedButtonIndicatorMapper,
    BUTTON_TRANSITION_MAXIMUM_BYTES,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountedAllocator;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

fn measured<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|active| active.set(true));
    let result = f();
    COUNTING.with(|active| active.set(false));
    (result, ALLOCATIONS.with(Cell::get))
}

#[test]
fn prepared_mapping_preserves_semantics_without_play_allocations() {
    let mut mapper = PreparedButtonIndicatorMapper::new().unwrap();
    let control = button_transition_value("button/primary", true, 0)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let (_, allocations) = measured(|| map_button_transition_to_indicator(&control));
    assert!(
        allocations > 0,
        "allocator probe must detect the allocating reference path"
    );
    for identity in ["button/primary", "semantic-control"] {
        for sequence in [0, 1, u64::MAX] {
            for pressed in [true, false] {
                let encoded = button_transition_value(identity, pressed, sequence)
                    .unwrap()
                    .canonical_bytes()
                    .unwrap();
                let expected = map_button_transition_to_indicator(&encoded).unwrap();
                let (result, allocations) = measured(|| mapper.map(&encoded));
                assert_eq!(result, Ok(expected));
                assert_eq!(allocations, 0);
            }
        }
    }
}

#[test]
fn malformed_inputs_do_not_allocate_or_poison_the_next_mapping() {
    let mut mapper = PreparedButtonIndicatorMapper::new().unwrap();
    let encoded = button_transition_value("button/primary", false, 1)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut trailing = encoded.clone();
    trailing.push(0);
    let oversized = vec![0; BUTTON_TRANSITION_MAXIMUM_BYTES as usize + 1];
    for malformed in [trailing.as_slice(), oversized.as_slice(), b"pressed"] {
        let (result, allocations) = measured(|| mapper.map(malformed));
        assert!(result.is_err());
        assert_eq!(allocations, 0);
    }
    for length in 0..encoded.len() {
        let (result, allocations) = measured(|| mapper.map(&encoded[..length]));
        assert!(result.is_err(), "accepted truncated length {length}");
        assert_eq!(allocations, 0);
    }
    let (result, allocations) = measured(|| mapper.map(&encoded));
    assert_eq!(result, Ok(InfoBool::FALSE));
    assert_eq!(allocations, 0);
}

#[test]
fn byte_mutations_match_reference_acceptance_without_allocating() {
    let mut mapper = PreparedButtonIndicatorMapper::new().unwrap();
    let encoded = button_transition_value("button/primary", true, 0)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    for index in 0..encoded.len() {
        let mut mutated = encoded.clone();
        mutated[index] ^= 0xff;
        let expected = map_button_transition_to_indicator(&mutated).ok();
        let (actual, allocations) = measured(|| mapper.map(&mutated));
        assert_eq!(actual.ok(), expected, "mutation at byte {index}");
        assert_eq!(allocations, 0);
    }
}
