use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct Allocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            REQUESTS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            REQUESTS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
extern crate alloc;
#[path = "../src/parser_canonical_u64_collection.rs"]
mod collection;
#[path = "../src/parser_canonical_schema.rs"]
mod parser_canonical_schema;
use collection::*;
use conduit_language::LanguageParserJointRuntimeBeam;
use conduit_plot::rust_binding::{
    PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
#[test]
fn fixed_collection_preparation_precedes_allocation_and_play_preserves_framing() {
    let _lock = TEST_LOCK.lock().unwrap();
    let family = PreparedNativeFamily::prepare(
        &[LanguageParserJointRuntimeBeam::PREPARED_DESCRIPTOR],
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: 262144,
            maximum_retained_bytes: usize::MAX,
            maximum_preparation_peak_bytes: usize::MAX,
            maximum_conversion_requested_bytes: usize::MAX,
        },
    )
    .unwrap();
    let mut composer = PreparedParserU64Collection::prepare::<LanguageParserJointRuntimeBeam>(
        &family,
        &["selected"],
        usize::MAX,
        usize::MAX,
    )
    .unwrap();
    let bound = composer.preparation_requested_bytes_bound();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refusal = PreparedParserU64Collection::prepare::<LanguageParserJointRuntimeBeam>(
        &family,
        &["selected"],
        bound - 1,
        usize::MAX,
    );
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(refusal, Err(CollectionRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    let expected_type = conduit_core::StructuredInfoType::collection(
        conduit_core::StructuredInfoType::leaf("value/u64".into()).unwrap(),
        Some(4),
    )
    .unwrap();
    let expected = conduit_core::StructuredInfoValue::collection(
        expected_type,
        [0, 1, u64::MAX, 4]
            .into_iter()
            .map(|n| {
                conduit_core::StructuredInfoValue::leaf(
                    conduit_core::StructuredInfoType::leaf("value/u64".into()).unwrap(),
                    n.to_le_bytes().to_vec(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let capacity = composer.retained_capacity_bytes();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let actual = composer.compose(&[0, 1, u64::MAX, 4]).unwrap();
    let equal = actual == expected.as_slice();
    TRACK.store(false, Ordering::Relaxed);
    assert!(equal);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    assert_eq!(
        composer.compose(&[1]).unwrap_err(),
        CollectionRefusal::Length
    );
    assert_eq!(composer.encoded(), expected);
    assert_eq!(composer.retained_capacity_bytes(), capacity);
    assert_eq!(composer.compose(&[0, 1, u64::MAX, 4]).unwrap(), expected);
}

#[test]
fn absent_exact_root_refuses_before_allocating_schema_or_output() {
    let _lock = TEST_LOCK.lock().unwrap();
    let family = PreparedNativeFamily::prepare(
        &[],
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: 262144,
            maximum_retained_bytes: usize::MAX,
            maximum_preparation_peak_bytes: usize::MAX,
            maximum_conversion_requested_bytes: usize::MAX,
        },
    )
    .unwrap();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let result = PreparedParserU64Collection::prepare::<LanguageParserJointRuntimeBeam>(
        &family,
        &["selected"],
        usize::MAX,
        usize::MAX,
    );
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(result, Err(CollectionRefusal::Descriptor)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
}
