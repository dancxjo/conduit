use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{PresentationCompositionKind, PresentationCompositionRelation};

#[test]
fn rhetorical_composition_round_trips_and_enforces_identity_bounds() {
    let kind = PresentationCompositionKind::semantic("biology/cell".into()).unwrap();
    let relation = PresentationCompositionRelation::new(
        "composition/cell".into(),
        kind,
        "subject/nucleus".into(),
        "subject/cell".into(),
    )
    .unwrap();
    let structured = relation.clone().into_structured().unwrap();
    assert_eq!(
        PresentationCompositionRelation::from_structured(structured).unwrap(),
        relation
    );

    assert!(PresentationCompositionKind::semantic(String::new()).is_err());
    assert!(PresentationCompositionKind::semantic("x".repeat(257)).is_err());
    assert!(PresentationCompositionRelation::new(
        String::new(),
        PresentationCompositionKind::Group,
        "source".into(),
        "target".into(),
    )
    .is_err());
}

#[test]
fn rhetorical_composition_preserves_legacy_serde_shape_and_order() {
    let relation = PresentationCompositionRelation::new(
        "composition/cell".into(),
        PresentationCompositionKind::semantic("biology/cell".into()).unwrap(),
        "subject/nucleus".into(),
        "subject/cell".into(),
    )
    .unwrap();
    let json = r#"{"identity":"composition/cell","source":"subject/nucleus","target":"subject/cell","kind":{"Semantic":"biology/cell"}}"#;
    assert_eq!(serde_json::to_string(&relation).unwrap(), json);
    assert_eq!(
        serde_json::from_str::<PresentationCompositionRelation>(json).unwrap(),
        relation
    );

    let bytes = postcard::to_allocvec(&relation).unwrap();
    let expected = [
        &[16][..],
        b"composition/cell",
        &[15][..],
        b"subject/nucleus",
        &[12][..],
        b"subject/cell",
        &[7, 12][..],
        b"biology/cell",
    ]
    .concat();
    assert_eq!(bytes, expected);
    assert_eq!(
        postcard::from_bytes::<PresentationCompositionRelation>(&bytes).unwrap(),
        relation
    );
}

#[test]
fn rhetorical_composition_has_no_handwritten_semantic_duplicate() {
    let source = include_str!("../src/rhetorical_composition.rs");
    assert!(!source.contains("pub enum PresentationCompositionKind"));
    assert!(!source.contains("pub struct PresentationCompositionRelation"));
}
