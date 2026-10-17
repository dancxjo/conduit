#![cfg(feature = "semantic-bindings")]
use conduit_language::LinguisticSyntacticLinkKind;
use conduit_speech::semantic::{SpeechRuleCondition, SpeechSyntacticLinkKind};
#[test]
fn speech_syntax_is_the_same_native_and_rust_language_type() {
    let language = LinguisticSyntacticLinkKind::Vocative;
    let speech: SpeechSyntacticLinkKind = language;
    assert_eq!(speech, LinguisticSyntacticLinkKind::Vocative);
    assert_eq!(
        SpeechSyntacticLinkKind::semantic_type().unwrap(),
        LinguisticSyntacticLinkKind::semantic_type().unwrap()
    );
    let condition = SpeechRuleCondition::current_word_has_syntactic_link(language).unwrap();
    assert!(matches!(
        condition,
        SpeechRuleCondition::CurrentWordHasSyntacticLink(LinguisticSyntacticLinkKind::Vocative)
    ));
}

#[test]
fn downstream_language_and_text_carriers_have_one_authoritative_owner() {
    use conduit_language::{
        LanguageId, LanguageText, LanguageTextId, LanguageTextRevisionId, VarietyId,
    };
    use conduit_speech::semantic;
    let language: semantic::LanguageId = LanguageId::new("language/fixture".into()).unwrap();
    let variety: semantic::VarietyId = VarietyId::new("variety/local".into()).unwrap();
    assert_eq!(variety.get(), "variety/local");
    let text: semantic::LanguageText = LanguageText::new(
        LanguageTextId::new("text/1".into()).unwrap(),
        language,
        LanguageTextRevisionId::new("revision/1".into()).unwrap(),
        "猫".into(),
    )
    .unwrap();
    assert_eq!(text.text(), "猫");
}
