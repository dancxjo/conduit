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
