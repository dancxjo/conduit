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
#[path = "../src/parser_canonical_composition.rs"]
mod parser_canonical_composition;
#[path = "../src/parser_canonical_schema.rs"]
mod parser_canonical_schema;
#[path = "../src/parser_canonical_refinement.rs"]
mod refinement;
use conduit_language::*;
use conduit_plot::rust_binding::*;
use parser_canonical_composition::*;
use refinement::*;
fn limits() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 128,
        maximum_input_bytes: 262144,
        maximum_retained_bytes: usize::MAX,
        maximum_preparation_peak_bytes: usize::MAX,
        maximum_conversion_requested_bytes: usize::MAX,
    }
}
#[test]
fn exact_leaf_composer_admits_complete_work_before_allocation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let family = PreparedNativeFamily::prepare(
        &[LanguageParserJointRuntimeBeam::PREPARED_DESCRIPTOR],
        limits(),
    )
    .unwrap();
    let mut composer =
        PreparedParserCanonicalComposer::prepare_field::<LanguageParserJointRuntimeBeam>(
            &family,
            &["beam", "epoch"],
            ParserCompositionLimits {
                maximum_output_bytes: 4096,
                maximum_preparation_requested_bytes: usize::MAX,
                maximum_retained_requested_bytes: usize::MAX,
            },
        )
        .unwrap();
    let receipt = composer.receipt();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refusal = PreparedParserCanonicalComposer::prepare_field::<LanguageParserJointRuntimeBeam>(
        &family,
        &["beam", "epoch"],
        ParserCompositionLimits {
            maximum_output_bytes: 4096,
            maximum_preparation_requested_bytes: receipt.preparation_requested_bytes_bound - 1,
            maximum_retained_requested_bytes: usize::MAX,
        },
    );
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(refusal, Err(ParserCompositionRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let value = composer.leaf(&17u64.to_le_bytes()).unwrap();
    let checked = conduit_core::validate_canonical_structured_value(value).is_ok();
    TRACK.store(false, Ordering::Relaxed);
    assert!(checked);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    assert!(receipt.retained_requested_bytes_bound >= 4096);
}
#[test]
fn complete_records_and_variants_preserve_every_original_field_without_allocation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut family = PreparedNativeFamily::prepare(
        &[LanguageParserJointRuntimeBeam::PREPARED_DESCRIPTOR],
        limits(),
    )
    .unwrap();
    let budget = ParserCompositionLimits {
        maximum_output_bytes: 262144,
        maximum_preparation_requested_bytes: usize::MAX,
        maximum_retained_requested_bytes: usize::MAX,
    };
    let mut record =
        PreparedParserCanonicalComposer::prepare::<LanguageParserBasis>(&family, budget).unwrap();
    let original = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("fixture/analysis".into()).unwrap(),
        LanguageTextRevisionId::new("fixture/revision".into()).unwrap(),
        LanguageTextId::new("fixture/text".into()).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap();
    let view = conduit_core::validate_canonical_structured_value(&original).unwrap();
    let fields = [
        view.record_field("analysis_revision").unwrap().unwrap(),
        view.record_field("source_revision").unwrap().unwrap(),
        view.record_field("text").unwrap().unwrap(),
    ];
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let composed = record.record(&fields).unwrap();
    let equal = composed == original;
    TRACK.store(false, Ordering::Relaxed);
    assert!(equal);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    family.decode::<LanguageParserBasis>(composed).unwrap();
    let mut variant =
        PreparedParserCanonicalComposer::prepare::<LanguageUniversalDependencyRelation>(
            &family, budget,
        )
        .unwrap();
    let original = LanguageUniversalDependencyRelation::Dep.encode().unwrap();
    let view = conduit_core::validate_canonical_structured_value(&original).unwrap();
    let tag = view.variant_tag().unwrap();
    let payload = view.variant_payload(tag).unwrap().unwrap();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let composed = variant.variant(tag, payload).unwrap();
    let equal = composed == original;
    TRACK.store(false, Ordering::Relaxed);
    assert!(equal);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    family
        .decode::<LanguageUniversalDependencyRelation>(composed)
        .unwrap();
}

