use conduit_ai::{
    FiniteClassification, FiniteEmbedding, StructuredResultInvalidity, ValidatedExtraction,
};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn classification_requires_one_exact_member_of_a_finite_unique_label_set() {
    let valid = FiniteClassification::from_strings(
        "conduit".into(),
        vec!["conduit".into(), "other".into()],
    )
    .unwrap();
    assert_eq!(valid.validate(), Ok(()));

    let invalid = FiniteClassification::from_strings(
        "invented".into(),
        vec!["conduit".into(), "other".into()],
    )
    .unwrap_err();
    assert_eq!(invalid, StructuredResultInvalidity::LabelNotAllowed);
    let invalid = FiniteClassification::from_strings(
        "invented".into(),
        vec!["invented".into(), "invented".into()],
    )
    .unwrap_err();
    assert_eq!(invalid, StructuredResultInvalidity::DuplicateMember);
}

#[test]
fn extraction_requires_a_named_schema_and_unique_bounded_fields() {
    let valid = ValidatedExtraction::from_strings(
        "conduit-proof/subject@1".into(),
        vec![("subject".into(), "Conduit".into())],
    )
    .unwrap();
    assert_eq!(valid.validate(), Ok(()));

    let invalid = ValidatedExtraction::from_strings(
        "conduit-proof/subject@1".into(),
        vec![
            ("subject".into(), "Conduit".into()),
            ("subject".into(), "Conduit".into()),
        ],
    )
    .unwrap_err();
    assert_eq!(invalid, StructuredResultInvalidity::DuplicateMember);
}

#[test]
fn classification_and_extraction_round_trip_through_native_owners() {
    let classification = FiniteClassification::from_strings(
        "conduit".into(),
        vec!["conduit".into(), "other".into()],
    )
    .unwrap();
    assert_eq!(
        FiniteClassification::from_structured(classification.clone().into_structured().unwrap())
            .unwrap(),
        classification
    );
    assert_eq!(
        serde_json::to_value(&classification).unwrap(),
        serde_json::json!({"label": "conduit", "allowed_labels": ["conduit", "other"]})
    );

    let extraction = ValidatedExtraction::from_strings(
        "conduit-proof/subject@1".into(),
        vec![("subject".into(), "Conduit".into())],
    )
    .unwrap();
    assert_eq!(
        ValidatedExtraction::from_structured(extraction.clone().into_structured().unwrap())
            .unwrap(),
        extraction
    );
    assert_eq!(
        serde_json::to_value(&extraction).unwrap(),
        serde_json::json!({
            "schema_identity": "conduit-proof/subject@1",
            "fields": [{"key": "subject", "value": "Conduit"}]
        })
    );

    let source = include_str!("../src/structured_result.rs");
    assert!(!source.contains("pub struct FiniteClassification"));
    assert!(!source.contains("pub struct ExtractedField"));
    assert!(!source.contains("pub struct ValidatedExtraction"));
}

#[test]
fn embedding_requires_exact_finite_dimensions_and_finite_values() {
    let valid = FiniteEmbedding::from_values("fixture/embedding-3@1".into(), vec![0.25, -0.5, 1.0])
        .unwrap();
    assert_eq!(valid.validate(), Ok(()));
    assert_eq!(valid.values_f32(), vec![0.25, -0.5, 1.0]);
    assert_eq!(
        FiniteEmbedding::from_structured(valid.clone().into_structured().unwrap()).unwrap(),
        valid
    );
    assert_eq!(
        serde_json::to_value(&valid).unwrap(),
        serde_json::json!({
            "profile_identity": "fixture/embedding-3@1",
            "dimensions": 3,
            "values": [0.25, -0.5, 1.0]
        })
    );
    assert_eq!(
        serde_json::from_value::<FiniteEmbedding>(serde_json::json!({
            "profile_identity": "fixture/embedding-3@1",
            "dimensions": 3,
            "values": [0.25, -0.5, 1.0]
        }))
        .unwrap(),
        valid
    );

    assert_eq!(
        FiniteEmbedding::from_values("fixture/embedding@1".into(), vec![]),
        Err(StructuredResultInvalidity::Empty)
    );
    assert_eq!(
        FiniteEmbedding::from_values("fixture/embedding@1".into(), vec![f32::NAN]),
        Err(StructuredResultInvalidity::NonFiniteValue)
    );
    let maximum = FiniteEmbedding::from_values("fixture/embedding-4096@1".into(), vec![0.0; 4_096]);
    assert!(maximum.is_ok(), "{maximum:?}");
    assert_eq!(
        FiniteEmbedding::from_values("fixture/embedding-4097@1".into(), vec![0.0; 4_097]),
        Err(StructuredResultInvalidity::MemberTooLarge)
    );
    assert!(
        serde_json::from_value::<FiniteEmbedding>(serde_json::json!({
            "profile_identity": "fixture/embedding-3@1",
            "dimensions": 2,
            "values": [0.25, -0.5, 1.0]
        }))
        .is_err()
    );

    let source = include_str!("../src/structured_result.rs");
    assert!(!source.contains("pub struct FiniteEmbedding"));
}
