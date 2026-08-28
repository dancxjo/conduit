use conduit_ai::{
    ExtractedField, FiniteClassification, FiniteEmbedding, StructuredResultInvalidity,
    ValidatedExtraction,
};

#[test]
fn classification_requires_one_exact_member_of_a_finite_unique_label_set() {
    let valid = FiniteClassification {
        label: "conduit".into(),
        allowed_labels: vec!["conduit".into(), "other".into()],
    };
    assert_eq!(valid.validate(), Ok(()));

    let mut invalid = valid.clone();
    invalid.label = "invented".into();
    assert_eq!(
        invalid.validate(),
        Err(StructuredResultInvalidity::LabelNotAllowed)
    );
    invalid.allowed_labels = vec!["invented".into(), "invented".into()];
    assert_eq!(
        invalid.validate(),
        Err(StructuredResultInvalidity::DuplicateMember)
    );
}

#[test]
fn extraction_requires_a_named_schema_and_unique_bounded_fields() {
    let valid = ValidatedExtraction {
        schema_identity: "conduit-proof/subject@1".into(),
        fields: vec![ExtractedField {
            key: "subject".into(),
            value: "Conduit".into(),
        }],
    };
    assert_eq!(valid.validate(), Ok(()));

    let mut invalid = valid.clone();
    invalid.fields.push(invalid.fields[0].clone());
    assert_eq!(
        invalid.validate(),
        Err(StructuredResultInvalidity::DuplicateMember)
    );
}

#[test]
fn embedding_requires_exact_finite_dimensions_and_finite_values() {
    let valid = FiniteEmbedding {
        profile_identity: "fixture/embedding-3@1".into(),
        dimensions: 3,
        values: vec![0.25, -0.5, 1.0],
    };
    assert_eq!(valid.validate(), Ok(()));

    let mut invalid = valid.clone();
    invalid.dimensions = 2;
    assert_eq!(
        invalid.validate(),
        Err(StructuredResultInvalidity::DimensionMismatch)
    );
    invalid.dimensions = 3;
    invalid.values[1] = f32::NAN;
    assert_eq!(
        invalid.validate(),
        Err(StructuredResultInvalidity::NonFiniteValue)
    );
}
