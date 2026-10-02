use conduit_audio::{RhythmRecoveryState, TimingClassification, TimingFeedback};
use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape, PROTOCOL_VERSION,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    adapt_rhythm_feedback, deterministic_arithmetic_fixture, deterministic_hint_request,
    deterministic_refused_response, deterministic_timeout, education_assessment_outcome_type,
    education_progress_type, education_question_type, education_rhythm_feedback_type,
    evaluate_arithmetic_response, install_education_catalogs,
    install_structured_music_plot_catalogs, EDUCATION_RHYTHM_FEEDBACK_KIND,
    MAXIMUM_EDUCATION_HINTS,
};

const ARITHMETIC_SOURCE: &str = include_str!("../../../plots/arithmetic-lesson/main.conduit");
const RHYTHM_SOURCE: &str = include_str!("../../../plots/rhythm-lesson/main.conduit");

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_structured_music_plot_catalogs(&mut startup, &mut profile).unwrap();
    install_education_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn unrelated_arithmetic_lesson_is_one_ordinary_plannable_plot() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(ARITHMETIC_SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "arithmetic-lesson", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 2);
    assert_eq!(authored.output_bindings.len(), 4);

    let host = host(education_proof_offers());
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_eq!(plan.fragments[0].placements.len(), 2);
    for placement in &plan.fragments[0].placements {
        assert_eq!(
            placement.host_calls[0].contract_id.as_str(),
            DOMAIN_PROOF_OPERATION
        );
        assert!(placement.resources.is_empty());
        assert!(placement.authority.is_empty());
    }
}

#[test]
fn arithmetic_fixture_emits_exact_assessment_feedback_and_current_progress() {
    let fixture = deterministic_arithmetic_fixture().unwrap();
    let result = evaluate_arithmetic_response(&fixture.question, &fixture.response).unwrap();
    assert_eq!(
        variant_tag(record_field(&result.assessment, "outcome")),
        "correct"
    );
    assert_eq!(
        variant_tag(record_field(
            record_field(&result.feedback, "provenance"),
            "evidence_class"
        )),
        "deterministic"
    );
    assert_eq!(
        variant_tag(record_field(&result.progress, "state")),
        "completed"
    );
    assert_eq!(
        leaf_text(record_field(&fixture.question, "response_profile")),
        "education/response/integer-text@1"
    );
}

#[test]
fn hint_request_timeout_and_refusal_are_typed_outcomes() {
    let fixture = deterministic_arithmetic_fixture().unwrap();
    let question_identity = leaf_text(record_field(&fixture.question, "question_identity"));
    let vectors = [
        (
            deterministic_hint_request(question_identity).unwrap(),
            "hint_requested",
            "hinting",
        ),
        (
            deterministic_timeout(question_identity).unwrap(),
            "timeout",
            "timed_out",
        ),
        (
            deterministic_refused_response(question_identity, "unsupported-input").unwrap(),
            "refused",
            "refused",
        ),
    ];
    for (response, outcome, progress) in vectors {
        let result = evaluate_arithmetic_response(&fixture.question, &response).unwrap();
        assert_eq!(
            variant_tag(record_field(&result.assessment, "outcome")),
            outcome
        );
        assert_eq!(
            variant_tag(record_field(&result.progress, "state")),
            progress
        );
    }
    let hint_result = evaluate_arithmetic_response(
        &fixture.question,
        &deterministic_hint_request(question_identity).unwrap(),
    )
    .unwrap();
    assert_eq!(
        variant_tag(record_field(&hint_result.feedback, "hint")),
        "provided"
    );
}

#[test]
fn rhythm_adapter_keeps_exact_timing_inside_the_same_feedback_substrate() {
    let timing = timing_value(TimingClassification::Late, 45_000);
    let adapted = adapt_rhythm_feedback(&timing).unwrap();
    assert_eq!(adapted.value_type(), &education_rhythm_feedback_type());
    assert_eq!(record_field(&adapted, "timing"), &timing);
    let feedback = record_field(&adapted, "feedback");
    assert_eq!(
        variant_tag(record_field(
            record_field(feedback, "assessment"),
            "outcome"
        )),
        "partial"
    );
    assert_eq!(
        leaf_text(record_field(
            record_field(feedback, "assessment"),
            "evaluation_profile"
        )),
        "education/evaluate/rhythm-timing@1"
    );

    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(RHYTHM_SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "rhythm-lesson", &profile).unwrap();
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == EDUCATION_RHYTHM_FEEDBACK_KIND));
    assert_eq!(authored.output_bindings.len(), 2);
}

#[test]
fn lesson_state_and_hints_are_bounded_without_retained_learner_history() {
    let question = education_question_type();
    let StructuredInfoTypeShape::Record { fields, .. } = question.shape() else {
        panic!("question must be a record")
    };
    let hints = fields
        .iter()
        .find(|field| field.name() == "hints")
        .unwrap()
        .value_type();
    let StructuredInfoTypeShape::Sequence {
        minimum_items,
        maximum_items,
        ..
    } = hints.shape()
    else {
        panic!("hints must be a bounded sequence")
    };
    assert_eq!((minimum_items, maximum_items), (0, MAXIMUM_EDUCATION_HINTS));

    let progress = education_progress_type();
    let rendered = format!("{progress:?}").to_ascii_lowercase();
    for forbidden in [
        "history",
        "student-profile",
        "gradebook",
        "classroom",
        "llm",
        "presenter",
    ] {
        assert!(!rendered.contains(forbidden), "progress leaked {forbidden}");
    }

    let outcome = education_assessment_outcome_type();
    let StructuredInfoTypeShape::Variant { cases, .. } = outcome.shape() else {
        panic!("assessment outcome must be a variant")
    };
    assert_eq!(
        cases.iter().map(|case| case.tag()).collect::<Vec<_>>(),
        [
            "correct",
            "hint_requested",
            "incorrect",
            "partial",
            "refused",
            "timeout",
        ]
    );
}

fn timing_value(classification: TimingClassification, delta_micros: i64) -> StructuredInfoValue {
    TimingFeedback::new(
        3,
        classification,
        delta_micros,
        3_000_000,
        true,
        3_045_000,
        RhythmRecoveryState::Recovering,
    )
    .unwrap()
    .into_structured()
    .unwrap()
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/education-proof"),
        boot_id: BootId::from("boot/education-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/education-proof@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    core::str::from_utf8(bytes).unwrap()
}
mod common;

use common::{education_proof_offers, DOMAIN_PROOF_OPERATION};
