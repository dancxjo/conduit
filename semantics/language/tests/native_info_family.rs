use conduit_language::{
    annotation_bundle_four_type, dependency_edge_type, linguistic_annotation_type,
    linguistic_segment_type, linguistic_token_type, linguistic_tokens_four_type, text_span_type,
    AnnotationBundleFour, LanguageTextId, LanguageTextRevisionId, LinguisticAnnotation,
    LinguisticDependencyEdge, LinguisticOffsetBasis, LinguisticSegment, LinguisticToken,
    LinguisticTokensFour, TextSpan,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[test]
fn complete_linguistic_family_is_owned_by_native_types() {
    assert_eq!(text_span_type(), TextSpan::semantic_type().unwrap());
    assert_eq!(
        linguistic_token_type(),
        LinguisticToken::semantic_type().unwrap()
    );
    assert_eq!(
        linguistic_segment_type(),
        LinguisticSegment::semantic_type().unwrap()
    );
    assert_eq!(
        linguistic_tokens_four_type(),
        LinguisticTokensFour::semantic_type().unwrap()
    );
    assert_eq!(
        linguistic_annotation_type(),
        LinguisticAnnotation::semantic_type().unwrap()
    );
    assert_eq!(
        dependency_edge_type(),
        LinguisticDependencyEdge::semantic_type().unwrap()
    );
    assert_eq!(
        annotation_bundle_four_type(),
        AnnotationBundleFour::semantic_type().unwrap()
    );
}

#[test]
fn text_span_round_trips_and_refuses_empty_identity() {
    let span = TextSpan::new(
        LinguisticOffsetBasis::unicode_scalar(),
        9,
        3,
        LanguageTextId::new("text/example".into()).unwrap(),
        LanguageTextRevisionId::new("source/1".into()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        TextSpan::from_structured(span.clone().into_structured().unwrap()).unwrap(),
        span
    );
    assert!(matches!(
        LanguageTextId::new(String::new()),
        Err(NativeBindingRefusal::ViolatedConstraint { .. })
    ));
    assert!(matches!(
        TextSpan::new(
            LinguisticOffsetBasis::unicode_scalar(),
            2,
            3,
            LanguageTextId::new("text/example".into()).unwrap(),
            LanguageTextRevisionId::new("source/1".into()).unwrap()
        ),
        Err(NativeBindingRefusal::ViolatedInvariant { .. })
    ));
}
