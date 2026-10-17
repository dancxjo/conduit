use conduit_core::IeeeF32;
use conduit_language::LanguageVariety;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;

pub fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "explicit contextual fixture".into(),
        SpeechEvidenceSource::Manual,
        Some("1".into()),
    )
    .unwrap()
}
pub fn basis(sequence: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        SpeechSegmentRevisionId::new("utterance/r1".into()).unwrap(),
        SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
pub fn reference(sequence: &str, ordinal: u32) -> LanguageSpeechTokenRef {
    let basis = basis(sequence);
    LanguageSpeechTokenRef::new(
        basis.inventory_id().clone(),
        basis.language().clone(),
        ordinal,
        basis.revision_id().clone(),
        basis.sequence_id().clone(),
        basis.utterance_id().clone(),
    )
    .unwrap()
}
pub fn text() -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRevisionId::new("text/r1".into()).unwrap(),
        "two words".into(),
    )
    .unwrap()
}
pub fn source() -> LanguageTextSegmentRef {
    LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Utterance,
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRange::new(9, 0).unwrap(),
        LanguageTextRevisionId::new("text/r1".into()).unwrap(),
        LanguageTextId::new("source".into()).unwrap(),
    )
    .unwrap()
}
pub fn confidence() -> SpeechConfidence {
    SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap()
}
pub fn phones() -> SpeechPhoneSequence {
    SpeechPhoneSequence::new(
        basis("phones"),
        BoundedSequence::try_from_iter((0..7).map(|_| {
            SpeechPhoneToken::new(
                BoundedSequence::new(),
                confidence(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                PhoneSpecification::known(PhoneId::new("phone/tap".into()).unwrap()).unwrap(),
                provenance(),
                None,
            )
            .unwrap()
        }))
        .unwrap(),
    )
    .unwrap()
}
pub fn phonemes() -> SpeechPhonemeSequence {
    SpeechPhonemeSequence::new(
        basis("phonemes"),
        BoundedSequence::try_from_iter((0..7).map(|_| {
            SpeechPhonemeToken::new(
                confidence(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                PhonemeSpecification::known(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap(),
                provenance(),
                BoundedSequence::new(),
                None,
            )
            .unwrap()
        }))
        .unwrap(),
    )
    .unwrap()
}
pub fn aligned(left: &[u32], right: &[u32]) -> SpeechRealizationCorrespondence {
    SpeechRealizationCorrespondence::aligned(
        BoundedSequence::try_from_iter(left.iter().map(|n| reference("phonemes", *n))).unwrap(),
        BoundedSequence::try_from_iter(right.iter().map(|n| reference("phones", *n))).unwrap(),
    )
    .unwrap()
}
pub fn groups() -> Vec<SpeechRealizationCorrespondence> {
    vec![
        aligned(&[0], &[0]),
        aligned(&[1], &[1, 2]),
        aligned(&[2, 3], &[3]),
        aligned(&[4, 5], &[4, 5]),
        SpeechRealizationCorrespondence::deleted(
            BoundedSequence::try_from_iter([reference("phonemes", 6)]).unwrap(),
            "explicit deletion".into(),
        )
        .unwrap(),
        SpeechRealizationCorrespondence::inserted(
            BoundedSequence::try_from_iter([reference("phones", 6)]).unwrap(),
            "explicit insertion".into(),
        )
        .unwrap(),
    ]
}
pub fn alignment(groups: Vec<SpeechRealizationCorrespondence>) -> SpeechRealizationAlignment {
    SpeechRealizationAlignment::new(
        BoundedSequence::try_from_iter(groups).unwrap(),
        SpeechIpaInventoryNotationBasis::new(
            SpeechInventoryId::new("inventory".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
            profile(),
            profile().revision().clone(),
            profile().variety().clone(),
        )
        .unwrap(),
        basis("phones"),
        basis("phonemes"),
        provenance(),
        source(),
    )
    .unwrap()
}
pub fn inventory() -> SpeechInventory {
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechPhoneme::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            None,
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            PhonemeId::new("phoneme/t".into()).unwrap(),
            "t".into(),
            BoundedSequence::new(),
            SpeechSegmentStatus::Core,
        )
        .unwrap()])
        .unwrap(),
        BoundedSequence::try_from_iter([SpeechPhone::new(
            BoundedSequence::new(),
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            PhoneId::new("phone/tap".into()).unwrap(),
            "ɾ".into(),
            SpeechSegmentStatus::Allophonic,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
pub fn profile() -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        BoundedSequence::new(),
        SpeechIpaNotationProfileId::new("ipa@1".into()).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new("inventory/v3".into()).unwrap(),
        BoundedSequence::try_from_iter([("stop", "t"), ("tap", "ɾ")].map(|(id, ipa)| {
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new(id.into()).unwrap(),
                SpeechIpaUnitKind::Segment,
                provenance(),
                SpeechIpaSpelling::new(ipa.into()).unwrap(),
            )
            .unwrap()
        }))
        .unwrap(),
        LanguageVariety::new(
            VarietyId::new("explicit/test-variety".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn bindings() -> (
    SpeechIpaPhoneDefinitionBinding,
    SpeechIpaPhonemeDefinitionBinding,
) {
    (
        SpeechIpaPhoneDefinitionBinding::new(
            PhoneId::new("phone/tap".into()).unwrap(),
            provenance(),
            BoundedSequence::try_from_iter([SpeechIpaUnitId::new("tap".into()).unwrap()]).unwrap(),
        )
        .unwrap(),
        SpeechIpaPhonemeDefinitionBinding::new(
            PhonemeId::new("phoneme/t".into()).unwrap(),
            provenance(),
            BoundedSequence::try_from_iter([SpeechIpaUnitId::new("stop".into()).unwrap()]).unwrap(),
        )
        .unwrap(),
    )
}

pub fn intent() -> SpeechUtteranceIntent {
    let events = (0..7).map(|ordinal| {
        SpeechUtteranceIntentEvent::segment(
            reference("phones", ordinal),
            PhoneSpecification::known(PhoneId::new("phone/tap".into()).unwrap()).unwrap(),
            if ordinal <= 2 {
                PhonemeSpecification::known(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap()
            } else {
                PhonemeSpecification::not_applicable()
            },
            SpeechSegmentProsodyIntent::new(
                SpeechDurationSpecification::unknown(),
                SpeechCycleSpecification::unspecified(),
                SpeechIntensitySpecification::not_applicable(),
            )
            .unwrap(),
            provenance(),
            BoundedSequence::try_from_iter([LanguageSegmentRef::text(
                source().kind().clone(),
                source().language().clone(),
                source().range().clone(),
                source().revision_id().clone(),
                source().text_id().clone(),
            )
            .unwrap()])
            .unwrap(),
            StressSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap()
    });
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        basis("phones").inventory_id().clone(),
        basis("phones").language().clone(),
        provenance(),
        basis("phones").revision_id().clone(),
        basis("phones").utterance_id().clone(),
    )
    .unwrap()
}
