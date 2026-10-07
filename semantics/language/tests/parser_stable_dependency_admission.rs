//! Portable arc proposals remain subject to the full retained Source fact.
use conduit_core::*;
use conduit_language::*;
use conduit_plot::rust_binding::{
    validate_native_contracts, validate_native_invariants, NativeRustBinding,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/scorer_model.rs"]
mod scorer_model;

#[test]
fn retained_source_fact_correlates_portable_endpoints_base_and_subtype() {
    let source = [
        joint::source(),
        include_str!("../syntax.conduit").into(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../parser_session_dependency.conduit").into(),
    ]
    .join("\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let native = checked
        .native_types
        .iter()
        .find(|t| t.name == "LanguageParserStableDependencyAdmission")
        .unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_slice(include_bytes!(
        "../training/ewt_joint_v2/native_stream_policy_replay.json"
    ))
    .unwrap();
    let bytes: Vec<u8> =
        serde_json::from_value(rows[1]["facts"][0]["native_fact_bytes"].clone()).unwrap();
    let fact = LanguageParserJointStableFact::from_structured(
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
    )
    .unwrap();
    let query = fact.query();
    assert_eq!(*query.dependent(), 0);
    let state = query.beam().candidate0().parser().state();
    let head = state.heads()[0];
    let token = |ordinal: usize| {
        LanguageAnalysisTokenRef::new(
            query.beam().basis().analysis_revision().clone(),
            query.beam().lexical().tape().tokens()[ordinal]
                .identity()
                .clone(),
        )
        .unwrap()
    };
    let dependent = token(0);
    let governor = if head == 4 {
        LanguageDependencyHead::Root
    } else {
        let t = token(head as usize);
        LanguageDependencyHead::token(t.revision().clone(), t.token().clone()).unwrap()
    };
    let relation = LanguageDependencyRelation::new(state.relation0().base().clone(), None).unwrap();
    assert_eq!(state.relation0().subtype().get(), "");
    let arc =
        LanguageDependencyArc::new(dependent.clone(), governor.clone(), relation.clone()).unwrap();
    let proposal = |arc: LanguageDependencyArc| {
        fixture::record(
            &native.value_type,
            vec![
                ("fact", fact.clone().into_structured().unwrap()),
                ("arc", arc.into_structured().unwrap()),
                (
                    "head",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                        head.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
                (
                    "subtype",
                    state
                        .relation0()
                        .subtype()
                        .clone()
                        .into_structured()
                        .unwrap(),
                ),
            ],
        )
    };
    let validate = |value: &StructuredInfoValue| {
        let dynamic = validate_native_contracts(value, &native.value_contracts)
            .and_then(|()| validate_native_invariants(value, &native.invariants));
        let generated =
            LanguageParserStableDependencyAdmission::from_structured(value.clone()).map(|_| ());
        assert_eq!(
            dynamic.is_ok(),
            generated.is_ok(),
            "generated admission must enforce the same complete Source laws"
        );
        generated
    };
    validate(&proposal(arc)).unwrap();
    let foreign = LanguageAnalysisTokenRef::new(
        LanguageAnalysisRevisionId::new("foreign/analysis".into()).unwrap(),
        dependent.token().clone(),
    )
    .unwrap();
    let foreign_governor = if head == 4 {
        LanguageDependencyHead::Root
    } else {
        LanguageDependencyHead::token(
            foreign.revision().clone(),
            token(head as usize).token().clone(),
        )
        .unwrap()
    };
    let changed_endpoint =
        LanguageDependencyArc::new(foreign, foreign_governor, relation.clone()).unwrap();
    assert!(validate(&proposal(changed_endpoint)).is_err());
    assert!(head < 4, "retained independent non-root fact");
    let other_base = if relation.base() == &LanguageUniversalDependencyRelation::Dep {
        LanguageUniversalDependencyRelation::Obj
    } else {
        LanguageUniversalDependencyRelation::Dep
    };
    let changed_base = LanguageDependencyArc::new(
        dependent.clone(),
        governor.clone(),
        LanguageDependencyRelation::new(other_base, None).unwrap(),
    )
    .unwrap();
    assert!(validate(&proposal(changed_base)).is_err());
    let changed_subtype = LanguageDependencyArc::new(
        dependent,
        governor,
        LanguageDependencyRelation::new(
            relation.base().clone(),
            Some(LanguageDependencySubtype::new("vocative".into()).unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(validate(&proposal(changed_subtype)).is_err());
}
