//! Requested-allocation probes for the unchanged canonical Type decoder.
use conduit_core::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static REQUESTED: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}
struct Counter;
#[global_allocator]
static ALLOCATOR: Counter = Counter;
fn charge(bytes: usize) {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        REQUESTED.with(|total| total.set(total.get().checked_add(bytes).unwrap()));
        LIVE.with(|live| {
            live.set(live.get().checked_add(bytes).unwrap());
            PEAK.with(|peak| peak.set(peak.get().max(live.get())));
        });
    }
}
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // Conservatively observe overlap of old and new buffers at realloc.
        charge(size);
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            LIVE.with(|live| live.set(live.get() - layout.size()));
        }
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACK.try_with(Cell::get).unwrap_or(false) {
            LIVE.with(|live| live.set(live.get() - layout.size()));
        }
        unsafe { System.dealloc(pointer, layout) }
    }
}
fn probe(encoded: &[u8]) -> usize {
    let bound = StructuredInfoType::canonical_decode_storage_bound(encoded).unwrap();
    REQUESTED.with(|total| total.set(0));
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
    TRACK.with(|tracking| tracking.set(true));
    let decoded = StructuredInfoType::from_canonical_bytes(encoded);
    TRACK.with(|tracking| tracking.set(false));
    let requested = REQUESTED.with(Cell::get);
    let decoded = decoded.unwrap();
    let retained = LIVE.with(Cell::get);
    let peak = PEAK.with(Cell::get);
    assert_eq!(retained, decoded.owned_heap_bytes());
    assert!(peak >= retained);
    assert!(bound >= peak, "bound {bound}, peak {peak}");
    eprintln!("type decode encoded={} retained={retained} peak={peak} requested={requested} bound={bound}", encoded.len());
    assert!(bound >= requested, "bound {bound}, requests {requested}");
    assert_eq!(decoded.canonical_byte_length().unwrap(), encoded.len());
    requested
}
fn text(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}
#[test]
fn leaf_and_law_free_nominal_charge_the_actual_constructor_paths() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let encoded = leaf.canonical_bytes().unwrap();
    assert_eq!(probe(&encoded), "value/u64".len()); // leaf has no validation buffer
    let nominal = StructuredInfoType::nominal(kind_id("n"), leaf).unwrap();
    let encoded = nominal.canonical_bytes().unwrap();
    let expected =
        1 + "value/u64".len() + core::mem::size_of::<StructuredInfoType>() + encoded.len();
    assert_eq!(
        StructuredInfoType::canonical_decode_storage_bound(&encoded).unwrap(),
        expected
    );
    assert_eq!(probe(&encoded), expected);
}
#[test]
fn nested_shapes_and_unsorted_full_aggregate_buffers_fit_requested_bound() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let empty_collection = StructuredInfoType::collection(leaf.clone(), Some(0)).unwrap();
    probe(&empty_collection.canonical_bytes().unwrap());
    let collection = StructuredInfoType::collection(leaf.clone(), Some(8)).unwrap();
    let sequence = StructuredInfoType::bounded_sequence(collection, 1, 16).unwrap();
    let variant = StructuredInfoType::variant(
        kind_id("variant"),
        vec![
            StructuredVariantCase::new("sequence", sequence).unwrap(),
            StructuredVariantCase::new("leaf", leaf.clone()).unwrap(),
        ],
    )
    .unwrap();
    let mut nested = StructuredInfoType::record(
        kind_id("record"),
        vec![
            StructuredFieldType::new("variant", variant).unwrap(),
            StructuredFieldType::new("leaf", leaf).unwrap(),
        ],
    )
    .unwrap();
    for _ in 0..8 {
        nested = StructuredInfoType::nominal(kind_id("nominal"), nested).unwrap();
    }
    probe(&nested.canonical_bytes().unwrap());
    // The decoder also accepts and sorts unsorted members. Exercise actual
    // stable-sort scratch at the full record/variant member ceilings.
    for tag in [2, 3] {
        let mut encoded = vec![tag];
        text(&mut encoded, &"s".repeat(MAXIMUM_STRUCTURED_NAME_BYTES));
        encoded.extend_from_slice(&64u32.to_le_bytes());
        for index in (0..64).rev() {
            text(&mut encoded, &format!("member{index:03}"));
            encoded.push(0);
            text(&mut encoded, "value/u64");
        }
        probe(&encoded);
    }
}