fn raw(score: i64) -> LanguageParserJointRuntimeRawHypothesis {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("fixture/analysis".into()).unwrap(),
        LanguageTextRevisionId::new("fixture/revision".into()).unwrap(),
        LanguageTextId::new("fixture/text".into()).unwrap(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let state = LanguageParserNumericState::new(
        basis,
        0,
        1,
        [5, 5, 5, 5, 4],
        relation.clone(),
        relation.clone(),
        relation.clone(),
        relation,
        [4, 4, 4, 4, 4],
        1,
        0,
    )
    .unwrap();
    LanguageParserJointRuntimeRawHypothesis::new(
        LanguageParserRawJointHypothesis::new(
            [0; 4],
            LanguageParserRawHypothesis::new(true, 1, score, state).unwrap(),
        )
        .unwrap(),
        0,
    )
    .unwrap()
}
#[test]
fn recursive_refinement_preflights_all_storage_and_preserves_native_law_boundary() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut family = PreparedNativeFamily::prepare(
        &[
            LanguageParserJointRuntimeRawHypothesis::PREPARED_DESCRIPTOR,
            LanguageParserJointRuntimeHypothesis::PREPARED_DESCRIPTOR,
        ],
        limits(),
    )
    .unwrap();
    let budget = ParserRefinementLimits {
        maximum_node_bytes: 262144,
        maximum_retained_requested_bytes: usize::MAX,
        maximum_preparation_requested_bytes: usize::MAX,
    };
    let mut composer = PreparedParserCanonicalRefinement::<
        LanguageParserJointRuntimeRawHypothesis,
        LanguageParserJointRuntimeHypothesis,
    >::prepare(&family, budget)
    .unwrap();
    let receipt = composer.receipt();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refusal = PreparedParserCanonicalRefinement::<
        LanguageParserJointRuntimeRawHypothesis,
        LanguageParserJointRuntimeHypothesis,
    >::prepare(
        &family,
        ParserRefinementLimits {
            maximum_preparation_requested_bytes: receipt.preparation_requested_bytes_bound - 1,
            ..budget
        },
    );
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(refusal, Err(ParserRefinementRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    let original = raw(1).encode().unwrap();
    let source = conduit_core::validate_canonical_structured_value(&original).unwrap();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let result = composer.compose(source).unwrap();
    TRACK.store(false, Ordering::Relaxed);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    let admitted = family
        .decode::<LanguageParserJointRuntimeHypothesis>(result)
        .unwrap();
    assert_eq!(*admitted.hypothesis().parser().score(), 1);
    let illegal = raw(2_000_001).encode().unwrap();
    let result = composer
        .compose(conduit_core::validate_canonical_structured_value(&illegal).unwrap())
        .unwrap();
    assert!(family
        .decode::<LanguageParserJointRuntimeHypothesis>(result)
        .is_err());
    assert!(receipt.retained_requested_bytes_bound >= 262144);
}

#[test]
fn nested_refinement_preserves_exact_parent_field_schema_and_reserves_before_allocation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut family = PreparedNativeFamily::prepare(
        &[
            LanguageParserJointRuntimeRawHypothesis::PREPARED_DESCRIPTOR,
            LanguageParserJointRuntimeHypothesis::PREPARED_DESCRIPTOR,
        ],
        limits(),
    )
    .unwrap();
    let budget = ParserRefinementLimits {
        maximum_node_bytes: 262144,
        maximum_retained_requested_bytes: usize::MAX,
        maximum_preparation_requested_bytes: usize::MAX,
    };
    let mut composer = PreparedParserCanonicalRefinement::<
        LanguageParserJointRuntimeRawHypothesis,
        LanguageParserJointRuntimeHypothesis,
    >::prepare_fields(&family, &["hypothesis"], &["hypothesis"], budget)
    .unwrap();
    let receipt = composer.receipt();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refused = PreparedParserCanonicalRefinement::<
        LanguageParserJointRuntimeRawHypothesis,
        LanguageParserJointRuntimeHypothesis,
    >::prepare_fields(
        &family,
        &["hypothesis"],
        &["hypothesis"],
        ParserRefinementLimits {
            maximum_preparation_requested_bytes: receipt.preparation_requested_bytes_bound - 1,
            ..budget
        },
    );
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(refused, Err(ParserRefinementRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    let original = raw(1).encode().unwrap();
    let parent = conduit_core::validate_canonical_structured_value(&original).unwrap();
    let child = parent.record_field("hypothesis").unwrap().unwrap();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let output = composer.compose(child).unwrap();
    TRACK.store(false, Ordering::Relaxed);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    let accepted = family
        .decode::<LanguageParserJointHypothesis>(output)
        .unwrap();
    assert_eq!(*accepted.parser().score(), 1);
    assert!(matches!(
        composer.compose(parent),
        Err(ParserRefinementRefusal::Type)
    ));
    assert!(matches!(
        PreparedParserCanonicalRefinement::<
            LanguageParserJointRuntimeRawHypothesis,
            LanguageParserJointRuntimeHypothesis,
        >::prepare_fields(&family, &["absent"], &["hypothesis"], budget),
        Err(ParserRefinementRefusal::Type)
    ));
}

#[test]
fn exact_variant_payload_prepares_fixed_default_relation_without_ordinary_native_encoding() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut family = PreparedNativeFamily::prepare(
        &[LanguageParserJointRuntimeBeam::PREPARED_DESCRIPTOR],
        limits(),
    )
    .unwrap();
    let budget = ParserCompositionLimits {
        maximum_output_bytes: 1024,
        maximum_preparation_requested_bytes: usize::MAX,
        maximum_retained_requested_bytes: usize::MAX,
    };
    let descriptor = LanguageUniversalDependencyRelation::PREPARED_DESCRIPTOR;
    let mut payload = PreparedParserCanonicalComposer::prepare_descriptor_steps(
        &family,
        descriptor,
        &[parser_canonical_schema::SchemaStep::Case("dep")],
        budget,
    )
    .unwrap();
    let mut relation =
        PreparedParserCanonicalComposer::prepare_descriptor_steps(&family, descriptor, &[], budget)
            .unwrap();
    let receipt = payload.receipt();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let refusal = PreparedParserCanonicalComposer::prepare_descriptor_steps(
        &family,
        descriptor,
        &[parser_canonical_schema::SchemaStep::Case("dep")],
        ParserCompositionLimits {
            maximum_preparation_requested_bytes: receipt.preparation_requested_bytes_bound - 1,
            ..budget
        },
    );
    TRACK.store(false, Ordering::Relaxed);
    assert!(matches!(refusal, Err(ParserCompositionRefusal::Pressure)));
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let unit = payload.leaf(&[]).unwrap();
    let output = relation
        .variant(
            "dep",
            conduit_core::validate_canonical_structured_value(unit).unwrap(),
        )
        .unwrap();
    TRACK.store(false, Ordering::Relaxed);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    assert_eq!(
        family
            .decode::<LanguageUniversalDependencyRelation>(output)
            .unwrap(),
        LanguageUniversalDependencyRelation::Dep
    );
    assert!(matches!(
        PreparedParserCanonicalComposer::prepare_descriptor_steps(
            &family,
            descriptor,
            &[parser_canonical_schema::SchemaStep::Field("dep")],
            budget
        ),
        Err(ParserCompositionRefusal::Type)
    ));
}

#[path = "../src/parser_session_rank.rs"]
mod parser_session_rank;

#[test]
fn original_integer_rank_observes_all_scores_and_source_mask_without_allocation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut scores = [0i64; 76];
    scores[0] = i64::MAX;
    scores[1] = i64::MIN;
    scores[2] = 7;
    scores[3] = 7;
    let mut allowed = [false; 76];
    for index in [1, 2, 3, 75] {
        allowed[index] = true;
    }
    let raw = raw(1);
    let state = raw.hypothesis().parser().state();
    let mask = LanguageParserLegalMask::new(allowed, state.basis().clone(), state.clone())
        .unwrap()
        .encode()
        .unwrap();
    let scores = LanguageParserV2ModelScores::new(scores)
        .unwrap()
        .encode()
        .unwrap();
    let scores = conduit_core::validate_canonical_structured_value(&scores).unwrap();
    let mask = conduit_core::validate_canonical_structured_value(&mask).unwrap();
    let mut rank = parser_session_rank::PreparedParserDriverRank::new();
    REQUESTS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let classes = rank.rank(scores, mask).unwrap();
    let equal = classes == [2, 3, 75, 1];
    TRACK.store(false, Ordering::Relaxed);
    assert!(equal);
    assert_eq!(REQUESTS.load(Ordering::Relaxed), 0);
    assert_eq!(rank.selected(0), Some((2, 7)));
    assert_eq!(rank.selected(4), None);
    assert!(matches!(
        rank.rank(mask, scores),
        Err(parser_session_rank::DriverRankRefusal::Type)
    ));
    assert!(matches!(
        parser_session_rank::u64_value(
            parser_session_rank::field(scores, &["scores"])
                .unwrap()
                .collection_index(0)
                .unwrap()
                .unwrap()
        ),
        Err(parser_session_rank::DriverRankRefusal::Type)
    ));
}
