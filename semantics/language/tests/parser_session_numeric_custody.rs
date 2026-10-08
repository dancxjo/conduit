#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
#![allow(dead_code, unused_attributes)]
extern crate alloc;
extern crate conduitos as actual_expression_owner;
#[path = "../src/lib.rs"]
mod language;
pub use language::revision;
pub use language::*;
#[path = "../src/parser_production_families.rs"]
mod families;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "../src/parser_source_native_parity.rs"]
mod native_parity;
#[path = "../src/parser_session_numeric_custody.rs"]
mod numeric_custody;
#[path = "../src/parser_session_numeric_plan.rs"]
mod numeric_plan;
#[path = "../src/parser_session_numeric_plan_storage.rs"]
mod numeric_plan_storage;
#[path = "common/parser_production_runtime.rs"]
mod runtime;
#[path = "../src/parser_session_source_plan.rs"]
mod source_plan;
use conduit_ai::integer_categorical_step::*;

use conduit_plot::rust_binding::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::{rc::Rc, sync::Arc};
struct CountRequests;
static COUNTING: AtomicBool = AtomicBool::new(false);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for CountRequests {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() && COUNTING.load(Ordering::Relaxed) {
            REQUESTED.fetch_add(layout.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, n) };
        if !q.is_null() && COUNTING.load(Ordering::Relaxed) {
            REQUESTED.fetch_add(n, Ordering::Relaxed);
        }
        q
    }
}
#[global_allocator]
static ALLOCATOR: CountRequests = CountRequests;
fn measured_requests<T>(f: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNTING.store(false, Ordering::Relaxed);
        }
    }
    REQUESTED.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let reset = Reset;
    let output = f();
    drop(reset);
    (output, REQUESTED.load(Ordering::Relaxed))
}

fn source() -> String {
    [
        include_str!("../types.conduit"),
        include_str!("../identity.conduit"),
        include_str!("../coverage.conduit"),
        include_str!("../syntax.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../revision_lineage.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_window8.conduit"),
        include_str!("../parser_window8_search.conduit"),
        include_str!("../parser_window8_facts.conduit"),
        include_str!("../parser_window8_lexical_selection.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_scorer.conduit"),
        include_str!("../parser_mask.conduit"),
        include_str!("../parser_joint.conduit"),
        include_str!("../parser_session_seed.conduit"),
        include_str!("../parser_session_completion.conduit"),
        include_str!("../parser_numeric_ports.conduit"),
        include_str!("../parser_session_numeric.conduit"),
        include_str!("../parser_joint_decode.conduit"),
        include_str!("../parser_available.conduit"),
        include_str!("../parser_revision.conduit"),
        include_str!("../parser_scorer_v2.conduit"),
        include_str!("../parser_session.conduit"),
        include_str!("../parser_session_policy.conduit"),
        include_str!("../parser_session_facts.conduit"),
        include_str!("../parser_session_dependency.conduit"),
        include_str!("../parser_session_committed_dependency.conduit"),
        include_str!("../parser_session_commit.conduit"),
        include_str!("../parser_session_rebase.conduit"),
        include_str!("../parser_session_branch.conduit"),
        include_str!("../parser_session_protection.conduit"),
        include_str!("../parser_session_protected_set.conduit"),
        include_str!("../parser_session_independent_admission.conduit"),
        include_str!("../parser_session_protected_rebase.conduit"),
        include_str!("../parser_session_protected_forest.conduit"),
        include_str!("../parser_session_protected_branch.conduit"),
        include_str!("../parser_session_protected_mask.conduit"),
        include_str!("../parser_session_custody.conduit"),
        include_str!("../parser_session_custody_origins.conduit"),
        include_str!("../parser_session_custody_frontier.conduit"),
        include_str!("../parser_session_custody_initialize.conduit"),
        include_str!("../parser_session_independent_commit.conduit"),
        include_str!("../parser_session_independent_commit_rebase.conduit"),
        include_str!("../discourse.conduit"),
        include_str!("../prosody.conduit"),
        include_str!("../pronunciation_selection.conduit"),
    ]
    .join("\n")
}
fn profile() -> Arc<PreparedCategoricalStep> {
    model_resource::categorical(
        include_bytes!("../training/ewt_joint_v2/ewt_joint.i16").to_vec(),
        parser_model_selection::pinned_v2_model_signature().unwrap(),
        1,
    )
}
fn mixed_source(profile: &PreparedCategoricalStep) -> String {
    format!("{}\nplot production-model (\n    features: LanguageParserV2ModelFeatures...| >> observed: LanguageParserV2ModelScores...|\n) {{\n features >> language-parser-v2-feature-indices() >> {}() >> language-parser-v2-score-observation() >> observed\n}}\n", source(), profile.kind_identity(true))
}
fn verification_limits() -> parser_session_execution::ParserSessionVerificationLimits {
    parser_session_execution::ParserSessionVerificationLimits {
        decoded_program_bytes: 512 * 1024 * 1024,
        preparation_peak_bytes: 1024 * 1024 * 1024,
        retained_bytes: 512 * 1024 * 1024,
    }
}
#[test]
fn actual_mixed_plan_preserves_complete_topology_and_original_model() {
    let profile = profile();
    let target = runtime::prepare_source_with_storage(
        profile.clone(),
        mixed_source(&profile),
        "production-model",
        Some(2),
    );
    numeric_plan::validate_numeric_plan_structure(&target.expanded_source, &target.original_plan)
        .unwrap();
    numeric_plan::validate_numeric_plan_seal_and_resource(
        &target.expanded_source,
        &target.original_plan,
        &profile,
    )
    .unwrap();
    let heap = numeric_plan_storage::numeric_plan_retained_bytes(&target.original_plan).unwrap();
    assert!(heap > 0);
    eprintln!("original whole mixed Plan heap={heap}");
    let mut changed = (*target.original_plan).clone();
    changed.fragments[0].connections.swap(0, 1);
    assert!(
        numeric_plan::validate_numeric_plan_structure(&target.expanded_source, &changed).is_err()
    );
}

