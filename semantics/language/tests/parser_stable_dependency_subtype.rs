//! Native subtype conformance fixture; this does not claim a trained subtype label.
use conduit_core::*;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_fixture.rs"]
mod fixture;

fn replace(
    value: &StructuredInfoValue,
    path: &[&str],
    replacement: &StructuredInfoValue,
) -> StructuredInfoValue {
    let Some((first, rest)) = path.split_first() else {
        return replacement.clone();
    };
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("exact native record path");
    };
    assert!(fields.iter().any(|field| field.name() == *first));
    fixture::record(
        value.value_type(),
        fields
            .iter()
            .map(|field| {
                (
                    field.name(),
                    if field.name() == *first {
                        replace(field.value(), rest, replacement)
                    } else {
                        field.value().clone()
                    },
                )
            })
            .collect(),
    )
}
#[test]
fn nonempty_subtype_material_is_retained_and_cannot_be_erased_or_relabelled() {
    let rows: Vec<serde_json::Value> = serde_json::from_slice(include_bytes!(
        "../training/ewt_joint_v2/native_stream_policy_replay.json"
    ))
    .unwrap();
    let bytes: Vec<u8> =
        serde_json::from_value(rows[1]["facts"][0]["native_fact_bytes"].clone()).unwrap();
    let mut material = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    let subtype = LanguageParserSubtype::new("vocative".into()).unwrap();
    // Deliberately uniform native fixture amendment: all surviving relations
    // agree on the full suffix. This is not output from the learned base scorer.
    for candidate in ["candidate0", "candidate1", "candidate2", "candidate3"] {
        material = replace(
            &material,
            &[
                "query",
                "beam",
                candidate,
                "parser",
                "state",
                "relation0",
                "subtype",
            ],
            &subtype.clone().into_structured().unwrap(),
        );
    }
    let fact = LanguageParserJointStableFact::from_structured(material).unwrap();
    let query = fact.query();
    let state = query.beam().candidate0().parser().state();
    let head = state.heads()[0];
    assert!(head < 4);
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
    let governor_token = token(head as usize);
    let governor = LanguageDependencyHead::token(
        governor_token.revision().clone(),
        governor_token.token().clone(),
    )
    .unwrap();
    let proposal = |portable: Option<LanguageDependencySubtype>| {
        let arc = LanguageDependencyArc::new(
            dependent.clone(),
            governor.clone(),
            LanguageDependencyRelation::new(state.relation0().base().clone(), portable).unwrap(),
        )
        .unwrap();
        fixture::record(
            &LanguageParserStableDependencyAdmission::semantic_type().unwrap(),
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
                ("subtype", subtype.clone().into_structured().unwrap()),
            ],
        )
    };
    let admitted = LanguageParserStableDependencyAdmission::from_structured(proposal(Some(
        LanguageDependencySubtype::new("vocative".into()).unwrap(),
    )))
    .unwrap();
    assert_eq!(admitted.fact(), &fact);
    assert_eq!(
        admitted.arc().relation().subtype().as_ref().unwrap().get(),
        "vocative"
    );
    assert!(LanguageParserStableDependencyAdmission::from_structured(proposal(None)).is_err());
    assert!(
        LanguageParserStableDependencyAdmission::from_structured(proposal(Some(
            LanguageDependencySubtype::new("voc".into()).unwrap()
        )))
        .is_err()
    );
}
