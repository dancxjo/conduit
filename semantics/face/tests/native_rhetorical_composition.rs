use conduit_face::{FaceCompositionKind, FaceCompositionRelation};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn rhetorical_composition_round_trips_and_enforces_identity_bounds() {
    let kind = FaceCompositionKind::semantic("biology/cell".into()).unwrap();
    let relation = FaceCompositionRelation::new(
        "composition/cell".into(),
        kind,
        "subject/nucleus".into(),
        "subject/cell".into(),
    )
    .unwrap();
    let structured = relation.clone().into_structured().unwrap();
    assert_eq!(
        FaceCompositionRelation::from_structured(structured).unwrap(),
        relation
    );

    assert!(FaceCompositionKind::semantic(String::new()).is_err());
    assert!(FaceCompositionKind::semantic("x".repeat(257)).is_err());
    assert!(FaceCompositionRelation::new(
        String::new(),
        FaceCompositionKind::Group,
        "source".into(),
        "target".into(),
    )
    .is_err());
}

#[test]
fn rhetorical_composition_preserves_legacy_serde_shape_and_order() {
    let relation = FaceCompositionRelation::new(
        "composition/cell".into(),
        FaceCompositionKind::semantic("biology/cell".into()).unwrap(),
        "subject/nucleus".into(),
        "subject/cell".into(),
    )
    .unwrap();
    let json = r#"{"identity":"composition/cell","source":"subject/nucleus","target":"subject/cell","kind":{"Semantic":"biology/cell"}}"#;
    assert_eq!(serde_json::to_string(&relation).unwrap(), json);
    assert_eq!(
        serde_json::from_str::<FaceCompositionRelation>(json).unwrap(),
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
        postcard::from_bytes::<FaceCompositionRelation>(&bytes).unwrap(),
        relation
    );
}

#[test]
fn rhetorical_composition_has_no_handwritten_semantic_duplicate() {
    let source = include_str!("../src/rhetorical_composition.rs");
    assert!(!source.contains("pub enum FaceCompositionKind"));
    assert!(!source.contains("pub struct FaceCompositionRelation"));
}