fn query() -> LanguageParserV2ChoiceQuery {
    let provenance =
        LinguisticDerivationProvenance::deterministic_rule("test/custody".into(), "1".into())
            .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("test/text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("test/revision".into()).unwrap(),
            "Hello, Travis!".into(),
        )
        .unwrap(),
        None,
        provenance,
        0,
        None,
    )
    .unwrap();
    let profile = parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let lexical = lexical::prepare_lexical_tape(&source, &profile, None).unwrap();
    let count = lexical.tape().tokens().len() as u64;
    assert_eq!(count, 4);
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("test/analysis".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let state = LanguageParserState::new(
        basis,
        0,
        1,
        [5, 5, 5, 5, 4],
        relation.clone(),
        relation.clone(),
        relation.clone(),
        relation,
        [4, 4, 4, 4, 4],
        count,
        0,
    )
    .unwrap();
    LanguageParserV2ChoiceQuery::new(
        [0; 4],
        0,
        0,
        LanguageParserAvailableLexical::new(lexical.tape().clone(), count).unwrap(),
        state,
    )
    .unwrap()
}
fn family_limits() -> families::ProductionFamilyLimits {
    families::ProductionFamilyLimits {
        family: PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_bytes: 1024 * 1024 * 1024,
            maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
            maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
        },
        maximum_retained_bytes: 7 * 1024 * 1024 * 1024,
        maximum_preparation_peak_bytes: 7 * 1024 * 1024 * 1024,
        maximum_static_artifact_bytes: 1024 * 1024 * 1024,
        maximum_combined_bytes: 12 * 1024 * 1024 * 1024,
        other_reserved_bytes: 1024 * 1024 * 1024,
        concurrent_native_values: 2,
    }
}
#[test]
fn actual_source_feature_plan_numeric_plan_and_complete_retained_replay() {
    use numeric_custody::*;
    use parser_session_canonical_ingress::{
        ParserCanonicalIngressLimits, PreparedCanonicalParserSessionPort,
        PreparedParserExecutionFrames,
    };
    use parser_session_execution::{
        verification::PreparedSourceVerification, ParserSessionEntry as Entry,
    };
    let profile = profile();
    let lexical = parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let selection =
        parser_model_selection::PreparedParserModelSelection::prepare(profile.clone(), &lexical)
            .unwrap();
    let target = runtime::prepare_source_with_storage(
        profile.clone(),
        mixed_source(&profile),
        "production-model",
        Some(1),
    );
    let original_plan = target.original_plan.clone();
    let (projector, _, _, projector_receipt) =
        PreparedSourceVerification::prepare(Entry::V2FeatureIndices, verification_limits())
            .unwrap();
    let (wrapper, _, _, wrapper_receipt) =
        PreparedSourceVerification::prepare(Entry::V2ScoreObservation, verification_limits())
            .unwrap();
    let plan_reservation = numeric_plan_storage::numeric_plan_preparation_reservation(
        &original_plan,
        profile.storage_receipt().unwrap().retained_heap_bytes_bound,
        [
            projector_receipt.preparation_peak_heap_bytes_bound,
            wrapper_receipt.preparation_peak_heap_bytes_bound,
        ],
        usize::MAX,
    )
    .unwrap();
    eprintln!(
        "mixed Plan original={} verification request reservation={}",
        plan_reservation.original_plan_retained_bytes,
        plan_reservation.temporary_requested_bytes_bound
    );
    let (too_small, preflight_requests) = measured_requests(|| {
        numeric_plan_storage::numeric_plan_preparation_reservation(
            &original_plan,
            profile.storage_receipt().unwrap().retained_heap_bytes_bound,
            [
                projector_receipt.preparation_peak_heap_bytes_bound,
                wrapper_receipt.preparation_peak_heap_bytes_bound,
            ],
            plan_reservation.temporary_requested_bytes_bound - 1,
        )
    });
    assert!(matches!(
        too_small,
        Err(numeric_plan_storage::NumericPlanStorageRefusal::Pressure)
    ));
    assert_eq!(preflight_requests, 0);
    let (validated, validation_requests) = measured_requests(|| {
        numeric_plan::validate_numeric_plan_seal_and_resource(
            &target.expanded_source,
            &original_plan,
            &profile,
        )
    });
    validated.unwrap();
    eprintln!(
        "complete original Plan verification requested={validation_requests}, reserved={}",
        plan_reservation.temporary_requested_bytes_bound
    );
    assert!(validation_requests <= plan_reservation.temporary_requested_bytes_bound);

    let families = families::PreparedProductionParserFamilies::prepare(family_limits()).unwrap();
    eprintln!("fresh full-family receipt: {:?}", families.receipt());
    let family = families
        .for_values::<LanguageParserV2ChoiceQuery, LanguageParserV2ModelFeatures>()
        .unwrap();
    native_parity::verify_source_native_parity(
        &target.checked_source,
        &family.borrow(),
        &[
            LanguageParserV2ChoiceQuery::PREPARED_DESCRIPTOR,
            LanguageParserV2ModelFeatures::PREPARED_DESCRIPTOR,
            LanguageParserV2ModelScores::PREPARED_DESCRIPTOR,
        ],
        1024 * 1024 * 1024,
    )
    .unwrap();
    let source_target = runtime::prepare_checked_source(
        profile.clone(),
        target.source_document.clone(),
        target.checked_source.clone(),
        Entry::V2ModelFeatures.name(),
        Some(1),
    );
    source_plan::validate_fixed_source_plan_seal(
        &source_target.expanded_source,
        &source_target.original_plan,
        Entry::V2ModelFeatures,
    )
    .unwrap();
    let mut source_port = PreparedCanonicalParserSessionPort::<
        LanguageParserV2ChoiceQuery,
        LanguageParserV2ModelFeatures,
        _,
    >::prepare(
        Entry::V2ModelFeatures,
        source_target,
        family.clone(),
        ParserCanonicalIngressLimits {
            maximum_invocations: 1,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_output_bytes: 4096,
        },
        verification_limits(),
    )
    .unwrap();
    eprintln!(
        "feature verification receipt: {:?}",
        source_port.verification_storage()
    );
    let original_query = query().encode().unwrap();
    eprintln!(
        "complete original query bytes={}; reserved input={}",
        original_query.len(),
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
    );
    let feature_execution = source_port
        .execute(
            &original_query,
            PreparedParserExecutionFrames::prepare(
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
                4096,
            )
            .unwrap(),
        )
        .unwrap();
    let original_features = feature_execution.output_bytes().to_vec();
    let canonical = PreparedCategoricalCanonicalAdmission::prepare(
        profile.clone(),
        CategoricalCanonicalAdmissionLimits {
            maximum_preparation_peak_bytes: 4 * 1024 * 1024,
            maximum_retained_bytes: 4 * 1024 * 1024,
        },
    )
    .unwrap();
    eprintln!(
        "categorical codecs receipt: {:?}",
        canonical.storage_receipt()
    );
    eprintln!(
        "fixed feature-index Source receipt: {:?}; fixed score observation Source receipt: {:?}",
        projector_receipt, wrapper_receipt
    );
    let mut port = PreparedParserNumericCustody::from_prepared(
        target,
        family.clone(),
        projector,
        wrapper,
        canonical,
        original_plan.clone(),
        1,
        &selection,
    )
    .unwrap();
    let mut output = Vec::with_capacity(4096);
    output.resize(4096, 0);
    let history = port
        .execute(
            feature_execution,
            ParserNumericFrames {
                indices: Vec::with_capacity(4096),
                scores: Vec::with_capacity(4096),
                output,
            },
        )
        .unwrap();
    assert_eq!(history.features.input_bytes(), original_query);
    assert_eq!(history.features.output_bytes(), original_features);
    assert!(Arc::ptr_eq(&profile, &history.original_model));
    assert!(Rc::ptr_eq(&original_plan, &history.original_plan));
    let (mut feature_replay, _, _, _) =
        PreparedSourceVerification::prepare(Entry::V2ModelFeatures, verification_limits()).unwrap();
    let (mut projection_replay, _, _, _) =
        PreparedSourceVerification::prepare(Entry::V2FeatureIndices, verification_limits())
            .unwrap();
    let (mut wrapper_replay, _, _, _) =
        PreparedSourceVerification::prepare(Entry::V2ScoreObservation, verification_limits())
            .unwrap();
    let mut numeric_replay = PreparedCategoricalCanonicalAdmission::prepare(
        profile,
        CategoricalCanonicalAdmissionLimits {
            maximum_preparation_peak_bytes: 4 * 1024 * 1024,
            maximum_retained_bytes: 4 * 1024 * 1024,
        },
    )
    .unwrap();
    let mut budget = ParserNumericReadmissionBudget {
        maximum_live_native_bytes: family
            .borrow()
            .storage_receipt()
            .conversion_requested_bytes_bound
            * 2,
    };
    let typed = history
        .replay_and_readmit(
            &mut feature_replay,
            &mut projection_replay,
            &mut wrapper_replay,
            &mut numeric_replay,
            &mut family.borrow_mut(),
            &original_plan,
            &mut budget,
        )
        .unwrap();
    assert_eq!(typed.output.scores().len(), 76);
}

#[test]
fn authored_mixed_front_is_canonical_syntax() {
    let text = mixed_source(&profile());
    let authored = format!(
        "plot production-model{}",
        text.rsplit("plot production-model").next().unwrap()
    );
    let parsed = conduit_plot::parse_syntax_document(&authored);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn complete_original_query_fits_explicit_ingress_reservation() {
    let encoded = query().encode().unwrap();
    eprintln!("complete original choice-query bytes={}", encoded.len());
    assert!(encoded.len() <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
}
