#![cfg(feature = "parser-model-selection")]
extern crate alloc;
pub use conduit_language::lexical_proposer_port;
#[path = "../src/parser_session_window8_model.rs"]
mod model;
#[path = "../src/parser_session_window8_corrected_declaration.rs"]
mod parser_session_window8_corrected_declaration;
mod common {
    #[path = "window8_corrected_candidate.rs"]
    pub mod candidate;
    #[path = "parser_fixture.rs"]
    pub mod fixture;
    #[path = "parser_model_resource.rs"]
    pub mod model_resource;
}
use conduit_plot::rust_binding::{
    PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
use model::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
static COUNTING: AtomicBool = AtomicBool::new(false);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);
struct Counter;
unsafe impl std::alloc::GlobalAlloc for Counter {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            REQUESTS.fetch_add(l.size(), Ordering::Relaxed);
        }
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            REQUESTS.fetch_add(n, Ordering::Relaxed);
        }
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOCATOR: Counter = Counter;
fn native_limits() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 256,
        maximum_input_bytes: 262144,
        maximum_retained_bytes: 100_000_000,
        maximum_preparation_peak_bytes: 1_000_000_000,
        maximum_conversion_requested_bytes: 1_000_000_000,
    }
}
#[test]
fn corrected_model_admission_preserves_owners_and_enforces_all_quotas() {
    let c = common::candidate::prepare();
    let selection = Arc::new(c.selected);
    let mut family = PreparedNativeFamily::prepare(
        &[conduit_ai::ModelSignature::PREPARED_DESCRIPTOR],
        native_limits(),
    )
    .unwrap();
    let generous = Window8ModelLimits {
        maximum_existing_selection_bytes: usize::MAX,
        maximum_retained_model_bytes: usize::MAX,
        maximum_signature_conversion_requested_bytes: usize::MAX,
        maximum_preparation_peak_bytes: usize::MAX,
    };
    let accepted = VerifiedWindow8Model::admit(selection.clone(), &mut family, generous).unwrap();
    let s = accepted.storage_receipt();
    assert!(std::ptr::eq(accepted.selection(), selection.as_ref()));
    assert!(std::ptr::eq(accepted.categorical(), c.scorer.as_ref()));
    assert!(std::ptr::eq(
        accepted.declaration(),
        selection.declaration()
    ));
    assert_eq!(
        accepted.expected_signature_frame(),
        parser_session_window8_corrected_declaration::EXPECTED_SIGNATURE
    );
    drop(accepted);
    let exact = Window8ModelLimits {
        maximum_existing_selection_bytes: s.existing_selection_bytes_bound,
        maximum_retained_model_bytes: s.complete_retained_model_bytes_bound,
        maximum_signature_conversion_requested_bytes: s.signature_conversion_requested_bytes_bound,
        maximum_preparation_peak_bytes: s.preparation_peak_bytes_bound,
    };
    for boundary in 0..4 {
        let mut limits = exact;
        match boundary {
            0 => limits.maximum_existing_selection_bytes -= 1,
            1 => limits.maximum_retained_model_bytes -= 1,
            2 => limits.maximum_signature_conversion_requested_bytes -= 1,
            _ => limits.maximum_preparation_peak_bytes -= 1,
        }
        REQUESTS.store(0, Ordering::Relaxed);
        COUNTING.store(true, Ordering::Relaxed);
        let result = VerifiedWindow8Model::admit(selection.clone(), &mut family, limits);
        COUNTING.store(false, Ordering::Relaxed);
        assert!(matches!(result, Err(Window8ModelRefusal::Pressure)));
        assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    }
    REQUESTS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let accepted = VerifiedWindow8Model::admit(selection.clone(), &mut family, exact);
    COUNTING.store(false, Ordering::Relaxed);
    let accepted = accepted.unwrap();
    let requested = REQUESTS.load(Ordering::Relaxed);
    assert!(
        requested <= s.signature_conversion_requested_bytes_bound + s.verified_owner_bytes_bound
    );
    assert_eq!(accepted.storage_receipt(), s);
    drop(accepted);
    let mut wrong_family = PreparedNativeFamily::prepare(
        &[conduit_data::TensorElement::PREPARED_DESCRIPTOR],
        native_limits(),
    )
    .unwrap();
    REQUESTS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let refused = VerifiedWindow8Model::admit(selection.clone(), &mut wrong_family, generous);
    COUNTING.store(false, Ordering::Relaxed);
    assert!(matches!(refused, Err(Window8ModelRefusal::SignatureFamily)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    let d = selection.declaration();
    let foreign = Arc::new(
        lexical_proposer_port::token_producer::model_definition::ProposalModelDefinition::new(
            &c.owner,
            "foreign-declaration".into(),
            d.feature_abi().into(),
            c.contracts.clone(),
            d.artifact().clone(),
            d.signature().clone(),
            d.dimensions(),
            d.maximum_score_magnitude(),
            d.training_manifest().into(),
            262144,
        )
        .unwrap(),
    );
    let foreign=Arc::new(lexical_proposer_port::token_producer::model_definition::PreparedProposalModelSelection::prepare(&c.owner,c.scorer.clone(),foreign,&c.contracts).unwrap());
    REQUESTS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let refused = VerifiedWindow8Model::admit(foreign, &mut family, generous);
    COUNTING.store(false, Ordering::Relaxed);
    assert!(matches!(refused, Err(Window8ModelRefusal::Declaration)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    println!("complete corrected model owner admission PASS; exact signature readmission; original immutable owners retained; four one-under limits, wrong family and foreign declaration refuse with zero requested allocations; signature/Arc requested={requested}; dynamic storage={s:?}; static Session inventory separate");
}
