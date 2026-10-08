use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
#[path = "../src/parser_source_native_parity.rs"]
mod parity;
use conduit_plot::rust_binding::{
    NativeFamilyConversionProfile, NativeFamilyTypeDescriptor, PreparedNativeFamily,
    PreparedNativeFamilyLimits,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use parity::{verify_source_native_parity, SourceNativeParityRefusal as R};

#[test]
fn complete_source_laws_and_exact_descriptor_readiness_are_required() {
    let mut checked = check_syntax_document(&parse_syntax_document(
        "type Interval = {\n start: U32\n end: U32\n where .start <= .end\n where .end < 10\n}\n"), &StartupCatalog::new()).unwrap();
    let original = &checked.native_types[0];
    let type_bytes = Box::leak(
        original
            .value_type
            .canonical_bytes()
            .unwrap()
            .into_boxed_slice(),
    );
    let laws = original
        .invariants
        .iter()
        .map(|law| &*Box::leak(law.canonical_bytes().unwrap().into_boxed_slice()))
        .collect::<Vec<_>>();
    let descriptor = Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes,
        laws: Box::leak(laws.into_boxed_slice()),
        contracts: &[],
        children: &[],
        conversion_profile: NativeFamilyConversionProfile::Record,
        maximum_inline_bytes: 128,
    }));
    let family = PreparedNativeFamily::prepare(
        &[descriptor],
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: 65536,
            maximum_retained_bytes: usize::MAX,
            maximum_preparation_peak_bytes: usize::MAX,
            maximum_conversion_requested_bytes: usize::MAX,
        },
    )
    .unwrap();
    let required =
        verify_source_native_parity(&checked, &family, &[descriptor], usize::MAX).unwrap();
    checked.native_types[0]
        .value_contracts
        .push(conduit_plot::NativeTypeValueContract {
            representation_path: "start".into(),
            contract: conduit_core::CheckedValueContract {
                value_kind: conduit_core::kind_id("value/u32"),
                maximum_bytes: 4,
                constraints: vec![],
            },
        });
    assert_eq!(
        verify_source_native_parity(&checked, &family, &[descriptor], required),
        Err(R::Metadata)
    );
    checked.native_types[0].value_contracts.clear();
    let required =
        verify_source_native_parity(&checked, &family, &[descriptor], usize::MAX).unwrap();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refused = verify_source_native_parity(&checked, &family, &[descriptor], required - 1);
    TRACK.store(false, Ordering::Relaxed);
    assert_eq!(refused, Err(R::Pressure));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    checked.native_types[0].invariants.swap(0, 1);
    assert_eq!(
        verify_source_native_parity(&checked, &family, &[descriptor], required),
        Err(R::Metadata)
    );
    checked.native_types[0].invariants.swap(0, 1);
    let foreign = Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes: descriptor.type_bytes,
        laws: descriptor.laws,
        contracts: descriptor.contracts,
        children: &[],
        conversion_profile: descriptor.conversion_profile,
        maximum_inline_bytes: descriptor.maximum_inline_bytes,
    }));
    assert_eq!(
        verify_source_native_parity(&checked, &family, &[foreign], required),
        Err(R::Readiness)
    );
}
