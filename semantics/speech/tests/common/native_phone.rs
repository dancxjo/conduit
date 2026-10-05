use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
pub fn material(spec: PhoneSpecification) -> (LanguageSegmentRef, SpeechPhoneSequence) {
    material_with_features(
        spec,
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
    )
}
pub fn material_with_features(
    spec: PhoneSpecification,
    features: SpeechFeatureBundle,
) -> (LanguageSegmentRef, SpeechPhoneSequence) {
    let inventory = SpeechInventoryId::new("inventory".into()).unwrap();
    let language = LanguageId::new("en".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("revision".into()).unwrap();
    let sequence = SpeechSegmentSequenceId::new("sequence".into()).unwrap();
    let utterance = SpeechUtteranceId::new("utterance".into()).unwrap();
    let reference = LanguageSegmentRef::phone(
        inventory.clone(),
        language.clone(),
        0,
        revision.clone(),
        sequence.clone(),
        utterance.clone(),
    )
    .unwrap();
    let basis =
        SpeechTokenSequenceBasis::new(inventory, language, revision, sequence, utterance).unwrap();
    let token = SpeechPhoneToken::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        features,
        spec,
        SpeechEvidenceProvenance::new("original".into(), SpeechEvidenceSource::Manual, None)
            .unwrap(),
        None,
    )
    .unwrap();
    (
        reference,
        SpeechPhoneSequence::new(basis, BoundedSequence::try_from_iter([token]).unwrap()).unwrap(),
    )
}
pub fn id(s: &str) -> PhoneId {
    PhoneId::new(s.into()).unwrap()
}
pub fn definition(identity: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        id(identity),
        "t".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
pub fn inventory(identity: &str, language: &str, phones: Vec<SpeechPhone>) -> SpeechInventory {
    SpeechInventory::new(
        SpeechInventoryId::new(identity.into()).unwrap(),
        LanguageId::new(language.into()).unwrap(),
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(phones).unwrap(),
    )
    .unwrap()
}
