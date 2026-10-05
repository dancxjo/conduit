use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn language_and_variety_are_distinct_and_relationship_is_explicit() {
    let language = LanguageId::new("language/fixture".into()).unwrap();
    let variety = VarietyId::new("session/local-profile".into()).unwrap();
    let relation = LanguageVariety::new(variety.clone(), language.clone()).unwrap();
    assert_eq!(relation.identity(), &variety);
    assert_eq!(relation.language(), &language);
    assert_ne!(
        LanguageId::semantic_type().unwrap(),
        VarietyId::semantic_type().unwrap()
    );
    assert!(LanguageId::new(String::new()).is_err());
    assert!(VarietyId::new(String::new()).is_err());
}

#[test]
fn non_latin_text_and_occurrences_use_the_same_native_contracts() {
    let language = LanguageId::new("language/japanese-fixture".into()).unwrap();
    let text = LanguageText::new(
        LanguageTextId::new("text/1".into()).unwrap(),
        language.clone(),
        LanguageTextRevisionId::new("revision/1".into()).unwrap(),
        "猫が寝る。".into(),
    )
    .unwrap();
    let reference = LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Morpheme,
        language,
        LanguageTextRange::new(2, 1).unwrap(),
        text.revision().clone(),
        text.identity().clone(),
    )
    .unwrap();
    let matched = LanguageTextReferenceMatch::new(text.clone(), reference, 5).unwrap();
    assert_eq!(*matched.reference().range().start(), 1);
    assert_eq!(
        LanguageText::decode(&text.clone().encode().unwrap()).unwrap(),
        text
    );
    // Correcting a source revision cannot silently preserve stale occurrences.
    let stale = LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Word,
        text.language().clone(),
        LanguageTextRange::new(1, 0).unwrap(),
        LanguageTextRevisionId::new("revision/old".into()).unwrap(),
        text.identity().clone(),
    )
    .unwrap();
    assert!(LanguageTextReferenceMatch::new(text, stale, 5).is_err());
}
