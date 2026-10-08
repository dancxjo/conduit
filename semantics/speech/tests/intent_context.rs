#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{intent_context::*, semantic::*};
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new("explicit-test".into(), SpeechEvidenceSource::Manual, None)
        .unwrap()
}
fn basis(revision: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        SpeechSegmentSequenceId::new("target-phones".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn intent(revision: &str) -> SpeechUtteranceIntent {
    SpeechUtteranceIntent::new(
        BoundedSequence::new(),
        basis(revision).inventory_id().clone(),
        basis(revision).language().clone(),
        provenance(),
        basis(revision).revision_id().clone(),
        basis(revision).utterance_id().clone(),
    )
    .unwrap()
}

fn context(
    language: &str,
    reference: Option<LanguageTextSegmentRef>,
) -> SpeechUtteranceIntentContext {
    SpeechUtteranceIntentContext::new(
        reference,
        provenance(),
        SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
        SpeechSpeakerReferenceSpecification::unknown(),
        SpeechStyleReferenceSpecification::unspecified(),
        LanguageVariety::new(
            VarietyId::new("declared-variety".into()).unwrap(),
            LanguageId::new(language.into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn original_context_and_material_are_retained_with_scalar_basis() {
    let i = intent("r1");
    let text = LanguageText::new(
        LanguageTextId::new("text".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRevisionId::new("text-r1".into()).unwrap(),
        "Olá".into(),
    )
    .unwrap();
    let reference = LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Utterance,
        text.language().clone(),
        LanguageTextRange::new(3, 0).unwrap(),
        text.revision().clone(),
        text.identity().clone(),
    )
    .unwrap();
    let c = context("en", Some(reference));
    let p = PreparedUtteranceIntentContext::prepare(&i, &c, Some(&text)).unwrap();
    assert!(core::ptr::eq(p.intent(), &i));
    assert!(core::ptr::eq(p.context(), &c));
    assert!(core::ptr::eq(p.text().unwrap(), &text));
    assert!(p.text_match().is_some());
    assert!(PreparedUtteranceIntentContext::prepare(&i, &c, None).is_err());
    let wrong = context("pt", None);
    assert!(PreparedUtteranceIntentContext::prepare(&i, &wrong, None).is_err());
    let absent = context("en", None);
    assert!(PreparedUtteranceIntentContext::prepare(&i, &absent, Some(&text)).is_err());
    assert!(SpeechReferenceProjectionCapability::new("audio_embedding_supported".into()).is_err());
}
#[test]
fn six_speaker_and_style_states_survive_as_unresolved_reference_metadata() {
    let identity = LanguageExternalIdentity::new(
        "speaker-style-reference-v1".into(),
        "named-reference".into(),
    )
    .unwrap();
    let confidence = SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap();
    let speakers = [
        SpeechSpeakerReferenceSpecification::known(
            identity.contract().clone(),
            identity.name().clone(),
        )
        .unwrap(),
        SpeechSpeakerReferenceSpecification::unknown(),
        SpeechSpeakerReferenceSpecification::unspecified(),
        SpeechSpeakerReferenceSpecification::not_applicable(),
        SpeechSpeakerReferenceSpecification::variable(
            BoundedSequence::try_from_iter([identity.clone()]).unwrap(),
        )
        .unwrap(),
        SpeechSpeakerReferenceSpecification::gradient(confidence.clone(), identity.clone())
            .unwrap(),
    ];
    let styles = [
        SpeechStyleReferenceSpecification::known(
            identity.contract().clone(),
            identity.name().clone(),
        )
        .unwrap(),
        SpeechStyleReferenceSpecification::unknown(),
        SpeechStyleReferenceSpecification::unspecified(),
        SpeechStyleReferenceSpecification::not_applicable(),
        SpeechStyleReferenceSpecification::variable(
            BoundedSequence::try_from_iter([identity.clone()]).unwrap(),
        )
        .unwrap(),
        SpeechStyleReferenceSpecification::gradient(confidence, identity).unwrap(),
    ];
    let i = intent("r1");
    for (speaker, style) in speakers.into_iter().zip(styles) {
        let original = SpeechUtteranceIntentContext::new(
            None,
            provenance(),
            SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
            speaker,
            style,
            context("en", None).variety().clone(),
        )
        .unwrap();
        let frame = original.clone().into_structured().unwrap();
        assert_eq!(
            SpeechUtteranceIntentContext::from_structured(frame).unwrap(),
            original
        );
        PreparedUtteranceIntentContext::prepare(&i, &original, None).unwrap();
        assert_eq!(
            original.reference_capability().profile(),
            "retained_metadata_only"
        );
    }
}
